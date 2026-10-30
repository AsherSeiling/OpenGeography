//! Surround elements (docs/LAYOUTS_PLAN.md L3, L6): legend, scale bar, north
//! arrow and sources. Everything geodetic comes from the frame's [`Factors`]
//! (computed from the layout's [`Body`](super::geo::Body)); nothing here names
//! an ellipsoid. Text is measured with the bundled Noto Sans faces so wrapping
//! and ellipsizing give the same result on every platform.

use super::export;
use super::frame::{class_color, PX_MM};
use super::geo::{self, Factors};
use super::model::{LayerStyle, Placement, RenderRequest, StyleKind};
use super::slots::{BAR_LABEL_PT, LEGEND_PT, SOURCES_PT, TITLE_PT};
use super::svg::{self, esc, mm, opacity};
use crate::layers::LayerInfo;
use std::fmt::Write;
use std::sync::OnceLock;

/// What a surround element needs to know about the map frame it belongs to.
pub struct FrameCtx {
    pub id: String,
    pub scale: f64,
    /// Frame rotation on the page, degrees clockwise.
    pub rotation: f64,
    pub factors: Factors,
    /// Frame centre in longitude/latitude, and whether the frame is projected.
    pub center_lonlat: Option<(f64, f64)>,
    pub projected: bool,
    /// Layers drawn in the frame, top first.
    pub drawn: Vec<String>,
    /// Citations of what else the frame drew (its base map).
    pub extra_cites: Vec<String>,
}

pub(super) const PT_MM: f64 = 25.4 / 72.0;

// ---------------------------------------------------------------- text

