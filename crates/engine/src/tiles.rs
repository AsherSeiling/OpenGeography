//! Mapbox Vector Tile generation with per-zoom generalization.
//!
//! The map only ever receives tiles, never raw features. For each tile:
//! - features larger than a pixel are simplified to half-pixel tolerance;
//! - features smaller than a pixel are thinned to one per grid cell;
//! - in very dense tiles the "small" threshold and the grid grow, so tile
//!   size stays bounded no matter how much data the layer has.

use crate::crs::pixel_size;
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
    let px = pixel_size(z);

    let count: i64 = conn.query_row(&format!("SELECT count(*) FROM {t} WHERE ST_Intersects(__g, {env})"), [], |r| r.get(0))?;
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
        "WITH cand AS (SELECT * FROM {t} WHERE ST_Intersects(__g, {env})),
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
