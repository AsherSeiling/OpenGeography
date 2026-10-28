//! Feature labels in print layouts (docs/LAYOUTS_PLAN.md L2): the same `LayerStyle.label` the map view uses,
//! drawn in page millimetres with the bundled Noto Sans, a halo as a stroked underlay, and greedy collision
//! avoidance on text boxes measured from the font. A label that does not fit is skipped and counted.

use super::frame::PX_MM;
use super::export;
use super::model::{FeatureLabel, LabelOverrides, LabelPatch, LabelStyle};
use serde::Serialize;
use super::surround::text_width;
use super::svg::{esc, mm};
use crate::db::{ident, lit};
use crate::layers::LayerInfo;
use anyhow::Result;
use duckdb::Connection;
use std::collections::HashMap;
use std::fmt::Write;

const PT_MM: f64 = 25.4 / 72.0;
/// Free space kept around each label's text, mm.
const PAD_MM: f64 = 0.3;
/// A point's label starts this far right of the symbol edge, mm.
const POINT_GAP_MM: f64 = 0.8;
/// Most features of one layer considered for labels; the rest are not labelled (and the frame says so).
pub const MAX_CANDIDATES: usize = 50_000;
const CELL_MM: f64 = 10.0;
/// Text lines are this many times the font size tall.
const LINE_HEIGHT: f64 = 1.2;

/// True when labels draw at map scale `1:scale`: hidden when zoomed out past `maxScale` or in past `minScale`.
pub fn visible_at(l: &LabelStyle, scale: f64) -> bool {
    !(l.max_scale.is_some_and(|m| scale > m as f64) || l.min_scale.is_some_and(|m| scale < m as f64))
}

/// Boxes of the labels placed so far in one frame, in page mm (`[x0, y0, x1, y1]`), with a grid for fast overlap tests.
#[derive(Default)]
pub struct Placer {
    boxes: Vec<[f64; 4]>,
    grid: HashMap<(i32, i32), Vec<usize>>,
}

fn cells(b: &[f64; 4]) -> impl Iterator<Item = (i32, i32)> {
    let (x0, x1) = ((b[0] / CELL_MM).floor() as i32, (b[2] / CELL_MM).floor() as i32);
    let (y0, y1) = ((b[1] / CELL_MM).floor() as i32, (b[3] / CELL_MM).floor() as i32);
    (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
}

impl Placer {
    fn fits(&self, b: &[f64; 4]) -> bool {
        cells(b).all(|c| {
            self.grid.get(&c).is_none_or(|ids| {
                ids.iter().all(|&i| {
                    let o = &self.boxes[i];
                    b[2] <= o[0] || o[2] <= b[0] || b[3] <= o[1] || o[3] <= b[1]
                })
            })
        })
    }
    fn add(&mut self, b: [f64; 4]) {
        let i = self.boxes.len();
        self.boxes.push(b);
        for c in cells(&b) {
            self.grid.entry(c).or_default().push(i);
        }
    }
}

/// A label as placed on the page; the UI draws a hit box over each one (L3).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlacedLabel {
    /// Map frame id (filled in by the frame).
    pub frame: String,
    pub layer: String,
    pub fid: i64,
    pub text: String,
    /// `[x0, y0, x1, y1]` in page mm, before rotation for points and polygons.
    #[serde(rename = "boxMm")]
    pub box_mm: [f64; 4],
    /// Degrees on the page, clockwise.
    pub rotation: f64,
    /// Edited (moved, rotated, resized or re-worded): placed first, never dropped.
    pub pinned: bool,
    /// Hidden by the user: not drawn; the box is where the automatic label would be.
    pub hidden: bool,
}

/// An edit that points at a feature that no longer exists; the renderer ignores it and the UI drops it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DroppedEdit {
    pub frame: String,
    pub layer: String,
    pub fid: i64,
}

