//! Map display in any CRS (docs/MAP_PROJECTION_PLAN.md M-D2, M-D3, M-D6).
//!
//! `crs_display_info` gives the UI a tile grid and proj4 string for a CRS.
//! Tiles in a CRS other than Web Mercator are cut from a *display table*
//! (`__disp_<layer>_<crsKey>`): the layer's source geometry clipped to the
//! CRS's area of use and projected, with the same hidden columns as a layer
//! table (`__fid`, `__d`, `__x0`, `__y0`, `__sz`), an R-tree and, for large
//! layers, per-zoom overview tables. `__disp_index` records each table with
//! the layer version it was built from; a table whose version differs from the
//! layer's is never served and is dropped on the next layer change.

use crate::crs::{self, MERC_R, WEB_MERCATOR, WGS84};
use crate::db::{ident, lit};
use crate::layers::{self, LayerInfo};
use crate::progress::{self, Task};
use crate::{history, Engine};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
use geographiclib_rs::InverseGeodesic;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Web Mercator's key: its tiles keep the existing path, cache and bytes.
pub const MERCATOR_KEY: &str = "3857";
const ZOOMS: usize = 23;
const TILE_SIZE: u32 = 256;
/// Layers above this many features get overview tables (as in `layers.rs`).
const OVERVIEW_MIN_FEATURES: i64 = 50_000;
/// Display tables share the node-snapshot budget (2 GB per project).
pub const BUDGET: u64 = crate::graph::DEFAULT_CACHE_BUDGET;
/// Coordinates beyond this (in CRS units) are PROJ's "far outside the
/// projection" results, not real positions.
const MAX_COORD: f64 = 5e8;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    /// [xmin, ymin, xmax, ymax] in the CRS.
    pub extent: [f64; 4],
    /// Top-left corner of tile (0, 0) at every zoom.
    pub origin: [f64; 2],
    /// CRS units per pixel for z = 0..=22.
    pub resolutions: Vec<f64>,
    pub tile_size: u32,
}

impl Grid {
    /// [xmin, ymin, xmax, ymax] of tile (z, x, y) in the CRS.
    pub fn tile_bounds(&self, z: u32, x: u32, y: u32) -> Result<[f64; 4]> {
        let res = *self.resolutions.get(z as usize).ok_or_else(|| anyhow!("Zoom {z} is outside the tile grid"))?;
        let span = res * self.tile_size as f64;
        let (x0, y1) = (self.origin[0] + x as f64 * span, self.origin[1] - y as f64 * span);
        Ok([x0, y1 - span, x0 + span, y1])
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    /// `3857` for Web Mercator, else the first 12 hex digits of the SHA-256 of
    /// the CRS's normalized PROJJSON.
    pub crs_key: String,
    pub proj4: String,
    pub geographic: bool,
    /// Metres per CRS unit (per degree at the equator for geographic CRSs).
    pub unit_m: f64,
    pub grid: Grid,
    /// [west, south, east, north] in degrees.
    pub area_of_use: [f64; 4],
    /// False when PROJ has no area of use and the grid came from the
    /// fallback extent the caller gave.
    pub area_of_use_known: bool,
    /// Metres PROJ's datum shift to WGS 84 moves the middle of the area of use: how far an online
    /// (proj4-reprojected) basemap can sit from data shown in this CRS. `None` when unknown.
    pub basemap_offset_m: Option<f64>,
    /// Set in world view (RD-10): the whole Earth cut at `lon_0 ± 180°`, between the stated latitudes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<WorldInfo>,
}

/// What the world view of a CRS shows.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    /// Central meridian; the cut is at `lon0 ± 180`.
    pub lon0: f64,
    pub lat_south: f64,
    pub lat_north: f64,
    /// Why the view stops short of a pole, when it does.
    pub note: Option<String>,
}

/// Prefix that asks for the world view of a CRS (`world:EPSG:5070`). `crs::resolve` strips it, so every other use of the CRS is unchanged.
pub const WORLD_PREFIX: &str = "world:";

/// Tiny inset of the world window so the cut meridian is seen from the same side by both edges.
const CUT_EPS: f64 = 1e-6;

fn proj_param(proj4: &str, name: &str) -> Option<f64> {
    proj4.split_whitespace().find_map(|t| t.strip_prefix(&format!("+{name}=")).and_then(|v| v.parse::<f64>().ok()))
}

