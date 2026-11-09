//! GIS engine: data storage, coordinate systems, vector tiles and the
//! geoprocessing graph. The Tauri app is a thin wrapper around [`Engine`].

/// The tile-URL layer id that stands for the bundled Natural Earth basemap (`/tiles/__naturalEarth/{crsKey}/{z}/{x}/{y}.png`).
pub const NATURAL_EARTH_TILE_ID: &str = "__naturalEarth";
/// Tile ids `__bm_<provider>` are on-screen raster basemaps warped to the map CRS by the engine (UR-19).
pub const BASEMAP_TILE_PREFIX: &str = "__bm_";
pub mod attrs;
pub mod catalog;
pub mod usng;
pub mod crs;
pub mod export_table;
pub mod fonts;
pub mod arcgis;
pub mod raster_ops;
pub mod spatial_stats;
pub mod kriging;
pub mod whitebox;
pub mod transform_report;
pub mod grids;
pub mod crs_warnings;
pub mod vertical;
pub mod remote_sensing;
pub mod jsonapis;
pub mod ogcapi;
pub mod overture;
pub mod stac;
pub mod census;
pub mod tissot;
pub mod world;
pub mod db;
pub mod display;
pub mod edit;
pub mod edit_tools;
pub mod graph;
pub mod history;
pub mod layers;
pub mod layout;
pub mod measure;
pub mod nodes;
pub mod geomkit;
pub mod network;
pub mod progress;
pub mod pyscript;
pub mod raster;
pub mod raster_style;
pub mod topo_symbols;
pub mod settings;
pub mod sources;
pub mod resources;
pub mod table;
pub mod tiger;
pub mod split;
pub mod tiles;
pub mod tnm;
pub mod tnm_vector;

use anyhow::{bail, anyhow, Context, Result};
use db::{ident, lit, Pool};
use duckdb::Connection;
use graph::{Cache, Graph, Host, NodeResult};
use layers::{ImportSource, LayerInfo};
use lru::LruCache;
use nodes::Tool;
use progress::Task;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Result of a National Map download: new layers, plus datasets that had no
/// features in the area or failed (e.g. a server down).
#[derive(serde::Serialize, Default, Debug)]
pub struct TnmImport {
    pub layers: Vec<LayerInfo>,
    pub empty: Vec<String>,
    pub failed: Vec<String>,
}

/// What the Topo Map Builder's data step made from an elevation raster.
#[derive(serde::Serialize, Debug)]
pub struct TopoData {
    pub contours: LayerInfo,
    pub hillshade: LayerInfo,
}

type TileKey = (String, u64, u32, u32, u32, String);

/// Id of the node-tree result layer for `owner` (e.g. L3 → L3r).
pub fn result_id(owner: &str) -> String {
    format!("{owner}r")
}

pub struct Engine {
    pool: RwLock<Pool>,
    project_dir: RwLock<Option<PathBuf>>,
    layers: RwLock<HashMap<String, LayerInfo>>,
    node_layers: Mutex<HashMap<String, String>>,
    next_id: AtomicU64,
    /// Connections in use, across pools (Save waits on it; SAVE-1).
    gate: std::sync::Arc<db::Gate>,
    tile_cache: Mutex<LruCache<TileKey, Arc<Vec<u8>>>>,
    tools: Vec<Box<dyn Tool>>,
    graph_cache: Mutex<Cache>,
    diffs: Mutex<HashMap<String, history::DiffMeta>>,
    display: display::Registry,
}

const PROJECT_DB: &str = "project.duckdb";
/// How long Save waits for work already running on the project before giving up (SAVE-1).
const SAVE_WAIT: std::time::Duration = std::time::Duration::from_secs(120);
const PROJECT_JSON: &str = "project.json";
/// Newest project format this build reads. The UI's `PROJECT_VERSION` (`ui/src/store/maps.ts`) is the same number.
const PROJECT_FORMAT: f64 = 2.0;

/// Refuse a project this build can't read, before anything is opened. Refusing later would leave the engine on the other project
/// while the UI still shows the old one, and the next Save would then overwrite the old project's data. A file with no version,
/// or `project.json` that isn't readable JSON, is an older project and opens as before.
fn check_project_format(ui_state: &str) -> Result<()> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(ui_state) else { return Ok(()) };
    let Some(version) = json.get("version") else { return Ok(()) };
    match version.as_f64() {
        Some(v) if v > PROJECT_FORMAT => Err(anyhow!("This project was saved by a newer version of GIS Studio (project format {v}; this version reads up to {PROJECT_FORMAT}). Update GIS Studio to open it. Nothing was changed.")),
        Some(v) if v >= 1.0 && v.fract() == 0.0 => Ok(()),
        _ => Err(anyhow!("This project file has an unreadable format version ({version}). Nothing was changed.")),
    }
}

/// Write `project.json` through a rename, so it is the earlier file or the new one, never half of either.
fn write_ui_state(dir: &Path, ui_state: &str) -> Result<()> {
    let tmp = dir.join(format!("{PROJECT_JSON}.saving"));
    std::fs::write(&tmp, ui_state)?;
    std::fs::rename(&tmp, dir.join(PROJECT_JSON))?;
    Ok(())
}

fn save_busy(wait: std::time::Duration) -> anyhow::Error {
    anyhow!("The project couldn't be saved yet: other work on it (an import, a map being prepared or a tool still running) hasn't finished after {} seconds. The project wasn't saved yet and nothing was lost. Try Save again when that work is done.", wait.as_secs().max(1))
}

impl Engine {
    pub fn new() -> Result<Self> {
        resources::ensure_proj_data();
        let gate = std::sync::Arc::new(db::Gate::default());
        let e = Self {
            pool: RwLock::new(Pool::open(None, gate.clone())?),
            gate,
            project_dir: RwLock::new(None),
            layers: RwLock::new(HashMap::new()),
            node_layers: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            tile_cache: Mutex::new(LruCache::new(NonZeroUsize::new(4096).unwrap())),
            tools: nodes::registry(),
            graph_cache: Mutex::new(Cache::default()),
            diffs: Mutex::new(HashMap::new()),
            display: display::Registry::default(),
        };
        e.init_schema()?;
        Ok(e)
    }

