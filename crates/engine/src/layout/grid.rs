//! Graticule and projected grid of a map frame (docs/LAYOUTS_PLAN.md C2.4).
//! The graticule is lines of constant longitude and latitude, sampled along
//! their length and transformed to the frame CRS, so they curve as the
//! projection dictates; labels come from [`Body`] (no hemisphere letters are
//! assumed here). The grid is straight lines at multiples of an interval in
//! frame-CRS units.

use super::geo::{Body, FrameCrs};
use super::model::{GridSpec, GridStyle, LabelFormat, Placement};
use super::slots::GRID_LABEL_PT;
use super::surround::{nice_floor, num, text_slot};
use super::svg::{self, esc};
use std::fmt::Write;

/// Lines drawn per set before we refuse (a typo'd interval would draw millions).
const MAX_LINES: usize = 200;
const SAMPLES: usize = 64;
const PT_MM: f64 = 25.4 / 72.0;
/// Default tick length, mm (crosses use 0.6 of it as the arm).
const TICK_MM: f64 = 2.5;

pub struct GridOutput {
    /// Lines, drawn inside the frame's clip.
    pub lines: String,
    /// A finer grid drawn lighter under the main lines (inside the clip too).
    pub sub: String,
    /// Edge labels, drawn outside it.
    pub labels: String,
}

/// Page position of a point of the frame CRS.
pub type ToPage<'a> = &'a dyn Fn(f64, f64) -> (f64, f64);

/// Degrees: 1-2-5-ish steps down to a hundred-thousandth of a degree.
const DEG_STEPS: [f64; 22] = [30.0, 20.0, 15.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.25, 0.2, 0.1, 0.05, 0.02, 0.01, 0.005, 0.002, 0.001, 0.0005, 0.0002, 0.0001, 0.00005, 0.00001];

fn auto_degrees(span: f64) -> f64 {
    DEG_STEPS.iter().copied().find(|s| *s <= span / 3.0).unwrap_or(0.00001)
}

/// Fewest decimals that show `step` exactly (0.25 → 2, 0.5 → 1, 5 → 0).
fn decimals(step: f64) -> usize {
    (0..=6).find(|d| (step * 10f64.powi(*d) - (step * 10f64.powi(*d)).round()).abs() < 1e-9).unwrap_or(6) as usize
}

/// Multiples of `step` in `[lo, hi]`, as integers k (value = k × step).
fn multiples(lo: f64, hi: f64, step: f64) -> std::ops::RangeInclusive<i64> {
    ((lo / step - 1e-9).ceil() as i64)..=((hi / step + 1e-9).floor() as i64)
}

fn id_part(v: f64) -> String {
    num(v).replace('-', "m").replace('.', "p")
}

fn path(pts: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        svg::cmd(&mut d, if i == 0 { 'M' } else { 'L' }, p.0, p.1);
    }
    d
}

fn label(out: &mut String, at: &Placement, slot: &str, x: f64, y: f64, anchor: &str, text: &str) {
    text_slot(out, x, y, text, "sans", at.slot_pt(slot, GRID_LABEL_PT), anchor, at, slot);
}

/// Where the polyline first crosses the horizontal line `y` inside `[x0, x1]`.
fn cross_y(pts: &[(f64, f64)], y: f64, x0: f64, x1: f64) -> Option<f64> {
    pts.windows(2).find_map(|w| {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        if (ay - y) * (by - y) > 0.0 || ay == by {
            return None;
        }
        let x = ax + (bx - ax) * (y - ay) / (by - ay);
        (x >= x0 && x <= x1).then_some(x)
    })
}

fn cross_x(pts: &[(f64, f64)], x: f64, y0: f64, y1: f64) -> Option<f64> {
    let swapped: Vec<(f64, f64)> = pts.iter().map(|p| (p.1, p.0)).collect();
    cross_y(&swapped, x, y0, y1)
}