/// Latitude range and central meridian of the world view of a projection, or why it has none.
pub fn world_window(proj4: &str) -> Result<WorldInfo> {
    let proj = proj4.split_whitespace().find_map(|t| t.strip_prefix("+proj=")).unwrap_or("");
    let lon0 = proj_param(proj4, "lon_0").unwrap_or(0.0);
    let (mut s, mut n, mut note) = (-90.0, 90.0, None);
    match proj {
        "tmerc" | "etmerc" | "utm" => bail!("Transverse Mercator and UTM describe a narrow band, not the whole Earth. Choose a world projection (a conic, cylindrical, pseudo-cylindrical or azimuthal one) for the world view."),
        "ortho" | "geos" | "nsper" | "tpers" | "gnom" => bail!("This projection shows one side of the Earth only, so it has no world view."),
        "merc" => {
            s = -85.0;
            n = 85.0;
            note = Some("Mercator stretches without limit toward the poles; the view stops at 85°.".into());
        }
        "lcc" => {
            let l1 = proj_param(proj4, "lat_1").unwrap_or(0.0);
            let l2 = proj_param(proj4, "lat_2").unwrap_or(l1);
            let mid = (l1 + l2) / 2.0;
            let far = (90.0 - mid.abs()).clamp(30.0, 85.0);
            if mid >= 0.0 {
                s = -far;
            } else {
                n = far;
            }
            note = Some(format!("A Lambert conformal conic stretches without limit toward the pole it is not centred on; the view stops at {:.0}°{}.", if mid >= 0.0 { -far } else { far }, if mid >= 0.0 { "S" } else { "N" }));
        }
        "stere" | "sterea" => {
            let lat0 = proj_param(proj4, "lat_0").unwrap_or(0.0);
            if lat0.abs() > 89.9 {
                if lat0 > 0.0 {
                    s = -30.0;
                } else {
                    n = 30.0;
                }
                note = Some("A polar stereographic projection stretches without limit toward the opposite pole; the view stops 30° past the equator.".into());
            }
        }
        _ => {}
    }
    Ok(WorldInfo { lon0, lat_south: s, lat_north: n, note })
}

/// World view only: geometries spanning more than 10° get a vertex every degree, so an edge that is a straight line in
/// lon/lat (a polygon's edge along a parallel, Antarctica's along -90°) curves in a conic instead of becoming a chord. Returns the
/// name of a temporary table `(fid, wkb)` of the densified geometries.
fn densify_large(conn: &Connection, layer: &str, to_ll: &str, tag: &str) -> Result<String> {
    let dense = ident(&format!("__w_dense_{tag}"));
    conn.execute_batch(&format!("CREATE OR REPLACE TEMP TABLE {dense} (fid BIGINT, wkb BLOB)"))?;
    let mut st = conn.prepare(&format!(
        "SELECT __fid, ST_AsWKB(g) FROM (SELECT __fid, {to_ll} AS g FROM {layer} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom))
         WHERE ST_XMax(g) - ST_XMin(g) > 10 OR ST_YMax(g) - ST_YMin(g) > 10"
    ))?;
    let rows: Vec<(i64, Vec<u8>)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut ins = conn.prepare(&format!("INSERT INTO {dense} VALUES (?, ?)"))?;
    for (fid, wkb) in rows {
        let Ok(g) = gdal::vector::Geometry::from_wkb(&wkb) else { continue };
        // SAFETY: `g` owns a valid OGR geometry for the call.
        unsafe { gdal_sys::OGR_G_Segmentize(g.c_geometry(), 1.0) };
        if let Ok(out) = g.wkb() {
            ins.execute(duckdb::params![fid, out])?;
        }
    }
    Ok(dense)
}

/// SQL for the part of lon/lat geometry `g` (longitudes in -180..180) inside the world window, as one collection: the geometry
/// is also tried shifted by ±360° so a window that reaches past ±180° (a cut meridian away from 180°) is filled from both sides.
fn world_clip_sql(w: &WorldInfo, g: &str) -> String {
    let (lo, hi) = (w.lon0 - 180.0 + CUT_EPS, w.lon0 + 180.0 - CUT_EPS);
    let env = format!("ST_MakeEnvelope({lo}, {}, {hi}, {})", w.lat_south, w.lat_north);
    let mut parts = vec![format!("ST_Intersection({g}, {env})")];
    if hi > 180.0 {
        parts.push(format!("ST_Intersection(ST_Translate({g}, 360, 0), {env})"));
    }
    if lo < -180.0 {
        parts.push(format!("ST_Intersection(ST_Translate({g}, -360, 0), {env})"));
    }
    format!("ST_Collect([{}])", parts.join(", "))
}

fn srs(def: &str) -> Result<SpatialRef> {
    crs::srs(def)
}

fn hash12(text: &str) -> String {
    crs::hash12(text)
}

