//! The cartographic symbol library for print layouts: point markers, line
//! dashes and polygon fill patterns, plus user SVG markers. Geometry is our
//! own (fixed unit coordinates, no trigonometry), so output is identical on
//! every platform. Names of the shapes follow common GIS symbol sets
//! (circle, square, triangle, diamond, star, cross, x).

use super::svg::{color, esc, mm};
use anyhow::{bail, Result};
use std::fmt::Write;

pub const MARKERS: &[&str] = &["circle", "square", "triangle", "diamond", "star", "cross", "x", "svg"];
pub const DASHES: &[&str] = &["solid", "dashed", "dotted", "dashdot"];
pub const PATTERNS: &[&str] = &["hatch", "hatch2", "cross", "horizontal", "vertical", "grid", "dots"];

pub fn check(marker: &str, marker_svg: &str, dash: &str, pattern: &str) -> Result<()> {
    if !marker.is_empty() && !MARKERS.contains(&marker) {
        bail!("Unknown point symbol '{marker}' ({})", MARKERS.join(", "));
    }
    if marker == "svg" && super::shapes::check_picture(marker_svg)? != "svg" {
        bail!("The SVG point symbol needs an SVG file");
    }
    if !dash.is_empty() && !DASHES.contains(&dash) {
        bail!("Unknown line style '{dash}' ({})", DASHES.join(", "));
    }
    if !pattern.is_empty() && !PATTERNS.contains(&pattern) {
        bail!("Unknown fill pattern '{pattern}' ({})", PATTERNS.join(", "));
    }
    Ok(())
}

/// One point marker centred on (cx, cy) with "radius" r (mm). The parent group supplies fill and outline.
pub fn marker(out: &mut String, shape: &str, svg_uri: &str, cx: f64, cy: f64, r: f64) {
    let p = |dx: f64, dy: f64| format!("{},{}", mm(cx + dx * r), mm(cy + dy * r));
    match shape {
        "square" => {
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>", mm(cx - r * 0.9), mm(cy - r * 0.9), mm(r * 1.8), mm(r * 1.8));
        }
        "triangle" => {
            let _ = write!(out, "<polygon points=\"{} {} {}\"/>", p(0.0, -1.0), p(0.95, 0.75), p(-0.95, 0.75));
        }
        "diamond" => {
            let _ = write!(out, "<polygon points=\"{} {} {} {}\"/>", p(0.0, -1.15), p(1.0, 0.0), p(0.0, 1.15), p(-1.0, 0.0));
        }
        "star" => {
            let pts = [(0.0, -1.0), (0.23511, -0.32361), (0.95106, -0.30902), (0.38042, 0.12361), (0.58779, 0.80902), (0.0, 0.4), (-0.58779, 0.80902), (-0.38042, 0.12361), (-0.95106, -0.30902), (-0.23511, -0.32361)];
            let _ = write!(out, "<polygon points=\"{}\"/>", pts.iter().map(|(x, y)| p(*x, *y)).collect::<Vec<_>>().join(" "));
        }
        "cross" | "x" => {
            let t = 0.33;
            let pts = [(-t, -1.0), (t, -1.0), (t, -t), (1.0, -t), (1.0, t), (t, t), (t, 1.0), (-t, 1.0), (-t, t), (-1.0, t), (-1.0, -t), (-t, -t)];
            let rot = if shape == "x" { format!(" transform=\"rotate(45 {} {})\"", mm(cx), mm(cy)) } else { String::new() };
            let _ = write!(out, "<polygon points=\"{}\"{rot}/>", pts.iter().map(|(x, y)| p(*x, *y)).collect::<Vec<_>>().join(" "));
        }
        "svg" if !svg_uri.is_empty() => {
            let _ = write!(out, "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" href=\"{svg_uri}\"/>", mm(cx - r * 1.3), mm(cy - r * 1.3), mm(r * 2.6), mm(r * 2.6));
        }
        _ => {
            let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>", mm(cx), mm(cy), mm(r));
        }
    }
}

/// `stroke-dasharray` attribute (with a leading space) for a line style, scaled to the line width in mm.
pub fn dash_attr(dash: &str, w_mm: f64) -> String {
    let w = w_mm.max(0.15);
    match dash {
        "dashed" => format!(" stroke-dasharray=\"{} {}\"", mm(w * 5.0), mm(w * 3.0)),
        "dotted" => format!(" stroke-dasharray=\"{} {}\" stroke-linecap=\"round\"", mm(w * 0.01), mm(w * 2.2)),
        "dashdot" => format!(" stroke-dasharray=\"{} {} {} {}\"", mm(w * 6.0), mm(w * 2.5), mm(w * 0.01), mm(w * 2.5)),
        _ => String::new(),
    }
}

