//! The one chart renderer (docs/STATISTICS_WORKSPACE_PLAN.md, "Charts have one renderer"): a `ChartSpec` becomes SVG text, the same in the sheet, in a
//! layout, and in a PNG or PDF. The sheet inserts the SVG inline and finds a clicked mark with `closest("[data-mark]")`; the index is the mark's
//! position counting every selectable mark of every series in order (series drawn as lines have none), which is how `ChartSpec` maps it to feature ids.
//!
//! Colours are the Okabe–Ito palette (Okabe and Ito 2008, "Color Universal Design"), safe for the common kinds of colour blindness. Text and axes use
//! `style.ink` (`currentColor` by default, so the sheet's light and dark themes both work); exports pass a fixed ink and a paper colour.

use super::result::{ChartKind, ChartSeries, ChartSpec, Mark, SeriesStyle};
use std::fmt::Write;

/// Okabe–Ito, in the order series take them (black is left out of the cycle; it is the ink).
pub const PALETTE: [&str; 7] = ["#0072B2", "#E69F00", "#009E73", "#D55E00", "#56B4E9", "#CC79A7", "#F0E442"];
const BLUE: &str = "#0072B2";
const VERMILLION: &str = "#D55E00";

#[derive(Clone, Debug)]
pub struct ChartStyle {
    /// Text, axis and grid colour.
    pub ink: String,
    /// Page colour behind the chart; none leaves it transparent.
    pub paper: Option<String>,
    pub font: String,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self { ink: "currentColor".into(), paper: None, font: "'Noto Sans', 'Helvetica Neue', Arial, sans-serif".into() }
    }
}

impl ChartStyle {
    /// For PNG, PDF and print: black on white.
    pub fn paper() -> Self {
        Self { ink: "#1a1a1a".into(), paper: Some("#ffffff".into()), font: "'Noto Sans'".into() }
    }
}

/// Render a chart at `w` x `h` (CSS pixels) with the default (theme-following) style.
pub fn chart_svg(spec: &ChartSpec, w: f64, h: f64) -> String {
    render_svg(spec, w, h, &ChartStyle::default())
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A number for an axis label or a note: up to 4 significant digits, no trailing zeros.
pub fn num(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-3..1e7).contains(&a) {
        return format!("{v:.3e}");
    }
    let digits = 3 - a.log10().floor() as i32;
    let s = format!("{v:.*}", digits.clamp(0, 8) as usize);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// "Nice" tick positions covering [lo, hi]: steps of 1, 2 or 5 times a power of ten, about `target` of them.
pub fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    if !(lo.is_finite() && hi.is_finite()) {
        return vec![];
    }
    if hi <= lo {
        return vec![lo];
    }
    let raw = (hi - lo) / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag);
    let first = (lo / step).ceil();
    let last = (hi / step).floor();
    (0..=((last - first) as i64).max(0)).map(|i| (first + i as f64) * step).map(|v| if v.abs() < step * 1e-9 { 0.0 } else { v }).collect()
}

struct Frame {
    l: f64,
    t: f64,
    w: f64,
    h: f64,
}

struct Scale {
    lo: f64,
    hi: f64,
    a: f64,
    b: f64,
}

impl Scale {
    fn new(lo: f64, hi: f64, a: f64, b: f64) -> Self {
        let (lo, hi) = if hi > lo { (lo, hi) } else { (lo - 0.5, hi + 0.5) };
        Self { lo, hi, a, b }
    }
    fn at(&self, v: f64) -> f64 {
        self.a + (v - self.lo) / (self.hi - self.lo) * (self.b - self.a)
    }
}

fn pad(lo: f64, hi: f64, frac: f64) -> (f64, f64) {
    let d = if hi > lo { hi - lo } else { lo.abs().max(1.0) * 0.5 };
    (lo - d * frac, hi + d * frac)
}

fn extent(vals: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    vals.filter(|v| v.is_finite()).fold(None, |a, v| Some(a.map_or((v, v), |(l, h)| (l.min(v), h.max(v)))))
}

fn all_marks(spec: &ChartSpec) -> impl Iterator<Item = &Mark> {
    spec.series.iter().flat_map(|s| s.marks.iter())
}