/// The CRS's canonical JSON (names, ids and axis order left out), so equal CRSs hash equally whatever their spelling.
fn normalized_projjson(s: &SpatialRef) -> Result<String> {
    crs::canonical_json(s)
}

fn mercator_key() -> &'static str {
    static KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    KEY.get_or_init(|| srs(WEB_MERCATOR).and_then(|s| normalized_projjson(&s)).map(|j| hash12(&j)).unwrap_or_default())
}

/// Grid, proj4 string and key for a CRS. `fallback` ([w, s, e, n] degrees) is
/// used only when PROJ has no area of use for a projected CRS.
pub fn display_info(crs: &str, fallback: Option<[f64; 4]>) -> Result<DisplayInfo> {
    let want_world = crs.trim_start().starts_with(WORLD_PREFIX);
    let s = srs(&crs::resolve(crs))?;
    let json = normalized_projjson(&s)?;
    let proj4 = s.to_proj4().map_err(|e| anyhow!("The map view can't draw '{crs}': its definition can't be written as a proj4 string ({e})."))?;
    if !proj4.contains("+proj=") {
        bail!("The map view can't draw '{crs}': its definition can't be written as a proj4 string.");
    }
    let geographic = s.is_geographic();
    if geographic && (s.angular_units() - std::f64::consts::PI / 180.0).abs() > 1e-9 {
        bail!("The map view can't draw '{crs}': its angular unit isn't the degree.");
    }
    let known = s.area_of_use().filter(|a| a.west_lon_degree.is_finite() && a.north_lat_degree > a.south_lat_degree);
    let mut hash_in = json.clone();
    let (aou, area_of_use_known) = match (&known, fallback) {
        (Some(a), _) => ([a.west_lon_degree, a.south_lat_degree, a.east_lon_degree, a.north_lat_degree], true),
        (None, _) if geographic => ([-180.0, -90.0, 180.0, 90.0], false),
        (None, Some(f)) => {
            // Expand by 10 % so the layers don't touch the edge of the grid.
            let (dx, dy) = ((f[2] - f[0]).abs() * 0.1, (f[3] - f[1]).abs() * 0.1);
            let e = [(f[0] - dx).max(-180.0), (f[1] - dy).max(-90.0), (f[2] + dx).min(180.0), (f[3] + dy).min(90.0)];
            hash_in.push_str(&format!("|fallback:{:.3},{:.3},{:.3},{:.3}", e[0], e[1], e[2], e[3]));
            (e, false)
        }
        (None, None) if want_world => ([-180.0, -90.0, 180.0, 90.0], false),
        (None, None) => bail!("'{crs}' has no known area of use, so the map grid can't be worked out. Pass the extent of the visible layers as a fallback."),
    };
    let is_mercator = hash12(&json) == mercator_key();
    // World view: Web Mercator and geographic CRSs already show the whole Earth; any other projection gets the globe cut at lon_0 ± 180°.
    let world = if want_world && !geographic && !is_mercator { Some(world_window(&proj4)?) } else { None };
    let (aou, area_of_use_known) = match &world {
        Some(w) => ([w.lon0 - 180.0 + CUT_EPS, w.lat_south, w.lon0 + 180.0 - CUT_EPS, w.lat_north], true),
        None => (aou, area_of_use_known),
    };
    if let Some(w) = &world {
        hash_in.push_str(&format!("|world:{:.6},{},{}", w.lon0, w.lat_south, w.lat_north));
    }
    let key = if is_mercator { MERCATOR_KEY.to_string() } else { hash12(&hash_in) };

    let (grid, unit_m) = if key == MERCATOR_KEY {
        let half = std::f64::consts::PI * MERC_R;
        (
            Grid {
                extent: [-half, -half, half, half],
                origin: [-half, half],
                resolutions: (0..ZOOMS as u32).map(crs::pixel_size).collect(),
                tile_size: TILE_SIZE,
            },
            1.0,
        )
    } else if geographic {
        // OpenLayers' degree grid: two 256 px tiles across 360° at z0.
        let r0 = 180.0 / TILE_SIZE as f64;
        (
            Grid {
                extent: [-180.0, -90.0, 180.0, 90.0],
                origin: [-180.0, 90.0],
                resolutions: (0..ZOOMS).map(|z| r0 / 2f64.powi(z as i32)).collect(),
                tile_size: TILE_SIZE,
            },
            s.angular_units() * s.semi_major().unwrap_or(6378137.0),
        )
    } else {
        (projected_grid(&s, aou)?, s.linear_units())
    };
    let basemap_offset_m = basemap_offset(&s, aou);
    Ok(DisplayInfo { crs_key: key, proj4, geographic, unit_m, grid, area_of_use: aou, area_of_use_known, basemap_offset_m, world })
}

