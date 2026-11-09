//! Shape editing (SHAPE_EDITING_PLAN, package E1): an edit session on one layer, kept as an ordered log of pending edits, saved to the
//! layer in one transaction or discarded.
//!
//! * The log `__edits_<layer>` holds (seq, step, op, fid, wkb in the LAYER's CRS, attributes JSON, undone). A step is one `edit_apply`
//!   call; undo and redo flip whole steps. The pending state of a feature is its last geometry and its merged attributes.
//! * Only the edit session sees pending edits. Tiles, tables, exports, layouts and node trees read the saved table until Save.
//! * Drawing happens in the map's CRS but a layer stores its own. A vertex that comes back from the map unchanged keeps its stored
//!   layer-CRS coordinate, Z and M exactly (`reconcile`); only changed or new vertices are transformed, by PROJ in the engine.
//! * Feature ids are stable: edited features keep theirs, new ones take ids above the highest ever used in the layer (`__edit_fids`).
//!   Save never calls `refresh_display`, which would renumber them.
use crate::db::{ident, lit};
use crate::layers::{self, LayerInfo};
use crate::progress::Task;
use crate::{crs, sources, Engine};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};

// ---------------------------------------------------------------- WKB (ISO and EWKB, little or big endian; Z and M kept)

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub m: f64,
}

/// A geometry as parts → rings → coordinates. A point has one part with one ring of one coordinate; a line one part with one ring;
/// a polygon one part with its rings; the multi types have one part per member.
#[derive(Clone, Debug, PartialEq)]
pub struct Geom {
    /// WKB base type: 1 Point, 2 LineString, 3 Polygon, 4 MultiPoint, 5 MultiLineString, 6 MultiPolygon.
    pub kind: u32,
    pub has_z: bool,
    pub has_m: bool,
    pub parts: Vec<Vec<Vec<Coord>>>,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.i + n > self.b.len() {
            bail!("The geometry is cut short (not valid WKB).");
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn u32(&mut self, le: bool) -> Result<u32> {
        let a: [u8; 4] = self.take(4)?.try_into().unwrap();
        Ok(if le { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) })
    }
    fn f64(&mut self, le: bool) -> Result<f64> {
        let a: [u8; 8] = self.take(8)?.try_into().unwrap();
        Ok(if le { f64::from_le_bytes(a) } else { f64::from_be_bytes(a) })
    }
}

fn read_header(r: &mut Reader) -> Result<(bool, u32, bool, bool)> {
    let le = match r.take(1)?[0] {
        1 => true,
        0 => false,
        _ => bail!("The geometry has an unknown byte order (not valid WKB)."),
    };
    let code = r.u32(le)?;
    let (mut z, mut m) = (code & 0x8000_0000 != 0, code & 0x4000_0000 != 0);
    if code & 0x2000_0000 != 0 {
        r.u32(le)?; // an SRID: the layer's CRS decides, not the blob
    }
    let low = code & 0x0FFF_FFFF;
    let (base, dims) = (low % 1000, low / 1000);
    match dims {
        1 => z = true,
        2 => m = true,
        3 => {
            z = true;
            m = true
        }
        _ => {}
    }
    Ok((le, base, z, m))
}

fn read_coords(r: &mut Reader, le: bool, n: usize, z: bool, m: bool) -> Result<Vec<Coord>> {
    if n > r.b.len() {
        bail!("The geometry is cut short (not valid WKB).");
    }
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.f64(le)?;
        let y = r.f64(le)?;
        let zz = if z { r.f64(le)? } else { f64::NAN };
        let mm = if m { r.f64(le)? } else { f64::NAN };
        v.push(Coord { x, y, z: zz, m: mm });
    }
    Ok(v)
}

fn read_one(r: &mut Reader) -> Result<Geom> {
    let (le, base, z, m) = read_header(r)?;
    let mut g = Geom { kind: base, has_z: z, has_m: m, parts: vec![] };
    match base {
        1 => g.parts.push(vec![read_coords(r, le, 1, z, m)?]),
        2 => {
            let n = r.u32(le)? as usize;
            g.parts.push(vec![read_coords(r, le, n, z, m)?]);
        }
        3 => g.parts.push(read_rings(r, le, z, m)?),
        4 | 5 | 6 => {
            let n = r.u32(le)?;
            for _ in 0..n {
                let c = read_one(r)?;
                if c.kind != base - 3 {
                    bail!("A multi-geometry holds a member of the wrong type (not valid WKB).");
                }
                g.has_z |= c.has_z;
                g.has_m |= c.has_m;
                g.parts.push(c.parts.into_iter().next().unwrap_or_default());
            }
        }
        k => bail!("Geometry type {k} (a geometry collection or curve) can't be edited yet."),
    }
    Ok(g)
}

fn read_rings(r: &mut Reader, le: bool, z: bool, m: bool) -> Result<Vec<Vec<Coord>>> {
    let n = r.u32(le)? as usize;
    let mut rings = Vec::new();
    for _ in 0..n {
        let k = r.u32(le)? as usize;
        rings.push(read_coords(r, le, k, z, m)?);
    }
    Ok(rings)
}

pub fn parse_wkb(b: &[u8]) -> Result<Geom> {
    let mut r = Reader { b, i: 0 };
    read_one(&mut r)
}

fn put_header(out: &mut Vec<u8>, base: u32, z: bool, m: bool) {
    out.push(1);
    out.extend_from_slice(&(base + if z { 1000 } else { 0 } + if m { 2000 } else { 0 }).to_le_bytes());
}

fn put_coords(out: &mut Vec<u8>, cs: &[Coord], z: bool, m: bool) {
    for c in cs {
        out.extend_from_slice(&c.x.to_le_bytes());
        out.extend_from_slice(&c.y.to_le_bytes());
        if z {
            out.extend_from_slice(&c.z.to_le_bytes());
        }
        if m {
            out.extend_from_slice(&c.m.to_le_bytes());
        }
    }
}

fn put_part(out: &mut Vec<u8>, kind: u32, z: bool, m: bool, rings: &[Vec<Coord>]) {
    put_header(out, kind, z, m);
    match kind {
        1 => put_coords(out, &rings[0], z, m),
        2 => {
            out.extend_from_slice(&(rings[0].len() as u32).to_le_bytes());
            put_coords(out, &rings[0], z, m);
        }
        _ => {
            out.extend_from_slice(&(rings.len() as u32).to_le_bytes());
            for ring in rings {
                out.extend_from_slice(&(ring.len() as u32).to_le_bytes());
                put_coords(out, ring, z, m);
            }
        }
    }
}

/// ISO WKB, little endian.
pub fn write_wkb(g: &Geom) -> Vec<u8> {
    let mut out = Vec::new();
    let (z, m) = (g.has_z, g.has_m);
    if g.kind <= 3 {
        put_part(&mut out, g.kind, z, m, &g.parts[0]);
    } else {
        put_header(&mut out, g.kind, z, m);
        out.extend_from_slice(&(g.parts.len() as u32).to_le_bytes());
        for p in &g.parts {
            put_part(&mut out, g.kind - 3, z, m, p);
        }
    }
    out
}

