//! Print layouts (docs/LAYOUTS_PLAN.md): the engine renders each page to SVG
//! in page millimetres, then converts it to PDF or PNG. C2.1 draws map frames
//! (vector layers), text, legends, scale bars, north arrows and sources;
//! rasters, graticules and basemaps arrive in C2.4.

pub mod basemap;
pub mod export;
pub mod frame;
pub mod geo;
pub mod grid;
pub mod labels;
pub mod model;
pub mod series;
pub mod shapes;
pub mod surround;
pub mod templates;
pub mod svg;

use crate::layers::LayerInfo;
use crate::Engine;
use crate::progress::Task;
use anyhow::{Context, Result};
use duckdb::Connection;
use model::{Element, GridSpec, Layout, RenderRequest};
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LayoutSvg {
    pub svg: String,
    pub warnings: Vec<String>,
    /// Basemap ids that were drawn (their citations are recorded by the engine).
    pub basemaps: Vec<String>,
    /// Labels as placed, by frame (L3): the UI draws a hit box over each.
    pub labels: Vec<labels::PlacedLabel>,
    /// Label edits whose feature no longer exists; the UI removes them.
    pub dropped_label_edits: Vec<labels::DroppedEdit>,
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
    let described = crate::crs::statement(crs);
    let out = text
        .replace("{layout_name}", &req.layout.name)
        .replace("{project}", &req.project)
        .replace("{date}", &req.date)
        .replace("{scale}", &if scale > 0.0 { format!("1:{}", grouped(scale)) } else { String::new() })
        .replace("{crs}", &described.crs)
        .replace("{datum}", &described.datum)
        .replace("{units}", &described.units)
        .replace("{page_name}", &req.page.as_ref().map_or(req.layout.name.clone(), |p| p.name.clone()))
        .replace("{page_number}", &req.page.as_ref().map_or(1, |p| p.number).to_string())
        .replace("{page_count}", &req.page.as_ref().map_or(1, |p| p.count).to_string());
    // {page_<field>}: any attribute of the series' index feature.
    match &req.page {
        Some(p) => p.attrs.iter().fold(out, |s, (k, v)| s.replace(&format!("{{page_{k}}}"), v)),
        None => out,
    }
}