/// How far (m) PROJ's best transform to WGS 84 moves the centre of the CRS's area of use, compared
/// with reading its geographic coordinates as WGS 84 (R1). Online basemaps are reprojected by proj4,
/// which has no datum grids, so this is how far they can sit from data tiles that PROJ projected.
/// `None` when the CRS has no geographic base or the transform fails.
fn basemap_offset(s: &SpatialRef, aou: [f64; 4]) -> Option<f64> {
    let mut geog = s.geog_cs().ok()?;
    geog.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let wgs = srs(WGS84).ok()?;
    let t = CoordTransform::new(&geog, &wgs).ok()?;
    let (lon, lat) = ((aou[0] + if aou[2] < aou[0] { aou[2] + 360.0 } else { aou[2] }) / 2.0, (aou[1] + aou[3]) / 2.0);
    let (mut x, mut y, mut z) = ([lon], [lat], [0.0]);
    t.transform_coords(&mut x, &mut y, &mut z).ok()?;
    let geod = crate::layout::geo::Body::get("earth").ok()?.geodesic();
    let d: f64 = geod.inverse(lat, lon, y[0], x[0]);
    d.is_finite().then_some(d)
}

/// Square grid around the area of use: its boundary (64 points per edge) is
/// transformed to the CRS, bounded, and the box expanded to a square.
fn projected_grid(s: &SpatialRef, aou: [f64; 4]) -> Result<Grid> {
    let mut geog = s.geog_cs().map_err(|e| anyhow!("The coordinate system has no geographic base: {e}"))?;
    geog.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let t = CoordTransform::new(&geog, s).map_err(|e| anyhow!("Cannot project lon/lat into this coordinate system: {e}"))?;
    let [w, so, mut e, n] = aou;
    let world_window_wraps = (e - w) > 359.0; // world view: keep longitudes as given
    if e < w {
        e += 360.0; // area of use crosses the antimeridian
    }
    const N: usize = 64;
    let mut ring = Vec::with_capacity(4 * N);
    for i in 0..N {
        let f = i as f64 / N as f64;
        ring.push((w + (e - w) * f, so));
        ring.push((e, so + (n - so) * f));
        ring.push((e - (e - w) * f, n));
        ring.push((w, n - (n - so) * f));
    }
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    let mut kept = 0;
    for (lon, lat) in ring {
        // (PROJ takes longitudes outside -180..180 and measures them from lon_0, so a window cut at lon_0 ± 180° is not wrapped.)
        let lon = if world_window_wraps { lon } else if lon > 180.0 { lon - 360.0 } else { lon };
        let (mut xs, mut ys) = ([lon], [lat]);
        // Points PROJ can't project (outside the projection's domain) are skipped.
        if t.transform_coords(&mut xs, &mut ys, &mut []).is_ok() && xs[0].abs() < MAX_COORD && ys[0].abs() < MAX_COORD {
            b = [b[0].min(xs[0]), b[1].min(ys[0]), b[2].max(xs[0]), b[3].max(ys[0])];
            kept += 1;
        }
    }
    if kept < 8 || !(b[2] > b[0] || b[3] > b[1]) {
        bail!("The area of use of this coordinate system can't be projected, so no map grid can be made.");
    }
    let side = (b[2] - b[0]).max(b[3] - b[1]);
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let extent = [cx - side / 2.0, cy - side / 2.0, cx + side / 2.0, cy + side / 2.0];
    Ok(Grid {
        extent,
        origin: [extent[0], extent[3]],
        resolutions: (0..ZOOMS).map(|z| side / TILE_SIZE as f64 / 2f64.powi(z as i32)).collect(),
        tile_size: TILE_SIZE,
    })
}

// ---------- display tables ----------

pub fn table_name(layer: &str, key: &str) -> String {
    format!("__disp_{layer}_{key}")
}

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS __disp_index (layer VARCHAR, crs_key VARCHAR, crs VARCHAR, version BIGINT,
            table_name VARCHAR, overviews VARCHAR, dropped BIGINT, rows BIGINT, bytes BIGINT,
            created TIMESTAMP, last_used TIMESTAMP, PRIMARY KEY (layer, crs_key))",
    )?;
    Ok(())
}

/// A usable display table: its row in `__disp_index` matches the layer version.
#[derive(Clone, Debug)]
pub struct Entry {
    pub table: String,
    pub overviews: Vec<u32>,
    pub dropped: i64,
}

