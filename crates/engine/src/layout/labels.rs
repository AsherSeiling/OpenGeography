//! Feature labels in print layouts (docs/LAYOUTS_PLAN.md L2): the same `LayerStyle.label` the map view uses,
//! drawn in page millimetres with the bundled Noto Sans, a halo as a stroked underlay, and greedy collision
//! avoidance on text boxes measured from the font. A label that does not fit is skipped and counted.

use super::frame::PX_MM;
use super::export;
use super::model::LabelStyle;
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
    pub fn boxes(&self) -> &[[f64; 4]] {
        &self.boxes
    }
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

#[derive(Default)]
pub struct LayerLabels {
    pub svg: String,
    pub placed: usize,
    /// Labels that did not fit (overlapped one already placed, or fell outside the frame).
    pub skipped: usize,
    /// More features than `MAX_CANDIDATES`: the later ones were not considered.
    pub truncated: bool,
}

/// Anchor query for one layer: rows of (text, x, y, dx, dy) in the frame CRS. `dx, dy` is the direction of the
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
            "SELECT t, ST_X(p), ST_Y(p), ST_X(b) - ST_X(a), ST_Y(b) - ST_Y(a) FROM (
               SELECT t, ST_LineInterpolatePoint(l, 0.5) AS p, ST_LineInterpolatePoint(l, 0.45) AS a, ST_LineInterpolatePoint(l, 0.55) AS b,
                      row_number() OVER (PARTITION BY __fid ORDER BY ST_Length(l) DESC) AS rn
               FROM (SELECT __fid, t, (UNNEST(ST_Dump(c))).geom AS l FROM ({clipped}))
               WHERE ST_GeometryType(l) = 'LINESTRING' AND ST_Length(l) > 0
             ) WHERE rn = 1 ORDER BY t"
        )
    } else {
        format!("SELECT t, ST_X(p), ST_Y(p), 0.0, 0.0 FROM (SELECT t, ST_PointOnSurface(c) AS p FROM ({clipped})) WHERE p IS NOT NULL")
    }
}

/// Axis-aligned box of a `w` × `h` mm rectangle centred on (cx, cy), turned by `deg`.
fn rotated_box(cx: f64, cy: f64, w: f64, h: f64, deg: f64) -> [f64; 4] {
    let (s, c) = deg.to_radians().sin_cos();
    let (hw, hh) = ((w * c.abs() + h * s.abs()) / 2.0, (w * s.abs() + h * c.abs()) / 2.0);
    [cx - hw, cy - hh, cx + hw, cy + hh]
}

/// Place and draw one layer's labels. `frame` is the frame's page box `[x0, y0, x1, y1]`; labels whose box leaves it are skipped.
#[allow(clippy::too_many_arguments)]
pub fn layer(
    conn: &Connection,
    info: &LayerInfo,
    label: &LabelStyle,
    frame_crs: &str,
    bbox3857: Option<[f64; 4]>,
    extent: [f64; 4],
    frame: [f64; 4],
    to_page: &dyn Fn(f64, f64) -> (f64, f64),
    placer: &mut Placer,
    point_radius_mm: f64,
    check: &dyn Fn() -> Result<()>,
) -> Result<LayerLabels> {
    let sql = anchor_sql(info, label, frame_crs, bbox3857, extent);
    let size_mm = label.size.max(1.0) * PT_MM;
    let h = size_mm * LINE_HEIGHT + 2.0 * PAD_MM;
    let halo = label.halo_width.max(0.0) * PX_MM;
    let is_point = info.geom_type == "Point";
    let is_line = info.geom_type == "Line";
    let (fill, halo_color) = (super::svg::color(&label.color, "#222222"), super::svg::color(&label.halo_color, "#ffffff"));
    let family = export::font_family();
    let mut out = LayerLabels::default();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut n = 0usize;
    while let Some(r) = rows.next()? {
        n += 1;
        if n > MAX_CANDIDATES {
            out.truncated = true;
            break;
        }
        if n % 512 == 0 {
            check()?;
        }
        let (text, x, y, dx, dy): (String, f64, f64, f64, f64) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?);
        let text = text.trim();
        if text.is_empty() || !x.is_finite() || !y.is_finite() {
            continue;
        }
        let w = text_width(text, "sans", label.size.max(1.0)) + 2.0 * PAD_MM;
        let (px, py) = to_page(x, y);
        // Page y runs down, so a line's angle on the page comes from its two page points.
        let (a, b) = (to_page(x - dx, y - dy), to_page(x + dx, y + dy));
        let mut deg = (b.1 - a.1).atan2(b.0 - a.0).to_degrees();
        if deg > 90.0 {
            deg -= 180.0;
        } else if deg < -90.0 {
            deg += 180.0;
        }
        let (bx, anchor, tx) = if is_point {
            let x0 = px + point_radius_mm + POINT_GAP_MM;
            ([x0 - PAD_MM, py - h / 2.0, x0 + w - PAD_MM, py + h / 2.0], "start", x0)
        } else if is_line {
            (rotated_box(px, py, w, h, deg), "middle", px)
        } else {
            ([px - w / 2.0, py - h / 2.0, px + w / 2.0, py + h / 2.0], "middle", px)
        };
        let inside = bx[0] >= frame[0] && bx[1] >= frame[1] && bx[2] <= frame[2] && bx[3] <= frame[3];
        if !inside || !placer.fits(&bx) {
            out.skipped += 1;
            continue;
        }
        placer.add(bx);
        out.placed += 1;
        let ty = py + size_mm * 0.35;
        let rot = if is_line { format!(" transform=\"rotate({} {} {})\"", mm(deg), mm(px), mm(py)) } else { String::new() };
        let attrs = format!("font-family=\"{family}\" font-size=\"{}\" text-anchor=\"{anchor}\" x=\"{}\" y=\"{}\"{rot}", mm(size_mm), mm(tx), mm(ty));
        if halo > 0.0 {
            let _ = write!(out.svg, "<text {attrs} fill=\"{halo_color}\" stroke=\"{halo_color}\" stroke-width=\"{}\" stroke-linejoin=\"round\">{}</text>", mm(halo * 2.0), esc(text));
        }
        let _ = write!(out.svg, "<text {attrs} fill=\"{fill}\">{}</text>", esc(text));
    }
    Ok(out)
}