fn faces() -> &'static [ttf_parser::Face<'static>; 3] {
    static F: OnceLock<[ttf_parser::Face<'static>; 3]> = OnceLock::new();
    F.get_or_init(|| {
        let face = |n: &str| ttf_parser::Face::parse(export::font_bytes(n), 0).expect("bundled font parses");
        [face("sans"), face("sans-bold"), face("sans-italic")]
    })
}

/// Width in mm of `text` set in `font` at `size_pt` (advance widths; no kerning).
pub fn text_width(text: &str, font: &str, size_pt: f64) -> f64 {
    let f = &faces()[match font {
        "sans-bold" => 1,
        "sans-italic" => 2,
        _ => 0,
    }];
    let upem = f.units_per_em() as f64;
    let units: f64 = text
        .chars()
        .map(|c| f.glyph_index(c).and_then(|g| f.glyph_hor_advance(g)).unwrap_or(f.units_per_em() / 2) as f64)
        .sum();
    units / upem * size_pt * PT_MM
}

/// `text`, cut and ended with "…" when it is wider than `max_mm`.
pub fn ellipsize(text: &str, font: &str, size_pt: f64, max_mm: f64) -> String {
    if text_width(text, font, size_pt) <= max_mm {
        return text.to_string();
    }
    let mut s: Vec<char> = text.chars().collect();
    while !s.is_empty() {
        s.pop();
        let t: String = s.iter().collect::<String>().trim_end().to_string() + "…";
        if text_width(&t, font, size_pt) <= max_mm {
            return t;
        }
    }
    String::new()
}

/// Greedy word wrap to `max_mm`; a word wider than the line is ellipsized.
pub fn wrap(text: &str, font: &str, size_pt: f64, max_mm: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if cur.is_empty() || text_width(&trial, font, size_pt) <= max_mm {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.into_iter().map(|l| ellipsize(&l, font, size_pt, max_mm)).collect()
}

fn font_attrs(font: &str) -> (&'static str, &'static str) {
    match font {
        "sans-bold" => ("700", "normal"),
        "sans-italic" => ("400", "italic"),
        _ => ("400", "normal"),
    }
}

/// One line of text in a slot of the element (`legend.title`, see `slots.rs`): the slot's style (font, bold, italic, underline, colour,
/// letter spacing, halo) is applied on top of `font`; `size_pt` is already the slot's size (`Placement::slot_pt`).
pub(super) fn text_slot(out: &mut String, x: f64, y: f64, text: &str, font: &str, size_pt: f64, anchor: &str, at: &Placement, slot: &str) {
    let st = at.slot_style(slot);
    let (mut weight, mut fstyle) = font_attrs(font);
    if let Some(b) = st.and_then(|t| t.bold) {
        weight = if b { "700" } else { "400" };
    }
    if let Some(i) = st.and_then(|t| t.italic) {
        fstyle = if i { "italic" } else { "normal" };
    }
    let mut extra = String::new();
    if st.and_then(|t| t.underline) == Some(true) {
        extra.push_str(" text-decoration=\"underline\"");
    }
    if let Some(ls) = st.and_then(|t| t.letter_spacing).filter(|v| v.is_finite() && v.abs() <= 10.0 && *v != 0.0) {
        let _ = write!(extra, " letter-spacing=\"{}\"", mm(ls));
    }
    if let (Some(hc), Some(hw)) = (st.and_then(|t| t.halo_color.as_deref()), st.and_then(|t| t.halo_width).filter(|w| w.is_finite() && *w > 0.0 && *w <= 5.0)) {
        let _ = write!(extra, " stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" paint-order=\"stroke\"", svg::color(hc, "#ffffff"), mm(hw * 2.0));
    }
    let _ = write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-weight=\"{weight}\" font-style=\"{fstyle}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"{anchor}\"{extra}>{}</text>",
        mm(x),
        mm(y),
        export::font_stack(st.and_then(|t| t.family.as_deref())),
        mm(size_pt * PT_MM),
        svg::color(st.and_then(|t| t.color.as_deref()).unwrap_or("#000000"), "#000000"),
        esc(text)
    );
}

/// Opens the element's group, rotated about its centre when it has a rotation.
pub(super) fn open_group(out: &mut String, tag: &str, at: &Placement) {
    let rot = if at.rotation != 0.0 { format!(" transform=\"rotate({} {} {})\"", mm(at.rotation), mm(at.x + at.w / 2.0), mm(at.y + at.h / 2.0)) } else { String::new() };
    let _ = write!(out, "<g id=\"{tag}-{}\"{rot}>", esc(&at.id));
}

/// 0.25 → "0.25", 2.0 → "2": round to 6 decimals first so 0.1 × 3 prints as 0.3.
pub(super) fn num(v: f64) -> String {
    format!("{}", (v * 1e6).round() / 1e6)
}

// ---------------------------------------------------------------- scale bar

/// Ground metres in one scale-bar unit.
fn unit_m(units: &str) -> (&'static str, f64) {
    match units {
        "m" => ("m", 1.0),
        "mi" => ("mi", 1609.344),
        "ft" => ("ft", 0.3048),
        _ => ("km", 1000.0),
    }
}

/// Largest 1-2-5 × 10^k value not above `max` (> 0).
pub(super) fn nice_floor(max: f64) -> f64 {
    let p = 10f64.powf(max.log10().floor());
    [5.0, 2.0, 1.0].iter().map(|m| m * p).find(|v| *v <= max * (1.0 + 1e-9)).unwrap_or(p)
}

/// The drawn bar: segment length in `units`, number of segments, length on the page.
#[derive(Debug, Clone, PartialEq)]
pub struct BarPlan {
    pub unit: &'static str,
    pub segment: f64,
    pub segments: u32,
    pub length_mm: f64,
}

/// Choose a round segment length so the bar fits `max_mm` (L3: the bar is
/// ground truth at the frame centre, `D · k / scale` on the page).
pub fn plan_bar(units: &str, segments: u32, k: f64, scale: f64, max_mm: f64, total: f64) -> Option<BarPlan> {
    let (unit, m) = unit_m(units);
    let n = segments.clamp(1, 10);
    if !(max_mm > 0.0 && k > 0.0 && scale > 0.0) {
        return None;
    }
    // Page length of one unit of ground.
    let per_unit = geo::bar_length_mm(m, k, scale);
    // A fixed ground length (the coordinate scale: 1 000 m in ten 100 m parts) instead of the longest round bar that fits.
    let segment = if total > 0.0 { total / n as f64 } else { nice_floor(max_mm / n as f64 / per_unit) };
    Some(BarPlan { unit, segment, segments: n, length_mm: segment * n as f64 * per_unit })
}


pub const BAR_STYLES: &[&str] = &["alternating", "hollow", "line", "ticks", "double"];
pub const ARROW_STYLES: &[&str] = &["simple", "compass", "arrow", "triangle", "half", "rose", "circle", "custom"];

/// Scale bar settings beyond size and position.
pub struct BarOpts<'a> {
    pub units: &'a str,
    pub segments: u32,
    pub ratio: bool,
    /// One of [`BAR_STYLES`]; empty = "alternating".
    pub style: &'a str,
    /// "double" style: the second bar's units.
    pub units2: &'a str,
    /// A sentence under the bar: "1 cm on the map = 240 m".
    pub verbal: bool,
    pub color: &'a str,
    /// Ground length of the whole bar in `units`; 0 = the longest round bar that fits.
    pub length: f64,
}