impl Entry {
    /// Table to read for tiles at zoom `z` (the coarsest overview at or above it).
    pub fn source(&self, z: u32) -> String {
        self.overviews.iter().filter(|&&l| l >= z).min().map(|&l| layers::overview_table(&self.table, l)).unwrap_or_else(|| self.table.clone())
    }
}

pub fn lookup(conn: &Connection, layer: &str, key: &str, version: u64) -> Result<Option<Entry>> {
    let mut stmt = conn.prepare("SELECT table_name, overviews, dropped FROM __disp_index WHERE layer = ? AND crs_key = ? AND version = ?")?;
    let mut rows = stmt.query(duckdb::params![layer, key, version as i64])?;
    Ok(match rows.next()? {
        Some(r) => {
            let ov: String = r.get(1)?;
            Some(Entry {
                table: r.get(0)?,
                overviews: ov.split(',').filter_map(|s| s.parse().ok()).collect(),
                dropped: r.get(2)?,
            })
        }
        None => None,
    })
}

fn drop_tables(conn: &Connection, table: &str) {
    for z in layers::OVERVIEW_LEVELS {
        let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&layers::overview_table(table, z))));
    }
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(table)));
}

/// Drop the layer's display tables built from a version other than `keep`
/// (`None`: all of them). Best effort: a table that can't be dropped now (a
/// build still winding down) is never served, because its version is stale,
/// and is dropped by the next sweep.
pub fn drop_for_layer(conn: &Connection, layer: &str, keep: Option<u64>) {
    let sql = match keep {
        Some(v) => format!("SELECT table_name, crs_key FROM __disp_index WHERE layer = ? AND version <> {v}"),
        None => "SELECT table_name, crs_key FROM __disp_index WHERE layer = ?".to_string(),
    };
    let rows: Vec<(String, String)> = match conn.prepare(&sql).and_then(|mut s| s.query_map([layer], |r| Ok((r.get(0)?, r.get(1)?))).and_then(|it| it.collect())) {
        Ok(r) => r,
        Err(_) => return,
    };
    for (table, key) in rows {
        drop_tables(conn, &table);
        let _ = conn.execute("DELETE FROM __disp_index WHERE layer = ? AND crs_key = ?", [layer, key.as_str()]);
    }
}

/// On project open: drop index rows and tables of layers that no longer exist
/// or changed, and any `__disp_*` table the index doesn't know.
pub fn sweep(conn: &Connection, versions: &HashMap<String, u64>) {
    let rows: Vec<(String, String, i64)> = conn
        .prepare("SELECT layer, table_name, version FROM __disp_index")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).and_then(|it| it.collect()))
        .unwrap_or_default();
    for (layer, table, version) in rows {
        if versions.get(&layer).map(|&v| v as i64) != Some(version) {
            drop_tables(conn, &table);
            let _ = conn.execute("DELETE FROM __disp_index WHERE table_name = ?", [table.as_str()]);
        }
    }
    let known: std::collections::HashSet<String> = conn
        .prepare("SELECT table_name FROM __disp_index")
        .and_then(|mut s| s.query_map([], |r| r.get(0)).and_then(|it| it.collect()))
        .unwrap_or_default();
    let all: Vec<String> = conn
        .prepare("SELECT table_name FROM information_schema.tables WHERE table_name LIKE '\\_\\_disp\\_%' ESCAPE '\\' AND table_name <> '__disp_index'")
        .and_then(|mut s| s.query_map([], |r| r.get(0)).and_then(|it| it.collect()))
        .unwrap_or_default();
    for t in all {
        let base = t.split("__ov").next().unwrap_or(&t);
        if !known.contains(base) {
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&t)));
        }
    }
}

/// Evict least-recently-used display tables until the total fits `budget`.
fn enforce_budget(conn: &Connection, budget: u64, keep: &str) {
    let rows: Vec<(String, String, i64)> = conn
        .prepare("SELECT layer, table_name, bytes FROM __disp_index ORDER BY last_used, created")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).and_then(|it| it.collect()))
        .unwrap_or_default();
    let mut total: i64 = rows.iter().map(|r| r.2).sum();
    for (_, table, bytes) in rows {
        if total <= budget as i64 {
            break;
        }
        if table == keep {
            continue;
        }
        drop_tables(conn, &table);
        let _ = conn.execute("DELETE FROM __disp_index WHERE table_name = ?", [table.as_str()]);
        total -= bytes;
    }
}

