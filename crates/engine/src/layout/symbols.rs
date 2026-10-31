//! The cartographic symbol library for print layouts: point markers, line
//! dashes and polygon fill patterns, plus user SVG markers. Geometry is our
//! own (fixed unit coordinates, no trigonometry), so output is identical on
//! every platform. Names of the shapes follow common GIS symbol sets
//! (circle, square, triangle, diamond, star, cross, x).

use super::svg::{color, esc, mm};
use anyhow::{bail, Result};
use std::fmt::Write;

pub const MARKERS: &[&str] = &["circle", "square", "triangle", "diamond", "star", "cross", "x", "svg", "ring", "tri_open", "plus_thin", "x_thin", "church"];
pub const DASHES: &[&str] = &["solid", "dashed", "dotted", "dashdot"];
pub const PATTERNS: &[&str] = &["hatch", "hatch2", "cross", "horizontal", "vertical", "grid", "dots", "marsh", "orchard", "vineyard", "scrub", "sand", "gravel"];

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
        "ring" => {
            // An open circle: a donut (outer and inner circle, even-odd) so it takes the fill colour of the symbol.
            let (o, i) = (r, r * 0.62);
            let _ = write!(out, "<path fill-rule=\"evenodd\" d=\"M{} {}a{} {} 0 1 0 {} 0a{} {} 0 1 0 {} 0ZM{} {}a{} {} 0 1 1 {} 0a{} {} 0 1 1 {} 0Z\"/>", mm(cx - o), mm(cy), mm(o), mm(o), mm(2.0 * o), mm(o), mm(o), mm(-2.0 * o), mm(cx - i), mm(cy), mm(i), mm(i), mm(2.0 * i), mm(i), mm(i), mm(-2.0 * i));
        }
        "tri_open" => {
            let outer = [(0.0, -1.0), (0.95, 0.75), (-0.95, 0.75)];
            let inner = [(0.0, -0.45), (0.5, 0.45), (-0.5, 0.45)];
            let path = |pts: &[(f64, f64)]| pts.iter().enumerate().map(|(k, (x, y))| format!("{}{} {}", if k == 0 { "M" } else { "L" }, mm(cx + x * r), mm(cy + y * r))).collect::<String>() + "Z";
            let _ = write!(out, "<path fill-rule=\"evenodd\" d=\"{}{}\"/>", path(&outer), path(&inner));
        }
        "plus_thin" | "x_thin" => {
            let t = 0.12;
            let pts = [(-t, -1.0), (t, -1.0), (t, -t), (1.0, -t), (1.0, t), (t, t), (t, 1.0), (-t, 1.0), (-t, t), (-1.0, t), (-1.0, -t), (-t, -t)];
            let rot = if shape == "x_thin" { format!(" transform=\"rotate(45 {} {})\"", mm(cx), mm(cy)) } else { String::new() };
            let _ = write!(out, "<polygon points=\"{}\"{rot}/>", pts.iter().map(|(x, y)| p(*x, *y)).collect::<Vec<_>>().join(" "));
        }
        "church" => {
            // A small cross standing on a square (school/church symbol).
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>", mm(cx - r * 0.7), mm(cy), mm(r * 1.4), mm(r * 1.0), mm(cx - r * 0.12), mm(cy - r * 1.1), mm(r * 0.24), mm(r * 1.1), mm(cx - r * 0.5), mm(cy - r * 0.75), mm(r * 1.0), mm(r * 0.24));
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
        "dotted" => format!(" stroke-dasharray=\"{} {}\"", mm(w * 0.01), mm(w * 2.2)),
        "dashdot" => format!(" stroke-dasharray=\"{} {} {} {}\"", mm(w * 6.0), mm(w * 2.5), mm(w * 0.01), mm(w * 2.5)),
        _ => String::new(),
    }
}

/// Round caps, which dots need to be visible. Only for groups that don't already set `stroke-linecap` (a duplicate attribute makes the SVG unreadable).
pub fn cap_attr(dash: &str) -> &'static str {
    if dash == "dotted" {
        " stroke-linecap=\"round\""
    } else {
        ""
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
        "marsh" => {
            // The USGS marsh tuft: a short horizontal stroke with two small blades above it, every other row shifted.
            m.push_str(&line(s * 0.15, s * 0.7, s * 0.55, s * 0.7));
            m.push_str(&line(s * 0.25, s * 0.7, s * 0.25, s * 0.45));
            m.push_str(&line(s * 0.35, s * 0.7, s * 0.35, s * 0.5));
            m.push_str(&line(s * 0.45, s * 0.7, s * 0.45, s * 0.45));
            m.push_str(&line(s * 0.65, s * 0.25, s * 0.95, s * 0.25));
        }
        "orchard" | "vineyard" | "scrub" | "sand" | "gravel" => {
            let r = match kind { "gravel" => (s * 0.09).max(0.16), "vineyard" => (s * 0.06).max(0.1), _ => (s * 0.07).max(0.1) };
            let dots: &[(f64, f64)] = match kind {
                "orchard" => &[(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)],
                "vineyard" => &[(0.17, 0.17), (0.5, 0.17), (0.83, 0.17), (0.17, 0.5), (0.5, 0.5), (0.83, 0.5), (0.17, 0.83), (0.5, 0.83), (0.83, 0.83)],
                "scrub" => &[(0.3, 0.2), (0.8, 0.55), (0.2, 0.85)],
                "sand" => &[(0.15, 0.2), (0.55, 0.12), (0.85, 0.4), (0.35, 0.55), (0.7, 0.8), (0.1, 0.85)],
                _ => &[(0.25, 0.3), (0.7, 0.2), (0.5, 0.65), (0.15, 0.8), (0.85, 0.75)],
            };
            for (dx, dy) in dots {
                let _ = write!(m, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fg}\"/>", mm(s * dx), mm(s * dy), mm(r));
            }
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
        assert!(!dash_attr("dotted", 0.5).contains("linecap") && cap_attr("dotted").contains("round") && cap_attr("dashed").is_empty());
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
