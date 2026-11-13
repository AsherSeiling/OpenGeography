//! Shared pieces of the spatial statistics tools (ST5, ST6): feature coordinates in the table's scan order, the weights parameters every
//! spatial tool offers, building a `Weights` from them, and joining per-feature results back onto the input layer.
//!
//! Row i of every vector here is row i of `SELECT ... FROM table` (DuckDB keeps insertion order), which is also `row_number() OVER ()` - 1; the
//! existing Hot Spot and Moran tools rely on the same fact.

use super::*;
use crate::stats::weights::{self, Weights};

pub struct Coords {
    /// Centroid of every row, None for a row without geometry.
    pub xy: Vec<Option<(f64, f64)>>,
    pub geographic: bool,
}

/// Centroids of every row of a layer in scan order.
pub fn coords(ctx: &Ctx, rel: &Rel) -> Result<Coords> {
    if !rel.has_geom {
        bail!("This tool needs a layer with geometry (a table of places on a map).");
    }
    let crs = need_crs(rel)?;
    let geographic = crate::crs::is_geographic(crs);
    let mut stmt = ctx.conn.prepare(&format!("SELECT CASE WHEN geom IS NULL OR ST_IsEmpty(geom) THEN NULL ELSE ST_X(ST_Centroid(geom)) END, CASE WHEN geom IS NULL OR ST_IsEmpty(geom) THEN NULL ELSE ST_Y(ST_Centroid(geom)) END FROM {}", ident(&rel.table)))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<f64>>(1)?)))?;
    let mut xy = vec![];
    for r in rows {
        let (x, y) = r?;
        xy.push(match (x, y) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some((x, y)),
            _ => None,
        });
    }
    Ok(Coords { xy, geographic })
}

/// The parameters every tool that needs neighbours offers.
pub fn weights_params() -> Vec<Param> {
    vec![
        Param::new("weights", "Neighbours", ParamKind::Enum).default("knn").options(&[("knn", "k nearest neighbours"), ("queen", "Queen contiguity (polygons sharing a point)"), ("rook", "Rook contiguity (polygons sharing an edge)"), ("band", "Distance band")]).help("How features are linked. Contiguity needs polygons. Distances are geodesic (metres) when the layer is in degrees."),
        Param::new("k", "k (nearest neighbours)", ParamKind::Number).default(8),
        Param::new("distance", "Distance band", ParamKind::Number).default(1000).help("Map units, or metres for a layer in degrees."),
    ]
}

/// Build weights for the rows `used` (indices into the table's scan order).
pub fn make_weights(ctx: &Ctx, rel: &Rel, p: &Params, used: &[usize], co: &Coords) -> Result<Weights> {
    let kind = p.str("weights").unwrap_or_else(|| "knn".into());
    let n = used.len();
    let cancel = || crate::progress::run_cancelled();
    match kind.as_str() {
        "queen" | "rook" => {
            if ctx.dimension(rel)? < 2 {
                bail!("Contiguity needs polygon features; this layer has points or lines. Use k nearest neighbours or a distance band.");
            }
            let pos: std::collections::HashMap<i64, usize> = used.iter().enumerate().map(|(i, r)| (*r as i64 + 1, i)).collect();
            let rook = if kind == "rook" { " AND ST_Length(ST_Intersection(ST_Boundary(a.geom), ST_Boundary(b.geom))) > 0" } else { "" };
            let sql = format!("WITH g AS (SELECT row_number() OVER () AS rn, geom FROM {t} ) SELECT a.rn, b.rn FROM g a JOIN g b ON a.rn < b.rn AND a.geom IS NOT NULL AND b.geom IS NOT NULL AND ST_Intersects(a.geom, b.geom){rook}", t = ident(&rel.table));
            let mut stmt = ctx.conn.prepare(&sql)?;
            let pairs: Vec<(usize, usize)> = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?.filter_map(|r| r.ok()).filter_map(|(a, b)| Some((*pos.get(&a)?, *pos.get(&b)?))).collect();
            Ok(weights::from_pairs(n, &pairs, if kind == "rook" { "rook contiguity" } else { "queen contiguity" }))
        }
        "band" => {
            let d = p.f64("distance").unwrap_or(1000.0);
            let xy: Vec<(f64, f64)> = used.iter().map(|r| co.xy[*r].unwrap()).collect();
            weights::distance_band(&xy, d, co.geographic, &cancel).map_err(|e| anyhow!(e))
        }
        _ => {
            let k = p.f64("k").unwrap_or(8.0);
            if !(k.is_finite() && k >= 1.0 && k.fract() == 0.0) {
                bail!("The number of neighbours must be a whole number, 1 or more (8 is usual)");
            }
            let xy: Vec<(f64, f64)> = used.iter().map(|r| co.xy[*r].unwrap()).collect();
            weights::knn(&xy, (k as usize).min(n.saturating_sub(1)).max(1), co.geographic, &cancel).map_err(|e| anyhow!(e))
        }
    }
}