/// Edge label of a line given as page points, below the frame (above it when
/// `flip`, or when the line doesn't reach the bottom edge).
fn meridian_label(out: &mut String, at: &Placement, slot: &str, pts: &[(f64, f64)], text: &str, flip: bool) {
    let pt = at.slot_pt(slot, GRID_LABEL_PT);
    let (y_bot, y_top) = (at.y + at.h, at.y);
    let bottom = cross_y(pts, y_bot, at.x, at.x + at.w).map(|x| (x, y_bot + 0.6 + pt * PT_MM * 0.9));
    let top = cross_y(pts, y_top, at.x, at.x + at.w).map(|x| (x, y_top - 0.8));
    if let Some((x, y)) = if flip { top.or(bottom) } else { bottom.or(top) } {
        label(out, at, slot, x, y, "middle", text);
    }
}

/// Edge label of a line, left of the frame (right of it when `flip`, or when
/// the line doesn't reach the left edge).
fn parallel_label(out: &mut String, at: &Placement, slot: &str, pts: &[(f64, f64)], text: &str, flip: bool) {
    let (x_left, x_right) = (at.x, at.x + at.w);
    let dy = at.slot_pt(slot, GRID_LABEL_PT) * PT_MM * 0.35;
    let left = cross_x(pts, x_left, at.y, at.y + at.h).map(|y| (x_left - 0.8, y + dy, "end"));
    let right = cross_x(pts, x_right, at.y, at.y + at.h).map(|y| (x_right + 0.8, y + dy, "start"));
    if let Some((x, y, anchor)) = if flip { right.or(left) } else { left.or(right) } {
        label(out, at, slot, x, y, anchor, text);
    }
}

/// Split a sampled line where a point failed to transform.
fn segments(pts: Vec<Option<(f64, f64)>>) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = vec![Vec::new()];
    for p in pts {
        match p {
            Some(p) => out.last_mut().unwrap().push(p),
            None if !out.last().unwrap().is_empty() => out.push(Vec::new()),
            None => {}
        }
    }
    out.retain(|s| s.len() > 1);
    out
}

/// Short marks where `pts` meets the frame's four edges, pointing into the frame.
fn edge_ticks(pts: &[(f64, f64)], at: &Placement, len: f64) -> String {
    let mut d = String::new();
    let (x0, x1, y0, y1) = (at.x, at.x + at.w, at.y, at.y + at.h);
    if let Some(x) = cross_y(pts, y1, x0, x1) {
        svg::cmd(&mut d, 'M', x, y1);
        svg::cmd(&mut d, 'L', x, y1 - len);
    }
    if let Some(x) = cross_y(pts, y0, x0, x1) {
        svg::cmd(&mut d, 'M', x, y0);
        svg::cmd(&mut d, 'L', x, y0 + len);
    }
    if let Some(y) = cross_x(pts, x0, y0, y1) {
        svg::cmd(&mut d, 'M', x0, y);
        svg::cmd(&mut d, 'L', x0 + len, y);
    }
    if let Some(y) = cross_x(pts, x1, y0, y1) {
        svg::cmd(&mut d, 'M', x1, y);
        svg::cmd(&mut d, 'L', x1 - len, y);
    }
    d
}

/// Stroke attributes of a line set. Ticks and crosses are never dashed.
pub fn line_attrs(spec: &GridSpec, default_color: &str, default_dashed: bool) -> String {
    let dashed = spec.dashed.unwrap_or(default_dashed) && spec.style == GridStyle::Lines;
    format!(
        "stroke=\"{}\" stroke-width=\"{}\"{}",
        svg::color(spec.color.as_deref().unwrap_or(""), default_color),
        num(spec.width.unwrap_or(0.15)),
        if dashed { " stroke-dasharray=\"1.2 0.8\"" } else { "" }
    )
}

/// One `<path>` for a line in the chosen style (nothing for crosses, which are drawn at the nodes).
fn line_path(out: &mut String, id: &str, pts: &[(f64, f64)], spec: &GridSpec, at: &Placement) {
    let d = match spec.style {
        GridStyle::Lines => path(pts),
        GridStyle::Ticks => edge_ticks(pts, at, spec.tick_length.unwrap_or(TICK_MM)),
        GridStyle::Crosses => return,
    };
    if !d.is_empty() {
        let _ = write!(out, "<path id=\"{id}\" d=\"{d}\"/>");
    }
}

