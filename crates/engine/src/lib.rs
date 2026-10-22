//! GIS engine: data storage, coordinate systems, vector tiles and the
//! geoprocessing graph. The Tauri app is a thin wrapper around [`Engine`].

pub mod attrs;
pub mod crs;
pub mod db;
pub mod graph;
pub mod layers;
pub mod measure;
pub mod nodes;
pub mod raster;
pub mod table;
pub mod tiger;
pub mod tiles;
pub mod tnm;

use anyhow::{anyhow, Result};
use db::{ident, lit, Pool};
use duckdb::Connection;
use graph::{Cache, Graph, Host, NodeResult};
use layers::{ImportSource, LayerInfo};
use lru::LruCache;
use nodes::{Tool, ToolSpec};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
}

const PROJECT_DB: &str = "project.duckdb";
const PROJECT_JSON: &str = "project.json";

impl Engine {
    pub fn new() -> Result<Self> {
        let e = Self {
            pool: RwLock::new(Pool::open(None)?),
            project_dir: RwLock::new(None),
            layers: RwLock::new(HashMap::new()),
            node_layers: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            tile_cache: Mutex::new(LruCache::new(NonZeroUsize::new(4096).unwrap())),
            tools: nodes::registry(),
            graph_cache: Mutex::new(Cache::default()),
        };
        e.init_schema()?;
        Ok(e)
    }

