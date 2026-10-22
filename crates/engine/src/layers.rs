//! Layers: import from files, storage as DuckDB tables, CRS metadata.
//!
//! Each layer is a table with the user's attribute columns, a native-CRS
//! `geom` column, and hidden display columns used for tiling:
//! `__fid` (stable feature id), `__g` (Web Mercator geometry, R-tree indexed),
//! `__x0`/`__y0` (bbox corner) and `__sz` (bbox size in meters).

use crate::crs::{self, merc_to_lonlat, MERC_MAX_LAT, WEB_MERCATOR, WGS84};
use crate::db::{ident, lit};
use anyhow::{anyhow, bail, Context, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const HIDDEN_COLS: [&str; 5] = ["__fid", "__g", "__x0", "__y0", "__sz"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LayerInfo {
    pub id: String,
    pub name: String,
    pub table: String,
    /// Point, Line, Polygon, Mixed, or Table (no geometry).
    pub geom_type: String,
    pub crs: Option<String>,
    pub crs_name: Option<String>,
    pub feature_count: i64,
    /// [west, south, east, north] in lon/lat.
    pub extent: Option<[f64; 4]>,
    /// [xmin, ymin, xmax, ymax] in the layer's own CRS.
    pub native_extent: Option<[f64; 4]>,
    pub fields: Vec<Field>,
    pub source: Option<String>,
    pub version: u64,
    pub temporary: bool,
    pub warnings: Vec<String>,
    /// Zoom levels with a pre-thinned overview table (`{table}__ov{z}`).
    #[serde(default)]
    pub overviews: Vec<u32>,
    /// Set on a layer's node-tree result: the id of the layer it belongs to.
    /// The UI draws the result in place of that layer.
    #[serde(default)]
    pub result_of: Option<String>,
    /// Definition query: an SQL filter limiting which features the layer has
    /// (on the map, in the table, for selection, export and tools).
    #[serde(default)]
    pub definition_query: Option<String>,
    /// Number of features passing the definition query.
    #[serde(default)]
    pub query_count: Option<i64>,
    /// Set for raster layers (geom_type "Raster").
    #[serde(default)]
    pub raster: Option<crate::raster::RasterInfo>,
}

impl LayerInfo {
    /// Vector features (points, lines, polygons).
    pub fn has_geometry(&self) -> bool {
        self.geom_type != "Table" && self.geom_type != "Raster"
    }

    pub fn is_raster(&self) -> bool {
        self.raster.is_some()
    }

    /// `AND (<definition query>)`, or empty when the layer has none.
    pub fn def_and(&self) -> String {
        match self.definition_query.as_deref().map(str::trim) {
            Some(q) if !q.is_empty() => format!(" AND ({q})"),
            _ => String::new(),
        }
    }
}

/// CRS of one GDAL layer as reported by `ST_Read_Meta`.
fn crs_from_meta(conn: &Connection, crs: &serde_json::Value) -> (Option<String>, Option<String>) {
    let s = |k: &str| crs.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(String::from);
    let name = s("name");
    let id = match (s("auth_name"), s("auth_code")) {
        // Prefer the code, unless PROJ can't use it; then the file's own WKT.
        (Some(a), Some(c)) => {
            let code = format!("{a}:{c}");
            if crs::validate(conn, &code).is_ok() { Some(crs::resolve(&code)) } else { s("wkt").or(Some(code)) }
        }
        _ => s("wkt"),
    };
    (id, name)
}

pub struct ImportSource {
    pub name: String,
    /// SELECT producing attribute columns plus a GEOMETRY column named `geom`
    /// (or no geometry column for plain tables).
    pub select: String,
    pub crs: Option<String>,
    pub crs_name: Option<String>,
    pub warnings: Vec<String>,
}

/// Work out what to read from a file. One file may hold several layers (GPKG).
pub fn sources_for_path(conn: &Connection, path: &Path) -> Result<Vec<ImportSource>> {
    let p = path.to_str().ok_or_else(|| anyhow!("Path is not valid UTF-8"))?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if matches!(ext.as_str(), "csv" | "tsv" | "txt") {
        return Ok(vec![csv_source(conn, p, &stem)?]);
    }
    if ext == "parquet" {
        return Ok(vec![parquet_source(conn, p, &stem)?]);
    }

    let meta: String = conn
        .query_row("SELECT to_json(layers)::VARCHAR FROM ST_Read_Meta(?)", [p], |r| r.get(0))
        .with_context(|| format!("Could not read {p}"))?;
    let layers: Vec<serde_json::Value> = serde_json::from_str(&meta)?;
    let mut out = Vec::new();
    for l in &layers {
        let lname = l["name"].as_str().unwrap_or(&stem).to_string();
        let gfields = l["geometry_fields"].as_array().cloned().unwrap_or_default();
        let read = format!("ST_Read({}, layer := {})", lit(p), lit(&lname));
        let display_name = if layers.len() == 1 { stem.clone() } else { lname.clone() };
        if gfields.is_empty() {
            out.push(ImportSource { name: display_name, select: format!("SELECT * FROM {read}"), crs: None, crs_name: None, warnings: vec![] });
            continue;
        }
        let (crs, crs_name) = crs_from_meta(conn, &gfields[0]["crs"]);
        let geom_col = geometry_column(conn, &format!("SELECT * FROM {read}"))?
            .ok_or_else(|| anyhow!("No geometry column found in {lname}"))?;
        let select = format!("SELECT * EXCLUDE ({g}), {g}::GEOMETRY AS geom FROM {read}", g = ident(&geom_col));
        let mut warnings = vec![];
        if crs.is_none() {
            warnings.push("This layer has no coordinate system. Use Define Projection before it can be drawn or processed.".into());
        }
        out.push(ImportSource { name: display_name, select, crs, crs_name, warnings });
    }
    Ok(out)
}

fn geometry_column(conn: &Connection, select: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(&format!("DESCRIBE {select}"))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (name, ty) = row?;
        if ty.to_uppercase().starts_with("GEOMETRY") {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn columns_of(conn: &Connection, select: &str) -> Result<Vec<Field>> {
    let mut stmt = conn.prepare(&format!("DESCRIBE {select}"))?;
    let rows = stmt.query_map([], |r| Ok(Field { name: r.get(0)?, ty: r.get(1)? }))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn csv_source(conn: &Connection, p: &str, stem: &str) -> Result<ImportSource> {
    let read = format!("read_csv({})", lit(p));
    let cols = columns_of(conn, &format!("SELECT * FROM {read}"))?;
    let find = |names: &[&str]| {
        cols.iter().find(|c| names.contains(&c.name.to_lowercase().as_str())).map(|c| c.name.clone())
    };
    let x = find(&["lon", "long", "longitude", "lng", "x", "easting"]);
    let y = find(&["lat", "latitude", "y", "northing"]);
    match (x, y) {
        (Some(x), Some(y)) => {
            let geographic = x.to_lowercase().starts_with("lon") || x.to_lowercase() == "lng";
            let (crs, crs_name, warnings) = if geographic {
                (Some(WGS84.to_string()), Some("WGS 84".to_string()),
                 vec![format!("Coordinates read from '{x}'/'{y}' and assumed to be WGS 84. Use Define Projection if they are NAD83 or another datum.")])
            } else {
                (None, None, vec![format!("Coordinates read from '{x}'/'{y}' but the coordinate system is unknown. Use Define Projection.")])
            };
            Ok(ImportSource {
                name: stem.into(),
                select: format!("SELECT *, ST_Point({}, {}) AS geom FROM {read}", ident(&x), ident(&y)),
                crs,
                crs_name,
                warnings,
            })
        }
        _ => Ok(ImportSource { name: stem.into(), select: format!("SELECT * FROM {read}"), crs: None, crs_name: None, warnings: vec![] }),
    }
}

fn parquet_source(conn: &Connection, p: &str, stem: &str) -> Result<ImportSource> {
    let read = format!("read_parquet({})", lit(p));
    let select = format!("SELECT * FROM {read}");
    match geometry_column(conn, &select)? {
        Some(g) => {
            let crs: Option<String> = conn
                .query_row(&format!("SELECT ST_CRS({}) FROM {read} LIMIT 1", ident(&g)), [], |r| r.get(0))
                .ok()
                .flatten();
            let warnings = if crs.is_none() { vec!["No coordinate system found in GeoParquet metadata.".into()] } else { vec![] };
            Ok(ImportSource {
                name: stem.into(),
                select: format!("SELECT * EXCLUDE ({q}), {q}::GEOMETRY AS geom FROM {read}", q = ident(&g)),
                crs,
                crs_name: None,
                warnings,
            })
        }
        None => Ok(ImportSource { name: stem.into(), select, crs: None, crs_name: None, warnings: vec![] }),
    }
}

/// Create (or replace) a layer table from a source SELECT and fill in its
/// display columns and metadata.
pub fn build_layer(conn: &Connection, id: &str, src: &ImportSource) -> Result<LayerInfo> {
    let table = format!("layer_{id}");
    conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} AS {}", ident(&table), src.select))?;
    let mut info = LayerInfo {
        id: id.into(),
        name: src.name.clone(),
        table,
        geom_type: "Table".into(),
        crs: src.crs.clone(),
        crs_name: src.crs_name.clone(),
        feature_count: 0,
        extent: None,
        native_extent: None,
        fields: vec![],
        source: None,
        version: 1,
        temporary: false,
        warnings: src.warnings.clone(),
        overviews: vec![],
        result_of: None,
        definition_query: None,
        query_count: None,
        raster: None,
    };
    refresh_display(conn, &mut info)?;
    Ok(info)
}

/// Recompute `__fid`/`__g`/bbox columns, the R-tree index and metadata.
/// Called after import and after the layer's CRS is (re)defined.
pub fn refresh_display(conn: &Connection, info: &mut LayerInfo) -> Result<()> {
    let t = ident(&info.table);
    let has_geom = geometry_column(conn, &format!("SELECT * FROM {t}"))?.is_some();
    let base = format!("SELECT * EXCLUDE ({}) FROM {t}", existing_hidden(conn, &info.table)?);
    let base = if base.contains("EXCLUDE ()") { format!("SELECT * FROM {t}") } else { base };

    if has_geom {
        if let Some(c) = &info.crs {
            crs::validate(conn, c)?;
        }
        // Native extent first, so we know if we must clip to the Mercator limits.
        let native: Option<(f64, f64, f64, f64)> = conn
            .query_row(
                &format!("SELECT min(ST_XMin(geom)), min(ST_YMin(geom)), max(ST_XMax(geom)), max(ST_YMax(geom)) FROM {t}"),
                [],
                |r| Ok(r.get::<_, Option<f64>>(0)?.map(|a| (a, r.get(1).unwrap_or(0.0), r.get(2).unwrap_or(0.0), r.get(3).unwrap_or(0.0)))),
            )?;
        info.native_extent = native.map(|(a, b, c, d)| [a, b, c, d]);

        let g = match &info.crs {
            None => "NULL::GEOMETRY".to_string(),
            Some(c) => {
                let needs_clip = match native {
                    Some((x0, y0, x1, y1)) => {
                        let (_, s) = crs::transform_point(conn, x0, y0, c, WGS84).unwrap_or((0.0, -90.0));
                        let (_, n) = crs::transform_point(conn, x1, y1, c, WGS84).unwrap_or((0.0, 90.0));
                        s < -MERC_MAX_LAT || n > MERC_MAX_LAT
                    }
                    None => false,
                };
                if needs_clip {
                    format!(
                        "ST_Transform(ST_Intersection(ST_Transform(geom, {c}, {w}, true), ST_MakeEnvelope(-180, -{m}, 180, {m})), {w}, {wm}, true)",
                        c = lit(c), w = lit(WGS84), wm = lit(WEB_MERCATOR), m = MERC_MAX_LAT
                    )
                } else {
                    format!("ST_Transform(geom, {}, {}, true)", lit(c), lit(WEB_MERCATOR))
                }
            }
        };
        conn.execute_batch(&format!(
            "CREATE OR REPLACE TABLE {t} AS
             SELECT *, ST_XMin(__g) AS __x0, ST_YMin(__g) AS __y0,
                    greatest(ST_XMax(__g) - ST_XMin(__g), ST_YMax(__g) - ST_YMin(__g)) AS __sz
             FROM (SELECT row_number() OVER () AS __fid, *, {g} AS __g FROM ({base}));
             CREATE INDEX {idx} ON {t} USING RTREE (__g);",
            idx = ident(&format!("{}_rtree", info.table))
        ))?;
        info.geom_type = detect_geom_type(conn, &info.table)?;
        info.extent = features_extent(conn, info, None)?;
        info.overviews = build_overviews(conn, &info.table, info.crs.is_some())?;
    } else {
        conn.execute_batch(&format!(
            "CREATE OR REPLACE TABLE {t} AS SELECT row_number() OVER () AS __fid, * FROM ({base})"
        ))?;
        info.geom_type = "Table".into();
    }

    info.feature_count = conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
    info.fields = columns_of(conn, &format!("SELECT * FROM {t}"))?
        .into_iter()
        .filter(|f| f.name != "geom" && !HIDDEN_COLS.contains(&f.name.as_str()))
        .collect();
    info.version += 1;
    Ok(())
}

/// Layers above this many features get an overview pyramid.
const OVERVIEW_MIN_FEATURES: i64 = 50_000;
/// Overview zooms, finest first. Tiles at zoom z read the coarsest overview
/// whose level is >= z (e.g. z5 reads ov6), so low zooms touch few rows.
pub const OVERVIEW_LEVELS: [u32; 3] = [8, 6, 4];

/// Build pre-generalized copies of a large layer: features smaller than a
/// pixel at that zoom are thinned to one per 2-pixel cell, larger ones are
/// simplified. Each level is built from the previous (finer) one.
fn build_overviews(conn: &Connection, table: &str, has_crs: bool) -> Result<Vec<u32>> {
    for z in OVERVIEW_LEVELS {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&overview_table(table, z))))?;
    }
    let n: i64 = conn.query_row(&format!("SELECT count(*) FROM {}", ident(table)), [], |r| r.get(0))?;
    if !has_crs || n < OVERVIEW_MIN_FEATURES {
        return Ok(vec![]);
    }
    let mut src = table.to_string();
    let mut built = Vec::new();
    for z in OVERVIEW_LEVELS {
        let px = crs::pixel_size(z);
        let ov = overview_table(table, z);
        conn.execute_batch(&format!(
            "CREATE TABLE {o} AS
             SELECT * REPLACE (CASE WHEN __sz > {big} THEN ST_Simplify(__g, {tol}) ELSE __g END AS __g)
               FROM {s} WHERE __sz >= {px}
             UNION ALL
             SELECT * FROM {s} WHERE __sz < {px}
               QUALIFY row_number() OVER (PARTITION BY floor(__x0 / {cell}), floor(__y0 / {cell}) ORDER BY __fid) = 1;
             CREATE INDEX {idx} ON {o} USING RTREE (__g);",
            o = ident(&ov),
            s = ident(&src),
            big = px * 4.0,
            tol = px / 2.0,
            cell = px * 2.0,
            idx = ident(&format!("{ov}_rtree")),
        ))?;
        built.push(z);
        src = ov;
    }
    Ok(built)
}

pub fn overview_table(table: &str, z: u32) -> String {
    format!("{table}__ov{z}")
}

/// Table to read for tiles at zoom `z`.
pub fn tile_source(info: &LayerInfo, z: u32) -> String {
    info.overviews
        .iter()
        .filter(|&&lvl| lvl >= z)
        .min()
        .map(|&lvl| overview_table(&info.table, lvl))
        .unwrap_or_else(|| info.table.clone())
}

/// Refresh a layer's field list, record count and extent after its rows or
/// columns changed (without rebuilding display columns or overviews).
pub fn refresh_meta(conn: &Connection, info: &mut LayerInfo) -> Result<()> {
    if info.is_raster() {
        return Ok(());
    }
    let t = ident(&info.table);
    info.feature_count = conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
    info.fields = columns_of(conn, &format!("SELECT * FROM {t}"))?
        .into_iter()
        .filter(|f| f.name != "geom" && !HIDDEN_COLS.contains(&f.name.as_str()))
        .collect();
    if info.has_geometry() && info.crs.is_some() {
        info.extent = features_extent(conn, info, None)?;
    }
    Ok(())
}

fn existing_hidden(conn: &Connection, table: &str) -> Result<String> {
    let cols = columns_of(conn, &format!("SELECT * FROM {}", ident(table)))?;
    Ok(cols
        .iter()
        .filter(|c| HIDDEN_COLS.contains(&c.name.as_str()))
        .map(|c| ident(&c.name))
        .collect::<Vec<_>>()
        .join(", "))
}

fn detect_geom_type(conn: &Connection, table: &str) -> Result<String> {
    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT ST_Dimension(geom) FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)",
        ident(table)
    ))?;
    let dims: Vec<i32> = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(match dims.as_slice() {
        [0] => "Point",
        [1] => "Line",
        [2] => "Polygon",
        [] => "Point",
        _ => "Mixed",
    }
    .into())
}