/// What a layer's labels look like in this frame: the map's setting with this layout's edits on top.
/// `None` when labels are off or have no field.
pub fn effective(base: Option<&LabelStyle>, patch: Option<&LabelPatch>, fields: &[String]) -> Option<LabelStyle> {
    let mut l = base.cloned().unwrap_or(LabelStyle { enabled: false, field: String::new(), size: 9.0, color: "#222222".into(), halo_color: "#ffffff".into(), halo_width: 1.5, min_scale: None, max_scale: None });
    if let Some(p) = patch {
        if let Some(v) = p.enabled {
            l.enabled = v;
        }
        if let Some(v) = &p.field {
            l.field = v.clone();
        }
        if let Some(v) = p.size {
            l.size = v;
        }
        if let Some(v) = &p.color {
            l.color = v.clone();
        }
        if let Some(v) = &p.halo_color {
            l.halo_color = v.clone();
        }
        if let Some(v) = p.halo_width {
            l.halo_width = v;
        }
    }
    (l.enabled && fields.iter().any(|f| f == &l.field)).then_some(l)
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Point,
    Line,
    Area,
}

/// One label before placement.
pub struct Candidate {
    layer: String,
    layer_name: String,
    fid: i64,
    text: String,
    px: f64,
    py: f64,
    deg: f64,
    kind: Kind,
    label: LabelStyle,
    edit: Option<FeatureLabel>,
    point_radius_mm: f64,
}

/// A layer's candidates in the frame, and what the edits point at.
#[derive(Default)]
pub struct LayerCandidates {
    pub list: Vec<Candidate>,
    pub truncated: bool,
    /// Edits whose feature is gone from the layer.
    pub dropped: Vec<i64>,
}

/// Anchor query for one layer: rows of (fid, text, x, y, dx, dy) in the frame CRS. `dx, dy` is the direction of the
/// line at the anchor (lines only). Polygons and points use ST_PointOnSurface of the part inside the frame.
fn anchor_sql(info: &LayerInfo, label: &LabelStyle, frame_crs: &str, bbox3857: Option<[f64; 4]>, extent: [f64; 4]) -> String {
    let (src, dst) = (crate::crs::resolve(info.crs.as_deref().unwrap_or("")), crate::crs::resolve(frame_crs));
    let g = if src == dst { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&src), lit(&dst)) };
    let filter = match bbox3857 {
        Some(b) => format!("ST_Intersects(__g, ST_MakeEnvelope({}, {}, {}, {}))", b[0], b[1], b[2], b[3]),
        None => "__g IS NOT NULL".into(),
    };
    let env = format!("ST_MakeEnvelope({}, {}, {}, {})", extent[0], extent[1], extent[2], extent[3]);
    let field = ident(&label.field);
    let clipped = format!(
        "SELECT __fid, t, c FROM (
           SELECT __fid, t, CASE WHEN ST_XMin(g) >= {} AND ST_XMax(g) <= {} AND ST_YMin(g) >= {} AND ST_YMax(g) <= {} THEN g ELSE ST_Intersection(g, {env}) END AS c
           FROM (SELECT __fid, CAST({field} AS VARCHAR) AS t, {g} AS g FROM {t} WHERE {field} IS NOT NULL AND {filter}{def})
         ) WHERE c IS NOT NULL AND NOT ST_IsEmpty(c) ORDER BY __fid LIMIT {limit}",
        extent[0],
        extent[2],
        extent[1],
        extent[3],
        t = ident(&info.table),
        def = info.def_and(),
        limit = MAX_CANDIDATES + 1,
    );
    if info.geom_type == "Line" {
        format!(
            "SELECT __fid, t, ST_X(p), ST_Y(p), ST_X(b) - ST_X(a), ST_Y(b) - ST_Y(a) FROM (
               SELECT __fid, t, ST_LineInterpolatePoint(l, 0.5) AS p, ST_LineInterpolatePoint(l, 0.45) AS a, ST_LineInterpolatePoint(l, 0.55) AS b,
                      row_number() OVER (PARTITION BY __fid ORDER BY ST_Length(l) DESC) AS rn
               FROM (SELECT __fid, t, (UNNEST(ST_Dump(c))).geom AS l FROM ({clipped}))
               WHERE ST_GeometryType(l) = 'LINESTRING' AND ST_Length(l) > 0
             ) WHERE rn = 1 ORDER BY __fid"
        )
    } else {
        format!("SELECT __fid, t, ST_X(p), ST_Y(p), 0.0, 0.0 FROM (SELECT __fid, t, ST_PointOnSurface(c) AS p FROM ({clipped})) WHERE p IS NOT NULL ORDER BY __fid")
    }
}