    fn init_schema(&self) -> Result<()> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS __layers (id VARCHAR PRIMARY KEY, info JSON);
             CREATE TABLE IF NOT EXISTS __model_layers (node VARCHAR PRIMARY KEY, layer VARCHAR);",
        )?;
        // Drop stale model cache tables from a previous session.
        let mut stmt = conn.prepare("SELECT table_name, table_type FROM information_schema.tables WHERE table_name LIKE '\\_\\_node\\_%' ESCAPE '\\'")?;
        let stale: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        for (t, ty) in stale {
            let kind = if ty == "VIEW" { "VIEW" } else { "TABLE" };
            conn.execute_batch(&format!("DROP {kind} IF EXISTS {}", ident(&t)))?;
        }
        let mut stmt = conn.prepare("SELECT info::VARCHAR FROM __layers")?;
        let infos: Vec<String> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let mut layers = self.layers.write();
        layers.clear();
        let mut max_id = 0;
        for j in infos {
            let info: LayerInfo = serde_json::from_str(&j)?;
            if let Some(n) = info.id.trim_start_matches('L').parse::<u64>().ok() {
                max_id = max_id.max(n);
            }
            layers.insert(info.id.clone(), info);
        }
        self.next_id.store(max_id + 1, Ordering::SeqCst);
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

    fn alloc_id(&self) -> String {
        format!("L{}", self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    fn save_meta(&self, conn: &Connection, info: &LayerInfo) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO __layers VALUES (?, ?)",
            duckdb::params![info.id, serde_json::to_string(info)?],
        )?;
        self.layers.write().insert(info.id.clone(), info.clone());
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
        if raster::is_raster_path(path) {
            return Ok(vec![self.import_raster(path, None)?]);
        }
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut out = Vec::new();
        for src in layers::sources_for_path(&conn, path)? {
            let id = self.alloc_id();
            let mut info = layers::build_layer(&conn, &id, &src)?;
            info.source = Some(path.display().to_string());
            self.save_meta(&conn, &info)?;
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

    /// Download USGS 3DEP elevation for a lon/lat box as a raster layer.
    pub fn download_elevation(&self, bbox: [f64; 4], resolution: f64, area_label: &str) -> Result<LayerInfo> {
        let (path, res) = tnm::download_elevation(bbox, resolution)?;
        let id = self.alloc_id();
        let name = format!("Elevation (3DEP {res} m) — {area_label}");
        let info = layers::raster_layer(&id, &name, &path, Some("USGS 3DEP elevation (The National Map)".into()))?;
        self.with_conn(|c| self.save_meta(c, &info))?;
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
    pub fn tiger_import(&self, datasets: &[String], state: &str, county: Option<&str>, area_label: &str) -> Result<Vec<LayerInfo>> {
        let cache = std::env::temp_dir().join("gis-studio-cache");
        let mut out = Vec::new();
        for id in datasets {
            let d = tiger::dataset(id)?;
            let path = tiger::fetch(id, state, county, &cache)?;
            for mut info in self.import(&path)? {
                info.name = format!("{} — {area_label}", d.label);
                info.source = Some(format!("Census TIGERweb: {}", d.label));
                self.with_conn(|c| self.save_meta(c, &info))?;
                out.push(info);
            }
        }
        Ok(out)
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
        if let Some(r) = &src_info.raster {
            return self.import_raster(Path::new(&r.path), Some(name));
        }
        let hidden = if src_info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
        let only = match fids {
            Some([]) => " AND FALSE".to_string(),
            Some(ids) => format!(" AND __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        let src = ImportSource {
            name: name.to_string(),
            select: format!("SELECT * EXCLUDE ({hidden}) FROM {} WHERE TRUE{}{only}", ident(&src_info.table), src_info.def_and()),
            crs: src_info.crs.clone(),
            crs_name: src_info.crs_name.clone(),
            warnings: vec![],
        };
        self.with_conn(|conn| {
            let id = self.alloc_id();
            let mut info = layers::build_layer(conn, &id, &src)?;
            info.source = src_info.source.clone();
            self.save_meta(conn, &info)?;
            Ok(info)
        })
    }

    pub fn rename_layer(&self, id: &str, name: &str) -> Result<LayerInfo> {
        let mut info = self.layer(id)?;
        info.name = name.to_string();
        self.with_conn(|c| self.save_meta(c, &info))?;
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
        let queue = Mutex::new(jobs);
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| loop {
                    let Some((z, x, y)) = queue.lock().pop() else { break };
                    let _ = self.tile(id, z, x, y, fields);
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

    pub fn identify(&self, id: &str, lon: f64, lat: f64, zoom: f64) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::identify(c, &info, lon, lat, zoom))
    }

    pub fn select_box(&self, id: &str, bbox: [f64; 4]) -> Result<Vec<i64>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::select_box(c, &info, bbox))
    }

    pub fn feature(&self, id: &str, fid: i64) -> Result<Vec<(String, Option<String>)>> {
        let info = self.layer(id)?;
        self.with_conn(|c| table::feature(c, &info, fid))
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

    pub fn tool_specs(&self) -> Vec<ToolSpec> {
        self.tools.iter().map(|t| t.spec()).collect()
    }

    pub fn run_graph(&self, graph: &Graph, target: Option<&str>, on_status: &mut dyn FnMut(&NodeResult)) -> Result<Vec<NodeResult>> {
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut cache = self.graph_cache.lock();
        graph::run(&conn, &self.tools, self, &mut cache, graph, target, on_status)
    }

    /// Show a node's cached output on the map as a temporary layer.
    pub fn preview_output(&self, output_table: &str, crs: Option<String>, name: &str) -> Result<LayerInfo> {
        if !output_table.starts_with("__node_") {
            return Err(anyhow!("Not a model output"));
        }
        let pool = self.pool.read();
        let conn = pool.get()?;
        let mut info = self.upsert_layer(&conn, "__preview__", name, &format!("SELECT * FROM {}", ident(output_table)), crs)?;
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
                self.graph_cache.lock().clear(&conn);
                let cur: String = conn.query_row("SELECT current_database()", [], |r| r.get(0))?;
                conn.execute_batch(&format!(
                    "ATTACH {} AS __dst; COPY FROM DATABASE {} TO __dst; DETACH __dst;",
                    lit(db_path.to_str().ok_or_else(|| anyhow!("Bad path"))?),
                    ident(&cur)
                ))?;
            }
            self.open_db(&db_path)?;
            *self.project_dir.write() = Some(dir.to_path_buf());
        }
        std::fs::write(dir.join(PROJECT_JSON), ui_state)?;
        Ok(())
    }

    /// Open a `.gproj` folder; returns its saved UI state.
    pub fn open_project(&self, dir: &Path) -> Result<String> {
        let db_path = dir.join(PROJECT_DB);
        if !db_path.exists() {
            return Err(anyhow!("{} is not a project folder (no {PROJECT_DB})", dir.display()));
        }
        self.open_db(&db_path)?;
        *self.project_dir.write() = Some(dir.to_path_buf());
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

    fn set_result(&self, conn: &Connection, owner: &str, sql: &str, crs: Option<String>) -> Result<LayerInfo> {
        let base = self.layer(owner)?;
        let id = result_id(owner);
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let src = ImportSource { name: base.name.clone(), select: sql.into(), crs: crs.clone(), crs_name: None, warnings: vec![] };
        let mut info = layers::build_layer(conn, &id, &src)?;
        info.version = prev_version + info.version;
        info.crs_name = if crs == base.crs { base.crs_name.clone() } else { None };
        info.result_of = Some(owner.to_string());
        self.save_meta(conn, &info)?;
        Ok(info)
    }

    fn set_result_raster(&self, owner: &str, path: &str) -> Result<LayerInfo> {
        let base = self.layer(owner)?;
        let id = result_id(owner);
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let mut info = layers::raster_layer(&id, &base.name, Path::new(path), base.source.clone())?;
        info.version = prev_version + 1;
        info.result_of = Some(owner.to_string());
        self.with_conn(|c| self.save_meta(c, &info))?;
        Ok(info)
    }

    fn upsert_layer(&self, conn: &Connection, node_id: &str, name: &str, sql: &str, crs: Option<String>) -> Result<LayerInfo> {
        let existing = self.node_layers.lock().get(node_id).cloned().filter(|l| self.layers.read().contains_key(l));
        let id = existing.unwrap_or_else(|| self.alloc_id());
        let prev_version = self.layers.read().get(&id).map(|l| l.version).unwrap_or(0);
        let src = ImportSource { name: name.into(), select: sql.into(), crs, crs_name: None, warnings: vec![] };
        let mut info = layers::build_layer(conn, &id, &src)?;
        info.version = prev_version + info.version;
        info.crs_name = None;
        conn.execute("INSERT OR REPLACE INTO __model_layers VALUES (?, ?)", [node_id, id.as_str()])?;
        self.node_layers.lock().insert(node_id.into(), id);
        self.save_meta(conn, &info)?;
        Ok(info)
    }
}
pub use duckdb;
