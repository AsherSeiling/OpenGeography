//! Shapes, pictures and element decoration (frame, background, drop shadow).
//! Names follow common print-layout tools (QGIS Print Layout "Add Shape" /
//! "Add Picture" and the item frame/background/shadow settings; ArcGIS Pro
//! Layouts graphics). Everything is plain SVG, so PDF/PNG/SVG output agree.

use super::model::{Decor, Placement};
use super::svg::{color, esc, mm, opacity};
use anyhow::{bail, Result};
use std::fmt::Write;

/// Largest embedded picture (the data URI is stored in the layout), bytes of the encoded text.
pub const MAX_PICTURE: usize = 12 * 1024 * 1024;

fn rotation(at: &Placement) -> String {
    if at.rotation != 0.0 {
        format!(" transform=\"rotate({} {} {})\"", mm(at.rotation), mm(at.x + at.w / 2.0), mm(at.y + at.h / 2.0))
    } else {
        String::new()
    }
}

/// Background, frame and drop shadow drawn under an element.
pub fn decor(out: &mut String, at: &Placement, d: &Decor) {
    let r = d.corner_radius.max(0.0).min(at.w.min(at.h) / 2.0);
    let rx = if r > 0.0 { format!(" rx=\"{}\"", mm(r)) } else { String::new() };
    let _ = write!(out, "<g id=\"decor-{}\"{}>", esc(&at.id), rotation(at));
    if d.shadow {
        let (dx, dy) = (d.shadow_offset.max(0.0), d.shadow_offset.max(0.0));
        let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"#000000\" fill-opacity=\"0.3\"/>", mm(at.x + dx), mm(at.y + dy), mm(at.w), mm(at.h));
    }
    let fill = d.background.as_deref().map_or("none".to_string(), |c| color(c, "#ffffff"));
    let (stroke, sw) = match (&d.frame_color, d.frame_width > 0.0) {
        (Some(c), true) => (color(c, "#000000"), mm(d.frame_width)),
        _ => ("none".to_string(), "0".to_string()),
    };
    if fill != "none" || stroke != "none" {
        let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h));
    }
    out.push_str("</g>");
}

fn dash(style: &str, w: f64) -> String {
    match style {
        "dashed" => format!(" stroke-dasharray=\"{} {}\"", mm(w * 4.0), mm(w * 2.5)),
        "dotted" => format!(" stroke-dasharray=\"{} {}\" stroke-linecap=\"round\"", mm(w * 0.01), mm(w * 2.0)),
        _ => String::new(),
    }
}

pub struct ShapeStyle<'a> {
    pub shape: &'a str,
    pub fill: &'a str,
    pub fill_opacity: f64,
    pub stroke: &'a str,
    pub stroke_width: f64,
    pub dash: &'a str,
    pub corner_radius: f64,
}

pub fn check_shape(shape: &str) -> Result<()> {
    if !matches!(shape, "rectangle" | "ellipse" | "triangle" | "line" | "arrow") {
        bail!("Unknown shape '{shape}' (rectangle, ellipse, triangle, line or arrow)");
    }
    Ok(())
}

