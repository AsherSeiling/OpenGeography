//! Print layouts (docs/LAYOUTS_PLAN.md): the engine renders each page to SVG
//! in page millimetres, then converts it to PDF or PNG. C2.1 draws map frames
//! (vector layers) and text; the other element kinds and basemaps arrive in
//! C2.3 and C2.4 and are reported as warnings meanwhile.

pub mod export;
pub mod frame;
pub mod geo;
pub mod model;
pub mod svg;

use crate::layers::LayerInfo;
use crate::Engine;
use anyhow::Result;
use duckdb::Connection;
use model::{Element, RenderRequest};
use std::fmt::Write;

#[derive(Debug, Clone)]
pub struct LayoutSvg {
    pub svg: String,
    pub warnings: Vec<String>,
}

/// "24000" → "24,000".
fn grouped(n: f64) -> String {
    let s = format!("{}", n.round() as i64);
    let mut o = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            o.push(',');
        }
        o.push(c);
    }
    o
}

fn tokens(text: &str, req: &RenderRequest) -> String {
    let first = req.layout.elements.iter().find_map(|e| match e {
        Element::MapFrame { crs, scale, .. } => Some((crs.as_str(), *scale)),
        _ => None,
    });
    let (crs, scale) = first.map_or(("", 0.0), |f| f);
    text.replace("{layout_name}", &req.layout.name)
        .replace("{project}", &req.project)
        .replace("{date}", &req.date)
        .replace("{scale}", &if scale > 0.0 { format!("1:{}", grouped(scale)) } else { String::new() })
        .replace("{crs}", crs)
        .replace("{page_name}", &req.layout.name)
        .replace("{page_number}", "1")
        .replace("{page_count}", "1")
}

fn text_svg(out: &mut String, at: &model::Placement, text: &str, font: &str, size: f64, align: &str, color: &str) {
    let size_mm = size.max(0.1) * 25.4 / 72.0;
    let (weight, style) = match font {
        "sans-bold" => ("700", "normal"),
        "sans-italic" => ("400", "italic"),
        _ => ("400", "normal"),
    };
    let (x, anchor) = match align {
        "center" => (at.x + at.w / 2.0, "middle"),
        "right" => (at.x + at.w, "end"),
        _ => (at.x, "start"),
    };
    let _ = write!(
        out,
        "<text font-family=\"{}\" font-weight=\"{weight}\" font-style=\"{style}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"{anchor}\"{}>",
        export::font_family(),
        svg::mm(size_mm),
        svg::color(color, "#000000"),
        if at.rotation != 0.0 { format!(" transform=\"rotate({} {} {})\"", svg::mm(at.rotation), svg::mm(at.x + at.w / 2.0), svg::mm(at.y + at.h / 2.0)) } else { String::new() }
    );
    for (i, line) in text.split('\n').enumerate() {
        let _ = write!(out, "<tspan x=\"{}\" y=\"{}\">{}</tspan>", svg::mm(x), svg::mm(at.y + size_mm * (1.0 + 1.25 * i as f64)), svg::esc(line));
    }
    out.push_str("</text>");
}

/// Render one page. `lookup` resolves a layer id (the UI sends ids of the
/// layers or node-tree results it wants drawn); `check` is polled between
/// layers so an export can be cancelled.
pub fn render_svg(conn: &Connection, req: &RenderRequest, lookup: &dyn Fn(&str) -> Option<LayerInfo>, check: &dyn Fn() -> Result<()>) -> Result<LayoutSvg> {
    let layout = &req.layout;
    layout.validate()?;
    let body = geo::Body::get(&layout.body)?;
    let p = &layout.page;
    let mut warnings = Vec::new();
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
        svg::color(&p.background, "#ffffff"),
        w = svg::mm(p.width),
        h = svg::mm(p.height)
    );
    for e in layout.elements.iter().filter(|e| e.at().visible) {
        check()?;
        match e {
            Element::MapFrame { at, crs, center, scale, layers, basemap, graticule, grid, border } => {
                let f = frame::FrameInput { at, crs, center: *center, scale: *scale, layers, border: border.as_ref() };
                let out = frame::render_frame(conn, &body, &f, req, lookup, check)?;
                s.push_str(&out.svg);
                warnings.extend(out.warnings);
                if basemap != "none" {
                    warnings.push(format!("Frame '{}': basemaps are drawn in a later release", at.id));
                }
                if graticule.is_some() || grid.is_some() {
                    warnings.push(format!("Frame '{}': graticule and grid are drawn in a later release", at.id));
                }
            }
            Element::Text { at, text, font, size, align, color } => text_svg(&mut s, at, &tokens(text, req), font, *size, align, color),
            other => warnings.push(format!("Element '{}' ({}) is drawn in a later release", other.at().id, other.kind())),
        }
    }
    s.push_str("</svg>");
    Ok(LayoutSvg { svg: s, warnings })
}

impl Engine {
    /// Render a layout page to SVG against the open project's layers.
    pub fn render_layout_svg(&self, req: &RenderRequest, check: &dyn Fn() -> Result<()>) -> Result<LayoutSvg> {
        let lookup = |id: &str| self.layer(id).ok();
        self.with_conn(|c| render_svg(c, req, &lookup, check))
    }
}