/// "1 cm = 240 m" at the nominal scale (`unit` picked so the number is readable).
pub fn verbal_scale(scale: f64, units: &str) -> String {
    let (unit, m) = unit_m(units);
    let ground_m = scale / 100.0; // one cm on the page
    let mut v = ground_m / m;
    let mut u = unit;
    // A km/mi bar at a large scale would read "0.0024 km"; fall back to metres/feet.
    if v < 1.0 && unit == "km" {
        v = ground_m;
        u = "m";
    } else if v < 1.0 && unit == "mi" {
        v = ground_m / 0.3048;
        u = "ft";
    }
    format!("1 cm on the map = {} {u}", num((v * 100.0).round() / 100.0))
}

fn draw_bar(out: &mut String, at: &Placement, x0: f64, y0: f64, bar_h: f64, plan: &BarPlan, style: &str, color: &str, labels_below: bool) -> f64 {
    let num_pt = at.slot_pt("scale_bar.numbers", BAR_LABEL_PT);
    let seg_mm = plan.length_mm / plan.segments as f64;
    let c = svg::color(color, "#000000");
    match style {
        "line" | "ticks" => {
            let tick = if style == "ticks" { bar_h } else { bar_h * 0.6 };
            let yl = y0 + bar_h;
            let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"0.4\"/>", mm(x0), mm(yl), mm(x0 + plan.length_mm), mm(yl));
            for i in 0..=plan.segments {
                let x = x0 + seg_mm * i as f64;
                let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"0.4\"/>", mm(x), mm(yl), mm(x), mm(yl - tick));
            }
        }
        _ => {
            for i in 0..plan.segments {
                let fill = if style == "hollow" || i % 2 == 1 { "#ffffff" } else { c.as_str() };
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"0.25\"/>", mm(x0 + seg_mm * i as f64), mm(y0), mm(seg_mm), mm(bar_h));
            }
        }
    }
    let _ = labels_below;
    let label_y = y0 + bar_h + 0.8 + num_pt * PT_MM * 0.8;
    for i in 0..=plan.segments {
        let v = plan.segment * i as f64;
        let label = if i == plan.segments { format!("{} {}", num(v), plan.unit) } else { num(v) };
        text_slot(out, x0 + seg_mm * i as f64, label_y, &label, "sans", num_pt, "middle", at, "scale_bar.numbers");
    }
    label_y
}

pub fn scale_bar(out: &mut String, warnings: &mut Vec<String>, at: &Placement, o: &BarOpts, ctx: &FrameCtx) {
    // Room for the half-width of the last label ("4 km") past the bar's end.
    let reserve = 7.0;
    let segments = if o.segments == 0 { 4 } else { o.segments };
    let style = if o.style.is_empty() { "alternating" } else { o.style };
    let max_len = at.w - 1.0 - reserve;
    let Some(plan) = plan_bar(o.units, segments, ctx.factors.k, ctx.scale, at.w - 1.0 - reserve, o.length) else {
        warnings.push(format!("Scale bar '{}' is too narrow to draw; make it wider", at.id));
        return;
    };
    if o.length > 0.0 && plan.length_mm > max_len + 1e-6 {
        warnings.push(format!("Scale bar '{}' is {:.0} mm long at this scale and does not fit its {:.0} mm box; make it wider", at.id, plan.length_mm, at.w));
    }
    open_group(out, "scalebar", at);
    let x0 = at.x + 1.0;
    let bar_h = 2.0f64.min(at.h / 3.0);
    let mut label_y = draw_bar(out, at, x0, at.y + 1.0, bar_h, &plan, style, o.color, true);
    if style == "double" {
        // The same ground length again in the second unit, below the first bar's labels.
        let u2 = if o.units2.is_empty() { "mi" } else { o.units2 };
        if let Some(p2) = plan_bar(u2, segments, ctx.factors.k, ctx.scale, at.w - 1.0 - reserve, o.length) {
            let y2 = label_y + 1.2;
            label_y = draw_bar(out, at, x0, y2, bar_h, &p2, "alternating", o.color, true);
        }
    }
    let mut line_y = label_y;
    let note_pt = at.slot_pt("scale_bar.notes", BAR_LABEL_PT);
    if o.verbal {
        line_y += note_pt * PT_MM * 1.4;
        text_slot(out, x0, line_y, &verbal_scale(ctx.scale, o.units), "sans", note_pt, "start", at, "scale_bar.notes");
    }
    if o.ratio {
        // "1:24,000" on its own line under the distances, so it never collides with them.
        line_y += note_pt * PT_MM * 1.4;
        text_slot(out, x0, line_y, &format!("1:{}", super::grouped(ctx.scale)), "sans", note_pt, "start", at, "scale_bar.notes");
    }
    if (o.ratio || o.verbal || style == "double") && line_y > at.y + at.h + 1e-6 {
        warnings.push(if o.ratio { format!("Scale bar '{}' is too short for its 1:N label; make it taller", at.id) } else { format!("Scale bar '{}' does not fit its height; make it taller", at.id) });
    }
    out.push_str("</g>");
}