/// Does a shape in geographic coordinates have a segment spanning more than half the world in longitude (it crosses the antimeridian)?
pub fn crosses_antimeridian(g: &Geom) -> bool {
    g.parts.iter().flatten().any(|ring| ring.windows(2).any(|w| (w[1].x - w[0].x).abs() > 180.0))
}

const ANTIMERIDIAN: &str = "That shape crosses the antimeridian (180° longitude). Shapes crossing it aren't handled in this version; draw it in two parts.";

impl Geom {
    fn coords(&self) -> impl Iterator<Item = &Coord> {
        self.parts.iter().flatten().flatten()
    }
    fn coords_mut(&mut self) -> impl Iterator<Item = &mut Coord> {
        self.parts.iter_mut().flatten().flatten()
    }
    /// The shape of the geometry (type and the number of coordinates in every ring): two geometries with the same shape correspond
    /// vertex for vertex.
    fn shape(&self) -> (u32, Vec<Vec<usize>>) {
        (self.kind, self.parts.iter().map(|p| p.iter().map(|r| r.len()).collect()).collect())
    }
    fn dimension(&self) -> i32 {
        match self.kind {
            1 | 4 => 0,
            2 | 5 => 1,
            _ => 2,
        }
    }
}

/// The layer-CRS geometry for a shape the map sent back.
///
/// `new_map` is what the map returned (map CRS), `new_layer` the same geometry transformed by PROJ (same vertex order),
/// `orig_map` the stored geometry as the engine sent it to the map and `orig_layer` the stored geometry. A vertex of `new_map` that
/// equals a vertex of `orig_map` bit for bit is untouched: it takes the stored layer coordinate with its Z and M. Every other vertex
/// takes the transformed coordinate; when the shape is the same as the original's (nothing added or removed) its Z and M are the
/// stored ones at that position, otherwise `z` (the session's choice) and no M.
pub fn reconcile(new_map: &Geom, new_layer: &Geom, orig_map: Option<&Geom>, orig_layer: Option<&Geom>, z: Option<f64>) -> Result<Geom> {
    if new_map.shape() != new_layer.shape() {
        bail!("The coordinate transform changed the shape of a geometry (internal error).");
    }
    let mut out = new_layer.clone();
    let mut has_z = new_map.has_z || new_layer.has_z;
    let mut has_m = new_map.has_m || new_layer.has_m;
    match (orig_map, orig_layer) {
        (Some(om), Some(ol)) if om.shape() == ol.shape() => {
            has_z |= ol.has_z;
            has_m |= ol.has_m;
            // Stored vertices by their map coordinates; a vertex consumes one entry, so a closed ring's two equal ends both match.
            let mut pool: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
            for (i, c) in om.coords().enumerate() {
                pool.entry((c.x.to_bits(), c.y.to_bits())).or_default().push(i);
            }
            for v in pool.values_mut() {
                v.reverse();
            }
            let stored: Vec<Coord> = ol.coords().copied().collect();
            let same_shape = om.shape() == new_map.shape();
            let nm: Vec<Coord> = new_map.coords().copied().collect();
            for (i, c) in out.coords_mut().enumerate() {
                let key = (nm[i].x.to_bits(), nm[i].y.to_bits());
                if let Some(j) = pool.get_mut(&key).and_then(|v| v.pop()) {
                    *c = stored[j];
                } else if same_shape {
                    c.z = if nm[i].z.is_nan() { stored[i].z } else { nm[i].z };
                    c.m = if nm[i].m.is_nan() { stored[i].m } else { nm[i].m };
                } else {
                    c.z = if !nm[i].z.is_nan() { nm[i].z } else { z.unwrap_or(f64::NAN) };
                    c.m = nm[i].m;
                }
            }
        }
        _ => {
            for (i, c) in out.coords_mut().enumerate() {
                let n = new_map.coords().nth(i).copied().unwrap_or(*c);
                c.z = if !n.z.is_nan() { n.z } else { z.unwrap_or(f64::NAN) };
                c.m = n.m;
            }
            has_z |= z.is_some();
        }
    }
    out.has_z = has_z;
    out.has_m = has_m;
    Ok(out)
}