/// A small cross at a page point when it lies inside the frame.
fn cross_mark(d: &mut String, at: &Placement, p: (f64, f64), arm: f64) {
    if p.0 < at.x || p.0 > at.x + at.w || p.1 < at.y || p.1 > at.y + at.h {
        return;
    }
    svg::cmd(d, 'M', p.0 - arm, p.1);
    svg::cmd(d, 'L', p.0 + arm, p.1);
    svg::cmd(d, 'M', p.0, p.1 - arm);
    svg::cmd(d, 'L', p.0, p.1 + arm);
}

/// Degrees, minutes and seconds, as many parts as the interval needs (45°30′ for half degrees, 45°30′15″ for 15″).
pub fn dms_label(v: f64, pos: char, neg: char, step_deg: f64) -> String {
    let step_s = step_deg * 3600.0;
    let frac = (step_s - step_s.round()).abs() > 1e-6 || step_s < 1.0;
    let total = (v.abs() * 3600.0 * if frac { 10.0 } else { 1.0 }).round() / if frac { 10.0 } else { 1.0 };
    let (d, m, sec) = ((total / 3600.0).floor() as i64, ((total % 3600.0) / 60.0).floor() as i64, total % 60.0);
    let hemi = if v < 0.0 { neg } else { pos };
    if (step_s % 3600.0).abs() < 1e-6 || (step_s % 3600.0 - 3600.0).abs() < 1e-6 {
        format!("{d}°{hemi}")
    } else if (step_s % 60.0).abs() < 1e-6 || (step_s % 60.0 - 60.0).abs() < 1e-6 {
        format!("{d}°{m:02}′{hemi}")
    } else if frac {
        format!("{d}°{m:02}′{sec:04.1}″{hemi}")
    } else {
        format!("{d}°{m:02}′{:02}″{hemi}", sec.round() as i64)
    }
}

/// A grid coordinate as the label format asks. `value` is in CRS units.
fn grid_text(value: f64, step: f64, unit_m: f64, fmt: LabelFormat) -> String {
    let metres = value * unit_m;
    let plain = |v: f64, st: f64| format!("{:.*}", decimals(st), v);
    match fmt {
        LabelFormat::Km => plain(metres / 1000.0, step * unit_m / 1000.0),
        LabelFormat::Short if step * unit_m >= 999.9 => format!("{:02}", ((metres / 1000.0).round() as i64).rem_euclid(100)),
        _ => plain(value, step),
    }
}

const LAT_BANDS: &[u8] = b"CDEFGHJKLMNPQRSTUVWX";

/// "15T WK": UTM zone, latitude band and 100 km square letters (MGRS lettering) of a UTM point; None outside 80°S–84°N.
pub fn mgrs_designation(zone: u8, south: bool, e: f64, n: f64, lat: f64) -> Option<String> {
    if !(-80.0..=84.0).contains(&lat) || !(100_000.0..900_000.0).contains(&e) {
        return None;
    }
    let band = LAT_BANDS[(((lat + 80.0) / 8.0).floor() as usize).min(LAT_BANDS.len() - 1)] as char;
    let set = ["ABCDEFGH", "JKLMNPQR", "STUVWXYZ"][((zone as usize) - 1) % 3].as_bytes();
    let col = set[(e / 100_000.0).floor() as usize - 1] as char;
    let n = if south { n - 10_000_000.0 } else { n };
    let row = b"ABCDEFGHJKLMNPQRSTUV"[(((n / 100_000.0).floor() as i64 + if zone % 2 == 0 { 5 } else { 0 }).rem_euclid(20)) as usize] as char;
    Some(format!("{zone}{band} {col}{row}"))
}

fn graticule_text(body: &Body, v: f64, lon: bool, step: f64, d: usize, fmt: LabelFormat) -> String {
    match fmt {
        LabelFormat::Dms => if lon { dms_label(((v + 180.0).rem_euclid(360.0)) - 180.0, 'E', 'W', step) } else { dms_label(v, 'N', 'S', step) },
        LabelFormat::Decimal => format!("{:.*}°", d, if lon { ((v + 180.0).rem_euclid(360.0)) - 180.0 } else { v }),
        _ => if lon { body.format_lon(v, d) } else { body.format_lat(v, d) },
    }
}