/// SQL for the geometry clip: the area of use as a lon/lat polygon.
fn clip_polygon_sql(aou: [f64; 4]) -> String {
    let [w, s, e, n] = aou;
    if e < w {
        // Crosses the antimeridian: the two halves.
        format!("ST_Collect([ST_MakeEnvelope({w}, {s}, 180, {n}), ST_MakeEnvelope(-180, {s}, {e}, {n})])")
    } else {
        format!("ST_MakeEnvelope({w}, {s}, {e}, {n})")
    }
}

/// Build (or rebuild) the display table of `info` for the CRS `di` describes.
/// One transaction: the tables and the `__disp_index` row appear together or
/// not at all, so a cancelled build leaves nothing behind.
pub fn build(conn: &Connection, info: &LayerInfo, crs_def: &str, di: &DisplayInfo, task: &Task) -> Result<Entry> {
    if !info.has_geometry() {
        bail!("'{}' has no geometry to draw", info.name);
    }
    let src = info.crs.as_deref().ok_or_else(|| anyhow!("'{}' has no coordinate system", info.name))?;
    let (layer, table) = (ident(&info.table), table_name(&info.id, &di.crs_key));
    let disp = ident(&table);
    let dst = lit(&crs::resolve(crs_def));
    let world = di.world.clone();
    let src = crs::resolve(src);
    let to_ll = if src == WGS84 { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&src), lit(WGS84)) };
    let rect = clip_polygon_sql(di.area_of_use);
    let px_of = |z: u32| di.grid.resolutions[z as usize];

    history::cancellable(conn, task.handle(), || {
        crate::atomically(conn, || {
            task.check()?;
            task.set(0.05, Some("Clipping to the area of use and projecting"));
            let total: i64 = conn.query_row(&format!("SELECT count(*) FROM {layer} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)"), [], |r| r.get(0))?;
            // The source rows as lon/lat; in world view the large ones are densified first.
            let from_src = if world.is_some() {
                let dense = densify_large(conn, &layer, &to_ll, &info.id)?;
                format!("(SELECT t.__fid, t.geom, coalesce(ST_GeomFromWKB(d.wkb), {to_ll}) AS g FROM {layer} t LEFT JOIN {dense} d ON d.fid = t.__fid WHERE t.geom IS NOT NULL AND NOT ST_IsEmpty(t.geom))")
            } else {
                format!("(SELECT __fid, geom, {to_ll} AS g FROM {layer} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom))")
            };
            // x: the part inside the area of use; c: only the source's kind of
            // geometry (a polygon touching the edge must not become a line).
            conn.execute_batch(&format!(
                "CREATE OR REPLACE TABLE {disp} AS
                 SELECT __fid, __d, ST_XMin(__d) AS __x0, ST_YMin(__d) AS __y0,
                        greatest(ST_XMax(__d) - ST_XMin(__d), ST_YMax(__d) - ST_YMin(__d)) AS __sz
                 FROM (SELECT __fid, ST_Transform(c, {wgs}, {dst}, true) AS __d
                       FROM (SELECT __fid, CASE WHEN ST_GeometryType(x)::VARCHAR = 'GEOMETRYCOLLECTION' OR ST_Dimension(x) < dim
                                                THEN ST_CollectionExtract(x, dim + 1) ELSE x END AS c
                             FROM (SELECT __fid, ST_Dimension(geom) AS dim,
                                          {clip} AS x
                                   FROM {from_src})
                             WHERE x IS NOT NULL AND NOT ST_IsEmpty(x))
                       WHERE c IS NOT NULL AND NOT ST_IsEmpty(c))
                 WHERE isfinite(ST_XMin(__d)) AND isfinite(ST_YMin(__d)) AND isfinite(ST_XMax(__d)) AND isfinite(ST_YMax(__d))
                   AND abs(ST_XMin(__d)) < {MAX_COORD} AND abs(ST_YMin(__d)) < {MAX_COORD}
                   AND abs(ST_XMax(__d)) < {MAX_COORD} AND abs(ST_YMax(__d)) < {MAX_COORD}",
                wgs = lit(WGS84),
                clip = match &world {
                    Some(w) => world_clip_sql(w, "g"),
                    None => format!("CASE WHEN ST_Within(g, {rect}) THEN g ELSE ST_Intersection(g, {rect}) END"),
                },
            ))?;
            task.check()?;
            task.set(0.6, Some("Indexing"));
            let rows: i64 = conn.query_row(&format!("SELECT count(*) FROM {disp}"), [], |r| r.get(0))?;
            conn.execute_batch(&format!("CREATE INDEX {} ON {disp} USING RTREE (__d)", ident(&format!("{table}_rtree"))))?;
            let overviews = if rows >= OVERVIEW_MIN_FEATURES {
                task.set(0.75, Some("Generalizing for low zooms"));
                layers::build_overview_tables(conn, &table, "__d", &px_of)?
            } else {
                vec![]
            };
            task.check()?;
            let geom_bytes: f64 = conn
                .query_row(&format!("SELECT coalesce(avg(octet_length(ST_AsWKB(__d))), 0) FROM (SELECT __d FROM {disp} LIMIT 1000)"), [], |r| r.get(0))
                .unwrap_or(0.0);
            let bytes = (rows as f64 * (geom_bytes + 32.0) * (1.0 + overviews.len() as f64 * 0.5)) as i64;
            let ov = overviews.iter().map(|z| z.to_string()).collect::<Vec<_>>().join(",");
            conn.execute("DELETE FROM __disp_index WHERE layer = ? AND crs_key = ?", [info.id.as_str(), di.crs_key.as_str()])?;
            conn.execute(
                "INSERT INTO __disp_index VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, now(), now())",
                duckdb::params![info.id, di.crs_key, crs_def, info.version as i64, table, ov, total - rows, rows, bytes],
            )?;
            Ok(Entry { table: table.clone(), overviews, dropped: total - rows })
        })
    })
}

