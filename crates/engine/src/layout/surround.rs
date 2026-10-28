//! Surround elements (docs/LAYOUTS_PLAN.md L3, L6): legend, scale bar, north
//! arrow and sources. Everything geodetic comes from the frame's [`Factors`]
//! (computed from the layout's [`Body`](super::geo::Body)); nothing here names
//! an ellipsoid. Text is measured with the bundled Noto Sans faces so wrapping
//! and ellipsizing give the same result on every platform.

use super::export;
use super::frame::{class_color, PX_MM};
use super::geo::{self, Factors};
use super::model::{LayerStyle, Placement, RenderRequest, StyleKind};
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
    /// Layers drawn in the frame, top first.
    pub drawn: Vec<String>,
    /// Citations of what else the frame drew (its base map).
    pub extra_cites: Vec<String>,
}

const PT_MM: f64 = 25.4 / 72.0;

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

/// One line of text with its baseline at `y`.
fn text_line(out: &mut String, x: f64, y: f64, text: &str, font: &str, size_pt: f64, anchor: &str) {
    let (weight, style) = font_attrs(font);
    let _ = write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-weight=\"{weight}\" font-style=\"{style}\" font-size=\"{}\" fill=\"#000000\" text-anchor=\"{anchor}\">{}</text>",
        mm(x),
        mm(y),
        export::font_family(),
        mm(size_pt * PT_MM),
        esc(text)
    );
}

/// Opens the element's group, rotated about its centre when it has a rotation.
fn open_group(out: &mut String, tag: &str, at: &Placement) {
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
pub fn plan_bar(units: &str, segments: u32, k: f64, scale: f64, max_mm: f64) -> Option<BarPlan> {
    let (unit, m) = unit_m(units);
    let n = segments.clamp(1, 10);
    if !(max_mm > 0.0 && k > 0.0 && scale > 0.0) {
        return None;
    }
    // Page length of one unit of ground.
    let per_unit = geo::bar_length_mm(m, k, scale);
    let segment = nice_floor(max_mm / n as f64 / per_unit);
    Some(BarPlan { unit, segment, segments: n, length_mm: segment * n as f64 * per_unit })
}

const BAR_LABEL_PT: f64 = 7.0;

pub fn scale_bar(out: &mut String, warnings: &mut Vec<String>, at: &Placement, units: &str, segments: u32, ratio: bool, ctx: &FrameCtx) {
    // Room for the half-width of the last label ("4 km") past the bar's end.
    let reserve = 7.0;
    let Some(plan) = plan_bar(units, if segments == 0 { 4 } else { segments }, ctx.factors.k, ctx.scale, at.w - 1.0 - reserve) else {
        warnings.push(format!("Scale bar '{}' is too narrow to draw; make it wider", at.id));
        return;
    };
    open_group(out, "scalebar", at);
    let x0 = at.x + 1.0;
    let bar_h = 2.0f64.min(at.h / 3.0);
    let y0 = at.y + 1.0;
    let seg_mm = plan.length_mm / plan.segments as f64;
    for i in 0..plan.segments {
        let _ = write!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"#000000\" stroke-width=\"0.25\"/>",
            mm(x0 + seg_mm * i as f64),
            mm(y0),
            mm(seg_mm),
            mm(bar_h),
            if i % 2 == 0 { "#000000" } else { "#ffffff" }
        );
    }
    let label_y = y0 + bar_h + 0.8 + BAR_LABEL_PT * PT_MM * 0.8;
    for i in 0..=plan.segments {
        let v = plan.segment * i as f64;
        let label = if i == plan.segments { format!("{} {}", num(v), plan.unit) } else { num(v) };
        text_line(out, x0 + seg_mm * i as f64, label_y, &label, "sans", BAR_LABEL_PT, "middle");
    }
    if ratio {
        // "1:24,000" on its own line under the distances, so it never collides with them.
        let ratio_y = label_y + BAR_LABEL_PT * PT_MM * 1.4;
        if ratio_y > at.y + at.h {
            warnings.push(format!("Scale bar '{}' is too short for its 1:N label; make it taller", at.id));
        }
        text_line(out, x0, ratio_y, &format!("1:{}", super::grouped(ctx.scale)), "sans", BAR_LABEL_PT, "start");
    }
    out.push_str("</g>");
}

// ---------------------------------------------------------------- north arrow