// ---------------------------------------------------------------- north arrow

/// North arrow settings. The artwork is our own SVG-free geometry; `custom` embeds the user's picture.
pub struct ArrowOpts<'a> {
    /// One of [`ARROW_STYLES`].
    pub style: &'a str,
    /// "true" (default, follows meridian convergence) or "grid" (the frame's up direction).
    pub north: &'a str,
    pub color: &'a str,
    /// Style "custom": a `data:image/...` URI whose top points north.
    pub source: &'a str,
}

/// Rotation of the arrow on the page: true north's bearing from grid up (or 0 for grid north) plus the frame rotation.
pub fn arrow_angle(north: &str, ctx: &FrameCtx) -> f64 {
    (if north == "grid" { 0.0 } else { ctx.factors.true_north_deg }) + ctx.rotation
}

pub fn north_arrow(out: &mut String, warnings: &mut Vec<String>, at: &Placement, o: &ArrowOpts, ctx: &FrameCtx) {
    let angle = arrow_angle(o.north, ctx);
    let (cx, cy) = (at.x + at.w / 2.0, at.y + at.h / 2.0);
    let c = svg::color(o.color, "#000000");
    open_group(out, "northarrow-outer", at);
    let _ = write!(out, "<g id=\"northarrow-{}\" transform=\"rotate({} {} {})\">", esc(&at.id), mm(angle), mm(cx), mm(cy));
    if o.style == "custom" {
        match super::shapes::check_picture(o.source) {
            Ok("") | Err(_) => warnings.push(format!("North arrow '{}': choose a PNG, JPEG or SVG picture for the custom style", at.id)),
            Ok(_) => {
                let _ = write!(out, "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" href=\"{}\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h), o.source);
            }
        }
        out.push_str("</g></g>");
        return;
    }
    let label_pt = at.slot_pt("north_arrow.letters", 9.0f64.min(at.h * 0.3 / PT_MM));
    let label_h = label_pt * PT_MM;
    let with_n = o.style != "rose";
    if with_n {
        text_slot(out, cx, at.y + label_h * 0.9, "N", "sans-bold", label_pt, "middle", at, "north_arrow.letters");
    }
    let top = if with_n { at.y + label_h * 1.2 } else { at.y };
    let (len, half) = (at.y + at.h - top, (at.w / 2.0).min((at.y + at.h - top) * 0.3));
    let pts = |v: &[(f64, f64)]| v.iter().map(|(x, y)| format!("{},{}", mm(*x), mm(*y))).collect::<Vec<_>>().join(" ");
    let poly = |out: &mut String, v: &[(f64, f64)], fill: &str| {
        let _ = write!(out, "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"0.25\" stroke-linejoin=\"miter\"/>", pts(v));
    };
    let bottom = top + len;
    match o.style {
        "compass" | "rose" => {
            // 4-point star, long points north-south; fixed ratios, no trigonometry. "rose" adds the four short diagonal points and the cardinal letters.
            let r = len.min(at.w) / 2.0;
            let mid = top + len / 2.0;
            let p = |dx: f64, dy: f64| (cx + dx * r, mid + dy * r);
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"0.25\"/>", mm(cx), mm(mid), mm(r * 0.8));
            if o.style == "rose" {
                let d = 0.55;
                poly(out, &[p(0.0, 0.0), p(d * 0.25, -d * 0.55), p(d, -d), p(d * 0.55, -d * 0.25)], "#ffffff");
                poly(out, &[p(0.0, 0.0), p(d * 0.55, d * 0.25), p(d, d), p(d * 0.25, d * 0.55)], "#ffffff");
                poly(out, &[p(0.0, 0.0), p(-d * 0.25, d * 0.55), p(-d, d), p(-d * 0.55, d * 0.25)], "#ffffff");
                poly(out, &[p(0.0, 0.0), p(-d * 0.55, -d * 0.25), p(-d, -d), p(-d * 0.25, -d * 0.55)], "#ffffff");
            }
            let star = [p(0.0, -1.0), p(0.2, -0.2), p(0.7, 0.0), p(0.2, 0.2), p(0.0, 1.0), p(-0.2, 0.2), p(-0.7, 0.0), p(-0.2, -0.2)];
            poly(out, &star, "#ffffff");
            let right = [p(0.0, -1.0), p(0.2, -0.2), p(0.7, 0.0), p(0.2, 0.2), p(0.0, 1.0), p(0.0, 0.0)];
            let _ = write!(out, "<polygon points=\"{}\" fill=\"{c}\"/>", pts(&right));
            if o.style == "rose" {
                let lp = at.slot_pt("north_arrow.letters", (r * 0.2 / PT_MM).clamp(4.0, 8.0));
                let (lx, ly) = (p(0.0, -1.0), p(0.0, 1.0));
                text_slot(out, lx.0, lx.1 - 0.6, "N", "sans-bold", lp, "middle", at, "north_arrow.letters");
                text_slot(out, ly.0, ly.1 + lp * PT_MM * 0.9, "S", "sans-bold", lp, "middle", at, "north_arrow.letters");
                text_slot(out, cx + r * 1.0 + 0.8, mid + lp * PT_MM * 0.35, "E", "sans-bold", lp, "start", at, "north_arrow.letters");
                text_slot(out, cx - r * 1.0 - 0.8, mid + lp * PT_MM * 0.35, "W", "sans-bold", lp, "end", at, "north_arrow.letters");
            }
        }
        "arrow" => {
            // Plain arrow with a shaft.
            let w = half.min(at.w / 2.0) * 0.9;
            let head = len * 0.45;
            poly(out, &[(cx, top), (cx + w, top + head), (cx + w * 0.3, top + head), (cx + w * 0.3, bottom), (cx - w * 0.3, bottom), (cx - w * 0.3, top + head), (cx - w, top + head)], &c);
        }
        "triangle" => {
            poly(out, &[(cx, top), (cx + half, bottom), (cx - half, bottom)], &c);
        }
        "half" => {
            // Topographic-map style: a line to a half arrowhead on the left.
            let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{c}\" stroke-width=\"0.35\"/>", mm(cx), mm(top), mm(cx), mm(bottom));
            poly(out, &[(cx, top), (cx - half * 0.7, top + len * 0.4), (cx, top + len * 0.3)], &c);
        }
        "circle" => {
            let r = len.min(at.w) / 2.0;
            let mid = top + len / 2.0;
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#ffffff\" stroke=\"{c}\" stroke-width=\"0.35\"/>", mm(cx), mm(mid), mm(r));
            poly(out, &[(cx, mid - r * 0.8), (cx + r * 0.45, mid + r * 0.6), (cx, mid + r * 0.25), (cx - r * 0.45, mid + r * 0.6)], &c);
        }
        _ => {
            // "simple": a kite, right half filled.
            let notch = top + len * 0.75;
            poly(out, &[(cx, top), (cx, notch), (cx - half, bottom)], "#ffffff");
            poly(out, &[(cx, top), (cx + half, bottom), (cx, notch)], &c);
        }
    }
    out.push_str("</g></g>");
}