pub fn render_svg(spec: &ChartSpec, w: f64, h: f64, style: &ChartStyle) -> String {
    render_into(spec, w, h, style, "")
}

/// The same drawing as a nested `<svg>` at (`x`, `y`) scaled to `out_w` x `out_h` (a layout page's millimetres). The chart is drawn in a `w` x `h`
/// pixel space, so the page shows exactly what the sheet shows.
pub fn render_nested(spec: &ChartSpec, w: f64, h: f64, style: &ChartStyle, at: (f64, f64, f64, f64)) -> String {
    render_into(spec, w, h, style, &format!(r#" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid meet""#, at.0, at.1, at.2, at.3))
}

fn render_into(spec: &ChartSpec, w: f64, h: f64, style: &ChartStyle, placed: &str) -> String {
    let mut o = String::new();
    let size = if placed.is_empty() { format!(r#" width="{w}" height="{h}""#) } else { placed.to_string() };
    let _ = write!(
        o,
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="stat-chart" data-kind="{kind}" viewBox="0 0 {w} {h}"{size} font-family="{font}" font-size="11" fill="{ink}" color="{ink}">"#,
        kind = kind_name(spec.kind),
        font = esc(&style.font),
        ink = esc(&style.ink),
    );
    if let Some(p) = &style.paper {
        let _ = write!(o, r#"<rect width="{w}" height="{h}" fill="{p}"/>"#);
    }
    let _ = write!(o, r#"<text class="title" x="{}" y="18" font-size="13" font-weight="600">{}</text>"#, 8, esc(&spec.title));
    match spec.kind {
        ChartKind::Heatmap => heatmap(&mut o, spec, w, h, style),
        ChartKind::ScatterMatrix => matrix(&mut o, spec, w, h, style),
        _ => cartesian(&mut o, spec, w, h, style),
    }
    // Notes under the chart: the density-binning note and the like are part of the picture, so they travel with it into a layout or a PNG.
    for (i, n) in spec.notes.iter().take(3).enumerate() {
        let _ = write!(o, r#"<text class="note" x="8" y="{}" font-size="9" opacity="0.75">{}</text>"#, h - 6.0 - (spec.notes.len().min(3) - 1 - i) as f64 * 11.0, esc(&shorten(n, ((w - 16.0) / 4.8) as usize)));
    }
    o.push_str("</svg>");
    // A fixed ink is written into every stroke, so a file opened in a drawing program or a PDF tool needs no CSS colour inheritance.
    if style.ink != "currentColor" {
        o = o.replace("currentColor", &esc(&style.ink));
    }
    o
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max.max(10) { s.to_string() } else { format!("{}…", s.chars().take(max.max(10) - 1).collect::<String>()) }
}

fn kind_name(k: ChartKind) -> &'static str {
    match k {
        ChartKind::Histogram => "histogram",
        ChartKind::Box => "box",
        ChartKind::Scatter => "scatter",
        ChartKind::Qq => "qq",
        ChartKind::Bar => "bar",
        ChartKind::Line => "line",
        ChartKind::Heatmap => "heatmap",
        ChartKind::ScatterMatrix => "scatter_matrix",
    }
}

fn series_colour(i: usize) -> &'static str {
    PALETTE[i % PALETTE.len()]
}

fn axes(o: &mut String, spec: &ChartSpec, f: &Frame, sx: &Scale, sy: &Scale, ink: &str, x_cats: bool) {
    let _ = ink;
    // grid and y ticks
    for t in nice_ticks(sy.lo, sy.hi, 6) {
        let y = sy.at(t);
        let _ = write!(o, r#"<line class="grid" x1="{:.1}" y1="{y:.1}" x2="{:.1}" y2="{y:.1}" stroke="currentColor" stroke-opacity="0.12"/>"#, f.l, f.l + f.w);
        let _ = write!(o, r#"<text class="tick" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#, f.l - 5.0, y + 3.5, num(t));
    }
    if x_cats {
        for (i, c) in spec.x.categories.iter().enumerate() {
            let x = sx.at(i as f64);
            let max = ((f.w / spec.x.categories.len().max(1) as f64) / 5.5) as usize;
            let _ = write!(o, r#"<text class="tick" x="{x:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, f.t + f.h + 14.0, esc(&shorten(c, max)));
        }
    } else {
        for t in nice_ticks(sx.lo, sx.hi, 7) {
            let x = sx.at(t);
            let _ = write!(o, r#"<line class="tick" x1="{x:.1}" y1="{:.1}" x2="{x:.1}" y2="{:.1}" stroke="currentColor" stroke-opacity="0.5"/>"#, f.t + f.h, f.t + f.h + 4.0);
            let _ = write!(o, r#"<text class="tick" x="{x:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, f.t + f.h + 15.0, num(t));
        }
    }
    let _ = write!(o, r#"<rect class="frame" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="none" stroke="currentColor" stroke-opacity="0.5"/>"#, f.l, f.t, f.w, f.h);
    let unit = |a: &super::result::Axis| match &a.unit {
        Some(u) => format!("{} ({})", a.label, u),
        None => a.label.clone(),
    };
    let _ = write!(o, r#"<text class="axis-label" x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"#, f.l + f.w / 2.0, f.t + f.h + 32.0, esc(&unit(&spec.x)));
    let _ = write!(o, r#"<text class="axis-label" transform="translate(13 {:.1}) rotate(-90)" text-anchor="middle">{}</text>"#, f.t + f.h / 2.0, esc(&unit(&spec.y)));
}

fn cartesian(o: &mut String, spec: &ChartSpec, w: f64, h: f64, style: &ChartStyle) {
    let notes = spec.notes.len().min(3) as f64 * 11.0;
    let legend = spec.series.iter().filter(|s| s.style != SeriesStyle::Line || spec.kind == ChartKind::Line).count() > 1 && matches!(spec.kind, ChartKind::Scatter | ChartKind::Line | ChartKind::Bar);
    let f = Frame { l: 52.0, t: 30.0, w: w - 52.0 - 14.0 - if legend { 110.0 } else { 0.0 }, h: h - 30.0 - 46.0 - notes };
    let marks = || all_marks(spec);
    // x domain
    let x_cats = !spec.x.categories.is_empty() && matches!(spec.kind, ChartKind::Box | ChartKind::Bar);
    let (mut xlo, mut xhi) = if x_cats {
        (-0.6, spec.x.categories.len() as f64 - 0.4)
    } else {
        let e = extent(marks().flat_map(|m| [m.x, m.x2]).flatten()).unwrap_or((0.0, 1.0));
        match spec.kind {
            ChartKind::Histogram => e,
            _ => pad(e.0, e.1, 0.04),
        }
    };
    if let Some(v) = spec.x.min {
        xlo = v;
    }
    if let Some(v) = spec.x.max {
        xhi = v;
    }
    // y domain
    let ys = marks().flat_map(|m| [m.y, m.y2, m.lo, m.hi]).flatten();
    let e = extent(ys).unwrap_or((0.0, 1.0));
    let (mut ylo, mut yhi) = match spec.kind {
        ChartKind::Histogram => (0.0, e.1 * 1.05),
        ChartKind::Bar => (e.0.min(0.0), e.1.max(0.0) + (e.1.max(0.0) - e.0.min(0.0)).max(1e-9) * 0.05),
        _ => pad(e.0, e.1, 0.06),
    };
    if let Some(v) = spec.y.min {
        ylo = v;
    }
    if let Some(v) = spec.y.max {
        yhi = v;
    }
    let sx = Scale::new(xlo, xhi, f.l, f.l + f.w);
    let sy = Scale::new(ylo, yhi, f.t + f.h, f.t);
    axes(o, spec, &f, &sx, &sy, &style.ink, x_cats);
    o.push_str(r#"<g class="marks">"#);
    let mut idx = 0usize;
    let max_value = marks().filter_map(|m| m.value).fold(0.0, f64::max);
    for (si, s) in spec.series.iter().enumerate() {
        let colour = series_colour(si);
        if matches!(s.style, SeriesStyle::Line | SeriesStyle::LineAndMarks) {
            let pts: Vec<String> = s.marks.iter().filter_map(|m| Some(format!("{:.1},{:.1}", sx.at(m.x?), sy.at(m.y?)))).collect();
            let c = if s.style == SeriesStyle::Line && spec.kind != ChartKind::Line { VERMILLION } else { colour };
            let _ = write!(o, r#"<polyline class="line" data-series="{}" fill="none" stroke="{c}" stroke-width="1.6" points="{}"/>"#, esc(&s.name), pts.join(" "));
            if s.style == SeriesStyle::Line {
                continue;
            }
        }
        for m in &s.marks {
            mark(o, spec, m, idx, colour, &sx, &sy, max_value);
            idx += 1;
        }
    }
    o.push_str("</g>");
    if legend {
        let mut y = f.t + 4.0;
        for (si, s) in spec.series.iter().enumerate().filter(|(_, s)| s.style != SeriesStyle::Line || spec.kind == ChartKind::Line) {
            let _ = write!(o, r#"<rect x="{:.1}" y="{y:.1}" width="10" height="10" fill="{}"/><text x="{:.1}" y="{:.1}">{}</text>"#, f.l + f.w + 10.0, series_colour(si), f.l + f.w + 24.0, y + 9.0, esc(&shorten(&s.name, 14)));
            y += 15.0;
        }
    }
}

fn data_attrs(idx: usize, m: &Mark) -> String {
    let mut a = format!(r#"class="mark" data-mark="{idx}""#);
    if !m.fids.is_empty() || m.bin.is_some() {
        a.push_str(r#" data-selectable="1""#);
    }
    a
}

#[allow(clippy::too_many_arguments)]
fn mark(o: &mut String, spec: &ChartSpec, m: &Mark, idx: usize, colour: &str, sx: &Scale, sy: &Scale, max_value: f64) {
    let title = |m: &Mark| m.label.as_ref().map(|l| format!("<title>{}</title>", esc(l))).unwrap_or_default();
    let attrs = data_attrs(idx, m);
    match spec.kind {
        ChartKind::Histogram => {
            if let (Some(x), Some(x2), Some(y)) = (m.x, m.x2, m.y) {
                let (a, b) = (sx.at(x), sx.at(x2));
                let _ = write!(o, r#"<rect {attrs} x="{a:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{BLUE}" stroke="white" stroke-width="0.8">{}</rect>"#, sy.at(y), (b - a - 0.0).max(0.5), (sy.at(0.0) - sy.at(y)).max(0.0), title(m));
            }
        }
        ChartKind::Bar => {
            if let (Some(x), Some(y)) = (m.x, m.y) {
                let half = (sx.at(1.0) - sx.at(0.0)).abs() * 0.35;
                let (y0, y1) = (sy.at(0.0), sy.at(y));
                let _ = write!(o, r#"<rect {attrs} x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{colour}">{}</rect>"#, sx.at(x) - half, y0.min(y1), half * 2.0, (y0 - y1).abs(), title(m));
            }
        }
        ChartKind::Box => {
            let Some(x) = m.x else { return };
            let cx = sx.at(x);
            if let (Some(q1), Some(q3)) = (m.y, m.y2) {
                let half = (sx.at(1.0) - sx.at(0.0)).abs() * 0.28;
                let (top, bot) = (sy.at(q3), sy.at(q1));
                let _ = write!(o, r#"<g {attrs}>{}<rect x="{:.1}" y="{top:.1}" width="{:.1}" height="{:.1}" fill="{colour}" fill-opacity="0.45" stroke="currentColor"/>"#, title(m), cx - half, half * 2.0, bot - top);
                if let Some(md) = m.median {
                    let _ = write!(o, r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor" stroke-width="2"/>"#, cx - half, sy.at(md), cx + half, sy.at(md));
                }
                if let Some(lo) = m.lo {
                    let _ = write!(o, r#"<line x1="{cx:.1}" y1="{bot:.1}" x2="{cx:.1}" y2="{:.1}" stroke="currentColor"/><line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor"/>"#, sy.at(lo), cx - half / 2.0, sy.at(lo), cx + half / 2.0, sy.at(lo));
                }
                if let Some(hi) = m.hi {
                    let _ = write!(o, r#"<line x1="{cx:.1}" y1="{top:.1}" x2="{cx:.1}" y2="{:.1}" stroke="currentColor"/><line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor"/>"#, sy.at(hi), cx - half / 2.0, sy.at(hi), cx + half / 2.0, sy.at(hi));
                }
                o.push_str("</g>");
            } else if let Some(y) = m.y {
                // an outlier: beyond the whiskers, drawn as an open circle
                let _ = write!(o, r#"<circle {attrs} cx="{cx:.1}" cy="{:.1}" r="3" fill="none" stroke="{VERMILLION}" stroke-width="1.3">{}</circle>"#, sy.at(y), title(m));
            }
        }
        _ => {
            // Scatter, Q–Q, line points. A density cell has a far corner and a count.
            if let (Some(x), Some(y), Some(x2), Some(y2)) = (m.x, m.y, m.x2, m.y2) {
                let (a, b) = (sx.at(x), sx.at(x2));
                let (c, d) = (sy.at(y), sy.at(y2));
                let op = if max_value > 0.0 { 0.15 + 0.85 * (m.value.unwrap_or(0.0) / max_value) } else { 0.5 };
                let _ = write!(o, r#"<rect {attrs} x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{BLUE}" fill-opacity="{op:.2}">{}</rect>"#, a.min(b), c.min(d), (b - a).abs(), (c - d).abs(), title(m));
            } else if let (Some(x), Some(y)) = (m.x, m.y) {
                let _ = write!(o, r#"<circle {attrs} cx="{:.1}" cy="{:.1}" r="{}" fill="{colour}" fill-opacity="0.75">{}</circle>"#, sx.at(x), sy.at(y), if spec.kind == ChartKind::Line { 3.0 } else { 2.6 }, title(m));
            }
        }
    }
}

/// Blue (-1) through white (0) to vermillion (+1): a diverging scale from the Okabe–Ito pair that stays distinct for colour-blind readers.
fn diverge(v: f64) -> String {
    let t = v.clamp(-1.0, 1.0);
    let (r, g, b) = if t < 0.0 { (0.0, 0x72 as f64, 0xB2 as f64) } else { (0xD5 as f64, 0x5E as f64, 0.0) };
    let k = t.abs();
    let mix = |c: f64| (255.0 + (c - 255.0) * k).round() as u8;
    format!("#{:02x}{:02x}{:02x}", mix(r), mix(g), mix(b))
}

fn heatmap(o: &mut String, spec: &ChartSpec, w: f64, h: f64, _style: &ChartStyle) {
    let n = spec.x.categories.len().max(1);
    let label_w = spec.y.categories.iter().map(|c| c.chars().count()).max().unwrap_or(4).min(16) as f64 * 5.6 + 10.0;
    let size = ((w - label_w - 16.0) / n as f64).min((h - 30.0 - 60.0) / spec.y.categories.len().max(1) as f64).max(8.0);
    let (l, t) = (label_w + 8.0, 34.0);
    let mut idx = 0usize;
    o.push_str(r#"<g class="marks">"#);
    for s in &spec.series {
        for m in &s.marks {
            if let (Some(x), Some(y)) = (m.x, m.y) {
                let v = m.value.unwrap_or(0.0);
                let (px, py) = (l + x * size, t + y * size);
                let _ = write!(o, r#"<g class="mark" data-mark="{idx}"><title>{}</title><rect x="{px:.1}" y="{py:.1}" width="{size:.1}" height="{size:.1}" fill="{}" stroke="white" stroke-width="0.6"/>"#, esc(m.label.as_deref().unwrap_or("")), diverge(v));
                if size >= 26.0 {
                    let _ = write!(o, r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="10" fill="#1a1a1a">{}</text>"##, px + size / 2.0, py + size / 2.0 + 3.5, num((v * 100.0).round() / 100.0));
                }
                o.push_str("</g>");
            }
            idx += 1;
        }
    }
    o.push_str("</g>");
    for (i, c) in spec.x.categories.iter().enumerate() {
        let _ = write!(o, r#"<text class="tick" transform="translate({:.1} {:.1}) rotate(-35)" text-anchor="end">{}</text>"#, l + (i as f64 + 0.5) * size + 4.0, t + spec.y.categories.len() as f64 * size + 12.0, esc(&shorten(c, 16)));
    }
    for (i, c) in spec.y.categories.iter().enumerate() {
        let _ = write!(o, r#"<text class="tick" x="{:.1}" y="{:.1}" text-anchor="end">{}</text>"#, l - 5.0, t + (i as f64 + 0.5) * size + 3.5, esc(&shorten(c, 16)));
    }
    // colour key
    let ky = t + spec.y.categories.len() as f64 * size + 40.0;
    for i in 0..=10 {
        let v = -1.0 + i as f64 * 0.2;
        let _ = write!(o, r#"<rect x="{:.1}" y="{ky:.1}" width="14" height="8" fill="{}"/>"#, l + i as f64 * 14.0, diverge(v));
    }
    let _ = write!(o, r#"<text class="tick" x="{l:.1}" y="{:.1}">-1</text><text class="tick" x="{:.1}" y="{:.1}" text-anchor="end">+1</text>"#, ky + 20.0, l + 154.0, ky + 20.0);
}

fn matrix(o: &mut String, spec: &ChartSpec, w: f64, h: f64, _style: &ChartStyle) {
    let n = spec.x.categories.len().max(1);
    let notes = spec.notes.len().min(3) as f64 * 11.0;
    let (l, t) = (10.0, 28.0);
    let cell = ((w - l - 8.0) / n as f64).min((h - t - 8.0 - notes) / n as f64).max(20.0);
    let mut range: Vec<Option<(f64, f64)>> = vec![None; n];
    for s in &spec.series {
        let Some((c, r)) = panel(&s.name) else { continue };
        for (k, vals) in [(c, s.marks.iter().filter_map(|m| m.x).collect::<Vec<_>>()), (r, s.marks.iter().filter_map(|m| m.y).collect::<Vec<_>>())] {
            if let Some(e) = extent(vals.into_iter()) {
                if k < n {
                    range[k] = Some(range[k].map_or(e, |a| (a.0.min(e.0), a.1.max(e.1))));
                }
            }
        }
    }
    let mut idx = 0usize;
    o.push_str(r#"<g class="marks">"#);
    for i in 0..n {
        for j in 0..n {
            let (px, py) = (l + i as f64 * cell, t + j as f64 * cell);
            let _ = write!(o, r#"<rect class="panel" x="{px:.1}" y="{py:.1}" width="{cell:.1}" height="{cell:.1}" fill="none" stroke="currentColor" stroke-opacity="0.4"/>"#);
            if i == j {
                let _ = write!(o, r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-weight="600">{}</text>"#, px + cell / 2.0, py + cell / 2.0 + 4.0, esc(&shorten(&spec.x.categories[i], (cell / 6.0) as usize)));
            }
        }
    }
    for (si, s) in spec.series.iter().enumerate() {
        let _ = si;
        let panel_ij = panel(&s.name);
        for m in &s.marks {
            if let (Some((i, j)), Some(x), Some(y)) = (panel_ij, m.x, m.y) {
                if let (Some(Some(rx)), Some(Some(ry))) = (range.get(i), range.get(j)) {
                    let sx = Scale::new(rx.0, rx.1, l + i as f64 * cell + 4.0, l + (i + 1) as f64 * cell - 4.0);
                    let sy = Scale::new(ry.0, ry.1, t + (j + 1) as f64 * cell - 4.0, t + j as f64 * cell + 4.0);
                    let _ = write!(o, r#"<circle {} cx="{:.1}" cy="{:.1}" r="1.8" fill="{BLUE}" fill-opacity="0.65"/>"#, data_attrs(idx, m), sx.at(x), sy.at(y));
                }
            }
            idx += 1;
        }
    }
    o.push_str("</g>");
}

/// Series of a scatter matrix are named "<column>,<row>" (indices into `x.categories`).
fn panel(name: &str) -> Option<(usize, usize)> {
    let (a, b) = name.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// All selectable marks in drawing order: `data-mark` indices point into this list.
pub fn selectable_marks(spec: &ChartSpec) -> Vec<&Mark> {
    spec.series.iter().filter(|s: &&ChartSeries| s.style != SeriesStyle::Line).flat_map(|s| s.marks.iter()).collect()
}