/// Lines of longitude and latitude through the frame.
pub fn graticule(body: &Body, crs: &FrameCrs, at: &Placement, extent: [f64; 4], spec: &GridSpec, to_page: ToPage, warnings: &mut Vec<String>) -> GridOutput {
    let mut out = GridOutput { lines: String::new(), sub: String::new(), labels: String::new() };
    // Longitude/latitude range of the frame, from a lattice over it.
    let mut ll = Vec::new();
    for i in 0..=16 {
        for j in 0..=16 {
            let (x, y) = (extent[0] + (extent[2] - extent[0]) * i as f64 / 16.0, extent[1] + (extent[3] - extent[1]) * j as f64 / 16.0);
            if let Ok(p) = crs.to_lonlat(x, y) {
                if p.0.is_finite() && p.1.is_finite() {
                    ll.push(p);
                }
            }
        }
    }
    if ll.len() < 17 {
        warnings.push(format!("Frame '{}': graticule not drawn, the frame is mostly outside the coordinate system's valid area", at.id));
        return out;
    }
    let f = |m: fn(f64, f64) -> f64, init: f64, k: fn(&(f64, f64)) -> f64| ll.iter().map(k).fold(init, m);
    let (lon_min, lon_max) = (f(f64::min, f64::INFINITY, |p| p.0), f(f64::max, f64::NEG_INFINITY, |p| p.0));
    let (lat_min, lat_max) = (f(f64::min, f64::INFINITY, |p| p.1), f(f64::max, f64::NEG_INFINITY, |p| p.1));
    if lon_max - lon_min > 180.0 || lat_max >= 89.999 || lat_min <= -89.999 {
        warnings.push(format!("Frame '{}': graticule not drawn when the frame crosses the antimeridian or contains a pole", at.id));
        return out;
    }
    let step = spec.step().unwrap_or_else(|| auto_degrees((lon_max - lon_min).min(lat_max - lat_min)));
    let (lons, lats) = (multiples(lon_min, lon_max, step), multiples(lat_min, lat_max, step));
    if lons.clone().count() + lats.clone().count() > MAX_LINES {
        warnings.push(format!("Frame '{}': graticule interval {} ° would draw more than {MAX_LINES} lines; use a larger interval", at.id, num(step)));
        return out;
    }
    let d = decimals(step);
    // Lines run a little beyond the frame so they reach its edges when curved.
    let (pad_lon, pad_lat) = ((lon_max - lon_min) * 0.15 + step, (lat_max - lat_min) * 0.15 + step);
    let (la0, la1) = ((lat_min - pad_lat).max(-89.9), (lat_max + pad_lat).min(89.9));
    let (lo0, lo1) = (lon_min - pad_lon, lon_max + pad_lon);
    let sample = |lon: f64, lat: f64| crs.from_lonlat(lon, lat).ok().filter(|p| p.0.is_finite() && p.1.is_finite()).map(|p| to_page(p.0, p.1));
    for k in lons {
        let lon = k as f64 * step;
        let pts: Vec<_> = (0..=SAMPLES).map(|i| sample(lon, la0 + (la1 - la0) * i as f64 / SAMPLES as f64)).collect();
        for (n, seg) in segments(pts).iter().enumerate() {
            line_path(&mut out.lines, &format!("graticule-{}-lon{}{}", esc(&at.id), id_part(lon), if n > 0 { format!("-{n}") } else { String::new() }), seg, spec, at);
            if spec.labels && n == 0 {
                meridian_label(&mut out.labels, at, "map_frame.graticule_labels", seg, &graticule_text(body, lon, true, step, d, spec.label_format), false);
            }
        }
    }
    for k in lats {
        let lat = k as f64 * step;
        let pts: Vec<_> = (0..=SAMPLES).map(|i| sample(lo0 + (lo1 - lo0) * i as f64 / SAMPLES as f64, lat)).collect();
        for (n, seg) in segments(pts).iter().enumerate() {
            line_path(&mut out.lines, &format!("graticule-{}-lat{}{}", esc(&at.id), id_part(lat), if n > 0 { format!("-{n}") } else { String::new() }), seg, spec, at);
            if spec.labels && n == 0 {
                parallel_label(&mut out.labels, at, "map_frame.graticule_labels", seg, &graticule_text(body, lat, false, step, d, spec.label_format), false);
            }
        }
    }
    if spec.style == GridStyle::Crosses {
        let arm = spec.tick_length.unwrap_or(TICK_MM) * 0.6;
        let mut d = String::new();
        for kl in multiples(lon_min, lon_max, step) {
            for ka in multiples(lat_min, lat_max, step) {
                if let Some(p) = sample(kl as f64 * step, ka as f64 * step) {
                    cross_mark(&mut d, at, p, arm);
                }
            }
        }
        if !d.is_empty() {
            let _ = write!(out.lines, "<path id=\"graticule-{}-crosses\" d=\"{d}\"/>", esc(&at.id));
        }
    }
    out
}