fn text_svg(out: &mut String, at: &model::Placement, text: &str, font: &str, family: Option<&str>, size: f64, align: &str, color: &str) {
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
        export::font_stack(family),
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
/// layers or node-tree results it wants drawn); `cites` gives the short
/// citations recorded for a layer id; `open_basemap` opens a basemap's tile
/// service; `check` is polled between layers so an
/// export can be cancelled.
pub fn render_svg(conn: &Connection, req: &RenderRequest, lookup: &dyn Fn(&str) -> Option<LayerInfo>, cites: &dyn Fn(&str) -> Vec<String>, open_basemap: &dyn Fn(&str) -> Result<gdal::Dataset>, check: &dyn Fn() -> Result<()>) -> Result<LayoutSvg> {
    let layout = &req.layout;
    layout.validate()?;
    let body = geo::Body::get(&layout.body)?;
    let p = &layout.page;
    let mut warnings = Vec::new();
    let mut basemaps: Vec<String> = Vec::new();
    let mut placed_labels = Vec::new();
    let mut dropped_label_edits = Vec::new();
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><rect width=\"{w}\" height=\"{h}\" fill=\"{}\"/>",
        svg::color(&p.background, "#ffffff"),
        w = svg::mm(p.width),
        h = svg::mm(p.height)
    );
    // Frames first: legends, scale bars and arrows read what each one drew.
    let mut frames: HashMap<&str, (String, surround::FrameCtx)> = HashMap::new();
    for e in layout.elements.iter().filter(|e| e.at().visible) {
        if let Element::MapFrame { at, crs, center, scale, layers, basemap, graticule, grid, border, label_overrides, .. } = e {
            check()?;
            let (graticule, grid) = (graticule.as_ref().map(|g| GridSpec::parse(g, "graticule")).transpose()?, grid.as_ref().map(|g| GridSpec::parse(g, "grid")).transpose()?);
            let f = frame::FrameInput { at, crs, center: *center, scale: *scale, layers, border: border.as_ref(), graticule: graticule.as_ref(), grid: grid.as_ref(), basemap, open_basemap, label_overrides };
            let out = frame::render_frame(conn, &body, &f, req, lookup, check)?;
            warnings.extend(out.warnings);
            placed_labels.extend(out.labels);
            dropped_label_edits.extend(out.dropped_label_edits);
            let mut extra_cites = Vec::new();
            if let Some(b) = &out.basemap {
                extra_cites.extend(crate::sources::for_basemap(b).map(|c| short_citation(&c)));
                if !basemaps.contains(b) {
                    basemaps.push(b.clone());
                }
            }
            let ctx = surround::FrameCtx { id: at.id.clone(), scale: *scale, rotation: at.rotation, factors: out.factors, drawn: out.drawn.iter().rev().cloned().collect(), extra_cites };
            frames.insert(at.id.as_str(), (out.svg, ctx));
        }
    }
    for e in layout.elements.iter().filter(|e| e.at().visible) {
        check()?;
        let ctx_of = |frame: &str, warnings: &mut Vec<String>| {
            let c = frames.get(frame).map(|f| &f.1);
            if c.is_none() {
                warnings.push(format!("Element '{}' belongs to map frame '{frame}', which is hidden; not drawn", e.at().id));
            }
            c
        };
        if let Some(d) = &e.at().decor {
            shapes::decor(&mut s, e.at(), d);
        }
        match e {
            Element::Shape { at, shape, fill, fill_opacity, stroke, stroke_width, dash, corner_radius } => {
                shapes::shape(&mut s, at, &shapes::ShapeStyle { shape, fill, fill_opacity: *fill_opacity, stroke, stroke_width: *stroke_width, dash, corner_radius: *corner_radius })
            }
            Element::Picture { at, source, fit, opacity } => shapes::picture(&mut s, &mut warnings, at, source, fit, *opacity),
            Element::MapFrame { at, .. } => s.push_str(&frames[at.id.as_str()].0),
            Element::Text { at, text, font, family, size, align, color } => text_svg(&mut s, at, &tokens(text, req), font, family.as_deref(), *size, align, color),
            Element::Legend { at, frame, title, columns } => {
                if let Some(c) = ctx_of(frame, &mut warnings) {
                    surround::legend(&mut s, &mut warnings, at, title, *columns, c, req, lookup);
                }
            }
            Element::ScaleBar { at, frame, units, segments, ratio, style, units2, verbal, color } => {
                if let Some(c) = ctx_of(frame, &mut warnings) {
                    surround::scale_bar(&mut s, &mut warnings, at, &surround::BarOpts { units, segments: *segments, ratio: *ratio, style, units2, verbal: *verbal, color }, c);
                }
            }
            Element::NorthArrow { at, frame, style, north, color, source } => {
                if let Some(c) = ctx_of(frame, &mut warnings) {
                    surround::north_arrow(&mut s, &mut warnings, at, &surround::ArrowOpts { style, north, color, source }, c);
                }
            }
            Element::Sources { at, frame, size } => {
                if let Some(c) = ctx_of(frame, &mut warnings) {
                    surround::sources(&mut s, &mut warnings, at, *size, c, cites, &c.extra_cites);
                }
            }
        }
    }
    s.push_str("</svg>");
    Ok(LayoutSvg { svg: s, warnings, basemaps, labels: placed_labels, dropped_label_edits })
}

/// "U.S. Census Bureau, TIGER/Line Shapefiles (2024)" for the Sources element.
fn short_citation(c: &crate::sources::Citation) -> String {
    // "TIGERweb: Topologically Integrated…" → "TIGERweb"; "3D Elevation Program (3DEP) — best…" → "3D Elevation Program (3DEP)".
    let title = c.title.split(": ").next().unwrap_or(&c.title).split(" — ").next().unwrap_or(&c.title);
    let mut s = if c.author.is_empty() { title.to_string() } else { format!("{}, {title}", c.author) };
    if !c.year.is_empty() && c.year != "n.d." {
        let _ = write!(s, " ({})", c.year);
    }
    s
}