// ---------- engine API ----------

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayStatus {
    /// Tiles in this CRS can be served now.
    pub ready: bool,
    pub building: bool,
    /// Features that lie wholly outside the CRS's area of use and aren't drawn.
    pub dropped: i64,
    /// Why the last build failed (not set after a cancel).
    pub error: Option<String>,
}

enum Build {
    /// (task id, layer version it builds from)
    Running(u64, u64),
    Failed(String),
}

/// Display state held by the engine: known CRS keys and running builds.
#[derive(Default)]
pub struct Registry {
    by_crs: Mutex<HashMap<String, DisplayInfo>>,
    by_key: Mutex<HashMap<String, String>>,
    builds: Mutex<HashMap<(String, String), Build>>,
}

impl Registry {
    /// Forget everything about builds (project switched).
    pub fn clear(&self) {
        self.cancel_all();
        self.builds.lock().clear();
    }

    fn cancel_all(&self) {
        for b in self.builds.lock().values() {
            if let Build::Running(id, _) = b {
                progress::cancel(*id);
            }
        }
    }

    /// Ask the layer's running builds from a version other than `keep` to
    /// stop (`None`: all of them).
    pub fn cancel_layer(&self, layer: &str, keep: Option<u64>) {
        for ((l, _), b) in self.builds.lock().iter() {
            if let (true, Build::Running(id, v)) = (l == layer, b) {
                if keep != Some(*v) {
                    progress::cancel(*id);
                }
            }
        }
    }
}

impl Engine {
    /// Grid, proj4 string and key for a CRS; remembers the key so tile
    /// requests that carry only the key can find the CRS.
    pub fn crs_display_info(&self, crs: &str, fallback: Option<[f64; 4]>) -> Result<DisplayInfo> {
        let di = display_info(crs, fallback)?;
        self.display.by_key.lock().insert(di.crs_key.clone(), crs.to_string());
        self.display.by_crs.lock().insert(crs.to_string(), di.clone());
        Ok(di)
    }

    /// The (CRS string, display info) a tile key stands for.
    pub(crate) fn display_key(&self, key: &str) -> Result<(String, DisplayInfo)> {
        let crs = self.display.by_key.lock().get(key).cloned().ok_or_else(|| anyhow!("Unknown map coordinate system key '{key}'. Call crs_display_info first."))?;
        let di = self.display.by_crs.lock().get(&crs).cloned().ok_or_else(|| anyhow!("Unknown map coordinate system '{crs}'"))?;
        Ok((crs, di))
    }

    fn display_for(&self, crs: &str) -> Result<DisplayInfo> {
        if let Some(d) = self.display.by_crs.lock().get(crs) {
            return Ok(d.clone());
        }
        self.crs_display_info(crs, None)
    }

    /// The layer's display table in the map CRS `crs`, when one is built: queries in map coordinates (identify, rectangle select) use it
    /// directly, so a pole (which Web Mercator can't hold) or any point a conic or polar CRS shows is found where it is drawn (P18).
    pub(crate) fn ready_display_table(&self, info: &LayerInfo, crs: &str) -> Option<String> {
        let di = self.display_for(crs).ok()?;
        if !Self::needs_display_table(info, &di) {
            return None;
        }
        self.with_conn(|c| Ok(lookup(c, &info.id, &di.crs_key, info.version)?.map(|e| e.table))).ok().flatten()
    }