// ---------------------------------------------------------------- legend

const ROW_MM: f64 = 5.0;
const SWATCH_W: f64 = 7.0;
const SWATCH_GAP: f64 = 2.5;
const LEGEND_PAD: f64 = 2.0;

#[derive(Clone, Copy)]
enum Sym {
    Poly,
    Line,
    Point,
    /// A raster layer: its elevation ramp (true) or a neutral chip (imagery).
    Raster(bool),
}

struct Row {
    /// None = a layer heading (no swatch).
    swatch: Option<(Sym, String)>,
    label: String,
    style: LayerStyle,
    /// Raster colour bar (hex stops) drawn across the row instead of a swatch.
    bar: Option<Vec<String>>,
    /// Low and high value printed under a colour bar.
    ends: Option<(String, String)>,
    /// Stable id for editing: the layer id (heading or single swatch) or `layer#i` (class i).
    key: String,
    layer: String,
}

impl Row {
    fn new(swatch: Option<(Sym, String)>, label: String, style: &LayerStyle) -> Row {
        Row { swatch, label, style: style.clone(), bar: None, ends: None, key: String::new(), layer: String::new() }
    }
}

fn format_break(v: f64) -> String {
    num(v)
}

/// Legend rows for one drawn layer, mirroring how `frame.rs` colours it.
fn layer_rows(info: &LayerInfo, style: &LayerStyle) -> Vec<Row> {
    if let Some(r) = &info.raster {
        use crate::raster_style::RasterLegend as L;
        let legend = style.raster.as_ref().and_then(|rs| crate::raster_style::legend(r, rs).ok().flatten());
        let mut rows = vec![Row::new(None, info.name.clone(), style)];
        return match legend {
            None => vec![Row::new(Some((Sym::Raster(r.kind != "rgb"), String::new())), info.name.clone(), style)],
            Some(L::Bar { colors, lo, hi }) => {
                rows.push(Row { bar: Some(colors), ..Row::new(None, String::new(), style) });
                rows.push(Row { ends: Some((crate::raster_style::fmt_num(lo), crate::raster_style::fmt_num(hi))), ..Row::new(None, String::new(), style) });
                rows
            }
            Some(L::Classes(items)) => {
                rows.extend(items.into_iter().map(|(c, l)| Row::new(Some((Sym::Poly, c)), l, style)));
                rows
            }
            Some(L::Note(t)) => {
                rows.push(Row { ends: Some((t, String::new())), ..Row::new(None, String::new(), style) });
                rows
            }
        };
    }
    let sym = match info.geom_type.as_str() {
        "Line" => Sym::Line,
        "Point" => Sym::Point,
        _ => Sym::Poly,
    };
    let field_ok = style.field.as_ref().is_some_and(|f| info.fields.iter().any(|x| &x.name == f));
    let classes: Vec<(String, String)> = match style.kind {
        StyleKind::Categorized if field_ok && style.classes.as_ref().is_some_and(|c| !c.is_empty()) => {
            let cs = style.classes.as_ref().unwrap();
            let mut v: Vec<(String, String)> = cs.iter().enumerate().map(|(i, c)| (c.label.clone(), class_color(style, i))).collect();
            v.push(("Other".into(), class_color(style, cs.len())));
            v
        }
        StyleKind::Graduated if field_ok && style.breaks.as_ref().is_some_and(|b| !b.is_empty()) => {
            let b = style.breaks.as_ref().unwrap();
            (0..=b.len())
                .map(|i| {
                    let label = if i == 0 {
                        format!("< {}", format_break(b[0]))
                    } else if i == b.len() {
                        format!("≥ {}", format_break(b[i - 1]))
                    } else {
                        format!("{} – {}", format_break(b[i - 1]), format_break(b[i]))
                    };
                    (label, class_color(style, i))
                })
                .collect()
        }
        _ => Vec::new(),
    };
    if classes.is_empty() {
        return vec![Row::new(Some((sym, class_color(style, 0))), info.name.clone(), style)];
    }
    let mut rows = vec![Row::new(None, info.name.clone(), style)];
    rows.extend(classes.into_iter().map(|(label, color)| Row::new(Some((sym, color)), label, style)));
    rows
}