/// Describe islands for the result's notes.
pub fn island_note(w: &Weights) -> Option<String> {
    let isl = w.islands();
    if isl.is_empty() {
        None
    } else {
        let shown: Vec<String> = isl.iter().take(10).map(|i| (i + 1).to_string()).collect();
        Some(format!("{} feature(s) have no neighbours under {} (islands): row(s) {}{}. They are kept in the layer but contribute nothing to spatial sums; their lagged values are 0.", isl.len(), w.kind, shown.join(", "), if isl.len() > 10 { ", ..." } else { "" }))
    }
}

/// SQL that returns the input layer with extra numeric columns joined on (None for rows that were not used). `values[c][i]` is the value of new
/// column c for table row i.
pub fn join_columns_sql(table: &str, names: &[&str], values: &[Vec<Option<f64>>]) -> String {
    let n = values.first().map_or(0, |v| v.len());
    let cols = names.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let rows: Vec<String> = (0..n)
        .map(|i| {
            let cells: Vec<String> = values.iter().map(|v| v[i].filter(|x| x.is_finite()).map_or("NULL".to_string(), |x| format!("CAST({x:e} AS DOUBLE)"))).collect();
            format!("({}, {})", i + 1, cells.join(", "))
        })
        .collect();
    if rows.is_empty() {
        return format!("SELECT * FROM {}", ident(table));
    }
    format!(
        "SELECT t.* EXCLUDE (__rn), {} FROM (SELECT row_number() OVER () AS __rn, * FROM {}) t LEFT JOIN (SELECT * FROM (VALUES {}) v(__rid, {cols})) g ON t.__rn = g.__rid",
        names.iter().map(|c| format!("g.{}", ident(c))).collect::<Vec<_>>().join(", "),
        ident(table),
        rows.join(", ")
    )
}

/// Scatter of values against their spatial lag (Moran scatter plot): the slope of the fitted line is Moran's I for row-standardised weights.
pub fn moran_scatter(x: &[f64], w: &[Vec<(usize, f64)>], name: &str) -> Option<crate::stats::ChartSpec> {
    let mean = x.iter().sum::<f64>() / x.len() as f64;
    let sd = (x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
    if sd <= 0.0 {
        return None;
    }
    let z: Vec<f64> = x.iter().map(|v| (v - mean) / sd).collect();
    let lag = weights::lag(w, &z);
    let pts: Vec<(i64, f64, f64)> = (0..z.len()).map(|i| (-1, z[i], lag[i])).collect();
    crate::stats::charts::scatter(&pts, &format!("{name} (standardised)"), &format!("Spatial lag of {name}"), true, 20_000).ok().map(|b| {
        let mut c = b.chart;
        c.title = format!("Moran scatter plot of {name}");
        c
    })
}