    /// Layers that are drawn straight from `__g` (or aren't drawn at all).
    fn needs_display_table(info: &LayerInfo, di: &DisplayInfo) -> bool {
        info.has_geometry() && info.crs.is_some() && di.crs_key != MERCATOR_KEY
    }

    pub fn display_status(&self, layer: &str, crs: &str) -> Result<DisplayStatus> {
        let info = self.layer(layer)?;
        let di = self.display_for(crs)?;
        if !Self::needs_display_table(&info, &di) {
            return Ok(DisplayStatus { ready: true, ..Default::default() });
        }
        let entry = self.with_conn(|c| {
            let e = lookup(c, layer, &di.crs_key, info.version)?;
            if e.is_some() {
                let _ = c.execute("UPDATE __disp_index SET last_used = now() WHERE layer = ? AND crs_key = ?", [layer, di.crs_key.as_str()]);
            }
            Ok(e)
        })?;
        let state = self.display.builds.lock();
        let (building, error) = match state.get(&(layer.to_string(), di.crs_key.clone())) {
            Some(Build::Running(..)) => (true, None),
            Some(Build::Failed(m)) => (false, Some(m.clone())),
            None => (false, None),
        };
        Ok(DisplayStatus { ready: entry.is_some(), building, dropped: entry.map_or(0, |e| e.dropped), error })
    }

    /// Start building the layer's display table for `crs` in the background
    /// (a cancellable "Projecting <layer> for display" task). Returns the
    /// status at once; poll `display_status`. Does nothing when the table is
    /// ready or already being built.
    pub fn build_display(self: &Arc<Self>, layer: &str, crs: &str) -> Result<DisplayStatus> {
        let info = self.layer(layer)?;
        let di = self.display_for(crs)?;
        if !Self::needs_display_table(&info, &di) || self.display_status(layer, crs)?.ready {
            return self.display_status(layer, crs);
        }
        let task = Task::start(format!("Projecting {} for display", info.name)).cancellable();
        let slot = (layer.to_string(), di.crs_key.clone());
        {
            let mut builds = self.display.builds.lock();
            if matches!(builds.get(&slot), Some(Build::Running(..))) {
                drop(builds);
                return self.display_status(layer, crs);
            }
            builds.insert(slot.clone(), Build::Running(task.id(), info.version));
        }
        let (engine, crs_owned) = (self.clone(), crs.to_string());
        std::thread::spawn(move || {
            let result = engine.run_display_build(&info, &crs_owned, &di, &task);
            let mut builds = engine.display.builds.lock();
            match result {
                Ok(()) => {
                    builds.remove(&slot);
                }
                Err(e) if progress::is_cancelled(&e) || e.to_string().contains("INTERRUPT") => {
                    builds.remove(&slot);
                }
                Err(e) => {
                    builds.insert(slot, Build::Failed(format!("{e:#}")));
                }
            }
        });
        self.display_status(layer, crs)
    }

    /// Build the display table now, on this thread (tests and the background worker).
    pub fn build_display_blocking(&self, layer: &str, crs: &str) -> Result<DisplayStatus> {
        let info = self.layer(layer)?;
        let di = self.display_for(crs)?;
        if Self::needs_display_table(&info, &di) && !self.display_status(layer, crs)?.ready {
            let task = Task::start(format!("Projecting {} for display", info.name)).cancellable();
            self.run_display_build(&info, crs, &di, &task)?;
        }
        self.display_status(layer, crs)
    }

    fn run_display_build(&self, info: &LayerInfo, crs: &str, di: &DisplayInfo, task: &Task) -> Result<()> {
        let entry = self.with_conn(|c| build(c, info, crs, di, task));
        let entry = match entry {
            Ok(e) => e,
            Err(e) => return Err(if task.check().is_err() { anyhow!(progress::CANCELLED) } else { e }),
        };
        // The layer changed while we built from the old version: drop the result.
        let current = self.layer(&info.id).map(|l| l.version).ok();
        self.with_conn(|c| {
            if current != Some(info.version) {
                match current {
                    Some(v) => drop_for_layer(c, &info.id, Some(v)),
                    None => drop_for_layer(c, &info.id, None),
                }
            } else {
                enforce_budget(c, BUDGET, &entry.table);
            }
            Ok(())
        })
    }

    /// Called when a layer's data changed (new version) or the layer was
    /// removed: stop running builds and drop outdated display tables.
    pub(crate) fn invalidate_display(&self, conn: &Connection, layer: &str, keep_version: Option<u64>) {
        self.display.cancel_layer(layer, keep_version);
        drop_for_layer(conn, layer, keep_version);
        self.display.builds.lock().retain(|(l, _), b| l != layer || matches!(b, Build::Running(..)));
    }
}
