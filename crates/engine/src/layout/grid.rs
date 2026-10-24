//! Graticule and projected grid of a map frame (docs/LAYOUTS_PLAN.md C2.4).
//! The graticule is lines of constant longitude and latitude, sampled along
//! their length and transformed to the frame CRS, so they curve as the
//! projection dictates; labels come from [`Body`] (no hemisphere letters are
//! assumed here). The grid is straight lines at multiples of an interval in
//! frame-CRS units.

use super::geo::{Body, FrameCrs};
use super::model::{GridSpec, Placement};
use super::surround::{nice_floor, num};
use super::svg::{self, esc, mm};
use std::fmt::Write;

/// Lines drawn per set before we refuse (a typo'd interval would draw millions).
const MAX_LINES: usize = 200;
const SAMPLES: usize = 64;
const LABEL_PT: f64 = 6.0;
const PT_MM: f64 = 25.4 / 72.0;

pub struct GridOutput {
    /// Lines, drawn inside the frame's clip.
    pub lines: String,
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

fn label(out: &mut String, x: f64, y: f64, anchor: &str, text: &str) {
    let _ = write!(out, "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" fill=\"#000000\" text-anchor=\"{anchor}\">{}</text>", mm(x), mm(y), super::export::font_family(), mm(LABEL_PT * PT_MM), esc(text));
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
fn meridian_label(out: &mut String, at: &Placement, pts: &[(f64, f64)], text: &str, flip: bool) {
    let (y_bot, y_top) = (at.y + at.h, at.y);
    let bottom = cross_y(pts, y_bot, at.x, at.x + at.w).map(|x| (x, y_bot + 0.6 + LABEL_PT * PT_MM * 0.9));
    let top = cross_y(pts, y_top, at.x, at.x + at.w).map(|x| (x, y_top - 0.8));
    if let Some((x, y)) = if flip { top.or(bottom) } else { bottom.or(top) } {
        label(out, x, y, "middle", text);
    }
}

/// Edge label of a line, left of the frame (right of it when `flip`, or when
/// the line doesn't reach the left edge).
fn parallel_label(out: &mut String, at: &Placement, pts: &[(f64, f64)], text: &str, flip: bool) {
    let (x_left, x_right) = (at.x, at.x + at.w);
    let dy = LABEL_PT * PT_MM * 0.35;
    let left = cross_x(pts, x_left, at.y, at.y + at.h).map(|y| (x_left - 0.8, y + dy, "end"));
    let right = cross_x(pts, x_right, at.y, at.y + at.h).map(|y| (x_right + 0.8, y + dy, "start"));
    if let Some((x, y, anchor)) = if flip { right.or(left) } else { left.or(right) } {
        label(out, x, y, anchor, text);
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

/// Lines of longitude and latitude through the frame.
pub fn graticule(body: &Body, crs: &FrameCrs, at: &Placement, extent: [f64; 4], spec: &GridSpec, to_page: ToPage, warnings: &mut Vec<String>) -> GridOutput {
    let mut out = GridOutput { lines: String::new(), labels: String::new() };
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
            let _ = write!(out.lines, "<path id=\"graticule-{}-lon{}{}\" d=\"{}\"/>", esc(&at.id), id_part(lon), if n > 0 { format!("-{n}") } else { String::new() }, path(seg));
            if spec.labels && n == 0 {
                meridian_label(&mut out.labels, at, seg, &body.format_lon(lon, d), false);
            }
        }
    }
    for k in lats {
        let lat = k as f64 * step;
        let pts: Vec<_> = (0..=SAMPLES).map(|i| sample(lo0 + (lo1 - lo0) * i as f64 / SAMPLES as f64, lat)).collect();
        for (n, seg) in segments(pts).iter().enumerate() {
            let _ = write!(out.lines, "<path id=\"graticule-{}-lat{}{}\" d=\"{}\"/>", esc(&at.id), id_part(lat), if n > 0 { format!("-{n}") } else { String::new() }, path(seg));
            if spec.labels && n == 0 {
                parallel_label(&mut out.labels, at, seg, &body.format_lat(lat, d), false);
            }
        }
    }
    out
}

/// Straight grid lines at multiples of the interval, in frame-CRS units.
/// `flip`: label the top and right edges, leaving the bottom and left to a graticule.
pub fn grid(crs: &FrameCrs, at: &Placement, extent: [f64; 4], spec: &GridSpec, flip: bool, to_page: ToPage, warnings: &mut Vec<String>) -> GridOutput {
    let mut out = GridOutput { lines: String::new(), labels: String::new() };
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
    let fmt = |v: f64| format!("{:.*}", d, v);
    for k in xs {
        let x = k as f64 * step;
        let pts = [to_page(x, extent[1]), to_page(x, extent[3])];
        let _ = write!(out.lines, "<path id=\"grid-{}-x{}\" d=\"{}\"/>", esc(&at.id), id_part(x), path(&pts));
        if spec.labels {
            meridian_label(&mut out.labels, at, &pts, &fmt(x), flip);
        }
    }
    for k in ys {
        let y = k as f64 * step;
        let pts = [to_page(extent[0], y), to_page(extent[2], y)];
        let _ = write!(out.lines, "<path id=\"grid-{}-y{}\" d=\"{}\"/>", esc(&at.id), id_part(y), path(&pts));
        if spec.labels {
            parallel_label(&mut out.labels, at, &pts, &fmt(y), flip);
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
}