/// Axis-aligned box of a `w` × `h` mm rectangle centred on (cx, cy), turned by `deg`.
fn rotated_box(cx: f64, cy: f64, w: f64, h: f64, deg: f64) -> [f64; 4] {
    let (s, c) = deg.to_radians().sin_cos();
    let (hw, hh) = ((w * c.abs() + h * s.abs()) / 2.0, (w * s.abs() + h * c.abs()) / 2.0);
    [cx - hw, cy - hh, cx + hw, cy + hh]
}

/// Read one layer's label candidates: features with a label in the frame, with this layout's edits attached.
#[allow(clippy::too_many_arguments)]
pub fn candidates(
    conn: &Connection,
    info: &LayerInfo,
    layer_id: &str,
    label: &LabelStyle,
    overrides: Option<&LabelOverrides>,
    frame_crs: &str,
    bbox3857: Option<[f64; 4]>,
    extent: [f64; 4],
    to_page: &dyn Fn(f64, f64) -> (f64, f64),
    point_radius_mm: f64,
    check: &dyn Fn() -> Result<()>,
) -> Result<LayerCandidates> {
    let sql = anchor_sql(info, label, frame_crs, bbox3857, extent);
    let kind = match info.geom_type.as_str() {
        "Point" => Kind::Point,
        "Line" => Kind::Line,
        _ => Kind::Area,
    };
    let mut out = LayerCandidates::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        if out.list.len() >= MAX_CANDIDATES {
            out.truncated = true;
            break;
        }
        if out.list.len() % 512 == 511 {
            check()?;
        }
        let (fid, text, x, y, dx, dy): (i64, String, f64, f64, f64, f64) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?);
        seen.insert(fid);
        let text = text.trim().to_string();
        if text.is_empty() || !x.is_finite() || !y.is_finite() {
            continue;
        }
        let (px, py) = to_page(x, y);
        // Page y runs down, so a line's angle on the page comes from its two page points.
        let (a, b) = (to_page(x - dx, y - dy), to_page(x + dx, y + dy));
        let mut deg = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        if deg > 90.0 {
            deg -= 180.0;
        } else if deg < -90.0 {
            deg += 180.0;
        }
        let edit = overrides.and_then(|o| o.features.get(&fid.to_string())).cloned();
        out.list.push(Candidate { layer: layer_id.into(), layer_name: info.name.clone(), fid, text, px, py, deg: if kind == Kind::Line { deg } else { 0.0 }, kind, label: label.clone(), edit, point_radius_mm });
    }
    // Edits for features not in the frame: fine if the feature exists elsewhere, dropped if it is gone.
    if let Some(o) = overrides {
        let missing: Vec<i64> = o.features.keys().filter_map(|k| k.parse::<i64>().ok()).filter(|f| !seen.contains(f)).collect();
        if !missing.is_empty() {
            let list = missing.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
            let mut st = conn.prepare(&format!("SELECT __fid FROM {} WHERE __fid IN ({list})", ident(&info.table)))?;
            let found: std::collections::BTreeSet<i64> = st.query_map([], |r| r.get::<_, i64>(0))?.filter_map(|r| r.ok()).collect();
            out.dropped = missing.into_iter().filter(|f| !found.contains(f)).collect();
        }
    }
    Ok(out)
}

#[derive(Default)]
pub struct Placed {
    pub svg: String,
    pub labels: Vec<PlacedLabel>,
    /// Automatic labels that did not fit, by layer name (in first-seen order).
    pub skipped: Vec<(String, usize)>,
}