/// Straight grid lines at multiples of the interval, in frame-CRS units.
/// `flip`: label the top and right edges, leaving the bottom and left to a graticule.
pub fn grid(crs: &FrameCrs, at: &Placement, extent: [f64; 4], spec: &GridSpec, flip: bool, to_page: ToPage, warnings: &mut Vec<String>) -> GridOutput {
    let mut out = GridOutput { lines: String::new(), sub: String::new(), labels: String::new() };
    if !crs.projected {
        warnings.push(format!("Frame '{}': a grid needs a projected coordinate system; use the graticule for geographic coordinates", at.id));
        return out;
    }
    let step = spec.step().unwrap_or_else(|| nice_floor(((extent[2] - extent[0]).min(extent[3] - extent[1])) / 3.0));
    let (xs, ys) = (multiples(extent[0], extent[2], step), multiples(extent[1], extent[3], step));
    if xs.clone().count() + ys.clone().count() > MAX_LINES {
        warnings.push(format!("Frame '{}': grid interval {} would draw more than {MAX_LINES} lines; use a larger interval", at.id, num(step)));
        return out;
    }
    let d = decimals(step);
    if spec.label_format == LabelFormat::Short && step * crs.unit_m < 999.9 {
        warnings.push(format!("Frame '{}': two-digit grid labels need an interval of 1000 m or more; full numbers are shown", at.id));
    }
    let fmt = |v: f64| grid_text(v, step, crs.unit_m, spec.label_format);
    for k in xs {
        let x = k as f64 * step;
        let pts = [to_page(x, extent[1]), to_page(x, extent[3])];
        line_path(&mut out.lines, &format!("grid-{}-x{}", esc(&at.id), id_part(x)), &pts, spec, at);
        if spec.labels {
            meridian_label(&mut out.labels, at, "map_frame.grid_labels", &pts, &fmt(x), flip);
        }
    }
    for k in ys {
        let y = k as f64 * step;
        let pts = [to_page(extent[0], y), to_page(extent[2], y)];
        line_path(&mut out.lines, &format!("grid-{}-y{}", esc(&at.id), id_part(y)), &pts, spec, at);
        if spec.labels {
            parallel_label(&mut out.labels, at, "map_frame.grid_labels", &pts, &fmt(y), flip);
        }
    }
    if let Some(sub) = spec.sub_interval {
        // Fine lines only between the main ones, and only when they divide the interval evenly.
        let ratio = step / sub;
        if sub >= step || (ratio - ratio.round()).abs() > 1e-6 {
            warnings.push(format!("Frame '{}': the sub-grid interval {} does not divide the grid interval {} evenly and is not drawn", at.id, num(sub), num(step)));
        } else if multiples(extent[0], extent[2], sub).count() + multiples(extent[1], extent[3], sub).count() > MAX_LINES {
            warnings.push(format!("Frame '{}': sub-grid interval {} would draw more than {MAX_LINES} lines; use a larger interval", at.id, num(sub)));
        } else {
            let main = |k: i64| k % (ratio.round() as i64) == 0;
            for k in multiples(extent[0], extent[2], sub).filter(|k| !main(*k)) {
                let x = k as f64 * sub;
                line_path(&mut out.sub, &format!("subgrid-{}-x{}", esc(&at.id), id_part(x)), &[to_page(x, extent[1]), to_page(x, extent[3])], spec, at);
            }
            for k in multiples(extent[1], extent[3], sub).filter(|k| !main(*k)) {
                let y = k as f64 * sub;
                line_path(&mut out.sub, &format!("subgrid-{}-y{}", esc(&at.id), id_part(y)), &[to_page(extent[0], y), to_page(extent[2], y)], spec, at);
            }
        }
    }
    if spec.style == GridStyle::Crosses {
        let arm = spec.tick_length.unwrap_or(TICK_MM) * 0.6;
        let mut d = String::new();
        for kx in multiples(extent[0], extent[2], step) {
            for ky in multiples(extent[1], extent[3], step) {
                cross_mark(&mut d, at, to_page(kx as f64 * step, ky as f64 * step), arm);
            }
        }
        if !d.is_empty() {
            let _ = write!(out.lines, "<path id=\"grid-{}-crosses\" d=\"{d}\"/>", esc(&at.id));
        }
    }
    if spec.designation {
        match (crs.utm_zone(), crs.to_lonlat(extent[0], extent[1])) {
            (Some((zone, south)), Ok((_, lat))) => {
                let (e, n) = (extent[0] * crs.unit_m, extent[1] * crs.unit_m);
                match mgrs_designation(zone, south, e, n, lat) {
                    Some(t) => {
                        let pt = at.slot_pt("map_frame.grid_labels", GRID_LABEL_PT);
                        label(&mut out.labels, at, "map_frame.grid_labels", at.x, at.y + at.h + 0.6 + pt * PT_MM * 2.5, "start", &t);
                    }
                    None => warnings.push(format!("Frame '{}': the grid zone designation is not available at this corner", at.id)),
                }
            }
            _ => warnings.push(format!("Frame '{}': the grid zone designation needs a UTM coordinate system", at.id)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_and_decimals() {
        assert_eq!(auto_degrees(10.0), 2.0);
        assert_eq!(auto_degrees(0.09), 0.02);
        assert_eq!(decimals(0.25), 2);
        assert_eq!(decimals(0.5), 1);
        assert_eq!(decimals(5.0), 0);
        assert_eq!(multiples(-96.4, -94.9, 0.5).collect::<Vec<_>>(), vec![-192, -191, -190]);
    }

    #[test]
    fn dms_labels_show_as_many_parts_as_the_interval_needs() {
        assert_eq!(dms_label(-93.0, 'E', 'W', 1.0), "93°W");
        assert_eq!(dms_label(45.5, 'N', 'S', 0.5), "45°30′N");
        assert_eq!(dms_label(45.25, 'N', 'S', 0.25), "45°15′N");
        assert_eq!(dms_label(-93.0 - 15.0 / 3600.0, 'E', 'W', 15.0 / 3600.0), "93°00′15″W");
        assert_eq!(dms_label(45.0 + 30.0 / 60.0 + 7.5 / 3600.0, 'N', 'S', 2.5 / 3600.0), "45°30′07.5″N");
    }

    #[test]
    fn grid_numbers_in_metres_km_and_two_digit_form() {
        assert_eq!(grid_text(4_985_000.0, 1000.0, 1.0, LabelFormat::Auto), "4985000");
        assert_eq!(grid_text(4_985_000.0, 1000.0, 1.0, LabelFormat::Km), "4985");
        assert_eq!(grid_text(4_985_500.0, 500.0, 1.0, LabelFormat::Km), "4985.5");
        assert_eq!(grid_text(4_985_000.0, 1000.0, 1.0, LabelFormat::Short), "85");
        assert_eq!(grid_text(500_000.0, 1000.0, 1.0, LabelFormat::Short), "00");
        // A finer interval cannot be written as two digits: the full number stays.
        assert_eq!(grid_text(4_985_250.0, 250.0, 1.0, LabelFormat::Short), "4985250");
    }

    #[test]
    fn mgrs_squares_match_the_mgrs_library() {
        // Reference values from the `mgrs` package the status bar uses.
        assert_eq!(mgrs_designation(15, false, 500_000.0, 4_982_950.0, 45.0).unwrap(), "15T WK");
        assert_eq!(mgrs_designation(15, false, 499_976.0, 4_982_994.0, 45.0004).unwrap(), "15T VK");
        assert_eq!(mgrs_designation(11, false, 278_124.0, 4_119_987.0, 37.2).unwrap(), "11S KB");
        assert!(mgrs_designation(15, false, 500_000.0, 9_000_000.0, 85.0).is_none());
    }
}