pub fn north_arrow(out: &mut String, at: &Placement, style: &str, ctx: &FrameCtx) {
    // True north on the page: its bearing from grid up plus the frame's own
    // rotation (both clockwise, like SVG's rotate()).
    let angle = ctx.factors.true_north_deg + ctx.rotation;
    let (cx, cy) = (at.x + at.w / 2.0, at.y + at.h / 2.0);
    open_group(out, "northarrow-outer", at);
    let _ = write!(out, "<g id=\"northarrow-{}\" transform=\"rotate({} {} {})\">", esc(&at.id), mm(angle), mm(cx), mm(cy));
    let label_pt = 9.0f64.min(at.h * 0.3 / PT_MM);
    let label_h = label_pt * PT_MM;
    text_line(out, cx, at.y + label_h * 0.9, "N", "sans-bold", label_pt, "middle");
    let top = at.y + label_h * 1.2;
    let (len, half) = (at.y + at.h - top, (at.w / 2.0).min((at.y + at.h - top) * 0.3));
    let pts = |v: &[(f64, f64)]| v.iter().map(|(x, y)| format!("{},{}", mm(*x), mm(*y))).collect::<Vec<_>>().join(" ");
    if style == "compass" {
        // 4-point star, long points north-south; fixed ratios, no trigonometry.
        let r = len.min(at.w) / 2.0;
        let mid = top + len / 2.0;
        let p = |dx: f64, dy: f64| (cx + dx * r, mid + dy * r);
        let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"0.25\"/>", mm(cx), mm(mid), mm(r * 0.8));
        let star = [p(0.0, -1.0), p(0.2, -0.2), p(0.7, 0.0), p(0.2, 0.2), p(0.0, 1.0), p(-0.2, 0.2), p(-0.7, 0.0), p(-0.2, -0.2)];
        let _ = write!(out, "<polygon points=\"{}\" fill=\"#ffffff\" stroke=\"#000000\" stroke-width=\"0.25\" stroke-linejoin=\"miter\"/>", pts(&star));
        let right = [p(0.0, -1.0), p(0.2, -0.2), p(0.7, 0.0), p(0.2, 0.2), p(0.0, 1.0), p(0.0, 0.0)];
        let _ = write!(out, "<polygon points=\"{}\" fill=\"#000000\"/>", pts(&right));
    } else {
        // "simple": a kite, right half black.
        let (bottom, notch) = (top + len, top + len * 0.75);
        let left = [(cx, top), (cx, notch), (cx - half, bottom)];
        let right = [(cx, top), (cx + half, bottom), (cx, notch)];
        let _ = write!(out, "<polygon points=\"{}\" fill=\"#ffffff\" stroke=\"#000000\" stroke-width=\"0.25\" stroke-linejoin=\"miter\"/>", pts(&left));
        let _ = write!(out, "<polygon points=\"{}\" fill=\"#000000\" stroke=\"#000000\" stroke-width=\"0.25\" stroke-linejoin=\"miter\"/>", pts(&right));
    }
    out.push_str("</g></g>");
}

// ---------------------------------------------------------------- legend

const ROW_MM: f64 = 5.0;
const SWATCH_W: f64 = 7.0;
const SWATCH_GAP: f64 = 2.5;
const LEGEND_PAD: f64 = 2.0;
const LEGEND_PT: f64 = 8.0;
const TITLE_PT: f64 = 10.0;

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
}

fn format_break(v: f64) -> String {
    num(v)
}

/// Legend rows for one drawn layer, mirroring how `frame.rs` colours it.
fn layer_rows(info: &LayerInfo, style: &LayerStyle) -> Vec<Row> {
    if let Some(r) = &info.raster {
        return vec![Row { swatch: Some((Sym::Raster(r.kind != "rgb"), String::new())), label: info.name.clone(), style: style.clone() }];
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
        return vec![Row { swatch: Some((sym, class_color(style, 0))), label: info.name.clone(), style: style.clone() }];
    }
    let mut rows = vec![Row { swatch: None, label: info.name.clone(), style: style.clone() }];
    rows.extend(classes.into_iter().map(|(label, color)| Row { swatch: Some((sym, color)), label, style: style.clone() }));
    rows
}