/// Layer id → short citations of every source it (or its inputs) used.
fn short_citations(conn: &Connection) -> Result<HashMap<String, Vec<String>>> {
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for entry in crate::sources::list(conn)? {
        let short = short_citation(&entry.citation);
        for u in entry.uses {
            let v = m.entry(u.layer_id).or_default();
            if !v.contains(&short) {
                v.push(short.clone());
            }
        }
    }
    Ok(m)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreview {
    /// PNG, base64-encoded.
    pub png: String,
    pub png_bytes: usize,
    pub warnings: Vec<String>,
    /// Labels as placed, for hit boxes in the Layout view (L3).
    pub labels: Vec<labels::PlacedLabel>,
    pub dropped_label_edits: Vec<labels::DroppedEdit>,
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
pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
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
        let mut req = req.clone();
        req.raster_dpi.get_or_insert(dpi); // a screen preview needn't carry 300 dpi images
        let out = self.render_layout_svg(&req, &|| Ok(()))?;
        let png = export::svg_to_png(&out.svg, dpi)?;
        Ok(LayoutPreview { png: base64::engine::general_purpose::STANDARD.encode(&png), png_bytes: png.len(), warnings: out.warnings, labels: out.labels, dropped_label_edits: out.dropped_label_edits })
    }

    /// Export a layout page to `path` as a cancellable task ("Exporting
    /// layout — <name>"). Cancel is checked between layers and before
    /// encoding; the file is written to a temp name and renamed on success.
    /// Returns the render warnings.
    pub fn export_layout(&self, req: &RenderRequest, format: ExportFormat, dpi: f64, path: &Path) -> Result<Vec<String>> {
        let task = Task::start(format!("Exporting layout — {}", req.layout.name)).cancellable();
        task.detail("Drawing the page");
        let mut req = req.clone();
        if format != ExportFormat::Svg {
            req.raster_dpi.get_or_insert(dpi); // PNG pixels, and the images inside a PDF
        }
        let out = self.render_layout_svg(&req, &|| task.check())?;
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
        self.render_layout_svg_with(req, &basemap::open, check)
    }

    /// As [`Engine::render_layout_svg`] with the basemap tile service supplied
    /// (the tests use a local raster instead of the network). Every basemap
    /// drawn is recorded in the project's sources.
    pub fn render_layout_svg_with(&self, req: &RenderRequest, open_basemap: &dyn Fn(&str) -> Result<gdal::Dataset>, check: &dyn Fn() -> Result<()>) -> Result<LayoutSvg> {
        // A series page (C3.2): move the followed frames to that page's feature first.
        let applied;
        let req = match req.series_page {
            Some(i) => {
                applied = self.apply_series_page(req, i)?.0;
                &applied
            }
            None => req,
        };
        let lookup = |id: &str| self.layer(id).ok();
        self.with_conn(|c| {
            let cites = short_citations(c)?;
            let out = render_svg(c, req, &lookup, &|id| cites.get(id).cloned().unwrap_or_default(), open_basemap, check)?;
            for b in &out.basemaps {
                if let Some(cite) = crate::sources::for_basemap(b) {
                    crate::sources::record_once(c, &cite, &format!("basemap:{b}"), &cite.title)?;
                }
            }
            Ok(out)
        })
    }

    /// Bundled and user layout templates (C3.1). `skipped` names user files that could not be read.
    pub fn layout_templates(&self) -> TemplateList {
        let dir = crate::resources::templates_dir();
        let (templates, skipped) = templates::list(dir.as_deref().unwrap_or_else(|| Path::new("")));
        TemplateList { templates, skipped }
    }

    pub fn layout_template(&self, id: &str) -> Result<model::Layout> {
        templates::load(id, &user_templates_dir()?)
    }

    pub fn save_layout_template(&self, name: &str, layout: &model::Layout) -> Result<templates::TemplateInfo> {
        templates::save(name, layout, &user_templates_dir()?)
    }

    pub fn delete_layout_template(&self, id: &str) -> Result<()> {
        templates::delete(id, &user_templates_dir()?)
    }
}

fn user_templates_dir() -> Result<std::path::PathBuf> {
    crate::resources::templates_dir().ok_or_else(|| anyhow::anyhow!("This computer has no per-user folder for templates"))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateList {
    pub templates: Vec<templates::TemplateInfo>,
    pub skipped: Vec<String>,
}