    fn init_schema(&self) -> Result<()> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS __layers (id VARCHAR PRIMARY KEY, info JSON);
             CREATE TABLE IF NOT EXISTS __model_layers (node VARCHAR PRIMARY KEY, layer VARCHAR);
             CREATE TABLE IF NOT EXISTS __layouts (id VARCHAR PRIMARY KEY, json JSON, updated TIMESTAMP);
             CREATE TABLE IF NOT EXISTS __defq_fids (layer VARCHAR, script VARCHAR, version BIGINT, __fid BIGINT);
             CREATE TABLE IF NOT EXISTS __defq_sets (layer VARCHAR, script VARCHAR, version BIGINT, kept BIGINT);
             CREATE TABLE IF NOT EXISTS __label_vals (layer VARCHAR, script VARCHAR, version BIGINT, __fid BIGINT, text VARCHAR);
             CREATE TABLE IF NOT EXISTS __label_sets (layer VARCHAR, script VARCHAR, version BIGINT, n BIGINT);
             CREATE TABLE IF NOT EXISTS __edit_sessions (layer VARCHAR PRIMARY KEY, map_crs VARCHAR, z_value DOUBLE, started TIMESTAMP, next_fid BIGINT, base_version BIGINT);
             ALTER TABLE __edit_sessions ADD COLUMN IF NOT EXISTS z_dem VARCHAR;
             CREATE TABLE IF NOT EXISTS __edit_fids (layer VARCHAR PRIMARY KEY, max_fid BIGINT);
             CREATE TABLE IF NOT EXISTS __edit_undo_meta (layer VARCHAR PRIMARY KEY, version BIGINT, inserted VARCHAR, restored VARCHAR);
             CREATE TABLE IF NOT EXISTS __edit_log (layer VARCHAR, saved_at TIMESTAMP, added BIGINT, changed BIGINT, deleted BIGINT);",
        )?;
        sources::init(&conn)?;
        display::init(&conn)?;
        self.display.clear();
        // Comparison tables are temporary: never kept from an earlier session.
        history::drop_all_diffs(&conn);
        self.diffs.lock().clear();
        // Reload persisted node snapshots (drops stale or orphaned ones).
        self.graph_cache.lock().load(&conn)?;
        let mut stmt = conn.prepare("SELECT info::VARCHAR FROM __layers")?;
        let infos: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let mut layers = self.layers.write();
        layers.clear();
        let mut max_id = 0;
        let project = self.project_dir();
        for j in infos {
            let mut info: LayerInfo = serde_json::from_str(&j)?;
            // Rasters saved in the project folder are stored relative to it.
            if let (Some(r), Some(dir)) = (info.raster.as_mut(), project.as_ref()) {
                if Path::new(&r.path).is_relative() {
                    r.path = dir.join(&r.path).display().to_string();
                }
            }
            if let Some(n) = info.id.trim_start_matches('L').parse::<u64>().ok() {
                max_id = max_id.max(n);
            }
            layers.insert(info.id.clone(), info);
        }
        self.next_id.store(max_id + 1, Ordering::SeqCst);
        // A shape-edit session left open (a crash, or the project closed while editing) is still pending: the layer is marked as being edited.
        let open_sessions: Vec<String> = match conn.prepare("SELECT layer FROM __edit_sessions") {
            Ok(mut s) => s.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?,
            Err(_) => vec![],
        };
        for info in layers.values_mut() {
            info.editing = open_sessions.contains(&info.id);
        }
        display::sweep(&conn, &layers.iter().map(|(k, v)| (k.clone(), v.version)).collect());
        drop(layers);
        let mut stmt = conn.prepare("SELECT node, layer FROM __model_layers")?;
        let nl: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        *self.node_layers.lock() = nl.into_iter().collect();
        drop(stmt);
        // Python definition queries of an opened project: sets that came with the project are never trusted (a shared project could carry a
        // forged one): they are dropped, scripts this computer approved run again now, and the others wait for approval.
        conn.execute_batch("DELETE FROM __defq_fids; DELETE FROM __defq_sets; DELETE FROM __label_vals; DELETE FROM __label_sets;")?;
        let pending: Vec<LayerInfo> = self.layers.read().values().filter(|l| l.definition_python.is_some() || l.label_python.is_some()).cloned().collect();
        for mut info in pending {
            let _ = self.ensure_definition(&conn, &mut info);
            let _ = self.ensure_labels(&conn, &mut info);
            self.layers.write().insert(info.id.clone(), info);
        }
        Ok(())
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        f(&conn)
    }

    /// Run SQL against the project database (integration tests only).
    #[doc(hidden)]
    pub fn with_conn_for_tests<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        self.with_conn(f)
    }

    fn alloc_id(&self) -> String {
        format!("L{}", self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    fn save_meta(&self, conn: &Connection, info: &LayerInfo) -> Result<()> {
        // A raster inside the project folder is stored relative to it, so the
        // project can move between folders and machines.
        let mut stored = info.clone();
        // New data: a Python definition query's fid set is for the old version, so it runs again (if its script is approved).
        self.ensure_definition(conn, &mut stored)?;
        self.ensure_labels(conn, &mut stored)?;
        let info = &stored.clone();
        if let (Some(r), Some(dir)) = (stored.raster.as_mut(), self.project_dir()) {
            if let Ok(rel) = Path::new(&r.path).strip_prefix(&dir) {
                r.path = rel.to_string_lossy().replace('\\', "/");
            }
        }
        conn.execute(
            "INSERT OR REPLACE INTO __layers VALUES (?, ?)",
            duckdb::params![info.id, serde_json::to_string(&stored)?],
        )?;
        self.layers.write().insert(info.id.clone(), info.clone());
        // New data: display tables built from an older version are stale.
        self.invalidate_display(conn, &info.id, Some(info.version));
        Ok(())
    }

    pub fn layer(&self, id: &str) -> Result<LayerInfo> {
        self.layers.read().get(id).cloned().ok_or_else(|| anyhow!("Layer {id} not found"))
    }

    pub fn list_layers(&self) -> Vec<LayerInfo> {
        let mut v: Vec<LayerInfo> = self.layers.read().values().cloned().collect();
        v.sort_by_key(|l| l.id.trim_start_matches('L').parse::<u64>().unwrap_or(0));
        v
    }

    // ---------- layers ----------

    pub fn import(&self, path: &Path) -> Result<Vec<LayerInfo>> {
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("data").to_string();
        let mut task = Task::start(format!("Adding {name}"));
        let out = self.import_with(path, &mut task)?;
        let format = path.extension().and_then(|e| e.to_str()).unwrap_or("data").to_uppercase();
        self.with_conn(|c| {
            for l in &out {
                let cite = sources::for_file(path, &format, if l.is_raster() { 0 } else { l.feature_count }, l.crs_name.as_deref().or(l.crs.as_deref()));
                sources::record(c, &cite, &l.id, &l.name)?;
            }
            Ok(())
        })?;
        Ok(out)
    }

    /// `import_with` for the world module (no file citation is recorded).
    pub(crate) fn import_with_public(&self, path: &Path, task: &mut Task) -> Result<Vec<LayerInfo>> {
        self.import_with(path, task)
    }

    /// Import, reporting each layer of the file as a part of `task`.
    fn import_with(&self, path: &Path, task: &mut Task) -> Result<Vec<LayerInfo>> {
        if !path.exists() {
            anyhow::bail!("File not found: {}", path.display());
        }
        if raster::is_raster_path(path) {
            task.detail("Copying raster and building overviews…");
            return Ok(vec![self.import_raster(path, None)?]);
        }
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut out = Vec::new();
        let sources = layers::sources_for_path(&conn, path)?;
        for (i, src) in sources.iter().enumerate() {
            task.part(i, sources.len());
            task.set(0.1, Some(format!("Reading {}…", src.name)));
            let id = self.alloc_id();
            let mut info = layers::build_layer(&conn, &id, src)?;
            info.source = Some(path.display().to_string());
            self.save_meta(&conn, &info)?;
            task.set(1.0, Some(format!("Indexed {} features", info.feature_count)));
            out.push(info);
        }
        Ok(out)
    }

    /// Add a raster file as a layer (copied into the raster store).
    pub fn import_raster(&self, path: &Path, name: Option<&str>) -> Result<LayerInfo> {
        let stored = raster::import(path)?;
        let id = self.alloc_id();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("raster");
        let info = layers::raster_layer(&id, name.unwrap_or(stem), &stored, Some(path.display().to_string()))?;
        self.with_conn(|c| self.save_meta(c, &info))?;
        Ok(info)
    }

    /// What a 3DEP download of this box would be: output CRS, cells, requests, size, notes (D-1, DEM-2, DEM-10).
    pub fn plan_elevation(&self, bbox: [f64; 4], resolution: f64) -> Result<tnm::ElevationPlan> {
        tnm::plan_elevation(bbox, resolution)
    }

    /// Download USGS 3DEP elevation for a lon/lat box as a raster layer, in tiles. More than 500 MB needs `confirmed`.
    pub fn download_elevation(&self, bbox: [f64; 4], resolution: f64, area_label: &str, confirmed: bool) -> Result<LayerInfo> {
        self.download_elevation_from(&tnm::ElevationService::default(), bbox, resolution, area_label, confirmed)
    }

    /// `download_elevation` from another service URL (tests).
    pub fn download_elevation_from(&self, service: &tnm::ElevationService, bbox: [f64; 4], resolution: f64, area_label: &str, confirmed: bool) -> Result<LayerInfo> {
        let task = Task::start(format!("Downloading elevation for {area_label}")).cancellable();
        let (path, plan) = tnm::download_elevation_from(service, bbox, resolution, confirmed, &|f, msg| {
            if f >= 0.0 {
                task.set(f, Some(msg.to_string()));
            }
            task.check()
        })?;
        let id = self.alloc_id();
        let cells = if plan.geographic { format!("~{:.0} m", plan.used_res) } else { format!("{:.0} m", plan.used_res) };
        let name = format!("Elevation (3DEP {cells}) — {area_label}");
        // 3DEP heights are on NAVD88 in the United States (the service documents it): recorded so mixing can be flagged.
        let _ = vertical::set(&path, "NAVD88");
        let info = layers::raster_layer(&id, &name, &path, Some("USGS 3DEP elevation (The National Map)".into()))?;
        self.with_conn(|c| {
            self.save_meta(c, &info)?;
            // One citation per job, not per tile.
            sources::record(c, &sources::for_3dep(), &info.id, &info.name)
        })?;
        Ok(info)
    }

    /// Colour-ramp library for raster symbology.
    pub fn raster_ramps(&self) -> Vec<raster_style::RampInfo> {
        raster_style::ramps()
    }

    /// Statistics, percentiles and a histogram of one raster band (for stretches and the histogram widget).
    pub fn raster_histogram(&self, id: &str, band: usize, bins: usize) -> Result<serde_json::Value> {
        let info = self.layer(id)?;
        let r = info.raster.as_ref().ok_or_else(|| anyhow!("'{}' is not a raster", info.name))?;
        if band == 0 || band > r.bands {
            return Err(anyhow!("Band {band} doesn't exist; '{}' has {} band(s)", info.name, r.bands));
        }
        let s = raster_style::band_stats(r, band)?;
        Ok(serde_json::json!({
            "min": s.min, "max": s.max, "mean": s.mean, "std": s.std,
            "p2": s.percentile(2.0), "p98": s.percentile(98.0),
            "counts": s.histogram(bins.clamp(2, 1024)),
        }))
    }

    /// Distinct values of a raster band (for the unique-values symbology), with GDAL colour-table colours.
    pub fn raster_unique_values(&self, id: &str, band: usize, max: usize) -> Result<raster_style::UniqueValues> {
        let info = self.layer(id)?;
        let r = info.raster.as_ref().ok_or_else(|| anyhow!("'{}' is not a raster", info.name))?;
        if band == 0 || band > r.bands {
            return Err(anyhow!("Band {band} doesn't exist; '{}' has {} band(s)", info.name, r.bands));
        }
        raster_style::unique_values(r, band, max.clamp(1, 2000))
    }

    /// The layout themes (look + palette).
    pub fn layout_themes(&self) -> Vec<layout::themes::Theme> {
        layout::themes::themes()
    }

    /// A layout with a theme's page look applied.
    pub fn apply_layout_theme(&self, layout: &layout::model::Layout, theme: &str) -> Result<layout::model::Layout> {
        layout::themes::apply(layout, theme)
    }

    /// A picture file as the data URI a layout Picture element stores.
    pub fn read_picture(&self, path: &Path) -> Result<String> {
        layout::shapes::read_picture(path)
    }

    pub fn save_raster_style(&self, path: &Path, style: &raster_style::RasterStyle) -> Result<()> {
        raster_style::save_file(path, style)
    }

    pub fn load_raster_style(&self, path: &Path) -> Result<raster_style::RasterStyle> {
        raster_style::load_file(path)
    }

    /// Class breaks for a raster band: `method` is equal | quantile | jenks | stddev.
    pub fn raster_classify(&self, id: &str, band: usize, method: &str, classes: usize) -> Result<Vec<f64>> {
        let info = self.layer(id)?;
        let r = info.raster.as_ref().ok_or_else(|| anyhow!("'{}' is not a raster", info.name))?;
        if band == 0 || band > r.bands {
            return Err(anyhow!("Band {band} doesn't exist; '{}' has {} band(s)", info.name, r.bands));
        }
        let m: raster_style::ClassMethod = serde_json::from_value(serde_json::json!(method)).map_err(|_| anyhow!("Unknown classification '{method}' (equal, quantile, jenks or stddev)"))?;
        raster_style::classify(&*raster_style::band_stats(r, band)?, m, classes)
    }

    /// Raster value at a lon/lat (identify on rasters).
    pub fn raster_value(&self, id: &str, lon: f64, lat: f64) -> Result<Option<f64>> {
        let info = self.layer(id)?;
        match &info.raster {
            Some(r) => raster::sample(r, lon, lat),
            None => Ok(None),
        }
    }

    pub fn remove_layer(&self, id: &str) -> Result<()> {
        let rid = result_id(id);
        if self.layers.read().contains_key(&rid) {
            self.remove_layer(&rid)?;
        }
        let info = self.layer(id)?;
        let pool = self.pool.read();
        let conn = pool.get()?;
        if !info.is_raster() {
            conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&info.table)))?;
        }
        for z in layers::OVERVIEW_LEVELS {
            conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&layers::overview_table(&info.table, z))))?;
        }
        conn.execute("DELETE FROM __layers WHERE id = ?", [id])?;
        conn.execute("DELETE FROM __model_layers WHERE layer = ?", [id])?;
        self.layers.write().remove(id);
        self.node_layers.lock().retain(|_, l| l != id);
        self.invalidate_display(&conn, id, None);
        Ok(())
    }

    // ---------- attribute editing ----------

    /// Run an edit on a layer and save its updated metadata.
    fn edit<T>(&self, id: &str, f: impl FnOnce(&Connection, &mut LayerInfo) -> Result<T>) -> Result<(LayerInfo, T)> {
        let mut info = self.layer(id)?;
        self.with_conn(|conn| {
            let out = f(conn, &mut info)?;
            self.save_meta(conn, &info)?;
            Ok((info, out))
        })
    }

    pub fn add_field(&self, id: &str, name: &str, ty: &str) -> Result<LayerInfo> {
        Ok(self.edit(id, |c, i| attrs::add_field(c, i, name, ty))?.0)
    }

    pub fn delete_field(&self, id: &str, name: &str) -> Result<LayerInfo> {
        Ok(self.edit(id, |c, i| attrs::delete_field(c, i, name))?.0)
    }

    pub fn rename_field(&self, id: &str, old: &str, new: &str) -> Result<LayerInfo> {
        Ok(self.edit(id, |c, i| attrs::rename_field(c, i, old, new))?.0)
    }

    pub fn update_cell(&self, id: &str, fid: i64, field: &str, value: Option<&str>) -> Result<LayerInfo> {
        Ok(self.edit(id, |c, i| attrs::update_cell(c, i, fid, field, value))?.0)
    }

    pub fn calculate_field(&self, id: &str, field: &str, expr: &str, new_type: Option<&str>, fids: Option<&[i64]>) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::calculate(c, i, field, expr, new_type, fids))
    }

    /// The field calculator with a Python script (see `attrs::calculate_python`). Returns the layer, the rows written and, when failed rows
    /// were skipped, the note to show.
    pub fn calculate_field_python(&self, id: &str, field: &str, script: &str, new_type: Option<&str>, fids: Option<&[i64]>, skip_failed: bool) -> Result<(LayerInfo, i64, Option<String>)> {
        let (info, (n, note)) = self.edit(id, |c, i| attrs::calculate_python(c, i, field, script, new_type, fids, skip_failed))?;
        Ok((info, n, note))
    }

    /// Select by attributes with a Python script (`def calculate(row): return True`).
    pub fn select_python(&self, id: &str, script: &str) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::select_python(c, &info, script))
    }

    pub fn calculate_geometry(&self, id: &str, field: &str, property: &str, area_units: &str, length_units: &str, fids: Option<&[i64]>) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::calculate_geometry(c, i, field, property, area_units, length_units, fids))
    }

    /// A new, empty table (no shapes) with typed fields. `fields` are (name, type) with types from `attrs::FIELD_TYPES`
    /// (text, integer, double, date, boolean).
    pub fn create_table(&self, name: &str, fields: &[(String, String)]) -> Result<LayerInfo> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Give the table a name.");
        }
        if fields.is_empty() {
            bail!("Add at least one field to the table.");
        }
        let mut seen: Vec<String> = Vec::new();
        let mut cols = Vec::new();
        for (f, ty) in fields {
            let f = f.trim();
            if f.is_empty() {
                bail!("A field has no name.");
            }
            if f.eq_ignore_ascii_case("geom") || layers::HIDDEN_COLS.contains(&f) {
                bail!("'{f}' is reserved. Choose another field name.");
            }
            if seen.iter().any(|s| s.eq_ignore_ascii_case(f)) {
                bail!("Two fields are named '{f}'. Field names must differ.");
            }
            seen.push(f.to_string());
            cols.push(format!("CAST(NULL AS {}) AS {}", attrs::sql_type(ty)?, ident(f)));
        }
        let src = ImportSource { name: name.into(), select: format!("SELECT {} WHERE FALSE", cols.join(", ")), crs: None, crs_name: None, warnings: vec![] };
        self.with_conn(|conn| {
            let id = self.alloc_id();
            let info = layers::build_layer(conn, &id, &src)?;
            self.save_meta(conn, &info)?;
            Ok(info)
        })
    }

    /// A blank row at the end of a table; returns the layer and the new row's id.
    pub fn add_row(&self, id: &str) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::add_row(c, i)).map(|(l, fid)| (l, fid))
    }

    /// The rows with these ids as text (to put back after a delete).
    pub fn rows_by_fid(&self, id: &str, fids: &[i64]) -> Result<Vec<attrs::RowData>> {
        let info = self.layer(id)?;
        self.with_conn(|c| attrs::rows_by_fid(c, &info, fids))
    }

    /// Put deleted rows back with their old ids (tables only).
    pub fn restore_rows(&self, id: &str, rows: &[attrs::RowData]) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::restore_rows(c, i, rows))
    }

    pub fn delete_rows(&self, id: &str, fids: &[i64]) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::delete_rows(c, i, fids))
    }

    // ---------- sample data ----------

    /// Download TIGER datasets for a state/county and add them as layers.
    pub fn tiger_import(&self, datasets: &[String], vintage: &str, state: &str, county: Option<&str>, area_label: &str) -> Result<Vec<LayerInfo>> {
        let cache = std::env::temp_dir().join("gis-studio-cache");
        let v = tiger::vintages().ok().and_then(|vs| vs.into_iter().find(|x| x.id == vintage));
        let year_label = v.as_ref().map(|v| if v.id == "Current" { "current".to_string() } else if v.decennial { format!("{} Census", v.year) } else { v.year.to_string() }).unwrap_or_else(|| vintage.to_string());
        let mut task = Task::start(format!("Downloading Census TIGER data for {area_label}")).cancellable();
        let mut out = Vec::new();
        for (i, id) in datasets.iter().enumerate() {
            let d = tiger::dataset(id)?;
            // Each dataset: download is 85% of its share, building the layer the rest.
            task.part(i, datasets.len());
            let path = tiger::fetch(id, vintage, state, county, &cache, &|f, msg| {
                task.set(f * 0.85, Some(msg));
                task.check()
            })?;
            task.set(0.9, Some(format!("{}: building layer…", d.label)));
            for mut info in self.import_with(&path, &mut Task::start(format!("Adding {}", d.label)))? {
                // Roads and water only exist as TIGERweb's current data.
                let yr = if d.by_year { year_label.clone() } else { "current".to_string() };
                info.name = format!("{} ({yr}) — {area_label}", d.label);
                info.source = Some(format!("Census TIGERweb ({yr}): {}", d.label));
                self.with_conn(|c| {
                    self.save_meta(c, &info)?;
                    for cite in sources::for_tiger(d.id, d.label, if d.by_year { vintage } else { "Current" }) {
                        sources::record(c, &cite, &info.id, &info.name)?;
                    }
                    Ok(())
                })?;
                out.push(info);
            }
        }
        Ok(out)
    }

    /// Download The National Map vector datasets for a lon/lat box and add them
    /// as layers, clipped to the box or to a polygon layer's shape when `clip`.
    /// Clip one asset of one STAC scene to the area and add it as a raster layer, cited.
    pub fn stac_import(&self, spec: &stac::ImportSpec) -> Result<LayerInfo> {
        let task = Task::start(format!("Downloading {} for {}", spec.label, spec.area_label)).cancellable();
        task.set(0.1, Some("Reading the image from the cloud…".to_string()));
        let clipped = stac::clip(&spec.href, spec.bbox, stac::MAX_CELLS)?;
        task.check()?;
        task.set(0.8, Some("Building overviews…".to_string()));
        let stored = raster::import(&clipped)?;
        let _ = std::fs::remove_file(&clipped);
        // Copernicus DEM heights are above the EGM2008 geoid.
        if spec.collection == "cop-dem-glo-30" || spec.collection == "cop-dem-glo-90" {
            let _ = vertical::set(&stored, "EGM2008");
        }
        let id = self.alloc_id();
        let info = layers::raster_layer(&id, &spec.label, &stored, Some(spec.href.clone()))?;
        self.with_conn(|c| {
            self.save_meta(c, &info)?;
            if let Some(cite) = stac::citation(&spec.collection, &spec.item_id, &spec.asset) {
                sources::record(c, &cite, &info.id, &info.name)?;
            } else {
                let mut cite = sources::for_file(std::path::Path::new(&spec.href), "STAC asset", 0, info.crs.as_deref());
                cite.key = format!("stac-asset:{}", spec.href);
                cite.kind = "Web service".into();
                cite.title = spec.label.clone();
                sources::record(c, &cite, &info.id, &info.name)?;
            }
            Ok(())
        })?;
        Ok(info)
    }

    /// Download one ArcGIS REST layer (catalog entry or pasted URL) for a lon/lat box and add it, cited. An area with no features adds nothing
    /// and returns an empty list.
    pub fn arcgis_import(&self, spec: &arcgis::Spec, bbox: [f64; 4], area_label: &str) -> Result<Vec<LayerInfo>> {
        let (cite, label) = arcgis::citation(spec)?;
        let (sources, cache_id) = arcgis::sources_of(spec)?;
        let task = Task::start(format!("Downloading {label} for {area_label}")).cancellable();
        let cache = std::env::temp_dir().join("gis-studio-cache");
        let refs: Vec<(&str, &str)> = sources.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let path = tnm_vector::fetch_sources(&label, &cache_id, &refs, bbox, &cache, &|f, msg| {
            if !f.is_nan() {
                task.set(f * 0.85, Some(msg.to_string()));
            } else {
                task.detail(msg.to_string());
            }
            task.check()
        })?;
        let Some(path) = path else { return Ok(vec![]) };
        task.set(0.9, Some(format!("{label}: building the layer…")));
        let mut out = Vec::new();
        for info in self.import_with(&path, &mut Task::start(format!("Adding {label}")))? {
            let info = self.rename_layer(&info.id, &label)?;
            self.with_conn(|c| sources::record(c, &cite, &info.id, &info.name))?;
            out.push(info);
        }
        Ok(out)
    }

    /// Datasets download in parallel; one failing doesn't stop the others.
    pub fn tnm_import(&self, datasets: &[String], bbox: [f64; 4], clip_layer: Option<&str>, clip: bool, area_label: &str) -> Result<TnmImport> {
        let cache = std::env::temp_dir().join("gis-studio-cache");
        let task = Task::start(format!("Downloading from The National Map for {area_label}")).cancellable();
        let clip_wkt = match (clip, clip_layer) {
            (false, _) => None,
            (true, Some(l)) => {
                task.detail("Reading the clip area…");
                Some(self.shape_wkt(l)?)
            }
            (true, None) => {
                let [w, s, e, n] = bbox;
                Some(format!("POLYGON(({w} {s},{e} {s},{e} {n},{w} {n},{w} {s}))"))
            }
        };
        let picked: Vec<&'static tnm_vector::Dataset> = datasets.iter().map(|id| tnm_vector::dataset(id)).collect::<Result<_>>()?;

        // Download: up to 4 at a time, each reporting into its slot; this
        // thread combines the slots into one bar. Workers are detached so
        // Cancel returns at once; a request already in flight finishes in the
        // background and lands in the download cache for next time.
        struct Slot {
            frac: f64,
            msg: String,
            since: std::time::Instant,
            result: Option<Result<Option<PathBuf>>>,
        }
        let slots: Arc<Mutex<Vec<Slot>>> = Arc::new(Mutex::new(
            picked.iter().map(|d| Slot { frac: 0.0, msg: format!("{}: waiting…", d.label), since: std::time::Instant::now(), result: None }).collect(),
        ));
        let queue = Arc::new(Mutex::new((0..picked.len()).rev().collect::<Vec<_>>()));
        let handle = task.handle();
        for _ in 0..picked.len().min(4) {
            let (slots, queue, picked, cache) = (slots.clone(), queue.clone(), picked.clone(), cache.clone());
            std::thread::spawn(move || loop {
                let Some(i) = queue.lock().pop() else { break };
                let report = |f: f64, msg: &str| {
                    let mut sl = slots.lock();
                    if !f.is_nan() {
                        sl[i].frac = f;
                    }
                    sl[i].msg = msg.to_string();
                    sl[i].since = std::time::Instant::now();
                    drop(sl);
                    handle.check()
                };
                let r = handle.check().and_then(|_| tnm_vector::fetch(picked[i].id, bbox, &cache, &report));
                slots.lock()[i].result = Some(r);
            });
        }
        loop {
            {
                let sl = slots.lock();
                let done = sl.iter().filter(|x| x.result.is_some()).count();
                if done == sl.len() {
                    break;
                }
                let overall = sl.iter().map(|x| if x.result.is_some() { 1.0 } else { x.frac }).sum::<f64>() / sl.len() as f64;
                // Name the dataset that has waited longest, so a slow server is visible.
                let detail = sl
                    .iter()
                    .filter(|x| x.result.is_none())
                    .min_by_key(|x| x.since)
                    .map(|x| {
                        let secs = x.since.elapsed().as_secs();
                        if secs >= 3 { format!("{} · waiting {secs} s", x.msg) } else { x.msg.clone() }
                    })
                    .unwrap_or_default();
                task.set(overall * 0.85, Some(format!("{done} of {} done · {detail}", sl.len())));
            }
            task.check()?;
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let fetched: Vec<Result<Option<PathBuf>>> = slots.lock().iter_mut().map(|x| x.result.take().expect("every dataset finished")).collect();

        // Build layers (DuckDB work, one at a time).
        let mut out = TnmImport::default();
        for (i, (d, r)) in picked.iter().zip(fetched).enumerate() {
            task.set(0.85 + 0.15 * i as f64 / picked.len() as f64, Some(format!("{}: building layer…", d.label)));
            let path = match r {
                Ok(Some(p)) => p,
                Ok(None) => {
                    out.empty.push(d.label.into());
                    continue;
                }
                Err(e) => {
                    out.failed.push(format!("{}: {e:#}", d.label));
                    continue;
                }
            };
            for mut info in self.import_with(&path, &mut Task::start(format!("Adding {}", d.label)))? {
                if let Some(w) = &clip_wkt {
                    info = match self.clip_layer_to(&info, w) {
                        Ok(i) => i,
                        Err(e) => {
                            let _ = self.remove_layer(&info.id);
                            out.failed.push(format!("{}: {e:#}", d.label));
                            continue;
                        }
                    };
                    if info.feature_count == 0 {
                        self.remove_layer(&info.id)?;
                        out.empty.push(d.label.into());
                        continue;
                    }
                }
                info.name = format!("{} — {area_label}", d.label);
                info.source = Some(format!("USGS The National Map: {}", d.label));
                self.with_conn(|c| {
                    self.save_meta(c, &info)?;
                    sources::record(c, &sources::for_tnm(d.id), &info.id, &info.name)
                })?;
                out.layers.push(info);
            }
        }
        if out.layers.is_empty() {
            match out.failed.first() {
                Some(f) => anyhow::bail!("{f}"),
                None => anyhow::bail!("The National Map has no {} in this area.", out.empty.join(", ").to_lowercase()),
            }
        }
        Ok(out)
    }

    /// A layer's features dissolved into one shape, as lon/lat WKT.
    fn shape_wkt(&self, id: &str) -> Result<String> {
        let info = self.layer(id)?;
        if !info.has_geometry() {
            anyhow::bail!("{} has no shapes to clip to", info.name);
        }
        let crs = info.crs.clone().ok_or_else(|| anyhow!("{} has no coordinate system", info.name))?;
        self.with_conn(|c| {
            let wkt: Option<String> = c.query_row(
                &format!(
                    "SELECT ST_AsText(ST_Transform(ST_Union_Agg(geom), {}, 'EPSG:4326', true)) FROM {} WHERE geom IS NOT NULL{}",
                    lit(&crs::resolve(&crs)),
                    ident(&info.table),
                    info.def_and()
                ),
                [],
                |r| r.get(0),
            )?;
            wkt.ok_or_else(|| anyhow!("{} has no shapes to clip to", info.name))
        })
    }

    /// Clip a lon/lat layer to a lon/lat WKT shape in place, keeping each
    /// feature's dimension (lines stay lines). Downloaded data can hold invalid
    /// polygons (self-intersections), which make GEOS throw a TopologyException
    /// in the intersection, so invalid features are repaired first
    /// (`ST_MakeValid`, same dimension kept) and the clip shape is made valid
    /// too. One transaction: a failure leaves the layer as downloaded.
    fn clip_layer_to(&self, info: &LayerInfo, wkt: &str) -> Result<LayerInfo> {
        let mut info = info.clone();
        self.with_conn(|c| {
            let t = ident(&info.table);
            let g = format!("ST_MakeValid(ST_GeomFromText({}))", lit(wkt));
            atomically(c, || {
                c.execute_batch(&format!(
                    "UPDATE {t} SET geom = ST_CollectionExtract(ST_MakeValid(geom), ST_Dimension(geom) + 1) WHERE geom IS NOT NULL AND NOT ST_IsValid(geom);
                     DELETE FROM {t} WHERE geom IS NULL OR ST_IsEmpty(geom) OR NOT ST_Intersects(geom, {g});
                     UPDATE {t} SET geom = ST_CollectionExtract(ST_Intersection(geom, {g}), ST_Dimension(geom) + 1) WHERE NOT ST_Within(geom, {g});
                     DELETE FROM {t} WHERE ST_IsEmpty(geom);"
                ))
                .map_err(|e| anyhow!("couldn't clip the features to the area ({e}). Untick \"Clip features to the area\" to keep whole features"))?;
                layers::refresh_display(c, &mut info)?;
                self.save_meta(c, &info)
            })
        })?;
        Ok(info)
    }

    // ---------- sources & citations ----------

    /// A model output's sources are those of every layer that fed it (a layer
    /// showing its own model result counts with the layer it belongs to).
    /// Replaces the output's previous provenance, so re-runs don't pile up.
    fn record_derived(&self, conn: &Connection, info: &LayerInfo, inputs: &[String]) -> Result<()> {
        let mut from: Vec<String> = Vec::new();
        for id in inputs {
            from.push(id.clone());
            if let Some(owner) = self.layers.read().get(id).and_then(|l| l.result_of.clone()) {
                from.push(owner);
            }
        }
        let refs: Vec<&str> = from.iter().map(String::as_str).filter(|f| *f != info.id).collect();
        sources::replace_derived(conn, &refs, &info.id, &format!("{} (model result)", info.name))
    }

    /// The project's data sources (annotated bibliography), in order of first use.
    pub fn sources(&self) -> Result<Vec<sources::SourceEntry>> {
        self.with_conn(sources::list)
    }

    pub fn update_source(&self, citation: &sources::Citation) -> Result<()> {
        self.with_conn(|c| sources::update(c, citation))
    }

    /// Add a citation the user wrote (form or pasted text) to a layer that has none, or to any layer. Layers that were made from this
    /// one (its model results) get it too; layers made later inherit it as they inherit every source.
    pub fn add_layer_source(&self, layer_id: &str, citation: &sources::Citation, accessed: Option<&str>) -> Result<sources::Citation> {
        let info = self.layer(layer_id)?;
        let derived: Vec<(String, String)> = self.layers.read().values().filter(|l| l.result_of.as_deref() == Some(layer_id)).map(|l| (l.id.clone(), l.name.clone())).collect();
        self.with_conn(|c| {
            let added = sources::add_user(c, citation, accessed, &info.id, &info.name)?;
            for (id, name) in &derived {
                sources::inherit(c, &[layer_id], id, name)?;
            }
            Ok(added)
        })
    }

    pub fn remove_source(&self, key: &str) -> Result<()> {
        self.with_conn(|c| sources::remove(c, key))
    }

    /// Log a source shown without a layer (e.g. a basemap).
    pub fn record_source(&self, citation: &sources::Citation, used_as: &str) -> Result<()> {
        self.with_conn(|c| {
            if c.query_row("SELECT count(*) FROM __source_uses WHERE key = ? AND layer_name = ?", [&citation.key, used_as], |r| r.get::<_, i64>(0))? == 0 {
                sources::record(c, citation, "", used_as)?;
            }
            Ok(())
        })
    }

    /// Set or clear (empty/None) a layer's definition query. The query is
    /// checked first; a bad query leaves the layer unchanged.
    pub fn set_definition_query(&self, id: &str, query: Option<&str>) -> Result<LayerInfo> {
        let q = query.map(str::trim).filter(|q| !q.is_empty()).map(String::from);
        let mut info = self.layer(id)?;
        self.with_conn(|conn| {
            info.query_count = match &q {
                Some(q) => Some(
                    conn.query_row(&format!("SELECT count(*) FROM {} WHERE ({q})", ident(&info.table)), [], |r| r.get(0))
                        .map_err(|e| anyhow!("{}", e.to_string().split("\n\nLINE").next().unwrap_or("Invalid query")))?,
                ),
                None => None,
            };
            info.definition_query = q;
            info.definition_python = None;
            info.definition_error = None;
            conn.execute("DELETE FROM __defq_fids WHERE layer = ?", [&info.id])?;
            conn.execute("DELETE FROM __defq_sets WHERE layer = ?", [&info.id])?;
            info.version += 1;
            self.save_meta(conn, &info)?;
            Ok(info)
        })
    }

    /// Set or clear a layer's Python definition query (`def calculate(row): return True` keeps a feature). The script runs once, here and
    /// whenever the layer's data changes, never at draw time: the ids of the features it keeps are stored apart from the layer, and the
    /// layer has no features until the script is approved and has run (see `LayerInfo::def_condition`). A failing script (any row) leaves
    /// the layer empty with the reason in `definition_error`. Replaces a SQL definition query.
    pub fn set_definition_python(&self, id: &str, script: Option<&str>) -> Result<LayerInfo> {
        let script = script.filter(|s| !s.trim().is_empty()).map(String::from);
        let mut info = self.layer(id)?;
        self.with_conn(|conn| {
            conn.execute("DELETE FROM __defq_fids WHERE layer = ?", [&info.id])?;
            conn.execute("DELETE FROM __defq_sets WHERE layer = ?", [&info.id])?;
            info.definition_query = None;
            info.definition_python = script;
            info.definition_error = None;
            info.query_count = None;
            info.version += 1;
            self.save_meta(conn, &info)?;
            self.layer(&info.id)
        })
    }

    /// Set or clear a layer's Python label expression: `def calculate(row): return "text"` gives each feature's label. It runs once (and again
    /// when the data changes), not while drawing; labels use the pseudo-field `__label`. Unapproved or failing, it gives no labels.
    pub fn set_label_python(&self, id: &str, script: Option<&str>) -> Result<LayerInfo> {
        let script = script.filter(|s| !s.trim().is_empty()).map(String::from);
        let mut info = self.layer(id)?;
        self.with_conn(|conn| {
            conn.execute("DELETE FROM __label_vals WHERE layer = ?", [&info.id])?;
            conn.execute("DELETE FROM __label_sets WHERE layer = ?", [&info.id])?;
            info.label_python = script;
            info.label_error = None;
            info.version += 1;
            self.save_meta(conn, &info)?;
            self.layer(&info.id)
        })
    }

    /// Like `ensure_definition`, for the label expression: text for each feature, kept in `__label_vals`.
    fn ensure_labels(&self, conn: &Connection, info: &mut LayerInfo) -> Result<()> {
        let Some(script) = info.label_python.clone().filter(|s| !s.trim().is_empty()) else { return Ok(()) };
        let hash = pyscript::script_hash(&script);
        let done: i64 = conn.query_row("SELECT count(*) FROM __label_sets WHERE layer = ? AND script = ? AND version = ?", duckdb::params![info.id, hash, info.version as i64], |r| r.get(0))?;
        if done > 0 || !pyscript::is_approved(&script) {
            return Ok(());
        }
        let run = pyscript::evaluate_layer(conn, info, &script, pyscript::Mode::Field, pyscript::OutType::Text, false, None, false);
        conn.execute("DELETE FROM __label_vals WHERE layer = ?", [&info.id])?;
        conn.execute("DELETE FROM __label_sets WHERE layer = ?", [&info.id])?;
        match run {
            Ok(ev) => {
                conn.execute(
                    &format!("INSERT INTO __label_vals SELECT ?, ?, ?, s.__fid, o.s FROM {} s JOIN {} o USING (__rn)", ident(&ev.snapshot), ident(&ev.staging)),
                    duckdb::params![info.id, hash, info.version as i64],
                )?;
                let n: i64 = conn.query_row("SELECT count(*) FROM __label_vals WHERE layer = ?", [&info.id], |r| r.get(0))?;
                conn.execute("INSERT INTO __label_sets VALUES (?, ?, ?, ?)", duckdb::params![info.id, hash, info.version as i64, n])?;
                let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}; DROP TABLE IF EXISTS {}", ident(&ev.snapshot), ident(&ev.staging)));
                info.label_error = None;
            }
            Err(e) => {
                info.label_error = Some(if progress::is_cancelled(&e) { "The label script was cancelled; nothing is labelled until it runs.".into() } else { format!("{e:#}") });
                conn.execute("INSERT INTO __label_sets VALUES (?, ?, ?, 0)", duckdb::params![info.id, hash, info.version as i64])?;
            }
        }
        Ok(())
    }

    /// The user approved this script on this computer: record it, and run the layers' Python definition queries that were waiting for it.
    pub fn approve_script(&self, script: &str) -> Result<pyscript::TrustInfo> {
        pyscript::approve(script)?;
        let hash = pyscript::script_hash(script);
        let waiting: Vec<LayerInfo> = self
            .layers
            .read()
            .values()
            .filter(|l| [&l.definition_python, &l.label_python].iter().any(|p| p.as_deref().is_some_and(|s| pyscript::script_hash(s) == hash)))
            .cloned()
            .collect();
        self.with_conn(|conn| {
            for mut info in waiting {
                info.version += 1; // tiles and tables drawn from the empty result are stale now
                self.save_meta(conn, &info)?;
            }
            Ok(())
        })?;
        Ok(pyscript::trust_info(script))
    }

    /// Make sure a Python definition query has its fid set for the layer's current data version: run the script when it is approved and
    /// not yet run for this (layer, script, version). Nothing here ever lets the layer through unfiltered: until the set exists the
    /// condition `def_condition` builds selects no features.
    fn ensure_definition(&self, conn: &Connection, info: &mut LayerInfo) -> Result<()> {
        let Some(script) = info.definition_python.clone().filter(|s| !s.trim().is_empty()) else { return Ok(()) };
        let hash = pyscript::script_hash(&script);
        let done: i64 = conn.query_row("SELECT count(*) FROM __defq_sets WHERE layer = ? AND script = ? AND version = ?", duckdb::params![info.id, hash, info.version as i64], |r| r.get(0))?;
        if done > 0 || !pyscript::is_approved(&script) {
            return Ok(());
        }
        // The script sees attributes only (and every feature: the definition query being defined is not applied to itself).
        let run = pyscript::evaluate_layer(conn, info, &script, pyscript::Mode::Filter, pyscript::OutType::Boolean, false, None, false);
        conn.execute("DELETE FROM __defq_fids WHERE layer = ?", [&info.id])?;
        conn.execute("DELETE FROM __defq_sets WHERE layer = ?", [&info.id])?;
        match run {
            Ok(ev) => {
                conn.execute(
                    &format!(
                        "INSERT INTO __defq_fids SELECT ?, ?, ?, s.__fid FROM {} s WHERE s.__rn IN (SELECT __rn FROM {})",
                        ident(&ev.snapshot),
                        ident(&ev.staging)
                    ),
                    duckdb::params![info.id, hash, info.version as i64],
                )?;
                let kept: i64 = conn.query_row("SELECT count(*) FROM __defq_fids WHERE layer = ?", [&info.id], |r| r.get(0))?;
                conn.execute("INSERT INTO __defq_sets VALUES (?, ?, ?, ?)", duckdb::params![info.id, hash, info.version as i64, kept])?;
                let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}; DROP TABLE IF EXISTS {}", ident(&ev.snapshot), ident(&ev.staging)));
                info.query_count = Some(kept);
                info.definition_error = None;
            }
            Err(e) => {
                if progress::is_cancelled(&e) {
                    info.definition_error = Some("The script was cancelled; the layer shows no features until it runs.".into());
                } else {
                    info.definition_error = Some(format!("{e:#}"));
                }
                // The marker records that this version was tried, so a failing script is not rerun on every save; a data change retries.
                conn.execute("INSERT INTO __defq_sets VALUES (?, ?, ?, 0)", duckdb::params![info.id, hash, info.version as i64])?;
                info.query_count = Some(0);
            }
        }
        Ok(())
    }

    /// Copy a layer (typically a node-tree result) into a new, independent
    /// layer. Respects its definition query; `fids` copies only those features.
    pub fn copy_layer(&self, id: &str, name: &str, fids: Option<&[i64]>) -> Result<LayerInfo> {
        let src_info = self.layer(id)?;
        let from: Vec<&str> = std::iter::once(id).chain(src_info.result_of.as_deref()).collect();
        if let Some(r) = &src_info.raster {
            let info = self.import_raster(Path::new(&r.path), Some(name))?;
            self.with_conn(|c| sources::inherit(c, &from, &info.id, &info.name))?;
            return Ok(info);
        }
        let only = match fids {
            Some([]) => " AND FALSE".to_string(),
            Some(ids) => format!(" AND __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        self.copy_layer_where(&src_info, name, &only)
    }

    /// A vector layer's rows that pass `extra` (SQL starting with " AND …", on the layer table) as a new layer.
    /// Keeps the CRS, the source and the citations of the input.
    pub(crate) fn copy_layer_where(&self, src_info: &LayerInfo, name: &str, extra: &str) -> Result<LayerInfo> {
        let from: Vec<&str> = std::iter::once(src_info.id.as_str()).chain(src_info.result_of.as_deref()).collect();
        let hidden = if src_info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
        let src = ImportSource {
            name: name.to_string(),
            select: format!("SELECT * EXCLUDE ({hidden}) FROM {} WHERE TRUE{}{extra}", ident(&src_info.table), src_info.def_and()),
            crs: src_info.crs.clone(),
            crs_name: src_info.crs_name.clone(),
            warnings: vec![],
        };
        self.with_conn(|conn| {
            let id = self.alloc_id();
            let mut info = layers::build_layer(conn, &id, &src)?;
            info.source = src_info.source.clone();
            self.save_meta(conn, &info)?;
            sources::inherit(conn, &from, &info.id, &info.name)?;
            Ok(info)
        })
    }

    pub fn rename_layer(&self, id: &str, name: &str) -> Result<LayerInfo> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("A layer needs a name; the old name was kept");
        }
        let mut info = self.layer(id)?;
        info.name = name.to_string();
        self.with_conn(|c| {
            self.save_meta(c, &info)?;
            sources::rename_layer(c, id, name)
        })?;
        Ok(info)
    }

    /// Set the CRS of a layer whose CRS is missing or wrong (no reprojection;
    /// coordinates are reinterpreted). Use the Project tool to reproject.
    pub fn define_projection(&self, id: &str, crs: &str, crs_name: Option<String>) -> Result<LayerInfo> {
        if self.layer(id)?.editing {
            anyhow::bail!("This layer is being edited. Save or discard the shape edits first; changing its coordinate system would renumber its features.");
        }
        let mut info = self.layer(id)?;
        let pool = self.pool.read();
        let conn = pool.get()?;
        crs::validate(&conn, crs)?;
        info.crs = Some(crs::resolve(crs));
        info.crs_name = crs_name;
        info.warnings.retain(|w| !w.contains("coordinate system") && !w.contains("Define Projection"));
        layers::refresh_display(&conn, &mut info)?;
        self.save_meta(&conn, &info)?;
        Ok(info)
    }

    pub fn suggest_crs(&self, id: &str) -> Result<Vec<crs::Suggestion>> {
        let info = self.layer(id)?;
        Ok(match info.native_extent {
            Some([a, b, c, d]) => crs::suggest(a, b, c, d),
            None => vec![],
        })
    }

    pub fn transform_point(&self, x: f64, y: f64, from: &str, to: &str) -> Result<(f64, f64)> {
        self.with_conn(|c| crs::transform_point(c, x, y, from, to))
    }

    /// Point scale factor k of `crs` at (x, y) in that CRS: ground distance = grid distance / k
    /// (1 for geographic CRSs). The map's `1:N` scale uses it (M-D5).
    pub fn map_scale_factor(&self, crs: &str, x: f64, y: f64) -> Result<f64> {
        let body = layout::geo::Body::get("earth")?;
        let fc = layout::geo::FrameCrs::new(crs, &body)?;
        Ok(layout::geo::factors(&body, &fc, x, y)?.k)
    }

    /// Known-answer check of the datum shifts (RD-3): `ok` is false when PROJ's datum data is missing.
    pub fn datum_data_status(&self) -> crs::DatumReport {
        self.with_conn(|c| Ok(crs::datum_data_report(c))).unwrap_or_else(|e| crs::DatumReport { ok: false, problems: vec![format!("The datum check could not run: {e}")], proj_data: None })
    }

    /// Name, code, kind, unit, datum and area of use of a definition (the picker's preview).
    pub fn crs_info(&self, crs: &str) -> Result<crs::CrsInfo> {
        crs::info(crs)
    }

    /// How accurate moving coordinates from `from` to `to` is over an area (west, south, east, north in degrees).
    pub fn transform_report(&self, from: &str, to: &str, aoi: Option<[f64; 4]>) -> Result<transform_report::TransformReport> {
        transform_report::transform_report(from, to, aoi)
    }

    /// The same for a layer's extent (minx, miny, maxx, maxy in `from`).
    pub fn transform_report_extent(&self, from: &str, to: &str, extent: Option<[f64; 4]>) -> Result<transform_report::TransformReport> {
        transform_report::report_for_extent(from, to, extent)
    }

    /// True when approximate datum shifts are refused (Strict); false (the default) warns and continues.
    pub fn datum_policy_strict(&self) -> bool {
        transform_report::is_strict()
    }

    pub fn set_datum_policy(&self, strict: bool) {
        transform_report::set_strict(strict)
    }

    /// Every text slot of every layout element (`layout/slots.rs`): the Properties pane builds its Text section from this.
    pub fn layout_text_slots(&self) -> Vec<layout::slots::Slot> {
        layout::slots::SLOTS.to_vec()
    }

    /// The Topo Map Builder's layout and self-check for a centre, scale and paper (`layout/topo.rs`).
    pub fn build_topo_layout(&self, spec: &layout::topo::TopoSpec, today: &str) -> Result<layout::topo::TopoBuild> {
        layout::topo::build(spec, today)
    }

    /// Topo Map Builder, data step: contour lines (with ELEV and INDEX fields; every `index_every`-th line is an index contour) and a
    /// north-west-lit hillshade, both made from one elevation raster as new layers that keep its sources. `interval` is in the
    /// raster's units (metres for 3DEP).
    pub fn topo_prepare_data(&self, dem: &str, interval: f64, index_every: u32, label: &str, units: &str) -> Result<TopoData> {
        let info = self.layer(dem)?;
        if info.raster.as_ref().map_or(true, |r| r.kind != "elevation") {
            return Err(anyhow!("'{}' is not an elevation raster; choose the elevation layer", info.name));
        }
        if !(interval.is_finite() && interval > 0.0) {
            return Err(anyhow!("The contour interval must be greater than zero"));
        }
        let src = PathBuf::from(&info.raster.as_ref().unwrap().path);
        // The numbers on the lines are in the sheet's units, whole (the field ELEV stays in the raster's metres): TP-6.
        let label_expr = if units == "imperial" { "CAST(round(ELEV * 3.280839895) AS BIGINT)::VARCHAR" } else { "CAST(round(ELEV) AS BIGINT)::VARCHAR" };
        // Contours go through the same graph machinery as the Contour tool, so the layer is exactly what the tool would make.
        let graph: graph::Graph = serde_json::from_value(serde_json::json!({
            "nodes": [
                { "id": "in", "tool": "input_layer", "params": { "layer": dem } },
                { "id": "ct", "tool": "contours", "params": { "interval": interval, "base": 0, "index_every": index_every } },
                { "id": "lb", "tool": "calculate_field", "params": { "field": "ELEV_LABEL", "expression": label_expr } },
                { "id": "out", "tool": "add_to_map", "params": { "name": format!("Contours — {label}") } }
            ],
            "edges": [
                { "source": "in", "target": "ct", "targetHandle": "in" },
                { "source": "ct", "target": "lb", "targetHandle": "in" },
                { "source": "lb", "target": "out", "targetHandle": "in" }
            ]
        }))?;
        let results = self.run_graph(&graph, None, &mut |_| {})?;
        if let Some(bad) = results.iter().find(|r| r.error.is_some()) {
            return Err(anyhow!("Contours could not be made: {}", bad.error.clone().unwrap_or_default()));
        }
        let contours = results.iter().find(|r| r.id == "out").and_then(|r| r.layer.clone()).ok_or_else(|| anyhow!("The contour layer was not created"))?;
        // Hillshade lit from the north-west at 45°, the cartographic convention.
        let shade = raster::hillshade(&src, 315.0, 45.0, 1.0, false)?;
        let id = self.alloc_id();
        let name = format!("Hillshade — {label}");
        let hillshade = layers::raster_layer(&id, &name, &shade, Some(format!("Hillshade of {}", info.name)))?;
        self.with_conn(|c| {
            self.save_meta(c, &hillshade)?;
            sources::inherit(c, &[dem], &hillshade.id, &hillshade.name)
        })?;
        Ok(TopoData { contours, hillshade })
    }

    /// A USNG/MGRS grid layer for a longitude/latitude box (the map view): squares of `interval` metres, each labelled with its reference,
    /// added as a new layer. Made by the USNG / MGRS Grid tool from a throw-away extent layer, which is removed again.
    pub fn usng_grid_layer(&self, bbox: [f64; 4], interval: f64, style: &str, datum: &str, name: &str) -> Result<LayerInfo> {
        let [w, s, e, n] = bbox;
        if !bbox.iter().all(|v| v.is_finite()) || w >= e || s >= n {
            return Err(anyhow!("The area for the grid is not a valid box"));
        }
        if usng::is_polar((s + n) / 2.0) {
            return Err(anyhow!("This area is in the polar (UPS) part of the grid; the grid layer draws the UTM part (80°S to 84°N)"));
        }
        let (s, n) = (s.max(-79.99), n.min(83.99));
        let wkt = format!("POLYGON(({w} {s}, {e} {s}, {e} {n}, {w} {n}, {w} {s}))");
        let extent = self.create_features("USNG extent", &[(wkt, Default::default())])?;
        let result = (|| {
            let graph: Graph = serde_json::from_value(serde_json::json!({
                "nodes": [
                    { "id": "in", "tool": "input_layer", "params": { "layer": extent.id } },
                    { "id": "g", "tool": "usng_grid", "params": { "interval": interval.to_string(), "style": style, "datum": datum, "field": style.to_uppercase(), "clip": false } },
                    { "id": "out", "tool": "add_to_map", "params": { "name": name } }
                ],
                "edges": [
                    { "source": "in", "target": "g", "targetHandle": "in" },
                    { "source": "g", "target": "out", "targetHandle": "in" }
                ]
            }))?;
            let results = self.run_graph(&graph, None, &mut |_| {})?;
            if let Some(bad) = results.iter().find(|r| r.error.is_some()) {
                return Err(anyhow!("{}", bad.error.clone().unwrap_or_default()));
            }
            results.iter().find(|r| r.id == "out").and_then(|r| r.layer.clone()).ok_or_else(|| anyhow!("The grid layer was not created"))
        })();
        let _ = self.remove_layer(&extent.id);
        result
    }

    /// The UTM coordinate system a USNG/MGRS grid uses at a place: zone (with the Norway/Svalbard exceptions), hemisphere and datum
    /// ("auto" = NAD 83 in zones 1–23 north, WGS 84 elsewhere), so choosing USNG or MGRS needs no coordinate system to be picked.
    pub fn usng_crs_at(&self, lon: f64, lat: f64, datum: &str) -> Result<usng::UsngCrs> {
        usng::crs_at(lon, lat, usng::Datum::parse(datum)?)
    }

    /// Every datum grid PROJ-data offers, sorted by continent, country and purpose, with where each is installed.
    pub fn list_datum_grids(&self) -> Vec<grids::GridEntry> {
        grids::list()
    }

    /// Download one datum grid to the user's grid folder (checksum-verified, cancellable). Grids above 50 MB need `confirmed`.
    pub fn download_datum_grid(&self, name: &str, confirmed: bool) -> Result<grids::GridEntry> {
        grids::download(name, confirmed)
    }

    /// Remove a datum grid the user downloaded (shipped grids are never removed).
    pub fn remove_datum_grid(&self, name: &str) -> Result<()> {
        grids::remove(name)
    }

    /// How well this installation moves coordinates in each checked region.
    pub fn datum_coverage(&self) -> Vec<(String, transform_report::TransformReport)> {
        transform_report::coverage()
    }

    /// Scale factor k and meridian convergence (degrees) of `crs` at (x, y) in that CRS.
    pub fn map_factors(&self, crs: &str, x: f64, y: f64) -> Result<(f64, f64)> {
        let body = layout::geo::Body::get("earth")?;
        let fc = layout::geo::FrameCrs::new(crs, &body)?;
        let f = layout::geo::factors(&body, &fc, x, y)?;
        Ok((f.k, f.convergence_deg))
    }

    /// True when two definitions mean the same CRS (by meaning, not spelling).
    pub fn same_crs(&self, a: &str, b: &str) -> bool {
        crs::same(a, b)
    }

    pub fn validate_crs(&self, crs: &str) -> Result<()> {
        self.with_conn(|c| crs::validate(c, crs))
    }

    // ---------- tiles ----------

    pub fn tile(&self, id: &str, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Arc<Vec<u8>>> {
        let info = self.layer(id)?;
        let key: TileKey = (id.to_string(), info.version, z, x, y, fields.join(","));
        if let Some(t) = self.tile_cache.lock().get(&key) {
            return Ok(t.clone());
        }
        let t = if let Some(r) = &info.raster {
            Arc::new(if info.crs.is_some() { raster::render_tile(r, z, x, y, &raster_style::RasterStyle::from_token(fields.first().map_or("", |f| f.as_str()))?)? } else { Vec::new() })
        } else {
            let pool = self.pool.read();
            let conn = pool.get()?;
            Arc::new(tiles::build_tile(&conn, &info, z, x, y, fields)?)
        };
        self.tile_cache.lock().put(key, t.clone());
        Ok(t)
    }

    /// A tile of the bundled Natural Earth basemap on the grid of the map CRS `key` (not a layer: it has no entry in the project).
    fn natural_earth_tile(&self, key: &str, z: u32, x: u32, y: u32) -> Result<Arc<Vec<u8>>> {
        let cache_key: TileKey = (NATURAL_EARTH_TILE_ID.to_string(), 0, z, x, y, key.to_string());
        if let Some(t) = self.tile_cache.lock().get(&cache_key) {
            return Ok(t.clone());
        }
        let (crs, bounds, size) = if key == display::MERCATOR_KEY {
            let world = 20037508.342789244;
            let span = 2.0 * world / 2f64.powi(z as i32);
            let (x0, y1) = (-world + x as f64 * span, world - y as f64 * span);
            (crs::WEB_MERCATOR.to_string(), [x0, y1 - span, x0 + span, y1], 256)
        } else {
            let (crs, di) = self.display_key(key)?;
            (crs, di.grid.tile_bounds(z, x, y)?, di.grid.tile_size)
        };
        let png = {
            let _slot = raster::WarpSlot::take();
            layout::basemap::natural_earth_tile(&crs, bounds, size)?
        };
        let t = Arc::new(png);
        self.tile_cache.lock().put(cache_key, t.clone());
        Ok(t)
    }

    /// A tile of an on-screen raster basemap (`provider`: `usgsTopo`, `osm` …) on the grid of the map CRS `key`.
    fn basemap_tile(&self, provider: &str, key: &str, z: u32, x: u32, y: u32) -> Result<Arc<Vec<u8>>> {
        let cache_key: TileKey = (format!("{BASEMAP_TILE_PREFIX}{provider}"), 0, z, x, y, key.to_string());
        if let Some(t) = self.tile_cache.lock().get(&cache_key) {
            return Ok(t.clone());
        }
        let (crs, bounds, size) = self.screen_tile_geometry(key, z, x, y)?;
        let png = {
            let _slot = raster::WarpSlot::take();
            layout::basemap::screen_raster_tile(provider, &crs, bounds, size)?
        };
        let t = Arc::new(png);
        self.tile_cache.lock().put(cache_key, t.clone());
        Ok(t)
    }

    /// The Web Mercator rectangle that covers `extent` of the map CRS `key` (so a vector basemap can be drawn over it in Web Mercator and
    /// warped back). None when no part of the rectangle can be placed in Web Mercator. Edges are sampled densely; a rectangle that reaches
    /// toward a pole, where Web Mercator ends, is given the whole width of the world.
    pub fn basemap_mercator_extent(&self, key: &str, extent: [f64; 4]) -> Result<Option<[f64; 4]>> {
        if !(extent[0] < extent[2] && extent[1] < extent[3]) || !extent.iter().all(|v| v.is_finite()) {
            bail!("The view has no extent");
        }
        let (crs, _) = if key == display::MERCATOR_KEY { (crs::WEB_MERCATOR.to_string(), ()) } else { (self.display_key(key)?.0, ()) };
        if key == display::MERCATOR_KEY {
            return Ok(Some(extent));
        }
        let world = 20037508.342789244;
        let lim = layout::basemap::MERCATOR_MAX_LAT;
        let n = 24;
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let (x, y) = (extent[0] + t * (extent[2] - extent[0]), extent[1] + t * (extent[3] - extent[1]));
            pts.extend([(x, extent[1]), (x, extent[3]), (extent[0], y), (extent[2], y)]);
        }
        for i in 1..n {
            for j in 1..n {
                pts.push((extent[0] + i as f64 / n as f64 * (extent[2] - extent[0]), extent[1] + j as f64 / n as f64 * (extent[3] - extent[1])));
            }
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut any = false;
        let mut polar = false;
        // One query for all the sample points (a point PROJ can't place comes back NULL and is skipped).
        let values = pts.iter().map(|(x, y)| format!("({x}, {y})")).collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT TRY(ST_X(p)), TRY(ST_Y(p)) FROM (SELECT TRY(ST_Transform(ST_Point(x, y), {}, {}, true)) AS p FROM (VALUES {values}) t(x, y))",
            db::lit(&crs::resolve(&crs)),
            db::lit(&crs::resolve(crs::WGS84))
        );
        let lonlat: Vec<(Option<f64>, Option<f64>)> = self.with_conn(|c| {
            let mut st = c.prepare(&sql)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<f64>>(1)?)))?;
            Ok(rows.collect::<Result<_, _>>()?)
        })?;
        for (lon, lat) in lonlat {
            let (Some(lon), Some(lat)) = (lon, lat) else { continue };
            if !(lon.is_finite() && lat.is_finite()) {
                continue;
            }
            if lat.abs() > 80.0 {
                polar = true;
            }
            let lat = lat.clamp(-lim, lim);
            let (mx, my) = (lon.to_radians() * 6378137.0, (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * 6378137.0);
            x0 = x0.min(mx);
            x1 = x1.max(mx);
            y0 = y0.min(my);
            y1 = y1.max(my);
            any = true;
        }
        if !any {
            return Ok(None);
        }
        if polar {
            x0 = -world;
            x1 = world;
        }
        // A little room around, so the warp's resampling has pixels on every side.
        let (px, py) = ((x1 - x0) * 0.02 + 1.0, (y1 - y0) * 0.02 + 1.0);
        Ok(Some([(x0 - px).max(-world), (y0 - py).max(-world), (x1 + px).min(world), (y1 + py).min(world)]))
    }

    /// A vector basemap the map view drew in Web Mercator (`png`, covering `merc`) placed in the rectangle `extent` of the map CRS `key`.
    pub fn warp_basemap_picture(&self, png: &[u8], merc: [f64; 4], key: &str, extent: [f64; 4], out_w: u32, out_h: u32) -> Result<Vec<u8>> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(png)).read_info().map_err(|e| anyhow!("The basemap picture is not a PNG: {e}"))?;
        let mut buf = vec![0u8; dec.output_buffer_size().ok_or_else(|| anyhow!("The basemap picture is too large"))?];
        let info = dec.next_frame(&mut buf).map_err(|e| anyhow!("The basemap picture can't be read: {e}"))?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            bail!("The basemap picture must be 8-bit RGBA");
        }
        let crs = if key == display::MERCATOR_KEY { crs::WEB_MERCATOR.to_string() } else { self.display_key(key)?.0 };
        let _slot = raster::WarpSlot::take();
        layout::basemap::warp_mercator_picture(&buf[..info.buffer_size()], info.width, info.height, merc, &crs, extent, out_w, out_h)
    }

    /// [`Engine::warp_basemap_picture`] with the pictures as base64 text, the way they cross the command interface.
    pub fn warp_basemap_picture_b64(&self, png_b64: &str, merc: [f64; 4], key: &str, extent: [f64; 4], out_w: u32, out_h: u32) -> Result<String> {
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD.decode(png_b64.trim()).map_err(|e| anyhow!("The basemap picture is not base64: {e}"))?;
        let out = self.warp_basemap_picture(&png, merc, key, extent, out_w, out_h)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    /// The CRS, the rectangle and the pixel size of tile (z, x, y) on the grid of the map CRS `key`.
    fn screen_tile_geometry(&self, key: &str, z: u32, x: u32, y: u32) -> Result<(String, [f64; 4], u32)> {
        if key == display::MERCATOR_KEY {
            let world = 20037508.342789244;
            let span = 2.0 * world / 2f64.powi(z as i32);
            let (x0, y1) = (-world + x as f64 * span, world - y as f64 * span);
            return Ok((crs::WEB_MERCATOR.to_string(), [x0, y1 - span, x0 + span, y1], 256));
        }
        let (crs, di) = self.display_key(key)?;
        Ok((crs, di.grid.tile_bounds(z, x, y)?, di.grid.tile_size))
    }

    /// A tile on the grid of the map CRS `key` (from `crs_display_info`).
    /// `3857` is the Web Mercator tile above; any other key reads the layer's
    /// display table and fails while that is still being built.
    pub fn tile_in_crs(&self, id: &str, key: &str, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Arc<Vec<u8>>> {
        if id == NATURAL_EARTH_TILE_ID {
            return self.natural_earth_tile(key, z, x, y);
        }
        if let Some(provider) = id.strip_prefix(BASEMAP_TILE_PREFIX) {
            return self.basemap_tile(provider, key, z, x, y);
        }
        if key == display::MERCATOR_KEY {
            return self.tile(id, z, x, y, fields);
        }
        let info = self.layer(id)?;
        if let Some(r) = &info.raster {
            // A raster tile on the map CRS's own grid: the file warped straight to that tile's rectangle (PROJ does the work, so a pole,
            // a conic or a datum with grids is handled as for the vectors). No display table is needed.
            let (crs, di) = self.display_key(key)?;
            let token = fields.first().map_or("", |f| f.as_str());
            let cache_key: TileKey = (id.to_string(), info.version, z, x, y, format!("{key}|{token}"));
            if let Some(t) = self.tile_cache.lock().get(&cache_key) {
                return Ok(t.clone());
            }
            if info.crs.is_none() {
                return Ok(Arc::new(Vec::new()));
            }
            let style = raster_style::RasterStyle::from_token(token)?;
            let bounds = di.grid.tile_bounds(z, x, y)?;
            let size = di.grid.tile_size;
            let png = {
                let _slot = raster::WarpSlot::take(); // at most one warp per core at a time (no GDAL handle is shared between threads)
                raster::render_extent_styled(r, &style, &crs, bounds, size, size)?
            };
            let t = Arc::new(png);
            self.tile_cache.lock().put(cache_key, t.clone());
            return Ok(t);
        }
        let (_, di) = self.display_key(key)?;
        let cache_key: TileKey = (id.to_string(), info.version, z, x, y, format!("{key}|{}", fields.join(",")));
        if let Some(t) = self.tile_cache.lock().get(&cache_key) {
            return Ok(t.clone());
        }
        if !info.has_geometry() || info.crs.is_none() {
            return Ok(Arc::new(Vec::new()));
        }
        let t = self.with_conn(|conn| {
            let entry = display::lookup(conn, id, key, info.version)?
                .ok_or_else(|| anyhow!("'{}' hasn't been projected for display in this coordinate system yet", info.name))?;
            tiles::build_tile_display(conn, &info, &entry, &di.grid, z, x, y, fields)
        })?;
        let t = Arc::new(t);
        self.tile_cache.lock().put(cache_key, t.clone());
        Ok(t)
    }

    /// Build a tile without the cache (benchmarks).
    pub fn tile_uncached(&self, id: &str, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Vec<u8>> {
        let info = self.layer(id)?;
        self.with_conn(|c| tiles::build_tile(c, &info, z, x, y, fields))
    }

    /// Pre-build tiles for zooms 0..=max_zoom in parallel so zoomed-out views
    /// are instant. Stops at the first zoom needing more than 1024 tiles;
    /// deeper zooms are built on demand.
    pub fn prewarm(&self, id: &str, max_zoom: u32, fields: &[String]) -> Result<usize> {
        let info = self.layer(id)?;
        let Some(ext) = info.extent else { return Ok(0) };
        let mut jobs = Vec::new();
        for z in 0..=max_zoom {
            let tiles = tiles::tiles_for_extent(ext, z);
            if tiles.len() > 1024 {
                break;
            }
            jobs.extend(tiles.into_iter().map(|(x, y)| (z, x, y)));
        }
        let n = jobs.len();
        let task = Task::start(format!("Drawing {}", info.name));
        let built = AtomicU64::new(0);
        let queue = Mutex::new(jobs);
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| loop {
                    let Some((z, x, y)) = queue.lock().pop() else { break };
                    let _ = self.tile(id, z, x, y, fields);
                    let b = built.fetch_add(1, Ordering::Relaxed) + 1;
                    if b % 8 == 0 || b as usize == n {
                        task.set(b as f64 / n.max(1) as f64, Some(format!("Map tiles {b} of {n}")));
                    }
                });
            }
        });
        Ok(n)
    }

    // ---------- attributes & selection ----------

    pub fn rows(&self, id: &str, q: &table::RowsQuery) -> Result<table::RowsPage> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::rows(c, &info, q))
    }

    pub fn select_where(&self, id: &str, filter: &str) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::select_where(c, &info, filter))
    }

    /// Select By Location on the map: features of `target` related to the
    /// features of `by` (only `by_fids` when given, e.g. its selection).
    /// Both layers' definition queries apply; `by` is reprojected if needed.
    #[allow(clippy::too_many_arguments)]
    pub fn select_by_location(&self, target: &str, by: &str, by_fids: Option<&[i64]>, relation: &str, distance: f64, units: &str, invert: bool) -> Result<Vec<i64>> {
        let a = self.layer(target)?;
        let b = self.layer(by)?;
        if !a.has_geometry() || !b.has_geometry() {
            anyhow::bail!("Select By Location needs two layers with shapes");
        }
        let crs = a.crs.clone().ok_or_else(|| anyhow!("{} has no coordinate system — use Define Projection first", a.name))?;
        let bcrs = b.crs.clone().ok_or_else(|| anyhow!("{} has no coordinate system — use Define Projection first", b.name))?;
        let bg = if crs::same(&bcrs, &crs) {
            "b.geom".to_string()
        } else {
            format!("ST_Transform(b.geom, {}, {}, true)", lit(&crs::resolve(&bcrs)), lit(&crs::resolve(&crs)))
        };
        let only = match by_fids {
            Some([]) => anyhow::bail!("{} has no selected features to select with", b.name),
            Some(ids) => format!(" AND __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        let d = nodes::to_meters(distance, units)?;
        let not = if invert { "NOT " } else { "" };
        let sql = format!(
            "WITH b AS (SELECT {bg} AS g FROM {bt} b WHERE geom IS NOT NULL{bdef}{only})
             SELECT a.__fid FROM {at} a WHERE a.geom IS NOT NULL{adef} AND {not}{test} ORDER BY a.__fid",
            at = ident(&a.table),
            bt = ident(&b.table),
            adef = a.def_and(),
            bdef = b.def_and(),
            test = nodes::select_by_location::location_test(relation, &crs, d)
        );
        self.with_conn(|c| {
            let mut stmt = c.prepare(&sql)?;
            let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
            Ok(ids)
        })
    }

    /// Features within `tolerance` of (x, y); position and tolerance are in `crs` (the map CRS) units.
    pub fn identify(&self, id: &str, x: f64, y: f64, crs: &str, tolerance: f64) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        if let Some(t) = self.ready_display_table(&info, crs) {
            return self.with_conn(|c| table::identify_display(c, &info, &t, x, y, tolerance));
        }
        self.with_conn(|c| table::identify(c, &info, x, y, crs, tolerance))
    }

    /// Features intersecting a box `[xmin, ymin, xmax, ymax]` given in `crs` (the map CRS).
    pub fn select_rect(&self, id: &str, bbox: [f64; 4], crs: &str) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        if let Some(t) = self.ready_display_table(&info, crs) {
            return self.with_conn(|c| table::select_rect_display(c, &info, &t, bbox));
        }
        self.with_conn(|c| table::select_rect(c, &info, bbox, crs))
    }

    pub fn feature(&self, id: &str, fid: i64) -> Result<Vec<(String, Option<String>)>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::feature(c, &info, fid))
    }

    pub fn measure_feature(&self, id: &str, fid: i64) -> Result<table::Measures> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::measure(c, &info, fid))
    }

    /// New layer from WKT features in lon/lat (WGS 84), each with optional
    /// attributes (e.g. a saved measurement or a resection fix).
    pub fn create_features(&self, name: &str, features: &[(String, serde_json::Map<String, serde_json::Value>)]) -> Result<LayerInfo> {
        if features.is_empty() {
            return Err(anyhow!("Nothing to add"));
        }
        let mut keys: Vec<String> = Vec::new();
        for (_, props) in features {
            for k in props.keys() {
                if !keys.contains(k) && k != "geom" {
                    keys.push(k.clone());
                }
            }
        }
        let numeric = |k: &str| features.iter().all(|(_, p)| p.get(k).map(|v| v.is_number() || v.is_null()).unwrap_or(true));
        let rows: Vec<String> = features
            .iter()
            .map(|(wkt, props)| {
                let cols: Vec<String> = keys
                    .iter()
                    .map(|k| {
                        let v = props.get(k).cloned().unwrap_or(serde_json::Value::Null);
                        let val = match &v {
                            serde_json::Value::Null => "NULL".to_string(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::String(s) => lit(s),
                            other => lit(&other.to_string()),
                        };
                        let ty = if numeric(k) { "DOUBLE" } else { "VARCHAR" };
                        format!("CAST({val} AS {ty}) AS {}", ident(k))
                    })
                    .collect();
                let sep = if cols.is_empty() { "" } else { ", " };
                format!("SELECT {}{sep}ST_GeomFromText({}) AS geom", cols.join(", "), lit(wkt))
            })
            .collect();
        let src = ImportSource { name: name.into(), select: rows.join(" UNION ALL "), crs: Some(crs::WGS84.into()), crs_name: Some("WGS 84".into()), warnings: vec![] };
        self.with_conn(|conn| {
            let id = self.alloc_id();
            let info = layers::build_layer(conn, &id, &src)?;
            self.save_meta(conn, &info)?;
            Ok(info)
        })
    }

    pub fn extent(&self, id: &str, fids: Option<Vec<i64>>) -> Result<Option<[f64; 4]>> {
        let info = self.layer(id)?;
        self.with_conn(|c| layers::features_extent(c, &info, fids.as_deref()))
    }

    pub fn unique_values(&self, id: &str, field: &str, limit: usize) -> Result<Vec<(String, i64)>> {
        let info = self.layer(id)?;
        self.with_conn(|c| layers::unique_values(c, &info, field, limit))
    }

    pub fn quantile_breaks(&self, id: &str, field: &str, classes: usize) -> Result<Vec<f64>> {
        let info = self.layer(id)?;
        self.with_conn(|c| layers::quantile_breaks(c, &info, field, classes))
    }

    // ---------- model ----------

    pub fn tool_specs(&self) -> Vec<nodes::SpecOut> {
        self.tools.iter().map(|t| nodes::SpecOut { spec: t.spec(), fields: t.output_fields(), keywords: nodes::keywords::keywords(t.spec().id), result_look: t.result_look() }).collect()
    }

    pub fn run_graph(&self, graph: &Graph, target: Option<&str>, on_status: &mut dyn FnMut(&NodeResult)) -> Result<Vec<NodeResult>> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut cache = self.graph_cache.lock();
        let total = graph::plan(graph, target).map(|o| o.len()).unwrap_or(1).max(1);
        // Name the layer whose tree is running (its Output node's layer).
        let owner = graph.nodes.iter().find(|n| n.tool == "layer_output").and_then(|n| n.params.get("layer")?.as_str()).and_then(|id| self.layer(id).ok());
        let task = Task::start(match owner {
            Some(l) => format!("Running model — {}", l.name),
            None => "Running model".into(),
        })
        .cancellable();
        let mut done = 0usize;
        let mut report = |r: &NodeResult| {
            let tool = graph.nodes.iter().find(|n| n.id == r.id).map(|n| n.tool.replace('_', " ")).unwrap_or_default();
            if matches!(r.status, graph::Status::Running) {
                task.detail(format!("Step {} of {total}: {tool}", done + 1));
            } else {
                done += 1;
                task.set(done as f64 / total as f64, None::<&str>);
            }
            on_status(r);
        };
        // Cancel interrupts the query that is running now (DuckDB), and the
        // executor stops before the next step. GDAL raster steps can't be
        // interrupted mid-way; they stop when they finish.
        let handle = task.handle();
        let interrupt = conn.interrupt_handle();
        let stop_rasters = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                // Keep interrupting until the run returns: DuckDB clears the
                // flag when a query starts, so a Cancel that lands between
                // two queries must hit the next one too.
                while !finished.load(Ordering::Relaxed) {
                    if handle.check().is_err() {
                        interrupt.interrupt();
                        stop_rasters.store(true, Ordering::Relaxed); // raster steps stop via GDAL's progress callback
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            });
            let out = progress::with_cancel_flag(stop_rasters.clone(), || graph::run(&conn, &self.tools, self, &mut cache, graph, target, &mut report, &|| handle.check().is_err()));
            finished.store(true, Ordering::Relaxed);
            out
        })
    }

    /// Show a node's cached output on the map as a temporary layer.
    pub fn preview_output(&self, output_table: &str, crs: Option<String>, name: &str) -> Result<LayerInfo> {
        if !output_table.starts_with("__node_") {
            return Err(anyhow!("Not a model output"));
        }
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut info = self.upsert_layer(&conn, "__preview__", name, &format!("SELECT * FROM {}", ident(output_table)), crs, &[])?;
        info.temporary = true;
        self.save_meta(&conn, &info)?;
        Ok(info)
    }

    pub fn clear_model_cache(&self) -> Result<()> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        self.graph_cache.lock().clear(&conn);
        Ok(())
    }

    /// Drop node snapshots the UI no longer shows (`live` = output table
    /// names; `pinned` stays), then apply the size budget. Returns the count dropped.
    pub fn prune_node_cache(&self, live: &[String], pinned: Option<&str>) -> Result<usize> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        let n = self.graph_cache.lock().prune(&conn, live, pinned);
        Ok(n)
    }

    /// Size budget for persisted node snapshots, in bytes (0 = keep none between saves).
    pub fn set_node_cache_budget(&self, bytes: u64) {
        self.graph_cache.lock().budget = bytes;
    }

    // ---------- attribute history ----------

    pub fn node_table_info(&self, table: &str) -> Result<history::NodeTableInfo> {
        self.with_conn(|c| history::table_info(c, table))
    }

    pub fn node_rows(&self, table: &str, q: &table::RowsQuery) -> Result<table::RowsPage> {
        self.with_conn(|c| history::node_rows(c, table, q))
    }

    /// Compare two node snapshots (D5). The result is a temporary table the
    /// returned `diff_id` names; cancellable like a model run.
    pub fn node_diff(&self, a: &str, b: &str, key: Option<&str>) -> Result<history::DiffSummary> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        let task = Task::start("Comparing tables").cancellable();
        let report = |f: f64, d: &str| task.set(f, Some(d));
        let (summary, meta) = history::cancellable(&conn, task.handle(), || history::node_diff(&conn, a, b, key, task.handle(), &report))?;
        self.diffs.lock().insert(summary.diff_id.clone(), meta);
        Ok(summary)
    }

    pub fn node_diff_rows(&self, diff_id: &str, kind: &str, offset: usize, limit: usize) -> Result<Vec<history::DiffRow>> {
        let meta = self.diffs.lock().get(diff_id).cloned().ok_or_else(|| anyhow!("This comparison is no longer available; run it again"))?;
        self.with_conn(|c| history::diff_rows(c, &meta, kind, offset, limit))
    }

    /// Drop a comparison table (the table pane closed the diff).
    pub fn drop_node_diff(&self, diff_id: &str) -> Result<()> {
        if let Some(meta) = self.diffs.lock().remove(diff_id) {
            self.with_conn(|c| {
                history::drop_diff(c, &meta.table);
                Ok(())
            })?;
        }
        Ok(())
    }

    // ---------- test data ----------

    /// Generate random features for benchmarking (`points` or `polygons`).
    pub fn generate_test_layer(&self, kind: &str, n: u64, bbox: [f64; 4]) -> Result<LayerInfo> {
        let [w, s, e, nn] = bbox;
        let geom = match kind {
            "points" => format!("ST_Point({w} + random() * {dx}, {s} + random() * {dy})", dx = e - w, dy = nn - s),
            _ => format!("ST_Buffer(ST_Point({w} + random() * {dx}, {s} + random() * {dy}), 0.002 + random() * 0.01, 2)", dx = e - w, dy = nn - s),
        };
        let src = ImportSource {
            name: format!("Test {kind} ({n})"),
            select: format!("SELECT i AS id, (random() * 10)::INT AS class, round(random() * 1000, 1) AS value, {geom} AS geom FROM range({n}) t(i)"),
            crs: Some(crs::WGS84.into()),
            crs_name: Some("WGS 84".into()),
            warnings: vec![],
        };
        let pool = self.pool.read();
        let conn = pool.get()?;
        let id = self.alloc_id();
        let info = layers::build_layer(&conn, &id, &src)?;
        self.save_meta(&conn, &info)?;
        Ok(info)
    }

    // ---------- projects ----------

    pub fn project_dir(&self) -> Option<PathBuf> {
        self.project_dir.read().clone()
    }

    /// Save to a `.gproj` folder: `project.duckdb` (data) + `project.json` (UI state).
    pub fn save_project(&self, dir: &Path, ui_state: &str) -> Result<()> {
        self.save_project_within(dir, ui_state, SAVE_WAIT)
    }

    /// [`Engine::save_project`] with a limit on how long it waits for work already running on the project (SAVE-1). Tests use a short one.
    ///
    /// Nothing is cancelled or dropped to make room: a checkpoint cannot run while another connection has a write open, so Save waits for those
    /// to finish (reported as a task, cancellable). The files are replaced only once the new copy is complete, and `project.json` is written
    /// last, so a Save As that fails before the swap leaves the target as it was, and after it the target is a matching pair.
    #[doc(hidden)]
    pub fn save_project_within(&self, dir: &Path, ui_state: &str, wait: std::time::Duration) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let same = self.project_dir().as_deref() == Some(dir);
        if same {
            self.bring_rasters_into(dir)?;
            self.checkpoint_when_quiet(wait)?;
        } else {
            let db_path = dir.join(PROJECT_DB);
            // Save As replaces the target, so let work in flight finish first: nothing of the target is touched if that takes too long.
            self.wait_until_quiet(wait)?;
            let tmp = dir.join(format!("{PROJECT_DB}.saving"));
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_file(dir.join(format!("{PROJECT_DB}.saving.wal")));
            // From here until the new pool is in place nothing else may write: a write after the copy would go to the old database and be lost.
            // New work waits on this lock for the few moments it takes; work already running finished above (or is waited for by the lock).
            {
                let mut pool = self.pool.write();
                // Build the copy beside the target and swap it in when it is whole: Save As onto an existing project is never left half-written.
                let copied: Result<()> = (|| {
                    let conn = pool.get()?;
                    let cur: String = conn.query_row("SELECT current_database()", [], |r| r.get(0))?;
                    conn.execute_batch(&format!(
                        "ATTACH {} AS __dst; COPY FROM DATABASE {} TO __dst; DETACH __dst;",
                        lit(tmp.to_str().ok_or_else(|| anyhow!("Bad path"))?),
                        ident(&cur)
                    ))?;
                    Ok(())
                })();
                if let Err(e) = copied {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e);
                }
                let _ = std::fs::remove_file(dir.join(format!("{PROJECT_DB}.wal")));
                std::fs::rename(&tmp, &db_path)?;
                // The target is a matching pair from here: the new database and its project.json, before anything else can fail.
                write_ui_state(dir, ui_state)?;
                *pool = Pool::open(Some(&db_path), self.gate.clone())?;
            }
            // Loading layers resolves rasters relative to the project folder, so it is set first (as in open_project) and put back if loading fails.
            let previous = self.project_dir.write().replace(dir.to_path_buf());
            *self.graph_cache.lock() = Cache::default();
            self.tile_cache.lock().clear();
            if let Err(e) = self.init_schema() {
                *self.project_dir.write() = previous;
                return Err(e);
            }
            self.bring_rasters_into(dir)?;
            // One plain attempt. Committed work is already durable in the write-ahead log beside the database, so a writer that holds the
            // checkpoint off is not a failed save.
            if let Err(e) = self.pool.read().get()?.execute_batch("CHECKPOINT") {
                if !e.to_string().contains("other write transactions") {
                    return Err(e.into());
                }
            }
            return Ok(());
        }
        write_ui_state(dir, ui_state)
    }

    /// Wait until no connection is in use, for at most `wait` (a task shows it and can cancel it).
    fn wait_until_quiet(&self, wait: std::time::Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + wait;
        let mut task: Option<progress::Task> = None;
        while self.gate.in_use() > 0 {
            let t = task.get_or_insert_with(|| progress::Task::start("Saving project").cancellable());
            t.check()?;
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(save_busy(wait));
            }
            t.detail(format!("Waiting for {} other task(s) on the project to finish", self.gate.in_use()));
            self.gate.wait_quiet((now + std::time::Duration::from_millis(500)).min(deadline));
        }
        Ok(())
    }

    /// CHECKPOINT the project database. DuckDB refuses while another connection has a write open ("Cannot CHECKPOINT: there are other write
    /// transactions active"); `FORCE CHECKPOINT` would abort that work, which is the user's. So: wait until every connection in use has been
    /// given back, then try again, for at most `wait`. Waiting is reported as a task and can be cancelled; running out of time is a plain sentence.
    fn checkpoint_when_quiet(&self, wait: std::time::Duration) -> Result<()> {
        let deadline = std::time::Instant::now() + wait;
        let mut task: Option<progress::Task> = None;
        let mut backoff = std::time::Duration::from_millis(10);
        loop {
            let tried = {
                let pool = self.pool.read();
                let conn = pool.get()?;
                conn.execute_batch("CHECKPOINT")
            };
            match tried {
                Ok(()) => return Ok(()),
                Err(e) if e.to_string().contains("other write transactions") => {
                    let t = task.get_or_insert_with(|| progress::Task::start("Saving project").cancellable());
                    t.check()?;
                    let now = std::time::Instant::now();
                    if now >= deadline {
                        return Err(save_busy(wait));
                    }
                    let n = self.gate.in_use();
                    t.detail(if n > 0 { format!("Waiting for {n} other task{} on the project to finish", if n == 1 { "" } else { "s" }) } else { "Waiting for other work on the project to finish".to_string() });
                    // Wake as soon as the last connection in use is given back (slices of half a second, so Cancel is honoured).
                    let quiet = self.gate.wait_quiet((now + std::time::Duration::from_millis(500)).min(deadline));
                    if quiet {
                        // Nothing of ours is running; the writer is not on the pool. Back off rather than spin.
                        std::thread::sleep(backoff);
                        backoff = (backoff * 2).min(std::time::Duration::from_millis(250));
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Copy every raster layer not yet in the project into `<project>/rasters`
    /// (with GDAL's .ovr / .aux.xml sidecars), so the project is
    /// self-contained and portable.
    fn bring_rasters_into(&self, dir: &Path) -> Result<()> {
        let rdir = dir.join("rasters");
        for mut info in self.list_layers().into_iter().filter(|l| l.raster.is_some()) {
            let src = PathBuf::from(&info.raster.as_ref().unwrap().path);
            if src.starts_with(dir) || !src.is_file() {
                continue;
            }
            std::fs::create_dir_all(&rdir)?;
            let name = src.file_name().ok_or_else(|| anyhow!("Bad raster path {}", src.display()))?.to_string_lossy().into_owned();
            let mut dest = rdir.join(&name);
            let mut n = 2;
            while dest.exists() {
                dest = rdir.join(format!("{}-{n}.{}", src.file_stem().unwrap_or_default().to_string_lossy(), src.extension().unwrap_or_default().to_string_lossy()));
                n += 1;
            }
            std::fs::copy(&src, &dest).with_context(|| format!("Copying {} into the project", src.display()))?;
            for side in ["ovr", "aux.xml"] {
                let s = PathBuf::from(format!("{}.{side}", src.display()));
                if s.is_file() {
                    std::fs::copy(&s, format!("{}.{side}", dest.display()))?;
                }
            }
            info.raster.as_mut().unwrap().path = dest.display().to_string();
            info.version += 1;
            self.with_conn(|c| self.save_meta(c, &info))?;
        }
        Ok(())
    }

    /// Open a `.gproj` folder; returns its saved UI state.
    pub fn open_project(&self, dir: &Path) -> Result<String> {
        let db_path = dir.join(PROJECT_DB);
        if !db_path.exists() {
            return Err(anyhow!("{} is not a project folder (no {PROJECT_DB})", dir.display()));
        }
        let ui_state = std::fs::read_to_string(dir.join(PROJECT_JSON)).unwrap_or_else(|_| "{}".into());
        check_project_format(&ui_state)?;
        // Set first: loading layers resolves raster paths stored relative to it.
        let previous = self.project_dir.write().replace(dir.to_path_buf());
        if let Err(e) = self.open_db(&db_path) {
            *self.project_dir.write() = previous;
            return Err(e);
        }
        Ok(ui_state)
    }

    pub fn new_project(&self) -> Result<()> {
        *self.pool.write() = Pool::open(None, self.gate.clone())?;
        *self.project_dir.write() = None;
        *self.graph_cache.lock() = Cache::default();
        self.tile_cache.lock().clear();
        self.init_schema()
    }

    fn open_db(&self, path: &Path) -> Result<()> {
        // A file is never open twice in one process: a second DuckDB instance on it does not see the first one's uncheckpointed writes, and
        // reading an R-tree stored there crashes the process (SIGBUS; BUGS.md RT-1). Opening the project that is already open (the app does
        // this after Save As) closes the old instance first, which also folds its write-ahead log into the file.
        {
            let mut pool = self.pool.write();
            if pool.has_open(path) {
                *pool = Pool::open(None, self.gate.clone())?;
            }
        }
        *self.pool.write() = Pool::open(Some(path), self.gate.clone())?;
        *self.graph_cache.lock() = Cache::default();
        self.tile_cache.lock().clear();
        self.init_schema()
    }
}

impl Host for Engine {
    fn layers(&self) -> HashMap<String, LayerInfo> {
        self.layers.read().clone()
    }

    fn set_result(&self, conn: &Connection, owner: &str, sql: &str, crs: Option<String>, inputs: &[String]) -> Result<LayerInfo> {
        let base = self.layer(owner)?;
        let id = result_id(owner);
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let src = ImportSource { name: base.name.clone(), select: sql.into(), crs: crs.clone(), crs_name: None, warnings: vec![] };
        // All-or-nothing, so a cancelled or failed run keeps the previous result.
        atomically(conn, || {
            let mut info = layers::build_layer(conn, &id, &src)?;
            info.version = prev_version + info.version;
            info.crs_name = if crs::same_opt(crs.as_deref(), base.crs.as_deref()) { base.crs_name.clone() } else { None };
            info.result_of = Some(owner.to_string());
            self.record_derived(conn, &info, inputs)?;
            self.save_meta(conn, &info)?;
            Ok(info)
        })
    }

    fn set_result_raster(&self, owner: &str, path: &str, inputs: &[String]) -> Result<LayerInfo> {
        let base = self.layer(owner)?;
        let id = result_id(owner);
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let mut info = layers::raster_layer(&id, &base.name, Path::new(path), base.source.clone())?;
        info.version = prev_version + 1;
        info.result_of = Some(owner.to_string());
        self.with_conn(|c| {
            atomically(c, || {
                self.record_derived(c, &info, inputs)?;
                self.save_meta(c, &info)
            })
        })?;
        Ok(info)
    }

    fn upsert_layer(&self, conn: &Connection, node_id: &str, name: &str, sql: &str, crs: Option<String>, inputs: &[String]) -> Result<LayerInfo> {
        let existing = self.node_layers.lock().get(node_id).cloned().filter(|l| self.layers.read().contains_key(l));
        let id = existing.unwrap_or_else(|| self.alloc_id());
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let src = ImportSource { name: name.into(), select: sql.into(), crs, crs_name: None, warnings: vec![] };
        let info = atomically(conn, || {
            let mut info = layers::build_layer(conn, &id, &src)?;
            info.version = prev_version + info.version;
            info.crs_name = None;
            conn.execute("INSERT OR REPLACE INTO __model_layers VALUES (?, ?)", [node_id, id.as_str()])?;
            self.record_derived(conn, &info, inputs)?;
            self.save_meta(conn, &info)?;
            Ok(info)
        })?;
        self.node_layers.lock().insert(node_id.into(), id);
        Ok(info)
    }
}

/// Run `f` in one DuckDB transaction: commit on success, roll back on any
/// error (including a Cancel interrupt), leaving the database as it was.
fn atomically<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("BEGIN TRANSACTION")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            // Cancel keeps interrupting until the run returns, so the ROLLBACK
            // itself can be interrupted. Retry, or the pooled connection goes
            // back with an aborted transaction open and the next run's first
            // statement fails ("Current transaction is aborted").
            for _ in 0..50 {
                match conn.execute_batch("ROLLBACK") {
                    Ok(()) => break,
                    Err(r) if r.to_string().contains("no transaction") => break,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
                }
            }
            Err(e)
        }
    }
}
pub use duckdb;

#[cfg(test)]
mod clip_tests {
    use super::Engine;

    /// The National Map can send invalid polygons (self-intersections); clipping
    /// them used to abort the import with a GEOS TopologyException.
    #[test]
    fn clipping_repairs_invalid_polygons_instead_of_failing() {
        let e = Engine::new().unwrap();
        // A bowtie (self-intersecting) polygon and a valid one, both crossing the clip box.
        let bowtie = "POLYGON((-119.3 37.9, -119.0 38.2, -119.0 37.9, -119.3 38.2, -119.3 37.9))".to_string();
        let square = "POLYGON((-119.25 37.95, -119.05 37.95, -119.05 38.15, -119.25 38.15, -119.25 37.95))".to_string();
        let layer = e.create_features("lands", &[(bowtie, Default::default()), (square, Default::default())]).unwrap();
        let clip = "POLYGON((-119.2 38.0, -119.1 38.0, -119.1 38.1, -119.2 38.1, -119.2 38.0))";
        let out = e.clip_layer_to(&layer, clip).expect("clipping an invalid polygon must not fail");
        assert_eq!(out.feature_count, 2, "both features are kept, clipped");
        let (valid, polys): (bool, i64) = e
            .with_conn(|c| {
                Ok(c.query_row(
                    &format!("SELECT bool_and(ST_IsValid(geom)), count(*) FILTER (WHERE ST_Dimension(geom) = 2) FROM {}", super::db::ident(&out.table)),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?)
            })
            .unwrap();
        assert!(valid, "results are valid geometry");
        assert_eq!(polys, 2, "polygons stay polygons");
    }
}