/// Place everything: edited (pinned) labels first, wherever the user put them, then the automatic ones in `cands` order
/// (top layer first) where they fit inside `frame` without touching another. Hidden labels are listed but not drawn.
pub fn place(cands: &[Candidate], frame: [f64; 4], placer: &mut Placer) -> Placed {
    let mut out = Placed::default();
    let mut drawn: Vec<(usize, String)> = Vec::new(); // (candidate index, svg), re-ordered back to candidate order
    let mut svgs: HashMap<usize, String> = HashMap::new();
    let geom = |c: &Candidate| {
        let e = c.edit.as_ref();
        let size_pt = e.and_then(|e| e.size_pt).filter(|s| *s > 0.0).unwrap_or(c.label.size).max(1.0);
        let text = e.and_then(|e| e.text.clone()).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| c.text.clone());
        let size_mm = size_pt * PT_MM;
        let h = size_mm * LINE_HEIGHT + 2.0 * PAD_MM;
        let w = text_width(&text, "sans", size_pt) + 2.0 * PAD_MM;
        let deg = e.and_then(|e| e.rotation).unwrap_or(c.deg);
        let (px, py) = (c.px + e.and_then(|e| e.dx_mm).unwrap_or(0.0), c.py + e.and_then(|e| e.dy_mm).unwrap_or(0.0));
        let (bx, anchor, tx) = match c.kind {
            Kind::Point => {
                let x0 = px + c.point_radius_mm + POINT_GAP_MM;
                ([x0 - PAD_MM, py - h / 2.0, x0 + w - PAD_MM, py + h / 2.0], "start", x0)
            }
            _ if deg != 0.0 => (rotated_box(px, py, w, h, deg), "middle", px),
            _ => ([px - w / 2.0, py - h / 2.0, px + w / 2.0, py + h / 2.0], "middle", px),
        };
        (text, size_mm, deg, px, py, bx, anchor, tx)
    };
    let draw = |c: &Candidate, g: &(String, f64, f64, f64, f64, [f64; 4], &str, f64)| -> String {
        let (text, size_mm, deg, px, py, _, anchor, tx) = g;
        let halo = c.label.halo_width.max(0.0) * PX_MM;
        let (fill, halo_color) = (super::svg::color(&c.label.color, "#222222"), super::svg::color(&c.label.halo_color, "#ffffff"));
        let rot = if *deg != 0.0 { format!(" transform=\"rotate({} {} {})\"", mm(*deg), mm(*px), mm(*py)) } else { String::new() };
        let attrs = format!("font-family=\"{}\" font-size=\"{}\" text-anchor=\"{anchor}\" x=\"{}\" y=\"{}\"{rot}", export::font_family(), mm(*size_mm), mm(*tx), mm(py + size_mm * 0.35));
        let mut s = String::new();
        if halo > 0.0 {
            let _ = write!(s, "<text {attrs} fill=\"{halo_color}\" stroke=\"{halo_color}\" stroke-width=\"{}\" stroke-linejoin=\"round\">{}</text>", mm(halo * 2.0), esc(text));
        }
        let _ = write!(s, "<text {attrs} fill=\"{fill}\">{}</text>", esc(text));
        s
    };
    let mut skipped: Vec<(String, usize)> = Vec::new();
    // Pass 1: hidden (listed) and pinned (placed, never dropped).
    let mut labels: HashMap<usize, PlacedLabel> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        let Some(e) = &c.edit else { continue };
        let g = geom(c);
        if e.hidden == Some(true) {
            labels.insert(i, PlacedLabel { frame: String::new(), layer: c.layer.clone(), fid: c.fid, text: g.0.clone(), box_mm: g.5, rotation: g.2, pinned: e.pinned(), hidden: true });
            continue;
        }
        if e.pinned() {
            placer.add(g.5);
            svgs.insert(i, draw(c, &g));
            labels.insert(i, PlacedLabel { frame: String::new(), layer: c.layer.clone(), fid: c.fid, text: g.0.clone(), box_mm: g.5, rotation: g.2, pinned: true, hidden: false });
        }
    }
    // Pass 2: automatic labels around the pinned ones.
    for (i, c) in cands.iter().enumerate() {
        if labels.contains_key(&i) {
            continue;
        }
        let g = geom(c);
        let inside = g.5[0] >= frame[0] && g.5[1] >= frame[1] && g.5[2] <= frame[2] && g.5[3] <= frame[3];
        if !inside || !placer.fits(&g.5) {
            match skipped.iter_mut().find(|(n, _)| n == &c.layer_name) {
                Some((_, n)) => *n += 1,
                None => skipped.push((c.layer_name.clone(), 1)),
            }
            continue;
        }
        placer.add(g.5);
        svgs.insert(i, draw(c, &g));
        labels.insert(i, PlacedLabel { frame: String::new(), layer: c.layer.clone(), fid: c.fid, text: g.0.clone(), box_mm: g.5, rotation: g.2, pinned: false, hidden: false });
    }
    for i in 0..cands.len() {
        if let Some(s) = svgs.remove(&i) {
            drawn.push((i, s));
        }
        if let Some(l) = labels.remove(&i) {
            out.labels.push(l);
        }
    }
    for (_, s) in drawn {
        out.svg.push_str(&s);
    }
    out.skipped = skipped;
    out
}