fn swatch(out: &mut String, x: f64, y: f64, sym: Sym, color: &str, style: &LayerStyle, row_mm: f64, patch: &str) {
    let sw = style.stroke_width.max(0.0) * PX_MM;
    let stroke = svg::color(&style.stroke, "#4d4d4d");
    let op = opacity(style.opacity);
    let ymid = y + row_mm / 2.0;
    match sym {
        Sym::Poly => {
            let (s, w) = if sw > 0.0 { (stroke.as_str(), mm(sw)) } else { ("none", "0".into()) };
            let (yy, h) = (ymid - 1.75, 3.5);
            match patch {
                "circle" => {
                    let _ = write!(out, "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"1.75\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"{s}\" stroke-width=\"{w}\"/>", mm(x + SWATCH_W / 2.0), mm(ymid), mm(SWATCH_W / 2.0));
                }
                "rounded" => {
                    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1.2\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"{s}\" stroke-width=\"{w}\"/>", mm(x), mm(yy), mm(SWATCH_W), mm(h));
                }
                _ => {
                    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"{s}\" stroke-width=\"{w}\"/>", mm(x), mm(yy), mm(SWATCH_W), mm(h));
                }
            }
        }
        Sym::Line => {
            let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-opacity=\"{op}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>", mm(x), mm(ymid), mm(x + SWATCH_W), mm(ymid), mm(sw.max(0.3)));
        }
        Sym::Raster(ramped) => {
            let chips: Vec<String> = if ramped {
                (0..4).map(|i| crate::raster::ramp(i as f32 / 3.0)).map(|c| format!("#{:02x}{:02x}{:02x}", c[0] as u8, c[1] as u8, c[2] as u8)).collect()
            } else {
                vec!["#c8c8c8".into()]
            };
            let w = SWATCH_W / chips.len() as f64;
            for (i, c) in chips.iter().enumerate() {
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"{c}\"/>", mm(x + w * i as f64), mm(ymid - 1.75), mm(w));
            }
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"none\" stroke=\"#4d4d4d\" stroke-width=\"0.2\"/>", mm(x), mm(ymid - 1.75), mm(SWATCH_W));
        }
        Sym::Point => {
            let r = (style.radius.max(0.0) * PX_MM).min(1.8);
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"#333333\" stroke-width=\"{}\"/>", mm(x + SWATCH_W / 2.0), mm(ymid), mm(r), mm(0.6 * PX_MM));
        }
    }
}

/// Settings of a legend beyond its position.
pub struct LegendOpts<'a> {
    pub title: &'a str,
    pub columns: u32,
    pub overrides: &'a std::collections::BTreeMap<String, super::model::LegendOverride>,
    pub custom: &'a [super::model::LegendCustom],
    /// "square" (default) | "rounded" | "circle" for area swatches.
    pub patch: &'a str,
    /// Text size in points; 0 = 8 (title 10).
    pub text_pt: f64,
}

