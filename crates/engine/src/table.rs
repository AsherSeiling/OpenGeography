//! Attribute table paging, identify and selection queries.

use crate::db::ident;
use crate::layers::LayerInfo;
use anyhow::{bail, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct RowsQuery {
    pub offset: usize,
    pub limit: usize,
    pub sort: Option<String>,
    pub desc: bool,
    /// SQL boolean expression typed by the user, e.g. `POP > 1000`.
    pub filter: Option<String>,
    /// Restrict to these feature ids ("show selected").
    pub fids: Option<Vec<i64>>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RowsPage {
    pub total: i64,
    pub fids: Vec<i64>,
    /// Values as display strings; `None` is NULL.
    pub rows: Vec<Vec<Option<String>>>,
}

fn where_clause(info: &LayerInfo, filter: &Option<String>, fids: &Option<Vec<i64>>) -> String {
    let mut parts = Vec::new();
    if let Some(q) = info.definition_query.as_ref().filter(|q| !q.trim().is_empty()) {
        parts.push(format!("({q})"));
    }
    if let Some(f) = filter.as_ref().filter(|f| !f.trim().is_empty()) {
        parts.push(format!("({f})"));
    }
    if let Some(ids) = fids {
        if ids.is_empty() {
            parts.push("FALSE".into());
        } else {
            parts.push(format!("__fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")));
        }
    }
    if parts.is_empty() { String::new() } else { format!("WHERE {}", parts.join(" AND ")) }
}

pub fn rows(conn: &Connection, info: &LayerInfo, q: &RowsQuery) -> Result<RowsPage> {
    let t = ident(&info.table);
    let w = where_clause(info, &q.filter, &q.fids);
    let total: i64 = conn.query_row(&format!("SELECT count(*) FROM {t} {w}"), [], |r| r.get(0))?;
    let order = match &q.sort {
        Some(s) if info.fields.iter().any(|f| &f.name == s) => {
            format!("ORDER BY {} {} NULLS LAST, __fid", ident(s), if q.desc { "DESC" } else { "ASC" })
        }
        Some(s) => bail!("Unknown sort field '{s}'"),
        None => "ORDER BY __fid".into(),
    };
    let cols: Vec<String> = info.fields.iter().map(|f| format!("CAST({} AS VARCHAR)", ident(&f.name))).collect();
    let select = if cols.is_empty() { "__fid".to_string() } else { format!("__fid, {}", cols.join(", ")) };
    let limit = q.limit.clamp(1, 5000);
    let mut stmt = conn.prepare(&format!("SELECT {select} FROM {t} {w} {order} LIMIT {limit} OFFSET {}", q.offset))?;
    let n = info.fields.len();
    let mut fids = Vec::new();
    let mut rows = Vec::new();
    let mut it = stmt.query([])?;
    while let Some(r) = it.next()? {
        fids.push(r.get::<_, i64>(0)?);
        let mut row = Vec::with_capacity(n);
        for i in 0..n {
            row.push(r.get::<_, Option<String>>(i + 1)?);
        }
        rows.push(row);
    }
    Ok(RowsPage { total, fids, rows })
}

/// Feature ids matching a filter expression (Select By Attributes).
pub fn select_where(conn: &Connection, info: &LayerInfo, filter: &str) -> Result<Vec<i64>> {
    let w = where_clause(info, &Some(filter.to_string()), &None);
    let mut stmt = conn.prepare(&format!("SELECT __fid FROM {} {w} ORDER BY __fid", ident(&info.table)))?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// SQL expression for a map-CRS ring carried into the layers' stored CRS (Web Mercator):
/// the ring is densified in the map CRS first so curved edges stay exact after the transform.
fn region_sql(ring: &[(f64, f64)], crs: &str) -> String {
    let pts: Vec<String> = ring.iter().map(|(x, y)| format!("{x:.9} {y:.9}")).collect();
    format!(
        "ST_Transform(ST_GeomFromText('POLYGON(({}))'), {}, 'EPSG:3857', true)",
        pts.join(", "),
        crate::db::lit(&crate::crs::resolve(crs))
    )
}

/// Points per box edge when a map-CRS rectangle is carried into the layers' CRS.
const EDGE_STEPS: usize = 64;

/// A closed, counter-clockwise ring around a map-CRS box, `EDGE_STEPS` points per edge.
fn box_ring(b: [f64; 4]) -> Vec<(f64, f64)> {
    let [x0, y0, x1, y1] = b;
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut ring = Vec::with_capacity(4 * EDGE_STEPS + 1);
    for i in 0..4 {
        let (ax, ay) = corners[i];
        let (bx, by) = corners[(i + 1) % 4];
        for s in 0..EDGE_STEPS {
            let t = s as f64 / EDGE_STEPS as f64;
            ring.push((ax + (bx - ax) * t, ay + (by - ay) * t));
        }
    }
    ring.push(ring[0]);
    ring
}

/// A closed 48-gon of `radius` map-CRS units around (x, y).
fn disc_ring(x: f64, y: f64, radius: f64) -> Vec<(f64, f64)> {
    let mut ring: Vec<(f64, f64)> = (0..48)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / 48.0;
            (x + radius * a.cos(), y + radius * a.sin())
        })
        .collect();
    ring.push(ring[0]);
    ring
}