/// A shape filling the element's box. Lines and arrows run along the box's
/// horizontal centre line (use rotation for other directions).
pub fn shape(out: &mut String, at: &Placement, s: &ShapeStyle) {
    let fill = color(s.fill, "none");
    let stroke = color(s.stroke, "#000000");
    let sw = s.stroke_width.max(0.0);
    let (x, y, w, h) = (at.x, at.y, at.w, at.h);
    let paint = format!("fill=\"{fill}\" fill-opacity=\"{}\" stroke=\"{stroke}\" stroke-width=\"{}\"{}", opacity(s.fill_opacity), mm(sw), dash(s.dash, sw));
    let _ = write!(out, "<g id=\"shape-{}\"{}>", esc(&at.id), rotation(at));
    match s.shape {
        "ellipse" => {
            let _ = write!(out, "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {paint}/>", mm(x + w / 2.0), mm(y + h / 2.0), mm(w / 2.0), mm(h / 2.0));
        }
        "triangle" => {
            let _ = write!(out, "<polygon points=\"{},{} {},{} {},{}\" {paint} stroke-linejoin=\"round\"/>", mm(x + w / 2.0), mm(y), mm(x + w), mm(y + h), mm(x), mm(y + h));
        }
        "line" | "arrow" => {
            let ym = y + h / 2.0;
            let sw = sw.max(0.1);
            if s.shape == "arrow" {
                // Head length 4 line widths (at least 2 mm), never longer than half the line.
                let head = (sw * 4.0).max(2.0).min(w / 2.0);
                let half = (head * 0.4).max(sw);
                let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{stroke}\" stroke-width=\"{}\"{}/>", mm(x), mm(ym), mm(x + w - head), mm(ym), mm(sw), dash(s.dash, sw));
                let _ = write!(out, "<polygon points=\"{},{} {},{} {},{}\" fill=\"{stroke}\"/>", mm(x + w), mm(ym), mm(x + w - head), mm(ym - half), mm(x + w - head), mm(ym + half));
            } else {
                let _ = write!(out, "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{stroke}\" stroke-width=\"{}\"{}/>", mm(x), mm(ym), mm(x + w), mm(ym), mm(sw), dash(s.dash, sw));
            }
        }
        _ => {
            let r = s.corner_radius.max(0.0).min(w.min(h) / 2.0);
            let rx = if r > 0.0 { format!(" rx=\"{}\"", mm(r)) } else { String::new() };
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} {paint}/>", mm(x), mm(y), mm(w), mm(h));
        }
    }
    out.push_str("</g>");
}

/// Read a PNG, JPEG or SVG file into the `data:` URI a Picture element stores. The type comes from the file's contents, not its name.
pub fn read_picture(path: &std::path::Path) -> Result<String> {
    use base64::Engine as _;
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("Could not read {}: {e}", path.display()))?;
    if bytes.len() > MAX_PICTURE / 4 * 3 {
        bail!("{} is larger than {} MB; resize it first", path.display(), MAX_PICTURE / 1024 / 1024 * 3 / 4);
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).to_ascii_lowercase();
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if head.contains("<svg") {
        "image/svg+xml"
    } else {
        bail!("{} is not a PNG, JPEG or SVG picture", path.display());
    };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// The media type of an embedded picture, or an error saying what is accepted.
pub fn check_picture(source: &str) -> Result<&'static str> {
    if source.is_empty() {
        return Ok("");
    }
    if source.len() > MAX_PICTURE {
        bail!("The picture is larger than {} MB; resize it first", MAX_PICTURE / 1024 / 1024);
    }
    for (prefix, kind) in [("data:image/png;base64,", "png"), ("data:image/jpeg;base64,", "jpeg"), ("data:image/svg+xml;base64,", "svg")] {
        if source.starts_with(prefix) {
            let b = &source[prefix.len()..];
            if b.is_empty() || !b.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')) {
                bail!("The picture data is not valid base64");
            }
            return Ok(kind);
        }
    }
    bail!("Pictures must be PNG, JPEG or SVG files")
}

