//! Shape editing, package E3: reshape tools, exact input and copy/paste between layers (docs/SHAPE_EDITING_PLAN.md).
//!
//! The reshape tools and paste work on the layer's own CRS (where the stored shapes are) and put their result into the session as one
//! undo step (`EditOp::layer_wkb`), so Undo, Save and Discard treat them like any other edit. Exact input and the circle are computed here
//! so every number comes from the engine: distances, bearings and circles are geodesic on the WGS 84 ellipsoid; coordinates typed in
//! the layer's CRS, as longitude and latitude or as a USNG/MGRS reference are moved into the map's CRS with PROJ.
use crate::db::{ident, lit};
use crate::edit::{bytes_to_hex, hex_to_bytes, parse_wkb, pending, session, transform_wkb, write_wkb, ApplyResult, Coord, EditOp, Geom, Pending};
use crate::layers::LayerInfo;
use crate::{crs, usng, Engine};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A reshape of existing features in the session.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reshape {
    /// Union of the features; the first (or `keep`) keeps its attributes and id, the others are deleted.
    Merge { fids: Vec<i64>, keep: Option<i64> },
    /// A multipart feature becomes one feature per part (the first part keeps the id; the others copy its attributes).
    Explode { fid: i64 },
    /// Features of the same shape type become one multipart feature (the first keeps attributes and id).
    MakeMultipart { fids: Vec<i64> },
    /// Remove every interior ring of the polygons.
    FillHoles { fids: Vec<i64> },
    /// Cut a hole: `hole` is a polygon (hex WKB, map CRS) that lies inside the feature.
    CutHole { fid: i64, hole: String },
    /// Split a polygon by a line (hex WKB, map CRS) that crosses it from edge to edge; the first piece keeps the id.
    Split { fid: i64, line: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PasteResult {
    pub pasted: usize,
    /// Fields of the source layer that the target has no field of the same name for (their values were left behind).
    pub unmatched_fields: Vec<String>,
    /// Values that do not fit the target field's type; they were pasted as empty.
    pub failed_values: usize,
    pub apply: ApplyResult,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    /// Hex WKB in the map CRS.
    pub geometry: String,
    pub note: String,
}

fn props_expr(info: &LayerInfo) -> String {
    if info.fields.is_empty() {
        "'{}'::JSON".to_string()
    } else {
        format!("to_json({{{}}})", info.fields.iter().map(|f| format!("{}: {}", lit(&f.name), ident(&f.name))).collect::<Vec<_>>().join(", "))
    }
}

/// The feature's shape as the session sees it (pending or saved), in the layer's CRS.
fn current_wkb(conn: &Connection, info: &LayerInfo, pend: &BTreeMap<i64, Pending>, fid: i64) -> Result<Vec<u8>> {
    if let Some(p) = pend.get(&fid) {
        if p.deleted {
            bail!("Feature {fid} was deleted in this session.");
        }
        if let Some(w) = &p.wkb {
            return Ok(w.clone());
        }
    }
    conn.query_row(&format!("SELECT ST_AsWKB(geom) FROM {} WHERE __fid = {fid}", ident(&info.table)), [], |r| r.get::<_, Option<Vec<u8>>>(0))
        .ok()
        .flatten()
        .ok_or_else(|| anyhow!("Feature {fid} has no shape in \"{}\".", info.name))
}

/// The feature's attributes as the session sees them.
fn current_attrs(conn: &Connection, info: &LayerInfo, pend: &BTreeMap<i64, Pending>, fid: i64) -> Result<Map<String, Value>> {
    let mut m = Map::new();
    if !pend.get(&fid).is_some_and(|p| p.inserted) {
        let a: Option<String> = conn.query_row(&format!("SELECT {}::VARCHAR FROM {} WHERE __fid = {fid}", props_expr(info), ident(&info.table)), [], |r| r.get(0)).ok();
        if let Some(Value::Object(o)) = a.and_then(|a| serde_json::from_str(&a).ok()) {
            m = o;
        }
    }
    if let Some(p) = pend.get(&fid) {
        for (k, v) in &p.attrs {
            m.insert(k.clone(), v.clone());
        }
    }
    Ok(m)
}

fn layer_op(kind: &str, fid: Option<i64>, wkb: Option<Vec<u8>>, attrs: Option<Map<String, Value>>) -> EditOp {
    EditOp { op: kind.into(), fid, attributes: attrs, layer_wkb: wkb, ..Default::default() }
}

fn distinct(fids: &[i64], at_least: usize, what: &str) -> Result<()> {
    let mut v = fids.to_vec();
    v.sort_unstable();
    v.dedup();
    if v.len() < at_least || v.len() != fids.len() {
        bail!("{what} needs {at_least} or more different features selected.");
    }
    Ok(())
}

impl Engine {
    fn require_session(&self, conn: &Connection, id: &str) -> Result<LayerInfo> {
        let info = self.layer(id)?;
        session(conn, id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited. Start editing first.", info.name))?;
        Ok(info)
    }

    /// Reshape features of the layer being edited; the whole reshape is one undo step.
    pub fn edit_reshape(&self, layer_id: &str, map_crs: &str, op: &Reshape) -> Result<ApplyResult> {
        let (info, ops) = self.with_conn(|conn| {
            let info = self.require_session(conn, layer_id)?;
            let pend = pending(conn, layer_id)?;
            let lc = info.crs.clone().unwrap_or_default();
            let mut ops: Vec<EditOp> = vec![];
            match op {
                Reshape::Merge { fids, keep } => {
                    distinct(fids, 2, "Merge")?;
                    let keep = keep.unwrap_or(fids[0]);
                    if !fids.contains(&keep) {
                        bail!("The feature to keep must be one of the merged features.");
                    }
                    let mut acc = current_wkb(conn, &info, &pend, fids[0])?;
                    for f in &fids[1..] {
                        let next = current_wkb(conn, &info, &pend, *f)?;
                        acc = conn.query_row("SELECT ST_AsWKB(ST_Union(ST_GeomFromWKB(?), ST_GeomFromWKB(?)))", duckdb::params![acc, next], |r| r.get::<_, Vec<u8>>(0))?;
                    }
                    ops.push(layer_op("update", Some(keep), Some(acc), None));
                    for f in fids.iter().filter(|f| **f != keep) {
                        ops.push(layer_op("delete", Some(*f), None, None));
                    }
                }
                Reshape::Explode { fid } => {
                    let g = parse_wkb(&current_wkb(conn, &info, &pend, *fid)?)?;
                    if g.kind <= 3 || g.parts.len() < 2 {
                        bail!("That feature has one part, so there is nothing to explode.");
                    }
                    let attrs = current_attrs(conn, &info, &pend, *fid)?;
                    for (i, part) in g.parts.iter().enumerate() {
                        let single = Geom { kind: g.kind - 3, has_z: g.has_z, has_m: g.has_m, parts: vec![part.clone()] };
                        if i == 0 {
                            ops.push(layer_op("update", Some(*fid), Some(write_wkb(&single)), None));
                        } else {
                            ops.push(layer_op("insert", None, Some(write_wkb(&single)), Some(attrs.clone())));
                        }
                    }
                }
                Reshape::MakeMultipart { fids } => {
                    distinct(fids, 2, "Make multipart")?;
                    let mut kind = 0;
                    let mut out = Geom { kind: 0, has_z: false, has_m: false, parts: vec![] };
                    for f in fids {
                        let g = parse_wkb(&current_wkb(conn, &info, &pend, *f)?)?;
                        let base = if g.kind > 3 { g.kind - 3 } else { g.kind };
                        if kind != 0 && kind != base {
                            bail!("Only features of the same shape type (all points, all lines or all polygons) can be made multipart.");
                        }
                        kind = base;
                        out.has_z |= g.has_z;
                        out.has_m |= g.has_m;
                        out.parts.extend(g.parts);
                    }
                    out.kind = kind + 3;
                    ops.push(layer_op("update", Some(fids[0]), Some(write_wkb(&out)), None));
                    for f in &fids[1..] {
                        ops.push(layer_op("delete", Some(*f), None, None));
                    }
                }
                Reshape::FillHoles { fids } => {
                    for f in fids {
                        let mut g = parse_wkb(&current_wkb(conn, &info, &pend, *f)?)?;
                        if g.kind != 3 && g.kind != 6 {
                            bail!("Feature {f} is not a polygon, so it has no holes to fill.");
                        }
                        let mut any = false;
                        for part in g.parts.iter_mut() {
                            if part.len() > 1 {
                                any = true;
                                part.truncate(1);
                            }
                        }
                        if any {
                            ops.push(layer_op("update", Some(*f), Some(write_wkb(&g)), None));
                        }
                    }
                    if ops.is_empty() {
                        bail!("The selected polygons have no holes.");
                    }
                }
                Reshape::CutHole { fid, hole } => {
                    let poly = current_wkb(conn, &info, &pend, *fid)?;
                    let h = transform_wkb(conn, &hex_to_bytes(hole)?, map_crs, &lc)?;
                    let (inside, out): (bool, Vec<u8>) = conn.query_row(
                        "SELECT ST_Within(h, p), ST_AsWKB(ST_Difference(p, h)) FROM (SELECT ST_GeomFromWKB(?) AS p, ST_GeomFromWKB(?) AS h)",
                        duckdb::params![poly, h],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?;
                    if !inside {
                        bail!("A hole must lie completely inside the polygon. Draw it smaller, or use Split.");
                    }
                    ops.push(layer_op("update", Some(*fid), Some(out), None));
                }
                Reshape::Split { fid, line } => {
                    let poly = current_wkb(conn, &info, &pend, *fid)?;
                    let l = transform_wkb(conn, &hex_to_bytes(line)?, map_crs, &lc)?;
                    let mut stmt = conn.prepare(
                        "SELECT ST_AsWKB(u.geom) FROM (SELECT p, unnest(ST_Dump(g)) AS u FROM
                           (SELECT p, ST_Polygonize([ST_Node(ST_Collect([ST_Boundary(p), l]))]) AS g FROM (SELECT ST_GeomFromWKB(?) AS p, ST_GeomFromWKB(?) AS l)))
                         WHERE ST_Within(ST_PointOnSurface(u.geom), p)",
                    )?;
                    let pieces: Vec<Vec<u8>> = stmt.query_map(duckdb::params![poly, l], |r| r.get::<_, Vec<u8>>(0))?.collect::<Result<_, _>>()?;
                    if pieces.len() < 2 {
                        bail!("The line does not cut all the way across the polygon (from one edge to another), so nothing was split.");
                    }
                    let attrs = current_attrs(conn, &info, &pend, *fid)?;
                    for (i, p) in pieces.into_iter().enumerate() {
                        if i == 0 {
                            ops.push(layer_op("update", Some(*fid), Some(p), None));
                        } else {
                            ops.push(layer_op("insert", None, Some(p), Some(attrs.clone())));
                        }
                    }
                }
            }
            Ok((info, ops))
        })?;
        let _ = info;
        self.edit_apply(layer_id, map_crs, &ops)
    }

    /// A geodesic circle (WGS 84 ellipsoid) of ground radius `radius_m` around `center` (map CRS), drawn with `vertices` vertices, as a
    /// polygon in the map CRS.
    pub fn edit_circle(&self, map_crs: &str, center: [f64; 2], radius_m: f64, vertices: usize) -> Result<Shape> {
        if !(radius_m.is_finite() && radius_m > 0.0) {
            bail!("The radius must be a number of metres above zero.");
        }
        if !(3..=720).contains(&vertices) {
            bail!("A circle needs 3 to 720 vertices.");
        }
        self.with_conn(|conn| {
            let (lon, lat) = crs::transform_point(conn, center[0], center[1], map_crs, crs::WGS84)?;
            let geod = Geodesic::wgs84();
            let mut ring = Vec::with_capacity(vertices + 1);
            for i in 0..vertices {
                let az = 360.0 * i as f64 / vertices as f64;
                let (la, lo): (f64, f64) = geod.direct(lat, lon, az, radius_m);
                let (x, y) = crs::transform_point(conn, lo, la, crs::WGS84, map_crs)?;
                ring.push(Coord { x, y, z: f64::NAN, m: f64::NAN });
            }
            ring.push(ring[0]);
            let g = Geom { kind: 3, has_z: false, has_m: false, parts: vec![vec![ring]] };
            Ok(Shape { geometry: bytes_to_hex(&write_wkb(&g)), note: format!("A geodesic circle of radius {radius_m} m (WGS 84 ellipsoid) drawn with {vertices} vertices.") })
        })
    }

    /// The geodesic distance in metres (WGS 84 ellipsoid) between two map-CRS positions.
    pub fn edit_distance(&self, map_crs: &str, a: [f64; 2], b: [f64; 2]) -> Result<f64> {
        self.with_conn(|conn| {
            let (lo1, la1) = crs::transform_point(conn, a[0], a[1], map_crs, crs::WGS84)?;
            let (lo2, la2) = crs::transform_point(conn, b[0], b[1], map_crs, crs::WGS84)?;
            Ok(Geodesic::wgs84().inverse(la1, lo1, la2, lo2))
        })
    }

    /// An axis-aligned rectangle between two corners in the map CRS: its edges are straight lines in that CRS.
    pub fn edit_rectangle(&self, corner_a: [f64; 2], corner_b: [f64; 2]) -> Result<Shape> {
        if corner_a[0] == corner_b[0] || corner_a[1] == corner_b[1] {
            bail!("A rectangle needs two corners that differ in both directions.");
        }
        let c = |x: f64, y: f64| Coord { x, y, z: f64::NAN, m: f64::NAN };
        let ring = vec![c(corner_a[0], corner_a[1]), c(corner_b[0], corner_a[1]), c(corner_b[0], corner_b[1]), c(corner_a[0], corner_b[1]), c(corner_a[0], corner_a[1])];
        let g = Geom { kind: 3, has_z: false, has_m: false, parts: vec![vec![ring]] };
        Ok(Shape { geometry: bytes_to_hex(&write_wkb(&g)), note: "A rectangle with straight edges in the map's coordinate system.".into() })
    }

    /// The point `distance_m` from `from` along `bearing_deg` (clockwise from north), both measured on the WGS 84 ellipsoid; in the map CRS.
    pub fn edit_destination(&self, map_crs: &str, from: [f64; 2], distance_m: f64, bearing_deg: f64) -> Result<[f64; 2]> {
        if !distance_m.is_finite() || distance_m < 0.0 || !bearing_deg.is_finite() {
            bail!("Give a distance of zero or more metres and a bearing in degrees.");
        }
        self.with_conn(|conn| {
            let (lon, lat) = crs::transform_point(conn, from[0], from[1], map_crs, crs::WGS84)?;
            let (la, lo): (f64, f64) = Geodesic::wgs84().direct(lat, lon, bearing_deg, distance_m);
            let (x, y) = crs::transform_point(conn, lo, la, crs::WGS84, map_crs)?;
            Ok([x, y])
        })
    }

    /// Typed coordinates, one per line, as map-CRS positions. `kind`: "xy" (x, y in the LAYER's CRS), "lonlat" (longitude, latitude in
    /// degrees, WGS 84) or "usng" (USNG/MGRS references, grid on WGS 84). Errors name the line.
    pub fn edit_parse_coords(&self, layer_id: &str, map_crs: &str, kind: &str, text: &str) -> Result<Vec<[f64; 2]>> {
        let info = self.layer(layer_id)?;
        self.with_conn(|conn| {
            let mut out = vec![];
            for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
                let at = |e: anyhow::Error| anyhow!("Line {}: {e}", n + 1);
                let nums = || -> Result<(f64, f64)> {
                    let p: Vec<&str> = line.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
                    match p.as_slice() {
                        [a, b] => Ok((a.parse::<f64>().map_err(|_| anyhow!("'{a}' is not a number"))?, b.parse::<f64>().map_err(|_| anyhow!("'{b}' is not a number"))?)),
                        _ => bail!("expected two numbers (for example 500000, 4000000), found '{}'", line.trim()),
                    }
                };
                let (x, y) = match kind {
                    "xy" => {
                        let (a, b) = nums().map_err(at)?;
                        let from = info.crs.clone().ok_or_else(|| anyhow!("\"{}\" has no coordinate system.", info.name))?;
                        crs::transform_point(conn, a, b, &from, map_crs).map_err(at)?
                    }
                    "lonlat" => {
                        let (lon, lat) = nums().map_err(at)?;
                        if lon.abs() > 180.0 || lat.abs() > 90.0 {
                            return Err(at(anyhow!("longitude must be within ±180 and latitude within ±90 (longitude first)")));
                        }
                        crs::transform_point(conn, lon, lat, crs::WGS84, map_crs).map_err(at)?
                    }
                    "usng" => {
                        let r = usng::parse(line.trim()).map_err(at)?;
                        let north = if r.zone == 0 { r.band == 'Y' || r.band == 'Z' } else { r.band >= 'N' };
                        let code = usng::utm_crs(r.zone, north, usng::Datum::Wgs84);
                        let to_ll = |e: f64, n: f64, _: bool| crs::transform_point(conn, e, n, &code, crs::WGS84);
                        let (e, n, _) = if r.zone == 0 { usng::to_ups(&r).map_err(at)? } else { usng::to_utm(&r, &to_ll).map_err(at)? };
                        let half = r.cell_m() / 2.0;
                        let (lon, lat) = to_ll(e + half, n + half, north).map_err(at)?;
                        crs::transform_point(conn, lon, lat, crs::WGS84, map_crs).map_err(at)?
                    }
                    other => bail!("Unknown coordinate kind '{other}' (use xy, lonlat or usng)."),
                };
                out.push([x, y]);
            }
            if out.is_empty() {
                bail!("Type at least one coordinate.");
            }
            Ok(out)
        })
    }

    /// Copy features of another layer into the layer being edited, as one undo step. Shapes are moved into the target's CRS explicitly
    /// (PROJ); fields match by name (case does not matter) and the rest are reported, never guessed.
    pub fn edit_paste(&self, from_layer: &str, fids: &[i64], to_layer: &str) -> Result<PasteResult> {
        if fids.is_empty() {
            bail!("Select features to copy first.");
        }
        let from = self.layer(from_layer)?;
        let (ops, unmatched, failed) = self.with_conn(|conn| {
            let to = self.require_session(conn, to_layer)?;
            if !from.has_geometry() {
                bail!("\"{}\" has no shapes to paste.", from.name);
            }
            let (fc, tc) = (
                from.crs.clone().ok_or_else(|| anyhow!("\"{}\" has no coordinate system, so its shapes can't be moved into \"{}\".", from.name, to.name))?,
                to.crs.clone().unwrap_or_default(),
            );
            let matched: Vec<(String, String, String)> = from
                .fields
                .iter()
                .filter_map(|f| to.fields.iter().find(|t| t.name.eq_ignore_ascii_case(&f.name)).map(|t| (f.name.clone(), t.name.clone(), t.ty.clone())))
                .collect();
            let unmatched: Vec<String> = from.fields.iter().filter(|f| !matched.iter().any(|m| m.0 == f.name)).map(|f| f.name.clone()).collect();
            let ids = fids.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
            let mut stmt = conn.prepare(&format!("SELECT __fid, ST_AsWKB(geom), {}::VARCHAR FROM {} WHERE geom IS NOT NULL AND __fid IN ({ids}) ORDER BY __fid", props_expr(&from), ident(&from.table)))?;
            let rows: Vec<(i64, Vec<u8>, Option<String>)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
            if rows.is_empty() {
                bail!("None of the selected features has a shape to paste.");
            }
            let mut ops = vec![];
            let mut failed = 0usize;
            for (_, wkb, props) in rows {
                let g = transform_wkb(conn, &wkb, &fc, &tc)?;
                let src: Map<String, Value> = props.and_then(|p| serde_json::from_str(&p).ok()).unwrap_or_default();
                let mut attrs = Map::new();
                for (s, t, ty) in &matched {
                    let v = src.get(s).cloned().unwrap_or(Value::Null);
                    if let Some(text) = crate::edit::json_text(&v) {
                        let fits: bool = conn.query_row(&format!("SELECT TRY_CAST(? AS {ty}) IS NOT NULL"), [text], |r| r.get(0)).unwrap_or(false);
                        if !fits {
                            failed += 1;
                            attrs.insert(t.clone(), Value::Null);
                            continue;
                        }
                    }
                    attrs.insert(t.clone(), v);
                }
                ops.push(layer_op("insert", None, Some(g), Some(attrs)));
            }
            Ok((ops, unmatched, failed))
        })?;
        let n = ops.len();
        let to = self.layer(to_layer)?;
        let apply = self.edit_apply(to_layer, to.crs.as_deref().unwrap_or(crs::WGS84), &ops)?;
        Ok(PasteResult { pasted: n, unmatched_fields: unmatched, failed_values: failed, apply })
    }
}