/// Features within `tolerance` (map-CRS units) of a clicked map-CRS point, nearest first. The disc
/// is built in the map CRS and carried into the layer's CRS, so the tolerance is right at any scale
/// and projection. Distances for the ordering are measured in the layer's stored (Web Mercator) CRS.
pub fn identify(conn: &Connection, info: &LayerInfo, x: f64, y: f64, crs: &str, tolerance: f64) -> Result<Vec<i64>> {
    if !info.has_geometry() || info.crs.is_none() {
        return Ok(vec![]);
    }
    if !(x.is_finite() && y.is_finite() && tolerance.is_finite() && tolerance > 0.0) {
        bail!("Identify needs a finite position and a positive tolerance.");
    }
    let region = region_sql(&disc_ring(x, y, tolerance), crs);
    let click = format!("ST_Transform(ST_Point({x}, {y}), {}, 'EPSG:3857', true)", crate::db::lit(&crate::crs::resolve(crs)));
    let sql = format!(
        "WITH q AS (SELECT {region} AS r, {click} AS p)
         SELECT __fid FROM {t}, q
         WHERE ST_Intersects(__g, q.r){d}
         ORDER BY ST_Distance(__g, q.p), __fid DESC LIMIT 25",
        t = ident(&info.table),
        d = info.def_and(),
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Identify through the layer's display table in the map CRS (`dtable`), where a pole or any place Web Mercator can't hold is a plain
/// coordinate: the search disc is in the map CRS already, so nothing is carried through EPSG:3857 (P18, bug G4).
pub fn identify_display(conn: &Connection, info: &LayerInfo, dtable: &str, x: f64, y: f64, tolerance: f64) -> Result<Vec<i64>> {
    if !(x.is_finite() && y.is_finite() && tolerance.is_finite() && tolerance > 0.0) {
        bail!("Identify needs a finite position and a positive tolerance.");
    }
    let pts: Vec<String> = disc_ring(x, y, tolerance).iter().map(|(a, b)| format!("{a:.9} {b:.9}")).collect();
    let sql = format!(
        "SELECT dt.__fid FROM {d} dt JOIN {t} lt ON lt.__fid = dt.__fid
         WHERE ST_Intersects(dt.__d, ST_GeomFromText('POLYGON(({ring}))')){def}
         ORDER BY ST_Distance(dt.__d, ST_Point({x}, {y})), dt.__fid DESC LIMIT 25",
        d = ident(dtable),
        t = ident(&info.table),
        ring = pts.join(", "),
        def = info.def_and(),
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Rectangle select through the display table in the map CRS (see `identify_display`).
pub fn select_rect_display(conn: &Connection, info: &LayerInfo, dtable: &str, bbox: [f64; 4]) -> Result<Vec<i64>> {
    if !bbox.iter().all(|v| v.is_finite()) {
        bail!("The selection box has to be finite.");
    }
    let (x0, y0, x1, y1) = (bbox[0].min(bbox[2]), bbox[1].min(bbox[3]), bbox[0].max(bbox[2]), bbox[1].max(bbox[3]));
    let sql = format!(
        "SELECT dt.__fid FROM {d} dt JOIN {t} lt ON lt.__fid = dt.__fid WHERE ST_Intersects(dt.__d, ST_MakeEnvelope({x0}, {y0}, {x1}, {y1})){def} ORDER BY dt.__fid",
        d = ident(dtable),
        t = ident(&info.table),
        def = info.def_and(),
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Features intersecting a map-CRS box `[xmin, ymin, xmax, ymax]` (rectangle select). The box is
/// densified in the map CRS before it is carried into the layer's CRS, so in a projected map the
/// curved edges are followed rather than approximated by a lon/lat envelope.
pub fn select_rect(conn: &Connection, info: &LayerInfo, bbox: [f64; 4], crs: &str) -> Result<Vec<i64>> {
    if !info.has_geometry() || info.crs.is_none() {
        return Ok(vec![]);
    }
    if !bbox.iter().all(|v| v.is_finite()) {
        bail!("The selection box has to be finite.");
    }
    let b = [bbox[0].min(bbox[2]), bbox[1].min(bbox[3]), bbox[0].max(bbox[2]), bbox[1].max(bbox[3])];
    let region = region_sql(&box_ring(b), crs);
    let sql = format!(
        "WITH q AS (SELECT {region} AS r) SELECT __fid FROM {t}, q WHERE ST_Intersects(__g, q.r){d} ORDER BY __fid",
        t = ident(&info.table),
        d = info.def_and(),
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// All attributes of one feature, for the identify popup.
pub fn feature(conn: &Connection, info: &LayerInfo, fid: i64) -> Result<Vec<(String, Option<String>)>> {
    let page = rows(conn, info, &RowsQuery { limit: 1, fids: Some(vec![fid]), ..Default::default() })?;
    let row = page.rows.into_iter().next().unwrap_or_default();
    Ok(info.fields.iter().map(|f| f.name.clone()).zip(row).collect())
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Measures {
    /// Geodesic area (m²), polygons only.
    pub area: Option<f64>,
    /// Geodesic perimeter (m), polygons only.
    pub perimeter: Option<f64>,
    /// Geodesic length (m), lines only.
    pub length: Option<f64>,
    /// Centroid lon/lat.
    pub centroid: Option<[f64; 2]>,
}

/// Geodesic measurements of one feature (on the WGS 84 ellipsoid).
pub fn measure(conn: &Connection, info: &LayerInfo, fid: i64) -> Result<Measures> {
    let Some(crs) = info.crs.as_deref() else { return Ok(Measures::default()) };
    if !info.has_geometry() {
        return Ok(Measures::default());
    }
    let ll = format!("ST_Transform(geom, {}, 'EPSG:4326', true)", crate::db::lit(&crate::crs::resolve(crs)));
    let sql = format!(
        "SELECT ST_Dimension(geom), ST_Area_Spheroid({ll}), ST_Perimeter_Spheroid({ll}), ST_Length_Spheroid({ll}), ST_X(ST_Centroid({ll})), ST_Y(ST_Centroid({ll}))
         FROM {} WHERE __fid = {fid}",
        ident(&info.table)
    );
    let r = conn.query_row(&sql, [], |r| {
        Ok((r.get::<_, Option<i32>>(0)?, r.get::<_, Option<f64>>(1)?, r.get::<_, Option<f64>>(2)?, r.get::<_, Option<f64>>(3)?, r.get::<_, Option<f64>>(4)?, r.get::<_, Option<f64>>(5)?))
    })?;
    Ok(Measures {
        area: if r.0 == Some(2) { r.1 } else { None },
        perimeter: if r.0 == Some(2) { r.2 } else { None },
        length: if r.0 == Some(1) { r.3 } else { None },
        centroid: match (r.4, r.5) { (Some(x), Some(y)) => Some([x, y]), _ => None },
    })
}