/// `fit`: "contain" (whole picture inside the box), "cover" (fills the box, cropped) or "stretch".
pub fn picture(out: &mut String, warnings: &mut Vec<String>, at: &Placement, source: &str, fit: &str, alpha: f64) {
    let _ = write!(out, "<g id=\"picture-{}\"{}>", esc(&at.id), rotation(at));
    match check_picture(source) {
        Ok("") => {
            // Empty frame: a dashed placeholder so the element is findable; not drawn in exports of the final page? It is part of the page, so say so.
            warnings.push(format!("Picture '{}' has no image yet", at.id));
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f0f0f0\" stroke=\"#999999\" stroke-width=\"0.25\" stroke-dasharray=\"2 1\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h));
        }
        Ok(_) => {
            let par = match fit {
                "cover" => "xMidYMid slice",
                "stretch" => "none",
                _ => "xMidYMid meet",
            };
            let id = format!("picclip-{}", esc(&at.id));
            let _ = write!(
                out,
                "<clipPath id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath><g clip-path=\"url(#{id})\"><image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{par}\" opacity=\"{}\" href=\"{source}\"/></g>",
                mm(at.x),
                mm(at.y),
                mm(at.w),
                mm(at.h),
                mm(at.x),
                mm(at.y),
                mm(at.w),
                mm(at.h),
                opacity(alpha)
            );
        }
        Err(e) => warnings.push(format!("Picture '{}': {e}", at.id)),
    }
    out.push_str("</g>");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> Placement {
        Placement { id: "s1".into(), x: 10.0, y: 20.0, w: 40.0, h: 20.0, rotation: 0.0, locked: false, name: String::new(), opacity: None, text_styles: Default::default(), visible: true, decor: None, group: None }
    }

    fn style(shape: &str) -> ShapeStyle<'_> {
        ShapeStyle { shape, fill: "#ff0000", fill_opacity: 1.0, stroke: "#000000", stroke_width: 0.5, dash: "solid", corner_radius: 0.0 }
    }

    #[test]
    fn each_shape_is_plain_svg_inside_its_box() {
        for (s, tag) in [("rectangle", "<rect"), ("ellipse", "<ellipse"), ("triangle", "<polygon"), ("line", "<line"), ("arrow", "<polygon")] {
            let mut o = String::new();
            shape(&mut o, &at(), &style(s));
            assert!(o.contains(tag) && o.starts_with("<g id=\"shape-s1\"") && o.ends_with("</g>"), "{s}: {o}");
        }
        assert!(check_shape("star").unwrap_err().to_string().contains("Unknown shape"));
        let mut o = String::new();
        shape(&mut o, &at(), &ShapeStyle { dash: "dashed", ..style("line") });
        assert!(o.contains("stroke-dasharray"));
        let mut o = String::new();
        shape(&mut o, &Placement { rotation: 30.0, ..at() }, &style("rectangle"));
        assert!(o.contains("rotate(30 30 30)"), "{o}");
    }

    #[test]
    fn decor_draws_shadow_then_background_and_frame() {
        let d = Decor { background: Some("#ffffee".into()), frame_color: Some("#333333".into()), frame_width: 0.4, shadow: true, shadow_offset: 1.5, corner_radius: 2.0 };
        let mut o = String::new();
        decor(&mut o, &at(), &d);
        let (sh, bg) = (o.find("fill-opacity=\"0.3\"").unwrap(), o.find("#ffffee").unwrap());
        assert!(sh < bg && o.contains("x=\"11.5\" y=\"21.5\"") && o.contains("rx=\"2\"") && o.contains("stroke=\"#333333\""), "{o}");
        let mut empty = String::new();
        decor(&mut empty, &at(), &Decor::default());
        assert!(!empty.contains("<rect"), "nothing to draw: {empty}");
    }

    #[test]
    fn read_picture_sniffs_the_content() {
        let d = tempfile::tempdir().unwrap();
        let png = d.path().join("logo.dat");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]).unwrap();
        assert!(read_picture(&png).unwrap().starts_with("data:image/png;base64,"));
        let svg = d.path().join("a.png");
        std::fs::write(&svg, "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        assert!(read_picture(&svg).unwrap().starts_with("data:image/svg+xml;base64,"), "named .png but it is SVG");
        let txt = d.path().join("n.txt");
        std::fs::write(&txt, "hello").unwrap();
        assert!(read_picture(&txt).unwrap_err().to_string().contains("not a PNG, JPEG or SVG"));
        assert!(read_picture(&d.path().join("missing.png")).is_err());
        assert!(check_picture(&read_picture(&png).unwrap()).is_ok());
    }

    #[test]
    fn pictures_are_validated_and_fitted() {
        assert!(check_picture("data:image/gif;base64,AAAA").is_err());
        assert!(check_picture("file:///etc/passwd").is_err());
        assert!(check_picture("data:image/png;base64,AA\"onload=").is_err());
        assert_eq!(check_picture("data:image/png;base64,iVBORw0KGgo=").unwrap(), "png");
        let (mut o, mut w) = (String::new(), Vec::new());
        picture(&mut o, &mut w, &at(), "data:image/png;base64,iVBORw0KGgo=", "cover", 0.5);
        assert!(o.contains("xMidYMid slice") && o.contains("clip-path") && o.contains("opacity=\"0.5\"") && w.is_empty());
        let (mut o, mut w) = (String::new(), Vec::new());
        picture(&mut o, &mut w, &at(), "", "contain", 1.0);
        assert!(w[0].contains("no image yet") && o.contains("stroke-dasharray"));
        let (mut o, mut w) = (String::new(), Vec::new());
        picture(&mut o, &mut w, &at(), "http://x/y.png", "contain", 1.0);
        assert!(!o.contains("<image") && w[0].contains("PNG, JPEG or SVG"));
    }
}
