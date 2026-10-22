//! Attribute table paging, identify and selection queries.

use crate::crs::{lonlat_to_merc, pixel_size};
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

/// Features near a clicked point, nearest first. Tolerance is a few pixels at
/// the current zoom.
pub fn identify(conn: &Connection, info: &LayerInfo, lon: f64, lat: f64, zoom: f64) -> Result<Vec<i64>> {
    if !info.has_geometry() || info.crs.is_none() {
        return Ok(vec![]);
    }
    let (x, y) = lonlat_to_merc(lon, lat);
    let tol = pixel_size(0) / 2f64.powf(zoom) * 4.0;
    let sql = format!(
        "SELECT __fid FROM {t}
         WHERE ST_Intersects(__g, ST_MakeEnvelope({x0}, {y0}, {x1}, {y1}))
           AND ST_DWithin(__g, ST_Point({x}, {y}), {tol}){d}
         ORDER BY ST_Distance(__g, ST_Point({x}, {y})), __fid DESC LIMIT 25",
        t = ident(&info.table),
        d = info.def_and(),
        x0 = x - tol, y0 = y - tol, x1 = x + tol, y1 = y + tol
    );
    let mut stmt = conn.prepare(&sql)?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Features intersecting a lon/lat box (rectangle select).
pub fn select_box(conn: &Connection, info: &LayerInfo, bbox: [f64; 4]) -> Result<Vec<i64>> {
    if !info.has_geometry() || info.crs.is_none() {
        return Ok(vec![]);
    }
    let (x0, y0) = lonlat_to_merc(bbox[0], bbox[1]);
    let (x1, y1) = lonlat_to_merc(bbox[2], bbox[3]);
    let mut stmt = conn.prepare(&format!(
        "SELECT __fid FROM {} WHERE ST_Intersects(__g, ST_MakeEnvelope({x0}, {y0}, {x1}, {y1})){} ORDER BY __fid",
        ident(&info.table),
        info.def_and()
    ))?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// All attributes of one feature, for the identify popup.
pub fn feature(conn: &Connection, info: &LayerInfo, fid: i64) -> Result<Vec<(String, Option<String>)>> {
    let page = rows(conn, info, &RowsQuery { limit: 1, fids: Some(vec![fid]), ..Default::default() })?;
    let row = page.rows.into_iter().next().unwrap_or_default();
    Ok(info.fields.iter().map(|f| f.name.clone()).zip(row).collect())
}
