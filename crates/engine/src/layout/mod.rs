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
use crate::progress::Task;
use anyhow::{Context, Result};
use duckdb::Connection;
use model::{Element, Layout, RenderRequest};
use std::fmt::Write;
use std::path::Path;

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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreview {
    /// PNG, base64-encoded.
    pub png: String,
    pub png_bytes: usize,
    pub warnings: Vec<String>,
}

/// Output formats of [`Engine::export_layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Pdf,
    Png,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Result<ExportFormat> {
        match s.to_ascii_lowercase().as_str() {
            "svg" => Ok(ExportFormat::Svg),
            "pdf" => Ok(ExportFormat::Pdf),
            "png" => Ok(ExportFormat::Png),
            other => anyhow::bail!("Unknown layout export format '{other}' (use svg, pdf or png)"),
        }
    }
}

/// Write `bytes` to a temp file beside `path`, then rename: a cancelled or
/// failed export never leaves a partial file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("Could not create {}", dir.display()))?;
    }
    let tmp = path.with_extension(format!("{}.part", path.extension().and_then(|e| e.to_str()).unwrap_or("out")));
    std::fs::write(&tmp, bytes).with_context(|| format!("Could not write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        anyhow::anyhow!("Could not save {}: {e}", path.display())
    })
}

impl Engine {
    /// Saved layouts, in name order.
    pub fn list_layouts(&self) -> Result<Vec<Layout>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT json::VARCHAR FROM __layouts ORDER BY updated, id")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(Layout::from_json(&r?)?);
            }
            out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
            Ok(out)
        })
    }

    /// Validate and store a layout (insert or replace).
    pub fn save_layout(&self, layout: &Layout) -> Result<()> {
        layout.validate()?;
        let json = serde_json::to_string(layout)?;
        self.with_conn(|c| {
            c.execute("INSERT OR REPLACE INTO __layouts VALUES (?, ?::JSON, now()::TIMESTAMP)", duckdb::params![layout.id, json])?;
            Ok(())
        })
    }

    pub fn delete_layout(&self, id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM __layouts WHERE id = ?", duckdb::params![id])?;
            Ok(())
        })
    }

    /// Render a page to a PNG preview (base64, so it crosses IPC as one string).
    pub fn render_layout_preview(&self, req: &RenderRequest, dpi: f64) -> Result<LayoutPreview> {
        use base64::Engine as _;
        let out = self.render_layout_svg(req, &|| Ok(()))?;
        let png = export::svg_to_png(&out.svg, dpi)?;
        Ok(LayoutPreview { png: base64::engine::general_purpose::STANDARD.encode(&png), png_bytes: png.len(), warnings: out.warnings })
    }

    /// Export a layout page to `path` as a cancellable task ("Exporting
    /// layout — <name>"). Cancel is checked between layers and before
    /// encoding; the file is written to a temp name and renamed on success.
    /// Returns the render warnings.
    pub fn export_layout(&self, req: &RenderRequest, format: ExportFormat, dpi: f64, path: &Path) -> Result<Vec<String>> {
        let task = Task::start(format!("Exporting layout — {}", req.layout.name)).cancellable();
        task.detail("Drawing the page");
        let out = self.render_layout_svg(req, &|| task.check())?;
        task.check()?;
        task.set(0.8, Some("Encoding"));
        let bytes = match format {
            ExportFormat::Svg => out.svg.into_bytes(),
            ExportFormat::Pdf => export::svg_to_pdf(&out.svg)?,
            ExportFormat::Png => export::svg_to_png(&out.svg, dpi)?,
        };
        task.check()?;
        task.set(0.95, Some("Writing the file"));
        write_atomic(path, &bytes)?;
        Ok(out.warnings)
    }

    /// Render a layout page to SVG against the open project's layers.
    pub fn render_layout_svg(&self, req: &RenderRequest, check: &dyn Fn() -> Result<()>) -> Result<LayoutSvg> {
        let lookup = |id: &str| self.layer(id).ok();
        self.with_conn(|c| render_svg(c, req, &lookup, check))
    }
}