/// One row of the legend as generated, for the editing list in Properties.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendRowInfo {
    pub legend: String,
    pub key: String,
    pub layer: String,
    /// The generated text, before any rename.
    pub label: String,
    /// A layer heading or single entry: hiding it hides the layer's whole block.
    pub layer_row: bool,
    pub hidden: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn legend(out: &mut String, warnings: &mut Vec<String>, infos: &mut Vec<LegendRowInfo>, at: &Placement, o: &LegendOpts, ctx: &FrameCtx, req: &RenderRequest, lookup: &dyn Fn(&str) -> Option<LayerInfo>) {
    let base_pt = if o.text_pt > 0.0 { o.text_pt } else { LEGEND_PT };
    let text_pt = at.slot_pt("legend.entries", base_pt);
    let head_pt = at.slot_pt("legend.headings", base_pt);
    let title_pt = at.slot_pt("legend.title", if o.text_pt > 0.0 { o.text_pt * 1.25 } else { TITLE_PT });
    let row_mm = ROW_MM.max(text_pt.max(head_pt) * PT_MM * 1.7);
    let mut rows: Vec<Row> = Vec::new();
    for id in &ctx.drawn {
        if let Some(info) = lookup(id) {
            let style = req.styles.get(id).cloned().unwrap_or_default();
            let start = rows.len();
            rows.extend(layer_rows(&info, &style));
            let mut ci = 0;
            for (j, r) in rows[start..].iter_mut().enumerate() {
                r.layer = id.clone();
                if j == 0 {
                    r.key = id.clone();
                } else if r.bar.is_none() && r.ends.is_none() {
                    r.key = format!("{id}#{ci}");
                    ci += 1;
                }
            }
        }
    }
    let hidden_layer = |layer: &str| o.overrides.get(layer).is_some_and(|v| v.hidden);
    for r in rows.iter().filter(|r| !r.key.is_empty()) {
        infos.push(LegendRowInfo { legend: at.id.clone(), key: r.key.clone(), layer: r.layer.clone(), label: r.label.clone(), layer_row: r.key == r.layer, hidden: o.overrides.get(&r.key).is_some_and(|v| v.hidden) });
    }
    let mut rows: Vec<Row> = rows
        .into_iter()
        .filter(|r| !hidden_layer(&r.layer) && !(r.key != r.layer && o.overrides.get(&r.key).is_some_and(|v| v.hidden)))
        .map(|mut r| {
            if let Some(l) = o.overrides.get(&r.key).and_then(|v| v.label.as_ref()).filter(|l| !l.trim().is_empty()) {
                r.label = l.clone();
            }
            r
        })
        .collect();
    for c in o.custom {
        rows.push(Row::new(Some((Sym::Poly, svg::color(&c.color, "#888888"))), c.label.clone(), &LayerStyle::default()));
    }
    open_group(out, "legend", at);
    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" stroke=\"#999999\" stroke-width=\"0.25\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h));
    let mut top = at.y + LEGEND_PAD;
    if !o.title.trim().is_empty() {
        let t = ellipsize(o.title, "sans-bold", title_pt, at.w - 2.0 * LEGEND_PAD);
        text_slot(out, at.x + LEGEND_PAD, top + title_pt * PT_MM * 0.85, &t, "sans-bold", title_pt, "start", at, "legend.title");
        top += title_pt * PT_MM + 3.0;
    }
    let ncols = o.columns.clamp(1, 6) as usize;
    let per_col = rows.len().div_ceil(ncols).max(1);
    let col_w = (at.w - 2.0 * LEGEND_PAD) / ncols as f64;
    if top + per_col as f64 * row_mm + LEGEND_PAD > at.y + at.h + 1e-6 {
        warnings.push(format!("Legend '{}' does not fit its height; enlarge it or use more columns", at.id));
    }
    for (i, row) in rows.iter().enumerate() {
        let (c, r) = (i / per_col, i % per_col);
        let (cx, ry) = (at.x + LEGEND_PAD + c as f64 * col_w, top + r as f64 * row_mm);
        let base = ry + row_mm / 2.0 + text_pt * PT_MM * 0.35;
        if let Some(cols) = &row.bar {
            let w = (col_w - 1.0).min(SWATCH_W * 3.0);
            let sw = w / cols.len() as f64;
            let by = ry + row_mm / 2.0 - 1.75;
            for (k, c) in cols.iter().enumerate() {
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"{c}\"/>", mm(cx + sw * k as f64), mm(by), mm(sw + 0.05));
            }
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"none\" stroke=\"#4d4d4d\" stroke-width=\"0.2\"/>", mm(cx), mm(by), mm(w));
            continue;
        }
        if let Some((lo, hi)) = &row.ends {
            let w = (col_w - 1.0).min(SWATCH_W * 3.0);
            if hi.is_empty() {
                text_slot(out, cx, base, &ellipsize(lo, "sans", text_pt, col_w - 1.0), "sans", text_pt, "start", at, "legend.entries");
            } else {
                text_slot(out, cx, base, lo, "sans", text_pt, "start", at, "legend.entries");
                text_slot(out, cx + w, base, hi, "sans", text_pt, "end", at, "legend.entries");
            }
            continue;
        }
        match &row.swatch {
            Some((sym, color)) => {
                swatch(out, cx, ry, *sym, color, &row.style, row_mm, o.patch);
                let tx = cx + SWATCH_W + SWATCH_GAP;
                text_slot(out, tx, base, &ellipsize(&row.label, "sans", text_pt, col_w - SWATCH_W - SWATCH_GAP - 1.0), "sans", text_pt, "start", at, "legend.entries");
            }
            None => text_slot(out, cx, base - (text_pt - head_pt) * PT_MM * 0.35, &ellipsize(&row.label, "sans-bold", head_pt, col_w - 1.0), "sans-bold", head_pt, "start", at, "legend.headings"),
        }
    }
    out.push_str("</g>");
}