fn swatch(out: &mut String, x: f64, y: f64, sym: Sym, color: &str, style: &LayerStyle) {
    let sw = style.stroke_width.max(0.0) * PX_MM;
    let stroke = svg::color(&style.stroke, "#4d4d4d");
    let op = opacity(style.opacity);
    let ymid = y + ROW_MM / 2.0;
    match sym {
        Sym::Poly => {
            let (s, w) = if sw > 0.0 { (stroke.as_str(), mm(sw)) } else { ("none", "0".into()) };
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"{s}\" stroke-width=\"{w}\"/>", mm(x), mm(y + 0.75), mm(SWATCH_W));
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
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"{c}\"/>", mm(x + w * i as f64), mm(y + 0.75), mm(w));
            }
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"3.5\" fill=\"none\" stroke=\"#4d4d4d\" stroke-width=\"0.2\"/>", mm(x), mm(y + 0.75), mm(SWATCH_W));
        }
        Sym::Point => {
            let r = (style.radius.max(0.0) * PX_MM).min(1.8);
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"#333333\" stroke-width=\"{}\"/>", mm(x + SWATCH_W / 2.0), mm(ymid), mm(r), mm(0.6 * PX_MM));
        }
    }
}

pub fn legend(out: &mut String, warnings: &mut Vec<String>, at: &Placement, title: &str, columns: u32, ctx: &FrameCtx, req: &RenderRequest, lookup: &dyn Fn(&str) -> Option<LayerInfo>) {
    let mut rows: Vec<Row> = Vec::new();
    for id in &ctx.drawn {
        if let Some(info) = lookup(id) {
            let style = req.styles.get(id).cloned().unwrap_or_default();
            rows.extend(layer_rows(&info, &style));
        }
    }
    open_group(out, "legend", at);
    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" stroke=\"#999999\" stroke-width=\"0.25\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h));
    let mut top = at.y + LEGEND_PAD;
    if !title.trim().is_empty() {
        let t = ellipsize(title, "sans-bold", TITLE_PT, at.w - 2.0 * LEGEND_PAD);
        text_line(out, at.x + LEGEND_PAD, top + TITLE_PT * PT_MM * 0.85, &t, "sans-bold", TITLE_PT, "start");
        top += TITLE_PT * PT_MM + 3.0;
    }
    let ncols = columns.clamp(1, 6) as usize;
    let per_col = rows.len().div_ceil(ncols).max(1);
    let col_w = (at.w - 2.0 * LEGEND_PAD) / ncols as f64;
    if top + per_col as f64 * ROW_MM + LEGEND_PAD > at.y + at.h + 1e-6 {
        warnings.push(format!("Legend '{}' does not fit its height; enlarge it or use more columns", at.id));
    }
    for (i, row) in rows.iter().enumerate() {
        let (c, r) = (i / per_col, i % per_col);
        let (cx, ry) = (at.x + LEGEND_PAD + c as f64 * col_w, top + r as f64 * ROW_MM);
        let base = ry + ROW_MM / 2.0 + LEGEND_PT * PT_MM * 0.35;
        match &row.swatch {
            Some((sym, color)) => {
                swatch(out, cx, ry, *sym, color, &row.style);
                let tx = cx + SWATCH_W + SWATCH_GAP;
                text_line(out, tx, base, &ellipsize(&row.label, "sans", LEGEND_PT, col_w - SWATCH_W - SWATCH_GAP - 1.0), "sans", LEGEND_PT, "start");
            }
            None => text_line(out, cx, base, &ellipsize(&row.label, "sans-bold", LEGEND_PT, col_w - 1.0), "sans-bold", LEGEND_PT, "start"),
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
    let size = if size > 0.0 { size } else { 6.0 };
    let lines = wrap(&format!("Data: {}.", seen.join("; ")), "sans", size, at.w);
    let line_h = size * PT_MM * 1.25;
    open_group(out, "sources", at);
    for (i, l) in lines.iter().enumerate() {
        text_line(out, at.x, at.y + size * PT_MM * 0.9 + line_h * i as f64, l, "sans", size, "start");
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
        let p = plan_bar("km", 4, 1.0, 24_000.0, 100.0).unwrap();
        assert_eq!((p.segment, p.segments), (0.5, 4));
        assert!(p.length_mm <= 100.0 && (p.length_mm - 83.333).abs() < 0.01, "{}", p.length_mm);
        // k > 1 shortens the ground a page millimetre covers, so the bar is longer per km.
        let q = plan_bar("km", 4, 1.0004, 24_000.0, 100.0).unwrap();
        assert!((q.length_mm / p.length_mm - 1.0004).abs() < 1e-9);
        let m = plan_bar("mi", 2, 1.0, 100_000.0, 60.0).unwrap();
        assert_eq!((m.unit, m.segment), ("mi", 1.0));
        assert!(plan_bar("km", 4, 1.0, 24_000.0, 0.0).is_none());
    }
}