// ---------------------------------------------------------------- the session

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditOp {
    /// "insert", "update" or "delete".
    pub op: String,
    pub fid: Option<i64>,
    /// WKB as hex, in the MAP's CRS (as `edit_features_in` sent it, with the edits).
    pub geometry: Option<String>,
    /// Field values as typed (text, numbers, booleans, null). Fields not named keep their values.
    pub attributes: Option<Map<String, Value>>,
    /// Repair an invalid shape (ST_MakeValid) instead of keeping it as drawn.
    pub fix: bool,
    /// A shape already in the LAYER's CRS (built by the engine's reshape and paste tools): used as it is, no map CRS involved.
    #[serde(skip)]
    pub layer_wkb: Option<Vec<u8>>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditStatus {
    pub layer: String,
    pub editing: bool,
    pub map_crs: Option<String>,
    pub z_value: Option<f64>,
    pub added: i64,
    pub changed: i64,
    pub deleted: i64,
    pub steps: i64,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Pending shapes that are not valid (self-crossing polygons and the like).
    pub invalid: Vec<i64>,
    /// The most recent Save can be undone (the layer has not changed since).
    pub can_undo_save: bool,
    /// The layer's shapes have Z (or M) values: the session's Z choice for new vertices matters.
    pub layer_has_z: bool,
    pub layer_has_m: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub status: EditStatus,
    /// The id of each insert, in the order of the inserts in the batch.
    pub fids: Vec<i64>,
    pub invalid: Vec<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditFeature {
    pub fid: i64,
    /// Hex WKB in the map CRS; null for a feature without geometry.
    pub geometry: Option<String>,
    pub attributes: Map<String, Value>,
    /// "inserted" or "changed" for a feature with pending edits.
    pub pending: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditFeatures {
    pub features: Vec<EditFeature>,
    pub truncated: bool,
    pub map_crs: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub layer: LayerInfo,
    pub added: i64,
    pub changed: i64,
    pub deleted: i64,
    pub invalid: i64,
    pub note: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EditLogRow {
    pub at: String,
    pub added: i64,
    pub changed: i64,
    pub deleted: i64,
}

pub(crate) struct Session {
    pub(crate) map_crs: String,
    pub(crate) z_value: Option<f64>,
    pub(crate) next_fid: i64,
}

fn undo_table(layer: &str) -> String {
    format!("__edits_undo_{layer}")
}

/// Has the layer stayed exactly as its latest Save left it (so that Save can be undone)?
fn can_undo_save(conn: &Connection, info: &LayerInfo) -> bool {
    !info.editing
        && conn.query_row("SELECT version FROM __edit_undo_meta WHERE layer = ?", [&info.id], |r| r.get::<_, i64>(0)).map(|v| v == info.version as i64).unwrap_or(false)
}

pub(crate) fn log_table(layer: &str) -> String {
    format!("__edits_{layer}")
}

pub(crate) fn hex_to_bytes(h: &str) -> Result<Vec<u8>> {
    let h = h.trim();
    if h.len() % 2 != 0 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("The geometry is not hexadecimal WKB.");
    }
    Ok((0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect())
}

pub fn bytes_to_hex(b: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(D[(x >> 4) as usize] as char);
        s.push(D[(x & 15) as usize] as char);
    }
    s
}

/// Transform WKB between CRSs with PROJ (the same call for sending shapes out and taking them back).
pub(crate) fn transform_wkb(conn: &Connection, wkb: &[u8], from: &str, to: &str) -> Result<Vec<u8>> {
    if from == to {
        return Ok(wkb.to_vec());
    }
    conn.query_row("SELECT ST_AsWKB(ST_Transform(ST_GeomFromWKB(?), ?, ?, true))", duckdb::params![wkb.to_vec(), from, to], |r| r.get::<_, Vec<u8>>(0))
        .map_err(|e| anyhow!("Could not move the shape between coordinate systems ({from} → {to}): {e}"))
}

pub(crate) fn session(conn: &Connection, layer: &str) -> Result<Option<Session>> {
    let mut s = conn.prepare("SELECT map_crs, z_value, next_fid FROM __edit_sessions WHERE layer = ?")?;
    let mut rows = s.query([layer])?;
    Ok(match rows.next()? {
        Some(r) => Some(Session { map_crs: r.get(0)?, z_value: r.get(1)?, next_fid: r.get(2)? }),
        None => None,
    })
}

/// A feature's pending state, from the non-undone log rows in order.
#[derive(Default, Clone)]
pub(crate) struct Pending {
    pub(crate) inserted: bool,
    pub(crate) deleted: bool,
    pub(crate) wkb: Option<Vec<u8>>,
    pub(crate) attrs: Map<String, Value>,
}

pub(crate) fn pending(conn: &Connection, layer: &str) -> Result<BTreeMap<i64, Pending>> {
    let mut s = conn.prepare(&format!("SELECT op, fid, wkb, attrs FROM {} WHERE NOT undone ORDER BY seq", ident(&log_table(layer))))?;
    let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<Vec<u8>>>(2)?, r.get::<_, Option<String>>(3)?)))?;
    let mut m: BTreeMap<i64, Pending> = BTreeMap::new();
    for row in rows {
        let (op, fid, wkb, attrs) = row?;
        let p = m.entry(fid).or_default();
        match op.as_str() {
            "insert" => {
                *p = Pending { inserted: true, ..Default::default() };
            }
            "delete" => p.deleted = true,
            _ => {}
        }
        if op != "delete" {
            if wkb.is_some() {
                p.wkb = wkb;
            }
            if let Some(Value::Object(o)) = attrs.and_then(|a| serde_json::from_str(&a).ok()) {
                for (k, v) in o {
                    p.attrs.insert(k, v);
                }
            }
        }
    }
    // An insert that was deleted again leaves nothing behind.
    // An insert that was deleted again, and an update that changes nothing, leave nothing behind.
    m.retain(|_, p| !(p.inserted && p.deleted) && (p.inserted || p.deleted || p.wkb.is_some() || !p.attrs.is_empty()));
    Ok(m)
}

pub(crate) fn json_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

/// `CAST('text' AS type)` for a value typed into field `name`, or NULL.
fn cast_sql(info: &LayerInfo, name: &str, v: &Value) -> Result<String> {
    let f = info.fields.iter().find(|f| f.name == name).ok_or_else(|| anyhow!("Field '{name}' not found in \"{}\".", info.name))?;
    Ok(match json_text(v) {
        None => "NULL".into(),
        Some(t) => format!("CAST({} AS {})", lit(&t), f.ty),
    })
}

fn check_editable(info: &LayerInfo) -> Result<()> {
    if info.is_raster() {
        bail!("\"{}\" is a raster. Shapes can only be edited on vector layers.", info.name);
    }
    if info.result_of.is_some() {
        bail!("\"{}\" is a node-tree result, which is rebuilt on every run. Make an editable copy first.", info.name);
    }
    if !info.has_geometry() {
        bail!("\"{}\" is a table with no shapes.", info.name);
    }
    if info.crs.is_none() {
        bail!("\"{}\" has no coordinate system, so shapes can't be edited yet. Use Define Projection first.", info.name);
    }
    Ok(())
}

impl Engine {
    /// Start an edit session on `layer_id`, or resume the one left open (a crash, or the project closed while editing).
    /// `map_crs` is the CRS the map draws in; `z_value` the Z given to new vertices on layers with Z (None: no Z value).
    pub fn edit_begin(&self, layer_id: &str, map_crs: &str, z_value: Option<f64>) -> Result<EditStatus> {
        let mut info = self.layer(layer_id)?;
        check_editable(&info)?;
        self.with_conn(|conn| {
            crs::validate(conn, map_crs)?;
            if session(conn, layer_id)?.is_some() {
                // Resuming: the map and the Z choice of this call apply.
                conn.execute("UPDATE __edit_sessions SET map_crs = ?, z_value = ? WHERE layer = ?", duckdb::params![map_crs, z_value, layer_id])?;
            } else {
                let recorded: i64 = conn.query_row("SELECT coalesce(max(max_fid), 0) FROM __edit_fids WHERE layer = ?", [layer_id], |r| r.get(0))?;
                let top: i64 = conn.query_row(&format!("SELECT coalesce(max(__fid), 0) FROM {}", ident(&info.table)), [], |r| r.get(0))?;
                conn.execute_batch(&format!(
                    "CREATE TABLE IF NOT EXISTS {t} (seq BIGINT, step BIGINT, op VARCHAR, fid BIGINT, wkb BLOB, attrs VARCHAR, undone BOOLEAN)",
                    t = ident(&log_table(layer_id))
                ))?;
                conn.execute(
                    "INSERT INTO __edit_sessions VALUES (?, ?, ?, now(), ?, ?)",
                    duckdb::params![layer_id, map_crs, z_value, recorded.max(top) + 1, info.version as i64],
                )?;
            }
            info.editing = true;
            self.save_meta(conn, &info)?;
            self.drop_tiles(layer_id);
            self.edit_status(conn, &info)
        })
    }

    /// What is pending on a layer (also how a reopened project learns that edits are waiting to be resumed or discarded).
    pub fn edit_pending(&self, layer_id: &str) -> Result<EditStatus> {
        let info = self.layer(layer_id)?;
        self.with_conn(|conn| self.edit_status(conn, &info))
    }