pub fn unique_values(conn: &Connection, info: &LayerInfo, field: &str, limit: usize) -> Result<Vec<(String, i64)>> {
    if !info.fields.iter().any(|f| f.name == field) {
        bail!("Field '{field}' not found");
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT CAST({f} AS VARCHAR) v, count(*) n FROM {t} WHERE TRUE{d} GROUP BY 1 ORDER BY n DESC, v LIMIT {limit}",
        d = info.def_and(),
        f = ident(field),
        t = ident(&info.table)
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Class breaks for graduated symbology (quantile method).
pub fn quantile_breaks(conn: &Connection, info: &LayerInfo, field: &str, classes: usize) -> Result<Vec<f64>> {
    if !info.fields.iter().any(|f| f.name == field) {
        bail!("Field '{field}' not found");
    }
    let qs: Vec<String> = (1..classes).map(|i| format!("{}", i as f64 / classes as f64)).collect();
    let sql = format!(
        "SELECT to_json(quantile_cont(TRY_CAST({f} AS DOUBLE), [{}]))::VARCHAR FROM {t} WHERE TRUE{d}",
        qs.join(","),
        f = ident(field),
        t = ident(&info.table),
        d = info.def_and()
    );
    let json: Option<String> = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(json.map(|j| serde_json::from_str(&j)).transpose()?.unwrap_or_default())
}

/// Lon/lat bounds of some features (or the whole layer when `fids` is None).
pub fn features_extent(conn: &Connection, info: &LayerInfo, fids: Option<&[i64]>) -> Result<Option<[f64; 4]>> {
    if !info.has_geometry() {
        return Ok(None);
    }
    let filter = match fids {
        Some(f) if !f.is_empty() => format!("WHERE __fid IN ({})", f.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")),
        Some(_) => return Ok(None),
        None => "WHERE TRUE".to_string(),
    };
    let filter = format!("{filter}{}", info.def_and());
    let r: Option<(f64, f64, f64, f64)> = conn.query_row(
        &format!("SELECT min(ST_XMin(__g)), min(ST_YMin(__g)), max(ST_XMax(__g)), max(ST_YMax(__g)) FROM {} {filter}", ident(&info.table)),
        [],
        |r| Ok(r.get::<_, Option<f64>>(0)?.map(|a| (a, r.get(1).unwrap_or(0.0), r.get(2).unwrap_or(0.0), r.get(3).unwrap_or(0.0)))),
    )?;
    Ok(r.map(|(x0, y0, x1, y1)| {
        let (w, s) = merc_to_lonlat(x0, y0);
        let (e, n) = merc_to_lonlat(x1, y1);
        [w, s, e, n]
    }))
}

/// Layer metadata for a raster file already in the raster store.
pub fn raster_layer(id: &str, name: &str, path: &std::path::Path, source: Option<String>) -> Result<LayerInfo> {
    let r = crate::raster::describe(path)?;
    let ds = gdal::Dataset::open(path)?;
    let (crs, crs_name) = crate::raster::crs_of(&ds);
    let extent = crate::raster::extent_lonlat(&ds).ok();
    let gt = ds.geo_transform()?;
    let (w, h) = ds.raster_size();
    let mut warnings = vec![];
    if crs.is_none() {
        warnings.push("This raster has no coordinate system and can't be drawn.".into());
    }
    Ok(LayerInfo {
        id: id.into(),
        name: name.into(),
        table: String::new(),
        geom_type: "Raster".into(),
        crs,
        crs_name,
        feature_count: (w * h) as i64,
        extent,
        native_extent: Some([gt[0], gt[3] + gt[5] * h as f64, gt[0] + gt[1] * w as f64, gt[3]]),
        fields: vec![],
        source,
        version: 1,
        temporary: false,
        warnings,
        overviews: vec![],
        result_of: None,
        definition_query: None,
        query_count: None,
        raster: Some(r),
    })
}
