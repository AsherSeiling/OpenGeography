//! Mapbox Vector Tile generation with per-zoom generalization.
//!
//! The map only ever receives tiles, never raw features. For each tile:
//! - features larger than a pixel are simplified to half-pixel tolerance;
//! - features smaller than a pixel are thinned to one per grid cell;
//! - in very dense tiles the "small" threshold and the grid grow, so tile
//!   size stays bounded no matter how much data the layer has.

use crate::crs::pixel_size;
use crate::display::{Entry, Grid};
use crate::db::{ident, lit};
use crate::layers::{self, LayerInfo};
use anyhow::Result;
use duckdb::Connection;

/// Target maximum number of features per tile before thinning kicks in harder.
const DENSE_TILE_FEATURES: f64 = 25_000.0;
pub const EXTENT: u32 = 4096;
const BUFFER: u32 = 64;

pub fn tile_envelope_sql(z: u32, x: u32, y: u32) -> String {
    format!("ST_TileEnvelope({z}, {x}, {y})")
}

/// Build one MVT tile. `fields` are extra attributes to include (for
/// data-driven styling); `__fid` is always included as the feature id.
pub fn build_tile(conn: &Connection, info: &LayerInfo, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Vec<u8>> {
    if !info.has_geometry() || info.crs.is_none() {
        return Ok(Vec::new());
    }
    let t = ident(&layers::tile_source(info, z));
    let env = tile_envelope_sql(z, x, y);
    let d = info.def_and();
    mvt(
        conn,
        info,
        &format!("SELECT count(*) FROM {t} WHERE ST_Intersects(__g, {env}){d}"),
        &format!("SELECT * FROM {t} WHERE ST_Intersects(__g, {env}){d}"),
        &env,
        pixel_size(z),
        fields,
    )
}

/// Build one MVT tile on a CRS grid from the layer's display table.
pub fn build_tile_display(conn: &Connection, info: &LayerInfo, entry: &Entry, grid: &Grid, z: u32, x: u32, y: u32, fields: &[String]) -> Result<Vec<u8>> {
    let [x0, y0, x1, y1] = grid.tile_bounds(z, x, y)?;
    let span = x1 - x0;
    // Tiles beyond the grid's extent are empty (a geographic grid is 2:1).
    if x as f64 >= ((grid.extent[2] - grid.extent[0]) / span).round() || y as f64 >= ((grid.extent[3] - grid.extent[1]) / span).round() {
        return Ok(Vec::new());
    }
    let env = format!("ST_MakeEnvelope({x0}, {y0}, {x1}, {y1})");
    let t = ident(&entry.source(z));
    let d = info.def_and();
    // Attributes (and the definition query) live in the layer table; join to
    // it only when the tile needs them.
    let needed: Vec<&String> = fields.iter().filter(|f| info.fields.iter().any(|x| &x.name == *f)).collect();
    let join = if needed.is_empty() && d.is_empty() { String::new() } else { format!(" JOIN {} lt ON lt.__fid = dt.__fid", ident(&info.table)) };
    let attrs: String = needed.iter().map(|f| format!(", lt.{} AS {}", ident(f), ident(f))).collect();
    let from = format!("{t} dt{join} WHERE ST_Intersects(dt.__d, {env}){d}");
    mvt(
        conn,
        info,
        &format!("SELECT count(*) FROM {from}"),
        &format!("SELECT dt.__fid, dt.__d AS __g, dt.__x0, dt.__y0, dt.__sz{attrs} FROM {from}"),
        &env,
        grid.resolutions[z as usize],
        fields,
    )
}

/// The shared MVT query. `count_sql` counts the tile's candidate features,
/// `cand_sql` selects them (`__fid`, `__g`, `__x0`, `__y0`, `__sz`, fields),
/// `env` is the tile's envelope and `px` the pixel size in CRS units.
fn mvt(conn: &Connection, info: &LayerInfo, count_sql: &str, cand_sql: &str, env: &str, px: f64, fields: &[String]) -> Result<Vec<u8>> {
    let count: i64 = conn.query_row(count_sql, [], |r| r.get(0))?;
    if count == 0 {
        return Ok(Vec::new());
    }
    let k = (count as f64 / DENSE_TILE_FEATURES).sqrt().max(1.0);
    let small = px * k;
    let cell = px * 2.0 * k;

    let mut props = vec!["'__fid': __fid".to_string()];
    let mut cols = vec!["__fid".to_string()];
    for f in fields {
        if info.fields.iter().any(|x| &x.name == f) {
            props.push(format!("{}: {}", lit(f), ident(f)));
            cols.push(ident(f));
        }
    }
    let cols = cols.join(", ");
    let simplify = if info.geom_type == "Point" {
        "__g".to_string()
    } else {
        format!("CASE WHEN __sz > {} THEN ST_Simplify(__g, {}) ELSE __g END", px * 4.0, px / 2.0)
    };

    let sql = format!(
        "WITH cand AS ({cand_sql}),
         big AS (SELECT {cols}, {simplify} AS g FROM cand WHERE __sz >= {small}),
         sm AS (SELECT {any_cols}, any_value(__g) AS g FROM cand WHERE __sz < {small}
                GROUP BY floor(__x0 / {cell}), floor(__y0 / {cell})),
         feats AS (SELECT * FROM big UNION ALL SELECT * FROM sm)
         SELECT ST_AsMVT({{'geom': ST_AsMVTGeom(g, ST_Extent({env}), {EXTENT}, {BUFFER}, true), {props}}}, 'layer', {EXTENT}, 'geom', '__fid')
         FROM feats WHERE g IS NOT NULL AND NOT ST_IsEmpty(g)",
        any_cols = cols
            .split(", ")
            .map(|c| format!("any_value({c}) AS {c}"))
            .collect::<Vec<_>>()
            .join(", "),
        props = props.join(", "),
    );
    let blob: Option<Vec<u8>> = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(blob.unwrap_or_default())
}

/// Tiles covering a lon/lat extent at zoom `z`.
pub fn tiles_for_extent(extent: [f64; 4], z: u32) -> Vec<(u32, u32)> {
    let n = 2u32.pow(z);
    let tx = |lon: f64| (((lon + 180.0) / 360.0 * n as f64).floor() as i64).clamp(0, n as i64 - 1) as u32;
    let ty = |lat: f64| {
        let lat = lat.clamp(-85.0511, 85.0511).to_radians();
        let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0 * n as f64;
        (y.floor() as i64).clamp(0, n as i64 - 1) as u32
    };
    let (x0, x1) = (tx(extent[0]), tx(extent[2]));
    let (y0, y1) = (ty(extent[3]), ty(extent[1]));
    let mut out = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            out.push((x, y));
        }
    }
    out
}