    fn edit_status(&self, conn: &Connection, info: &LayerInfo) -> Result<EditStatus> {
        let Some(s) = session(conn, &info.id)? else {
            return Ok(EditStatus { layer: info.id.clone(), can_undo_save: can_undo_save(conn, info), ..Default::default() });
        };
        let t = ident(&log_table(&info.id));
        let p = pending(conn, &info.id)?;
        let (mut added, mut changed, mut deleted) = (0, 0, 0);
        let mut invalid = vec![];
        for (fid, st) in &p {
            if st.inserted {
                added += 1;
            } else if st.deleted {
                deleted += 1;
            } else {
                changed += 1;
            }
            if !st.deleted {
                if let Some(w) = &st.wkb {
                    if !conn.query_row("SELECT ST_IsValid(ST_GeomFromWKB(?))", [w.clone()], |r| r.get::<_, bool>(0)).unwrap_or(true) {
                        invalid.push(*fid);
                    }
                }
            }
        }
        let zm = conn
            .query_row(&format!("SELECT ST_AsWKB(geom) FROM {} WHERE geom IS NOT NULL LIMIT 1", ident(&info.table)), [], |r| r.get::<_, Vec<u8>>(0))
            .ok()
            .and_then(|b| parse_wkb(&b).ok())
            .map(|g| (g.has_z, g.has_m))
            .unwrap_or((false, false));
        let steps: i64 = conn.query_row(&format!("SELECT count(DISTINCT step) FROM {t} WHERE NOT undone"), [], |r| r.get(0))?;
        let undone: i64 = conn.query_row(&format!("SELECT count(DISTINCT step) FROM {t} WHERE undone"), [], |r| r.get(0))?;
        Ok(EditStatus {
            layer: info.id.clone(),
            editing: true,
            map_crs: Some(s.map_crs),
            z_value: s.z_value,
            added,
            changed,
            deleted,
            steps,
            can_undo: steps > 0,
            can_redo: undone > 0,
            invalid,
            can_undo_save: false,
            layer_has_z: zm.0,
            layer_has_m: zm.1,
        })
    }

