//! GIS engine: data storage, coordinate systems, vector tiles and the
//! geoprocessing graph. The Tauri app is a thin wrapper around [`Engine`].

pub mod attrs;
pub mod catalog;
pub mod crs;
pub mod export_table;
pub mod arcgis;
pub mod raster_ops;
pub mod spatial_stats;
pub mod kriging;
pub mod whitebox;
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
pub mod graph;
pub mod history;
pub mod layers;
pub mod layout;
pub mod measure;
pub mod nodes;
pub mod progress;
pub mod raster;
pub mod sources;
pub mod resources;
pub mod table;
pub mod tiger;
pub mod split;
pub mod tiles;
pub mod tnm;
pub mod tnm_vector;

use anyhow::{anyhow, Context, Result};
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
    tile_cache: Mutex<LruCache<TileKey, Arc<Vec<u8>>>>,
    tools: Vec<Box<dyn Tool>>,
    graph_cache: Mutex<Cache>,
    diffs: Mutex<HashMap<String, history::DiffMeta>>,
    display: display::Registry,
}

const PROJECT_DB: &str = "project.duckdb";
const PROJECT_JSON: &str = "project.json";

impl Engine {
    pub fn new() -> Result<Self> {
        resources::ensure_proj_data();
        let e = Self {
            pool: RwLock::new(Pool::open(None)?),
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
             CREATE TABLE IF NOT EXISTS __layouts (id VARCHAR PRIMARY KEY, json JSON, updated TIMESTAMP);",
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
        display::sweep(&conn, &layers.iter().map(|(k, v)| (k.clone(), v.version)).collect());
        let mut stmt = conn.prepare("SELECT node, layer FROM __model_layers")?;
        let nl: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        *self.node_layers.lock() = nl.into_iter().collect();
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
        let info = layers::raster_layer(&id, &name, &path, Some("USGS 3DEP elevation (The National Map)".into()))?;
        self.with_conn(|c| {
            self.save_meta(c, &info)?;
            // One citation per job, not per tile.
            sources::record(c, &sources::for_3dep(), &info.id, &info.name)
        })?;
        Ok(info)
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

    pub fn calculate_geometry(&self, id: &str, field: &str, property: &str, area_units: &str, length_units: &str, fids: Option<&[i64]>) -> Result<(LayerInfo, i64)> {
        self.edit(id, |c, i| attrs::calculate_geometry(c, i, field, property, area_units, length_units, fids))
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
            info.version += 1;
            self.save_meta(conn, &info)?;
            Ok(info)
        })
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
            Arc::new(if info.crs.is_some() { raster::render_tile(r, z, x, y)? } else { Vec::new() })
        } else {
            let pool = self.pool.read();
            let conn = pool.get()?;
            Arc::new(tiles::build_tile(&conn, &info, z, x, y, fields)?)
        };
        self.tile_cache.lock().put(key, t.clone());
        Ok(t)
    }

    /// A tile on the grid of the map CRS `key` (from `crs_display_info`).
    /// `3857` is the Web Mercator tile above; any other key reads the layer's
    /// display table and fails while that is still being built.
    pub fn tile_in_crs(&self, id: &str, key: &str, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Arc<Vec<u8>>> {
        if key == display::MERCATOR_KEY {
            return self.tile(id, z, x, y, fields);
        }
        let info = self.layer(id)?;
        if info.is_raster() {
            return Err(anyhow!("Rasters aren't drawn in this coordinate system yet"));
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
        self.tools.iter().map(|t| nodes::SpecOut { spec: t.spec(), fields: t.output_fields() }).collect()
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
        std::fs::create_dir_all(dir)?;
        let same = self.project_dir().as_deref() == Some(dir);
        if same {
            self.bring_rasters_into(dir)?;
            let pool = self.pool.read();
            pool.get()?.execute_batch("CHECKPOINT")?;
        } else {
            let db_path = dir.join(PROJECT_DB);
            if db_path.exists() {
                std::fs::remove_file(&db_path)?;
            }
            {
                let pool = self.pool.read();
                let conn = pool.get()?;
                let cur: String = conn.query_row("SELECT current_database()", [], |r| r.get(0))?;
                conn.execute_batch(&format!(
                    "ATTACH {} AS __dst; COPY FROM DATABASE {} TO __dst; DETACH __dst;",
                    lit(db_path.to_str().ok_or_else(|| anyhow!("Bad path"))?),
                    ident(&cur)
                ))?;
            }
            *self.project_dir.write() = Some(dir.to_path_buf());
            self.open_db(&db_path)?;
            self.bring_rasters_into(dir)?;
            self.pool.read().get()?.execute_batch("CHECKPOINT")?;
        }
        std::fs::write(dir.join(PROJECT_JSON), ui_state)?;
        Ok(())
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
        // Set first: loading layers resolves raster paths stored relative to it.
        let previous = self.project_dir.write().replace(dir.to_path_buf());
        if let Err(e) = self.open_db(&db_path) {
            *self.project_dir.write() = previous;
            return Err(e);
        }
        Ok(std::fs::read_to_string(dir.join(PROJECT_JSON)).unwrap_or_else(|_| "{}".into()))
    }

    pub fn new_project(&self) -> Result<()> {
        *self.pool.write() = Pool::open(None)?;
        *self.project_dir.write() = None;
        *self.graph_cache.lock() = Cache::default();
        self.tile_cache.lock().clear();
        self.init_schema()
    }

    fn open_db(&self, path: &Path) -> Result<()> {
        *self.pool.write() = Pool::open(Some(path))?;
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