// ---------------------------------------------------------------- sources

/// "Data: A; B." from the citations of every layer drawn in the frame (L6),
/// each once, in draw order (top layer first).
pub fn sources(out: &mut String, warnings: &mut Vec<String>, at: &Placement, size: f64, ctx: &FrameCtx, cites: &dyn Fn(&str) -> Vec<String>, extra: &[String]) {
    let mut seen: Vec<String> = Vec::new();
    for c in ctx.drawn.iter().flat_map(|id| cites(id)).chain(extra.iter().cloned()) {
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    if seen.is_empty() {
        return;
    }
    let size = at.slot_pt("sources.text", if size > 0.0 { size } else { SOURCES_PT });
    let lines = wrap(&format!("Data: {}.", seen.join("; ")), "sans", size, at.w);
    let line_h = size * PT_MM * 1.25;
    open_group(out, "sources", at);
    for (i, l) in lines.iter().enumerate() {
        text_slot(out, at.x, at.y + size * PT_MM * 0.9 + line_h * i as f64, l, "sans", size, "start", at, "sources.text");
    }
    out.push_str("</g>");
    if size * PT_MM * 0.9 + line_h * (lines.len().saturating_sub(1)) as f64 + size * PT_MM * 0.3 > at.h + 1e-6 {
        warnings.push(format!("Sources '{}' does not fit its height; enlarge it or reduce its text size", at.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_measured_wrapped_and_ellipsized() {
        let w = text_width("Legend", "sans", 10.0);
        assert!(w > 5.0 && w < 20.0, "{w}");
        assert!(text_width("Legend", "sans-bold", 10.0) > w);
        let lines = wrap("one two three four five six seven", "sans", 8.0, 20.0);
        assert!(lines.len() > 1 && lines.iter().all(|l| text_width(l, "sans", 8.0) <= 20.0 + 1e-9), "{lines:?}");
        let e = ellipsize("A very long layer name indeed", "sans", 8.0, 20.0);
        assert!(e.ends_with('…') && text_width(&e, "sans", 8.0) <= 20.0, "{e}");
        assert_eq!(ellipsize("Short", "sans", 8.0, 20.0), "Short");
    }

    #[test]
    fn bar_plan_is_round_and_fits() {
        // 1:24,000 with k = 1: 1 km = 41.667 mm; 4 segments in 100 mm → 0.5 km segments.
        let p = plan_bar("km", 4, 1.0, 24_000.0, 100.0, 0.0).unwrap();
        assert_eq!((p.segment, p.segments), (0.5, 4));
        assert!(p.length_mm <= 100.0 && (p.length_mm - 83.333).abs() < 0.01, "{}", p.length_mm);
        // k > 1 shortens the ground a page millimetre covers, so the bar is longer per km.
        let q = plan_bar("km", 4, 1.0004, 24_000.0, 100.0, 0.0).unwrap();
        assert!((q.length_mm / p.length_mm - 1.0004).abs() < 1e-9);
        let m = plan_bar("mi", 2, 1.0, 100_000.0, 60.0, 0.0).unwrap();
        assert_eq!((m.unit, m.segment), ("mi", 1.0));
        assert!(plan_bar("km", 4, 1.0, 24_000.0, 0.0, 0.0).is_none());
    }
}