    /// Features in and near `extent` ([west, south, east, north] in lon/lat), as the session sees them: saved features with the pending
    /// edits applied, shapes in the map CRS. At most `limit` saved features are loaded; pending ones are always included.
    pub fn edit_features_in(&self, layer_id: &str, map_crs: &str, extent: [f64; 4], limit: usize) -> Result<EditFeatures> {
        let info = self.layer(layer_id)?;
        let lc = info.crs.clone().ok_or_else(|| anyhow!("\"{}\" has no coordinate system.", info.name))?;
        self.with_conn(|conn| {
            session(conn, layer_id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited. Start editing first.", info.name))?;
            crs::validate(conn, map_crs)?;
            let pend = pending(conn, layer_id)?;
            let props = if info.fields.is_empty() {
                "'{}'::JSON".to_string()
            } else {
                format!("to_json({{{}}})", info.fields.iter().map(|f| format!("{}: {}", lit(&f.name), ident(&f.name))).collect::<Vec<_>>().join(", "))
            };
            let to_map = if lc == map_crs { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&lc), lit(map_crs)) };
            let [w, so, e, n] = extent;
            let (so, n) = (so.clamp(-85.0, 85.0), n.clamp(-85.0, 85.0));
            let q = format!(
                "SELECT __fid, ST_AsWKB({to_map}), {props}::VARCHAR FROM {t} WHERE geom IS NOT NULL AND ST_Intersects(__g, ST_Transform(ST_MakeEnvelope({w}, {so}, {e}, {n}), {wgs}, {wm}, true)){d} ORDER BY __fid LIMIT {lim}",
                t = ident(&info.table),
                wgs = lit(crs::WGS84),
                wm = lit(crs::WEB_MERCATOR),
                d = info.def_and(),
                lim = limit + 1
            );
            let mut stmt = conn.prepare(&q)?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<Vec<u8>>>(1)?, r.get::<_, Option<String>>(2)?)))?;
            let mut features = Vec::new();
            let mut truncated = false;
            for row in rows {
                let (fid, wkb, attrs) = row?;
                if features.len() >= limit {
                    truncated = true;
                    break;
                }
                if pend.contains_key(&fid) {
                    continue; // shown from the pending state below
                }
                let attributes = attrs.and_then(|a| serde_json::from_str::<Map<String, Value>>(&a).ok()).unwrap_or_default();
                features.push(EditFeature { fid, geometry: wkb.map(|b| bytes_to_hex(&b)), attributes, pending: None });
            }
            drop(stmt);
            for (fid, st) in &pend {
                if st.deleted {
                    continue;
                }
                let mut attributes = Map::new();
                if !st.inserted {
                    let a: Option<String> = conn
                        .query_row(&format!("SELECT {props}::VARCHAR FROM {} WHERE __fid = {fid}", ident(&info.table)), [], |r| r.get(0))
                        .ok();
                    attributes = a.and_then(|a| serde_json::from_str(&a).ok()).unwrap_or_default();
                }
                for (k, v) in &st.attrs {
                    attributes.insert(k.clone(), v.clone());
                }
                let wkb = match &st.wkb {
                    Some(w) => Some(transform_wkb(conn, w, &lc, map_crs)?),
                    None => conn
                        .query_row(&format!("SELECT ST_AsWKB({to_map}) FROM {} WHERE __fid = {fid}", ident(&info.table)), [], |r| r.get::<_, Option<Vec<u8>>>(0))
                        .ok()
                        .flatten(),
                };
                features.push(EditFeature { fid: *fid, geometry: wkb.map(|b| bytes_to_hex(&b)), attributes, pending: Some(if st.inserted { "inserted" } else { "changed" }.into()) });
            }
            Ok(EditFeatures { features, truncated, map_crs: map_crs.to_string() })
        })
    }

    /// Shapes of any layer in and near `extent` ([west, south, east, north] in lon/lat) as hex WKB in the map CRS, for snapping to the
    /// vertices of other layers. Never changes anything. At most `limit` features.
    pub fn snap_features_in(&self, layer_id: &str, map_crs: &str, extent: [f64; 4], limit: usize) -> Result<Vec<String>> {
        let info = self.layer(layer_id)?;
        let lc = info.crs.clone().ok_or_else(|| anyhow!("\"{}\" has no coordinate system.", info.name))?;
        if !info.has_geometry() {
            return Ok(vec![]);
        }
        self.with_conn(|conn| {
            crs::validate(conn, map_crs)?;
            let to_map = if lc == map_crs { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&lc), lit(map_crs)) };
            let [w, so, e, n] = extent;
            let (so, n) = (so.clamp(-85.0, 85.0), n.clamp(-85.0, 85.0));
            let mut stmt = conn.prepare(&format!(
                "SELECT ST_AsWKB({to_map}) FROM {t} WHERE geom IS NOT NULL AND ST_Intersects(__g, ST_Transform(ST_MakeEnvelope({w}, {so}, {e}, {n}), {wgs}, {wm}, true)){d} LIMIT {limit}",
                t = ident(&info.table),
                wgs = lit(crs::WGS84),
                wm = lit(crs::WEB_MERCATOR),
                d = info.def_and()
            ))?;
            let rows = stmt.query_map([], |r| r.get::<_, Vec<u8>>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?.iter().map(|b| bytes_to_hex(b)).collect())
        })
    }

    /// One undo step: a batch of inserts, updates and deletes. Shapes arrive in the map CRS.
    pub fn edit_apply(&self, layer_id: &str, map_crs: &str, ops: &[EditOp]) -> Result<ApplyResult> {
        let info = self.layer(layer_id)?;
        if ops.is_empty() {
            bail!("Nothing to apply.");
        }
        self.with_conn(|conn| {
            let s = session(conn, layer_id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited. Start editing first.", info.name))?;
            crs::validate(conn, map_crs)?;
            let lc = info.crs.clone().unwrap_or_default();
            let t = ident(&log_table(layer_id));
            let pend = pending(conn, layer_id)?;
            let mut next_fid = s.next_fid;
            let mut fids = vec![];
            let mut invalid = vec![];
            let mut rows: Vec<(String, i64, Option<Vec<u8>>, Option<String>)> = vec![];
            let mut touched: std::collections::HashSet<i64> = Default::default();
            for op in ops {
                // Validate everything first; nothing is written unless the whole batch is good.
                let attrs_json = match &op.attributes {
                    Some(a) => {
                        for (k, v) in a {
                            cast_sql(&info, k, v)?;
                            if let Some(t) = json_text(v) {
                                // A value that doesn't fit its field is an error now, not at Save.
                                let f = info.fields.iter().find(|f| &f.name == k).unwrap();
                                conn.query_row(&format!("SELECT CAST(? AS {})", f.ty), [t.clone()], |_| Ok(()))
                                    .map_err(|_| anyhow!("'{t}' does not fit the field '{k}' ({}).", f.ty))?;
                            }
                        }
                        Some(serde_json::to_string(a)?)
                    }
                    None => None,
                };
                let map_wkb = op.geometry.as_deref().map(hex_to_bytes).transpose()?;
                let kind = op.op.as_str();
                let fid = match kind {
                    "insert" => {
                        let f = next_fid;
                        next_fid += 1;
                        fids.push(f);
                        f
                    }
                    "update" | "delete" => {
                        let f = op.fid.ok_or_else(|| anyhow!("An {kind} needs a feature id."))?;
                        let exists = pend.get(&f).is_some_and(|p| p.inserted) || conn.query_row(&format!("SELECT count(*) FROM {} WHERE __fid = {f}", ident(&info.table)), [], |r| r.get::<_, i64>(0))? > 0;
                        if !exists {
                            bail!("Feature {f} does not exist in \"{}\".", info.name);
                        }
                        if pend.get(&f).is_some_and(|p| p.deleted) {
                            bail!("Feature {f} was already deleted in this session.");
                        }
                        f
                    }
                    other => bail!("Unknown edit '{other}' (use insert, update or delete)."),
                };
                touched.insert(fid);
                let mut wkb: Option<Vec<u8>> = None;
                if map_wkb.is_some() || op.layer_wkb.is_some() {
                    let g = if let Some(lw) = &op.layer_wkb {
                        parse_wkb(lw)?
                    } else {
                    let mw = map_wkb.clone().unwrap();
                    let new_map = parse_wkb(&mw)?;
                    if crs::is_geographic(map_crs) && crosses_antimeridian(&new_map) {
                        bail!("{ANTIMERIDIAN}");
                    }
                    let new_layer_bytes = transform_wkb(conn, &mw, map_crs, &lc)?;
                    let new_layer = parse_wkb(&new_layer_bytes)?;
                    let (orig_map, orig_layer) = if kind == "update" {
                        let base: Option<Vec<u8>> = match pend.get(&fid).and_then(|p| p.wkb.clone()) {
                            Some(w) => Some(w),
                            None => conn
                                .query_row(&format!("SELECT ST_AsWKB(geom) FROM {} WHERE __fid = {fid}", ident(&info.table)), [], |r| r.get::<_, Option<Vec<u8>>>(0))
                                .ok()
                                .flatten(),
                        };
                        match base {
                            Some(b) => {
                                let om = parse_wkb(&transform_wkb(conn, &b, &lc, map_crs)?)?;
                                (Some(om), Some(parse_wkb(&b)?))
                            }
                            None => (None, None),
                        }
                    } else {
                        (None, None)
                    };
                    reconcile(&new_map, &new_layer, orig_map.as_ref(), orig_layer.as_ref(), s.z_value)?
                    };
                    if crs::is_geographic(&lc) && crosses_antimeridian(&g) {
                        bail!("{ANTIMERIDIAN}");
                    }
                    match info.geom_type.as_str() {
                        "Point" if g.dimension() != 0 => bail!("\"{}\" holds points; a line or polygon can't be added to it.", info.name),
                        "Line" if g.dimension() != 1 => bail!("\"{}\" holds lines; a point or polygon can't be added to it.", info.name),
                        "Polygon" if g.dimension() != 2 => bail!("\"{}\" holds polygons; a point or line can't be added to it.", info.name),
                        _ => {}
                    }
                    let mut bytes = write_wkb(&g);
                    // Mercator can't hold the poles; a feature there would fail Save, so it is refused here with a reason.
                    let ll = transform_wkb(conn, &bytes, &lc, crs::WGS84)?;
                    if parse_wkb(&ll)?.coords().any(|c| !c.y.is_finite() || c.y.abs() > crs::MERC_MAX_LAT) {
                        bail!("A vertex is beyond ±{:.2}° latitude, which the map can't hold. Keep shapes inside that range.", crs::MERC_MAX_LAT);
                    }
                    let valid: bool = conn.query_row("SELECT ST_IsValid(ST_GeomFromWKB(?))", [bytes.clone()], |r| r.get(0))?;
                    if !valid {
                        if op.fix {
                            bytes = conn.query_row("SELECT ST_AsWKB(ST_MakeValid(ST_GeomFromWKB(?)))", [bytes], |r| r.get::<_, Vec<u8>>(0))?;
                        } else {
                            invalid.push(fid);
                        }
                    }
                    wkb = Some(bytes);
                } else if kind == "insert" {
                    bail!("A new feature needs a shape.");
                }
                rows.push((kind.to_string(), fid, wkb, attrs_json));
            }
            // Everything was checked above. Now the writes, together: a failure leaves the log, the redo steps and the id counter as they were.
            conn.execute_batch("BEGIN TRANSACTION")?;
            let written = (|| -> Result<()> {
                // Steps undone and not redone are gone once a new edit is made.
                conn.execute(&format!("DELETE FROM {t} WHERE undone"), [])?;
                let step: i64 = conn.query_row(&format!("SELECT coalesce(max(step), 0) + 1 FROM {t}"), [], |r| r.get(0))?;
                let mut seq: i64 = conn.query_row(&format!("SELECT coalesce(max(seq), 0) FROM {t}"), [], |r| r.get(0))?;
                for (kind, fid, wkb, attrs) in rows {
                    seq += 1;
                    conn.execute(&format!("INSERT INTO {t} VALUES (?, ?, ?, ?, ?, ?, false)"), duckdb::params![seq, step, kind, fid, wkb, attrs])?;
                }
                conn.execute("UPDATE __edit_sessions SET next_fid = ?, map_crs = ? WHERE layer = ?", duckdb::params![next_fid, map_crs, layer_id])?;
                conn.execute("INSERT INTO __edit_fids VALUES (?, ?) ON CONFLICT (layer) DO UPDATE SET max_fid = excluded.max_fid", duckdb::params![layer_id, next_fid - 1])?;
                Ok(())
            })();
            match written {
                Ok(()) => conn.execute_batch("COMMIT")?,
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
            self.drop_tiles(layer_id);
            Ok(ApplyResult { status: self.edit_status(conn, &info)?, fids, invalid })
        })
    }

    /// Forget cached tiles of a layer (its pending edits changed what its tiles should show).
    fn drop_tiles(&self, layer_id: &str) {
        let mut c = self.tile_cache.lock();
        let keys: Vec<_> = c.iter().filter(|(k, _)| k.0 == layer_id).map(|(k, _)| k.clone()).collect();
        for k in keys {
            c.pop(&k);
        }
    }

    /// Undo the latest step of the session.
    pub fn edit_undo(&self, layer_id: &str) -> Result<EditStatus> {
        self.flip_step(layer_id, true)
    }

    /// Redo the step undone last.
    pub fn edit_redo(&self, layer_id: &str) -> Result<EditStatus> {
        self.flip_step(layer_id, false)
    }

    fn flip_step(&self, layer_id: &str, undo: bool) -> Result<EditStatus> {
        let info = self.layer(layer_id)?;
        self.with_conn(|conn| {
            session(conn, layer_id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited.", info.name))?;
            let t = ident(&log_table(layer_id));
            let step: Option<i64> = if undo {
                conn.query_row(&format!("SELECT max(step) FROM {t} WHERE NOT undone"), [], |r| r.get(0))?
            } else {
                conn.query_row(&format!("SELECT min(step) FROM {t} WHERE undone"), [], |r| r.get(0))?
            };
            let Some(step) = step else { bail!("{}", if undo { "Nothing to undo." } else { "Nothing to redo." }) };
            conn.execute(&format!("UPDATE {t} SET undone = ? WHERE step = ?"), duckdb::params![undo, step])?;
            self.drop_tiles(layer_id);
            self.edit_status(conn, &info)
        })
    }

    /// Drop the pending edits. The saved layer is untouched. New ids already handed out are not reused.
    pub fn edit_discard(&self, layer_id: &str) -> Result<LayerInfo> {
        let mut info = self.layer(layer_id)?;
        self.with_conn(|conn| {
            if let Some(s) = session(conn, layer_id)? {
                conn.execute("INSERT INTO __edit_fids VALUES (?, ?) ON CONFLICT (layer) DO UPDATE SET max_fid = greatest(max_fid, excluded.max_fid)", duckdb::params![layer_id, s.next_fid - 1])?;
            }
            conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&log_table(layer_id))))?;
            conn.execute("DELETE FROM __edit_sessions WHERE layer = ?", [layer_id])?;
            info.editing = false;
            self.save_meta(conn, &info)?;
            self.drop_tiles(layer_id);
            Ok(info)
        })
    }

    /// Apply the pending edits to the layer in one transaction: the table, its display columns and every overview level change
    /// together or not at all. The layer's version is bumped inside the transaction, so node trees, tiles and Python sets that read it
    /// run again afterwards.
    pub fn edit_save(&self, layer_id: &str) -> Result<SaveResult> {
        let mut info = self.layer(layer_id)?;
        // Nothing to save: close the session without touching the data. A new version would re-run every node tree that reads the layer.
        let effective = self.with_conn(|conn| {
            session(conn, layer_id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited, so there is nothing to save.", info.name))?;
            Ok(pending(conn, layer_id)?.len())
        })?;
        if effective == 0 {
            let layer = self.edit_discard(layer_id)?;
            return Ok(SaveResult { layer, added: 0, changed: 0, deleted: 0, invalid: 0, note: Some("There were no changes to save.".into()) });
        }
        let task = Task::start(format!("Saving edits to {}", info.name)).cancellable();
        let out = self.with_conn(|conn| {
            let s = session(conn, layer_id)?.ok_or_else(|| anyhow!("\"{}\" is not being edited, so there is nothing to save.", info.name))?;
            let pend = pending(conn, layer_id)?;
            let lc = info.crs.clone().unwrap_or_default();
            conn.execute_batch("BEGIN TRANSACTION")?;
            let res = (|| -> Result<(i64, i64, i64, i64, LayerInfo)> {
                let mut new = info.clone();
                let all = crate::attrs::tables(&info);
                for t in &all {
                    conn.execute_batch(&format!("DROP INDEX IF EXISTS {}", ident(&format!("{t}_rtree"))))?;
                }
                let main = ident(&info.table);
                // Before-images of the touched rows, for the most recent Save only (undone by `edit_undo_save`).
                let touched: Vec<String> = pend.iter().filter(|(_, p)| !p.inserted).map(|(f, _)| f.to_string()).collect();
                let new_ids: Vec<i64> = pend.iter().filter(|(_, p)| p.inserted).map(|(f, _)| *f).collect();
                conn.execute_batch(&format!(
                    "CREATE OR REPLACE TABLE {} AS SELECT * FROM {main} WHERE __fid IN ({})",
                    ident(&undo_table(layer_id)),
                    if touched.is_empty() { "NULL".to_string() } else { touched.join(", ") }
                ))?;
                for z in &info.overviews {
                    conn.execute_batch(&format!(
                        "CREATE OR REPLACE TABLE {} AS SELECT * FROM {} WHERE __fid IN ({})",
                        ident(&format!("{}__ov{z}", undo_table(layer_id))),
                        ident(&layers::overview_table(&info.table, *z)),
                        if touched.is_empty() { "NULL".to_string() } else { touched.join(", ") }
                    ))?;
                }
                conn.execute(
                    "INSERT OR REPLACE INTO __edit_undo_meta VALUES (?, ?, ?, ?)",
                    duckdb::params![layer_id, info.version as i64 + 1, serde_json::to_string(&new_ids)?, serde_json::to_string(&touched)?],
                )?;
                let (mut added, mut changed, mut deleted) = (0i64, 0i64, 0i64);
                let mut rewrite: Vec<i64> = vec![];
                let mut gone: Vec<i64> = vec![];
                let total = pend.len().max(1);
                for (i, (fid, st)) in pend.iter().enumerate() {
                    task.set(i as f64 / total as f64 * 0.8, Some(format!("Saving feature {} of {}", i + 1, total)));
                    task.check()?;
                    if st.deleted {
                        gone.push(*fid);
                        deleted += 1;
                        continue;
                    }
                    let cols: Vec<String> = st.attrs.keys().map(|k| ident(k)).collect();
                    let vals = st.attrs.iter().map(|(k, v)| cast_sql(&info, k, v)).collect::<Result<Vec<_>>>()?;
                    let geom_sql = st.wkb.as_ref().map(|w| format!("ST_GeomFromHEXWKB({})", lit(&bytes_to_hex(w).to_uppercase())));
                    if st.inserted {
                        let g = geom_sql.ok_or_else(|| anyhow!("Feature {fid} has no shape."))?;
                        conn.execute_batch(&format!(
                            "INSERT INTO {main} (__fid, {cols}{comma}geom, __g, __x0, __y0, __sz) SELECT {fid}, {vals}{comma}g, m, ST_XMin(m), ST_YMin(m), greatest(ST_XMax(m) - ST_XMin(m), ST_YMax(m) - ST_YMin(m)) FROM (SELECT g, ST_Transform(g, {c}, {wm}, true) AS m FROM (SELECT {g} AS g))",
                            cols = cols.join(", "),
                            vals = vals.join(", "),
                            comma = if cols.is_empty() { "" } else { ", " },
                            c = lit(&lc),
                            wm = lit(crs::WEB_MERCATOR)
                        ))?;
                        added += 1;
                    } else {
                        let mut set: Vec<String> = cols.iter().zip(&vals).map(|(c, v)| format!("{c} = {v}")).collect();
                        if let Some(g) = &geom_sql {
                            set.push("geom = n.g".into());
                            set.push("__g = n.m".into());
                            set.push("__x0 = ST_XMin(n.m)".into());
                            set.push("__y0 = ST_YMin(n.m)".into());
                            set.push("__sz = greatest(ST_XMax(n.m) - ST_XMin(n.m), ST_YMax(n.m) - ST_YMin(n.m))".into());
                            conn.execute_batch(&format!(
                                "UPDATE {main} SET {} FROM (SELECT g, ST_Transform(g, {c}, {wm}, true) AS m FROM (SELECT {g} AS g)) n WHERE {main}.__fid = {fid}",
                                set.join(", "),
                                c = lit(&lc),
                                wm = lit(crs::WEB_MERCATOR)
                            ))?;
                        } else if !set.is_empty() {
                            conn.execute_batch(&format!("UPDATE {main} SET {} WHERE __fid = {fid}", set.join(", ")))?;
                        }
                        changed += 1;
                    }
                    rewrite.push(*fid);
                }
                for chunk in gone.chunks(500) {
                    let ids = chunk.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
                    for t in &all {
                        conn.execute_batch(&format!("DELETE FROM {} WHERE __fid IN ({ids})", ident(t)))?;
                    }
                }
                // Overview levels are approximate: a changed or new feature goes in unsimplified, rebuilt at the next full refresh.
                for z in &info.overviews {
                    let ov = ident(&layers::overview_table(&info.table, *z));
                    for chunk in rewrite.chunks(500) {
                        let ids = chunk.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
                        conn.execute_batch(&format!("DELETE FROM {ov} WHERE __fid IN ({ids}); INSERT INTO {ov} BY NAME SELECT * FROM {main} WHERE __fid IN ({ids})"))?;
                    }
                }
                task.set(0.9, Some("Rebuilding indexes…"));
                for t in &all {
                    conn.execute_batch(&format!("CREATE INDEX {} ON {} USING RTREE (__g)", ident(&format!("{t}_rtree")), ident(t)))?;
                }
                layers::refresh_meta(conn, &mut new)?;
                new.native_extent = native_extent(conn, &info.table);
                let invalid: i64 = conn
                    .query_row(&format!("SELECT count(*) FROM {main} WHERE geom IS NOT NULL AND NOT ST_IsValid(geom) AND __fid IN (SELECT unnest([{}]::BIGINT[]))", if rewrite.is_empty() { "0".to_string() } else { rewrite.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",") }), [], |r| r.get(0))
                    .unwrap_or(0);
                new.version += 1;
                new.editing = false;
                conn.execute("INSERT OR REPLACE INTO __layers VALUES (?, ?)", duckdb::params![new.id, serde_json::to_string(&new)?])?;
                conn.execute("INSERT INTO __edit_fids VALUES (?, ?) ON CONFLICT (layer) DO UPDATE SET max_fid = greatest(max_fid, excluded.max_fid)", duckdb::params![layer_id, s.next_fid - 1])?;
                conn.execute("INSERT INTO __edit_log VALUES (?, now(), ?, ?, ?)", duckdb::params![layer_id, added, changed, deleted])?;
                conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&log_table(layer_id))))?;
                conn.execute("DELETE FROM __edit_sessions WHERE layer = ?", [layer_id])?;
                Ok((added, changed, deleted, invalid, new))
            })();
            match res {
                Ok(v) => {
                    conn.execute_batch("COMMIT")?;
                    Ok(v)
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(e)
                }
            }
        });
        let (added, changed, deleted, invalid, new) = out?;
        info = new;
        // After the commit: the in-memory layer, caches and the Python sets that depend on the data version.
        let mut note = None;
        self.with_conn(|conn| {
            if let Err(e) = self.save_meta(conn, &info) {
                note = Some(format!("Saved, but refreshing dependent results failed: {e}"));
            }
            let day: String = conn.query_row("SELECT strftime(current_date, '%Y-%m-%d')", [], |r| r.get(0))?;
            let c = sources::Citation {
                key: format!("user-edit:{}", info.id),
                author: user_name(),
                year: day[..4].to_string(),
                title: format!("Edits to {}", info.name),
                kind: "Dataset".into(),
                annotation: format!("Modified by the user on {day}: {added} feature(s) added, {changed} changed, {deleted} deleted. The layer keeps the sources it was made from."),
                complete: false,
                ..Default::default()
            };
            let _ = sources::record(conn, &c, &info.id, &info.name);
            Ok(())
        })?;
        if invalid > 0 {
            note = Some(format!("{invalid} saved shape(s) are not valid (they cross themselves or are degenerate). They were kept as drawn."));
        }
        Ok(SaveResult { layer: info, added, changed, deleted, invalid, note })
    }

    /// Undo the most recent Save, as a new version: features it added are removed, features it changed or deleted come back as they
    /// were with their ids, and the overview levels follow. Refused, with the reason, when the layer has changed since or is being edited.
    pub fn edit_undo_save(&self, layer_id: &str) -> Result<LayerInfo> {
        let info = self.layer(layer_id)?;
        if info.editing {
            bail!("\"{}\" is being edited. Save or discard those edits before undoing the last Save.", info.name);
        }
        let new = self.with_conn(|conn| {
            let meta: Option<(i64, String, String)> = conn
                .query_row("SELECT version, inserted, restored FROM __edit_undo_meta WHERE layer = ?", [layer_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .ok();
            let Some((version, inserted, restored)) = meta else { bail!("There is no saved edit to undo on \"{}\".", info.name) };
            if version != info.version as i64 {
                bail!("\"{}\" has changed since that Save, so the Save can't be undone any more. Make the changes again by hand or undo them first.", info.name);
            }
            let inserted: Vec<i64> = serde_json::from_str(&inserted)?;
            let restored: Vec<i64> = serde_json::from_str::<Vec<String>>(&restored)?.iter().filter_map(|x| x.parse().ok()).collect();
            let all = crate::attrs::tables(&info);
            let main = ident(&info.table);
            conn.execute_batch("BEGIN TRANSACTION")?;
            let res = (|| -> Result<LayerInfo> {
                let mut new = info.clone();
                for t in &all {
                    conn.execute_batch(&format!("DROP INDEX IF EXISTS {}", ident(&format!("{t}_rtree"))))?;
                }
                let ids: Vec<String> = inserted.iter().chain(restored.iter()).map(|f| f.to_string()).collect();
                for chunk in ids.chunks(500) {
                    for t in &all {
                        conn.execute_batch(&format!("DELETE FROM {} WHERE __fid IN ({})", ident(t), chunk.join(", ")))?;
                    }
                }
                conn.execute_batch(&format!("INSERT INTO {main} BY NAME SELECT * FROM {}", ident(&undo_table(layer_id))))?;
                // Overview levels get back exactly the rows they had (their thinned or simplified form), from the before-images.
                for z in &info.overviews {
                    let ov = ident(&layers::overview_table(&info.table, *z));
                    let img = ident(&format!("{}__ov{z}", undo_table(layer_id)));
                    conn.execute_batch(&format!("INSERT INTO {ov} BY NAME SELECT * FROM {img}"))?;
                    conn.execute_batch(&format!("DROP TABLE IF EXISTS {img}"))?;
                }
                for t in &all {
                    conn.execute_batch(&format!("CREATE INDEX {} ON {} USING RTREE (__g)", ident(&format!("{t}_rtree")), ident(t)))?;
                }
                layers::refresh_meta(conn, &mut new)?;
                new.native_extent = native_extent(conn, &info.table);
                new.version += 1;
                conn.execute("INSERT OR REPLACE INTO __layers VALUES (?, ?)", duckdb::params![new.id, serde_json::to_string(&new)?])?;
                conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&undo_table(layer_id))))?;
                conn.execute("DELETE FROM __edit_undo_meta WHERE layer = ?", [layer_id])?;
                Ok(new)
            })();
            match res {
                Ok(n) => {
                    conn.execute_batch("COMMIT")?;
                    Ok(n)
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(e)
                }
            }
        })?;
        self.with_conn(|conn| self.save_meta(conn, &new))?;
        self.drop_tiles(layer_id);
        Ok(new)
    }

    /// What Save has recorded for a layer, newest first.
    pub fn edit_history(&self, layer_id: &str) -> Result<Vec<EditLogRow>> {
        self.with_conn(|conn| {
            let mut s = conn.prepare("SELECT strftime(saved_at, '%Y-%m-%d %H:%M:%S'), added, changed, deleted FROM __edit_log WHERE layer = ? ORDER BY saved_at DESC")?;
            let rows = s.query_map([layer_id], |r| Ok(EditLogRow { at: r.get(0)?, added: r.get(1)?, changed: r.get(2)?, deleted: r.get(3)? }))?;
            Ok(rows.collect::<Result<_, _>>()?)
        })
    }

    /// A new, empty vector layer with typed fields. `geom_type` is point, multipoint, line or polygon; the CRS is never left blank.
    /// An `id` field is added unless the fields already have one. The layer is not put into an edit session; call `edit_begin`.
    pub fn create_layer(&self, name: &str, geom_type: &str, crs_code: &str, fields: &[(String, String)]) -> Result<LayerInfo> {
        let name = name.trim();
        if name.is_empty() {
            bail!("Give the new layer a name.");
        }
        let gt = match geom_type.to_lowercase().as_str() {
            "point" | "multipoint" => "Point",
            "line" | "linestring" | "multiline" => "Line",
            "polygon" | "multipolygon" => "Polygon",
            other => bail!("Unknown geometry type '{other}' (use point, multipoint, line or polygon)."),
        };
        if crs_code.trim().is_empty() {
            bail!("Choose a coordinate system for the new layer; it is never left blank.");
        }
        let mut cols: Vec<(String, String)> = vec![];
        for (n, t) in fields {
            let n = n.trim();
            if n.is_empty() || n.eq_ignore_ascii_case("geom") || layers::HIDDEN_COLS.contains(&n) {
                bail!("'{n}' can't be used as a field name.");
            }
            if cols.iter().any(|(c, _)| c.eq_ignore_ascii_case(n)) {
                bail!("The field '{n}' is listed twice.");
            }
            cols.push((n.to_string(), crate::attrs::sql_type(t)?.to_string()));
        }
        if !cols.iter().any(|(c, _)| c.eq_ignore_ascii_case("id")) {
            cols.insert(0, ("id".into(), "BIGINT".into()));
        }
        let id = self.alloc_id();
        let mut info = self.with_conn(|conn| {
            crs::validate(conn, crs_code)?;
            let table = format!("layer_{id}");
            let defs = cols.iter().map(|(n, t)| format!("{} {t}", ident(n))).collect::<Vec<_>>().join(", ");
            let scratch = format!("__new_{id}");
            conn.execute_batch(&format!("CREATE OR REPLACE TABLE {} ({defs}, geom GEOMETRY)", ident(&scratch)))?;
            let _ = table;
            let src = layers::ImportSource { name: name.to_string(), select: format!("SELECT * FROM {}", ident(&scratch)), crs: Some(crs_code.to_string()), crs_name: None, warnings: vec![] };
            let built = layers::build_layer(conn, &id, &src);
            conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(&scratch)))?;
            let mut info = built?;
            info.geom_type = gt.into();
            info.source = Some("Created by the user".into());
            self.save_meta(conn, &info)?;
            let year: String = conn.query_row("SELECT strftime(current_date, '%Y')", [], |r| r.get(0))?;
            let c = sources::Citation {
                key: format!("user:{}", info.id),
                author: user_name(),
                year,
                title: name.to_string(),
                kind: "Dataset".into(),
                annotation: format!("A {} layer made in GIS Studio by the user, in {}. Edit this citation to say who made it and how.", gt.to_lowercase(), crs_code),
                complete: false,
                ..Default::default()
            };
            sources::record(conn, &c, &info.id, name)?;
            Ok(info)
        })?;
        info = self.layer(&info.id)?;
        Ok(info)
    }
}

/// The author of what the user makes. The operating system's login name is never read: citations travel in shared projects.
fn user_name() -> String {
    "the user".into()
}

fn native_extent(conn: &Connection, table: &str) -> Option<[f64; 4]> {
    conn.query_row(
        &format!("SELECT min(ST_XMin(geom)), min(ST_YMin(geom)), max(ST_XMax(geom)), max(ST_YMax(geom)) FROM {}", ident(table)),
        [],
        |r| Ok(r.get::<_, Option<f64>>(0)?.map(|a| [a, r.get(1).unwrap_or(0.0), r.get(2).unwrap_or(0.0), r.get(3).unwrap_or(0.0)])),
    )
    .unwrap_or(None)
}