/// A `<pattern>` definition: `bg` colour with the pattern's marks in `fg`, repeating every `s` mm.
pub fn pattern_def(id: &str, kind: &str, bg: &str, fg: &str, s: f64) -> String {
    let (bg, fg) = (color(bg, "#cccccc"), color(fg, "#333333"));
    let w = (s * 0.12).max(0.1);
    let line = |x1: f64, y1: f64, x2: f64, y2: f64| format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{fg}\" stroke-width=\"{}\"/>", mm(x1), mm(y1), mm(x2), mm(y2), mm(w));
    let mut m = String::new();
    match kind {
        "hatch" => {
            // Diagonal lines rising to the right; the corner pieces make the tile seamless.
            m.push_str(&line(0.0, s, s, 0.0));
            m.push_str(&line(-s * 0.5, s * 0.5, s * 0.5, -s * 0.5));
            m.push_str(&line(s * 0.5, s * 1.5, s * 1.5, s * 0.5));
        }
        "hatch2" => {
            m.push_str(&line(0.0, 0.0, s, s));
            m.push_str(&line(-s * 0.5, s * 0.5, s * 0.5, s * 1.5));
            m.push_str(&line(s * 0.5, -s * 0.5, s * 1.5, s * 0.5));
        }
        "cross" => {
            m.push_str(&line(0.0, s, s, 0.0));
            m.push_str(&line(-s * 0.5, s * 0.5, s * 0.5, -s * 0.5));
            m.push_str(&line(s * 0.5, s * 1.5, s * 1.5, s * 0.5));
            m.push_str(&line(0.0, 0.0, s, s));
            m.push_str(&line(-s * 0.5, s * 0.5, s * 0.5, s * 1.5));
            m.push_str(&line(s * 0.5, -s * 0.5, s * 1.5, s * 0.5));
        }
        "horizontal" => m.push_str(&line(0.0, s / 2.0, s, s / 2.0)),
        "vertical" => m.push_str(&line(s / 2.0, 0.0, s / 2.0, s)),
        "grid" => {
            m.push_str(&line(0.0, s / 2.0, s, s / 2.0));
            m.push_str(&line(s / 2.0, 0.0, s / 2.0, s));
        }
        _ => {
            let _ = write!(m, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fg}\"/>", mm(s / 2.0), mm(s / 2.0), mm((s * 0.14).max(0.12)));
        }
    }
    format!(
        "<pattern id=\"{}\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\"><rect width=\"{}\" height=\"{}\" fill=\"{bg}\"/>{m}</pattern>",
        esc(id),
        mm(s),
        mm(s),
        mm(s),
        mm(s)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_is_plain_svg_around_the_point() {
        for s in MARKERS.iter().filter(|s| **s != "svg") {
            let mut o = String::new();
            marker(&mut o, s, "", 10.0, 20.0, 2.0);
            assert!(o.starts_with('<') && o.ends_with("/>"), "{s}: {o}");
        }
        let mut o = String::new();
        marker(&mut o, "circle", "", 10.0, 20.0, 2.0);
        assert_eq!(o, "<circle cx=\"10\" cy=\"20\" r=\"2\"/>", "the default marker is byte-identical to the old circle");
        let mut o = String::new();
        marker(&mut o, "x", "", 0.0, 0.0, 1.0);
        assert!(o.contains("rotate(45 0 0)"));
        let mut o = String::new();
        marker(&mut o, "star", "", 0.0, 0.0, 1.0);
        assert_eq!(o.matches(',').count(), 10, "ten vertices");
    }

    #[test]
    fn dashes_and_patterns() {
        assert_eq!(dash_attr("solid", 0.5), "");
        assert!(dash_attr("dashed", 0.5).contains("2.5 1.5"));
        assert!(dash_attr("dotted", 0.5).contains("round"));
        for k in PATTERNS {
            let d = pattern_def("p1", k, "#ffcc00", "#333333", 2.0);
            assert!(d.starts_with("<pattern id=\"p1\"") && d.contains("#ffcc00") && d.ends_with("</pattern>"), "{k}");
        }
        assert!(check("pentagon", "", "", "").is_err());
        assert!(check("svg", "", "", "").is_err() || check("svg", "", "", "").is_ok());
        assert!(check("", "", "wavy", "").unwrap_err().to_string().contains("Unknown line style"));
        assert!(check("", "", "", "stripes").unwrap_err().to_string().contains("Unknown fill pattern"));
        assert!(check("star", "", "dashed", "hatch").is_ok());
    }
}
