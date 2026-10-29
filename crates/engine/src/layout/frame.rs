//! Map frame → SVG (docs/LAYOUTS_PLAN.md L3): per vector layer, transform to
//! the frame CRS in DuckDB, clip, generalize for print, and write styled
//! paths in page millimetres.

use super::geo::{self, Body, FrameCrs};
use super::grid;
use super::model::{FrameLayers, GridSpec, LayerStyle, Placement, RenderRequest, StyleKind};
use super::svg::{self, cmd, esc, mm, opacity};
use crate::db::{ident, lit};
use crate::layers::LayerInfo;
use anyhow::{anyhow, Result};
use duckdb::Connection;
use std::collections::HashMap;
use std::fmt::Write;

/// UI styles are in CSS pixels (96 per inch); the page is in millimetres.
pub(super) const PX_MM: f64 = 25.4 / 96.0;
/// Features are cut at the frame plus this much paper, so strokes reach the edge.
const CLIP_MARGIN_MM: f64 = 2.0;
/// Print generalization tolerance on the page (L3).
const SIMPLIFY_MM: f64 = 0.1;

pub struct FrameInput<'a> {
    pub at: &'a Placement,
    pub crs: &'a str,
    pub center: [f64; 2],
    pub scale: f64,
    pub layers: &'a FrameLayers,
    pub border: Option<&'a super::model::Border>,
    pub graticule: Option<&'a GridSpec>,
    pub grid: Option<&'a GridSpec>,
    /// "none" or a printable basemap id (see `basemap::IDS`).
    pub basemap: &'a str,
    /// Opens a basemap's tile service (the network in the app; a fixture in tests).
    pub open_basemap: &'a dyn Fn(&str) -> Result<gdal::Dataset>,
    /// Label edits of this layout, by layer id (L3).
    pub label_overrides: &'a std::collections::BTreeMap<String, super::model::LabelOverrides>,
}

pub struct FrameOutput {
    pub svg: String,
    pub warnings: Vec<String>,
    /// Layers drawn, bottom first.
    pub drawn: Vec<String>,
    pub extent: [f64; 4],
    pub factors: geo::Factors,
    /// The basemap that was drawn, if any.
    pub basemap: Option<String>,
    /// Labels as placed (and the hidden ones), for the UI's hit boxes (L3).
    pub labels: Vec<super::labels::PlacedLabel>,
    /// Label edits that pointed at features that no longer exist.
    pub dropped_label_edits: Vec<super::labels::DroppedEdit>,
}

/// Layer ids a frame draws, top first, with their visibility applied.
pub fn frame_layer_ids(layers: &FrameLayers, req: &RenderRequest) -> Vec<String> {
    match layers {
        FrameLayers::Follow(_) => req.order.iter().filter(|id| req.visible.get(*id).copied().unwrap_or(true)).cloned().collect(),
        FrameLayers::List(l) => l.iter().filter(|l| l.visible).map(|l| l.id.clone()).collect(),
    }
}

struct Mapper {
    xmin: f64,
    ymax: f64,
    sx: f64,
    sy: f64,
    ox: f64,
    oy: f64,
}

impl Mapper {
    fn xy(&self, x: f64, y: f64) -> (f64, f64) {
        (self.ox + (x - self.xmin) * self.sx, self.oy + (self.ymax - y) * self.sy)
    }
}

/// Path data and points of one feature, already in page mm.
#[derive(Default)]
struct Shapes {
    polys: String,
    lines: String,
    points: Vec<(f64, f64)>,
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl Rd<'_> {
    fn u32(&mut self, le: bool) -> Option<u32> {
        let v: [u8; 4] = self.b.get(self.p..self.p + 4)?.try_into().ok()?;
        self.p += 4;
        Some(if le { u32::from_le_bytes(v) } else { u32::from_be_bytes(v) })
    }
    fn f64(&mut self, le: bool) -> Option<f64> {
        let v: [u8; 8] = self.b.get(self.p..self.p + 8)?.try_into().ok()?;
        self.p += 8;
        Some(if le { f64::from_le_bytes(v) } else { f64::from_be_bytes(v) })
    }
    fn coord(&mut self, le: bool, extra: usize, m: &Mapper) -> Option<(f64, f64)> {
        let (x, y) = (self.f64(le)?, self.f64(le)?);
        for _ in 0..extra {
            self.f64(le)?;
        }
        Some(m.xy(x, y))
    }
    /// Read points as a path fragment. `close` makes a ring. Points that
    /// round to the previous one are dropped; a ring left with < 3 is omitted.
    fn path(&mut self, le: bool, extra: usize, m: &Mapper, close: bool, out: &mut String) -> Option<()> {
        let n = self.u32(le)? as usize;
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            let p = self.coord(le, extra, m)?;
            let r = ((p.0 * 1000.0).round(), (p.1 * 1000.0).round());
            if pts.last().map_or(true, |q| ((q.0 * 1000.0).round(), (q.1 * 1000.0).round()) != r) {
                pts.push(p);
            }
        }
        if close && pts.len() > 1 && pts.first().map(|p| ((p.0 * 1000.0).round(), (p.1 * 1000.0).round())) == pts.last().map(|p| ((p.0 * 1000.0).round(), (p.1 * 1000.0).round())) {
            pts.pop();
        }
        if pts.len() < if close { 3 } else { 2 } {
            return Some(());
        }
        for (i, p) in pts.iter().enumerate() {
            cmd(out, if i == 0 { 'M' } else { 'L' }, p.0, p.1);
        }
        if close {
            out.push('Z');
        }
        Some(())
    }
    fn geometry(&mut self, m: &Mapper, s: &mut Shapes) -> Option<()> {
        let le = *self.b.get(self.p)? == 1;
        self.p += 1;
        let t = self.u32(le)?;
        let (base, extra) = if t & 0xE000_0000 != 0 {
            let z = (t & 0x8000_0000 != 0) as usize + (t & 0x4000_0000 != 0) as usize;
            if t & 0x2000_0000 != 0 {
                self.u32(le)?; // SRID
            }
            (t & 0xFFFF, z)
        } else {
            (t % 1000, [0usize, 1, 1, 2][((t / 1000) as usize).min(3)])
        };
        match base {
            1 => {
                let p = self.coord(le, extra, m)?;
                if p.0.is_finite() && p.1.is_finite() {
                    s.points.push(p);
                }
            }
            2 => self.path(le, extra, m, false, &mut s.lines)?,
            3 => {
                for _ in 0..self.u32(le)? {
                    self.path(le, extra, m, true, &mut s.polys)?;
                }
            }
            4..=7 => {
                for _ in 0..self.u32(le)? {
                    self.geometry(m, s)?;
                }
            }
            _ => return None,
        }
        Some(())
    }
}

/// Order the class buckets are drawn in, first = bottom (B7). Unique values: a class higher in the list is drawn on
/// top, and "everything else" (the last bucket) under all of them. Other styles keep their class order.
fn bucket_draw_order(style: &LayerStyle, buckets: usize) -> Vec<usize> {
    if style.kind == StyleKind::Categorized && buckets > 1 {
        std::iter::once(buckets - 1).chain((0..buckets - 1).rev()).collect()
    } else {
        (0..buckets).collect()
    }
}

/// Class index of a feature (the last bucket is "everything else").
struct Classes {
    n: usize,
    by_value: HashMap<String, usize>,
}

fn class_count(style: &LayerStyle) -> usize {
    match style.kind {
        StyleKind::Single => 1,
        StyleKind::Categorized => style.classes.as_ref().map_or(0, |c| c.len()) + 1,
        StyleKind::Graduated => style.breaks.as_ref().map_or(0, |b| b.len()) + 1,
    }
}

pub(super) fn class_color(style: &LayerStyle, i: usize) -> String {
    let c = match style.kind {
        StyleKind::Single => None,
        StyleKind::Categorized => style.classes.as_ref().and_then(|c| c.get(i)).map(|c| c.color.as_str()),
        StyleKind::Graduated => style.colors.as_ref().and_then(|c| c.get(i)).map(|c| c.as_str()),
    };
    svg::color(c.unwrap_or(&style.fill), "#808080")
}

#[derive(Default)]
struct Bucket {
    polys: String,
    lines: String,
    points: String,
}

fn bucket_svg(info: &LayerInfo, style: &LayerStyle, i: usize, b: &Bucket, out: &mut String) {
    let color = class_color(style, i);
    let stroke = svg::color(&style.stroke, "#4d4d4d");
    let sw = style.stroke_width.max(0.0) * PX_MM;
    let op = opacity(style.opacity);
    if !b.polys.is_empty() {
        let (s, w) = if sw > 0.0 { (stroke.as_str(), mm(sw)) } else { ("none", "0".into()) };
        let _ = write!(out, "<g fill=\"{color}\" fill-opacity=\"{op}\" fill-rule=\"evenodd\" stroke=\"{s}\" stroke-width=\"{w}\" stroke-linejoin=\"round\">{}</g>", b.polys);
    }
    if !b.lines.is_empty() {
        let (c, o) = if info.geom_type == "Line" { (color.as_str(), op.as_str()) } else { (stroke.as_str(), "1") };
        let _ = write!(out, "<g fill=\"none\" stroke=\"{c}\" stroke-opacity=\"{o}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</g>", mm(sw), b.lines);
    }
    if !b.points.is_empty() {
        let ring = if style.radius > 2.0 { format!(" stroke=\"#333333\" stroke-width=\"{}\"", mm(0.6 * PX_MM)) } else { String::new() };
        let _ = write!(out, "<g fill=\"{color}\" fill-opacity=\"{op}\"{ring}>{}</g>", b.points);
    }
}

/// SQL for one layer: WKB in the frame CRS, clipped and generalized, plus the
/// class value column when the style needs one. Ordered by feature id so the
/// output never depends on DuckDB's parallel scan.
#[allow(clippy::too_many_arguments)]
fn layer_sql(info: &LayerInfo, style: &LayerStyle, frame_crs: &str, bbox3857: Option<[f64; 4]>, clip: [f64; 4], tol: f64, field_ok: bool) -> String {
    let (src, dst) = (crate::crs::resolve(info.crs.as_deref().unwrap_or("")), crate::crs::resolve(frame_crs));
    let mut g = if src == dst { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&src), lit(&dst)) };
    if info.geom_type != "Point" {
        g = format!("ST_SimplifyPreserveTopology({g}, {tol})");
    }
    let (value, vcol) = match (style.kind, &style.field) {
        (StyleKind::Categorized, Some(f)) if field_ok => (format!("CAST({} AS VARCHAR) AS v,", ident(f)), ", v"),
        (StyleKind::Graduated, Some(f)) if field_ok => (format!("TRY_CAST({} AS DOUBLE) AS v,", ident(f)), ", v"),
        _ => (String::new(), ""),
    };
    let filter = match bbox3857 {
        Some(b) => format!("ST_Intersects(__g, ST_MakeEnvelope({}, {}, {}, {}))", b[0], b[1], b[2], b[3]),
        None => "__g IS NOT NULL".into(),
    };
    let env = format!("ST_MakeEnvelope({}, {}, {}, {})", clip[0], clip[1], clip[2], clip[3]);
    format!(
        "SELECT ST_AsWKB(c){vcol} FROM (
           SELECT CASE WHEN ST_XMin(g) >= {} AND ST_XMax(g) <= {} AND ST_YMin(g) >= {} AND ST_YMax(g) <= {} THEN g ELSE ST_Intersection(g, {env}) END AS c{vcol}, __fid
           FROM (SELECT {g} AS g, {value} __fid FROM {t} WHERE {filter}{def})
         ) WHERE c IS NOT NULL AND NOT ST_IsEmpty(c) ORDER BY __fid",
        clip[0],
        clip[2],
        clip[1],
        clip[3],
        t = ident(&info.table),
        def = info.def_and(),
    )
}

fn draw_layer(conn: &Connection, info: &LayerInfo, style: &LayerStyle, frame_crs: &str, bbox3857: Option<[f64; 4]>, clip: [f64; 4], tol: f64, m: &Mapper, radius_mm: f64) -> Result<String> {
    let field_ok = style.field.as_ref().is_some_and(|f| info.fields.iter().any(|x| &x.name == f));
    let sql = layer_sql(info, style, frame_crs, bbox3857, clip, tol, field_ok);
    let n = class_count(style);
    let classes = Classes {
        n,
        by_value: match (style.kind, &style.classes) {
            (StyleKind::Categorized, Some(c)) if field_ok => c.iter().enumerate().rev().map(|(i, c)| (c.value.clone(), i)).collect(),
            _ => HashMap::new(),
        },
    };
    let mut buckets: Vec<Bucket> = (0..classes.n.max(1)).map(|_| Bucket::default()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let has_v = field_ok && style.kind != StyleKind::Single;
    while let Some(r) = rows.next()? {
        let wkb: Vec<u8> = r.get(0)?;
        let idx = if !has_v {
            0
        } else if style.kind == StyleKind::Categorized {
            let v: Option<String> = r.get(1)?;
            v.and_then(|v| classes.by_value.get(&v).copied()).unwrap_or(classes.n - 1)
        } else {
            let v: Option<f64> = r.get(1)?;
            let v = v.unwrap_or(0.0); // MapLibre: to-number with fallback 0
            // MapLibre `step`: the last break <= value picks the colour.
            style.breaks.as_ref().map_or(0, |b| b.iter().filter(|b| v >= **b).count())
        };
        let mut s = Shapes::default();
        if (Rd { b: &wkb, p: 0 }).geometry(m, &mut s).is_none() {
            continue;
        }
        let b = &mut buckets[idx.min(classes.n.max(1) - 1)];
        if !s.polys.is_empty() {
            let _ = write!(b.polys, "<path d=\"{}\"/>", s.polys);
        }
        if !s.lines.is_empty() {
            let _ = write!(b.lines, "<path d=\"{}\"/>", s.lines);
        }
        for p in s.points {
            let _ = write!(b.points, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>", mm(p.0), mm(p.1), mm(radius_mm));
        }
    }
    let mut out = String::new();
    for i in bucket_draw_order(style, buckets.len()) {
        bucket_svg(info, style, i, &buckets[i], &mut out);
    }
    Ok(out)
}

/// 32 points per side of the extent, closed by the caller if needed.
fn densified(e: [f64; 4]) -> Vec<(f64, f64)> {
    let mut v = Vec::with_capacity(128);
    for i in 0..32 {
        let t = i as f64 / 32.0;
        let (dx, dy) = ((e[2] - e[0]) * t, (e[3] - e[1]) * t);
        v.push((e[0] + dx, e[1]));
        v.push((e[2], e[1] + dy));
        v.push((e[2] - dx, e[3]));
        v.push((e[0], e[3] - dy));
    }
    v
}

/// Pixel size of an image covering the frame at `dpi`.
fn image_px(at: &Placement, dpi: f64) -> (u32, u32) {
    let px = |mm: f64| (mm / 25.4 * dpi).round().max(1.0) as u32;
    (px(at.w), px(at.h))
}

/// A PNG as an SVG `<image>` over the whole frame.
fn image_tag(at: &Placement, png: &[u8], opacity_v: f64) -> String {
    use base64::Engine as _;
    format!(
        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" opacity=\"{}\" href=\"data:image/png;base64,{}\"/>",
        mm(at.x),
        mm(at.y),
        mm(at.w),
        mm(at.h),
        opacity(opacity_v),
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

pub fn render_frame(conn: &Connection, body: &Body, f: &FrameInput, req: &RenderRequest, lookup: &dyn Fn(&str) -> Option<LayerInfo>, check: &dyn Fn() -> Result<()>) -> Result<FrameOutput> {
    let at = f.at;
    if f.crs.trim().is_empty() {
        return Err(anyhow!("Set a coordinate system for the map frame '{}'", at.id));
    }
    let crs = FrameCrs::new(f.crs, body)?;
    let mut warnings = Vec::new();
    if !crs.projected {
        warnings.push(format!("Frame '{}': geographic coordinates, so scale varies across the frame; choose a projected CRS for a true-scale map (scale is ≈ 1:{} at the centre).", at.id, f.scale));
    }
    let extent = geo::frame_extent(body, &crs, f.center, f.scale, at.w, at.h)?;
    let factors = geo::factors(body, &crs, f.center[0], f.center[1])?;
    let var = geo::scale_variation(body, &crs, extent)?;
    if var > 0.01 {
        warnings.push(format!("Frame '{}': scale varies {:.1}% across this frame", at.id, var * 100.0));
    }

    let m = Mapper { xmin: extent[0], ymax: extent[3], sx: at.w / (extent[2] - extent[0]), sy: at.h / (extent[3] - extent[1]), ox: at.x, oy: at.y };
    // Ground margin for strokes, and the clip box in frame-CRS units.
    let unit = geo::mm_in_crs_units(body, &crs, f.center, f.scale)?;
    let margin = CLIP_MARGIN_MM * unit;
    let clip = [extent[0] - margin, extent[1] - margin, extent[2] + margin, extent[3] + margin];
    let tol = SIMPLIFY_MM * unit;
    let bbox3857 = crs.bounds_in(crate::crs::WEB_MERCATOR, &densified(clip)).ok().map(|b| {
        let l = crate::crs::MERC_MAX_LAT;
        let (_, ymax) = crate::crs::lonlat_to_merc(0.0, l);
        [b[0], b[1].max(-ymax), b[2], b[3].min(ymax)]
    });

    let mut body_svg = String::new();
    let mut drawn = Vec::new();
    let dpi = req.raster_dpi.unwrap_or(300.0).clamp(72.0, 300.0);
    let mut basemap = None;
    if f.basemap != "none" {
        check()?;
        let (w_px, h_px) = image_px(at, dpi);
        match (f.open_basemap)(f.basemap).and_then(|ds| super::basemap::render(&ds, f.crs, extent, w_px, h_px)) {
            Ok(png) => {
                let _ = write!(body_svg, "<g id=\"basemap-{}\">{}</g>", esc(&at.id), image_tag(at, &png, 1.0));
                basemap = Some(f.basemap.to_string());
            }
            Err(e) => warnings.push(format!("Frame '{}': the {} basemap could not be drawn ({e}); check the network connection", at.id, f.basemap)),
        }
    }
    let ids = frame_layer_ids(f.layers, req);
    for id in ids.iter().rev() {
        check()?;
        let Some(info) = lookup(id) else {
            warnings.push(format!("Layer {id} not found; skipped"));
            continue;
        };
        if let Some(r) = &info.raster {
            if info.crs.is_none() {
                warnings.push(format!("Layer {} skipped: no coordinate system", info.name));
                continue;
            }
            let (w_px, h_px) = image_px(at, dpi);
            let rstyle = req.styles.get(id).and_then(|s| s.raster.clone()).unwrap_or_default();
            let png = crate::raster::render_extent_styled(r, &rstyle, f.crs, extent, w_px, h_px).map_err(|e| anyhow!("Layer {} could not be drawn: {e}", info.name))?;
            let op = req.styles.get(id).map_or(1.0, |s| s.opacity);
            let _ = write!(body_svg, "<g id=\"layer-{}\">{}</g>", esc(id), image_tag(at, &png, op));
            drawn.push(id.clone());
            continue;
        }
        if !info.has_geometry() {
            continue;
        }
        if info.crs.is_none() {
            warnings.push(format!("Layer {} skipped: no coordinate system", info.name));
            continue;
        }
        let style = req.styles.get(id).cloned().unwrap_or_default();
        let r = draw_layer(conn, &info, &style, f.crs, bbox3857, clip, tol, &m, style.radius.max(0.0) * PX_MM)
            .map_err(|e| anyhow!("Layer {} could not be drawn: {e}", info.name))?;
        let _ = write!(body_svg, "<g id=\"layer-{}\">{r}</g>", esc(id));
        drawn.push(id.clone());
    }

    // Feature labels (L2, L3): the map's setting with this layout's edits on top. Top layer first, so the top layer wins
    // when labels collide; edited (pinned) labels are placed before any automatic one.
    let frame_box = [at.x, at.y, at.x + at.w, at.y + at.h];
    let mut cands = Vec::new();
    let mut dropped = Vec::new();
    for id in &ids {
        let Some(info) = lookup(id) else { continue };
        let style = req.styles.get(id);
        let ov = f.label_overrides.get(id);
        let fields: Vec<String> = info.fields.iter().map(|x| x.name.clone()).collect();
        let Some(label) = super::labels::effective(style.and_then(|s| s.label.as_ref()), ov.and_then(|o| o.style.as_ref()), &fields) else { continue };
        if info.raster.is_some() || !info.has_geometry() || info.crs.is_none() || !super::labels::visible_at(&label, f.scale) {
            continue;
        }
        check()?;
        let radius = style.map_or(3.5, |s| s.radius).max(0.0) * PX_MM;
        let mut c = super::labels::candidates(conn, &info, id, &label, ov, f.crs, bbox3857, extent, &|x, y| m.xy(x, y), radius, check)
            .map_err(|e| anyhow!("Labels of {} could not be drawn: {e}", info.name))?;
        if c.truncated {
            warnings.push(format!("{} has more than {} features in this frame; only the first {} were considered for labels", info.name, super::labels::MAX_CANDIDATES, super::labels::MAX_CANDIDATES));
        }
        if !c.dropped.is_empty() {
            warnings.push(format!("{} label edit{} of {} pointed at features that no longer exist and {} ignored", c.dropped.len(), if c.dropped.len() == 1 { "" } else { "s" }, info.name, if c.dropped.len() == 1 { "was" } else { "were" }));
            dropped.extend(c.dropped.iter().map(|fid| super::labels::DroppedEdit { frame: at.id.clone(), layer: id.clone(), fid: *fid }));
        }
        cands.append(&mut c.list);
    }
    let mut placer = super::labels::Placer::default();
    let placed = super::labels::place(&cands, frame_box, &mut placer);
    for (name, n) in &placed.skipped {
        warnings.push(format!("{n} label{} of {name} didn't fit", if *n == 1 { "" } else { "s" }));
    }
    for f in &placed.missing_fonts {
        warnings.push(format!("Font \"{f}\" is not installed on this computer; labels use Noto Sans instead"));
    }
    let mut placed_labels = placed.labels;
    for l in &mut placed_labels {
        l.frame = at.id.clone();
    }
    if !placed.svg.is_empty() {
        let _ = write!(body_svg, "<g id=\"labels-{}\">{}</g>", esc(&at.id), placed.svg);
    }

    let mut labels = String::new();
    let to_page = |x: f64, y: f64| m.xy(x, y);
    if let Some(spec) = f.grid {
        let g = grid::grid(&crs, at, extent, spec, f.graticule.is_some(), &to_page, &mut warnings);
        if !g.lines.is_empty() {
            let _ = write!(body_svg, "<g id=\"grid-{}\" fill=\"none\" stroke=\"#26567a\" stroke-width=\"0.15\">{}</g>", esc(&at.id), g.lines);
        }
        labels.push_str(&g.labels);
    }
    if let Some(spec) = f.graticule {
        let g = grid::graticule(body, &crs, at, extent, spec, &to_page, &mut warnings);
        if !g.lines.is_empty() {
            let _ = write!(body_svg, "<g id=\"graticule-{}\" fill=\"none\" stroke=\"#4d4d4d\" stroke-width=\"0.15\" stroke-dasharray=\"1.2 0.8\">{}</g>", esc(&at.id), g.lines);
        }
        labels.push_str(&g.labels);
    }

    let id = esc(&at.id);
    let (cx, cy) = (at.x + at.w / 2.0, at.y + at.h / 2.0);
    let rot = if at.rotation != 0.0 { format!(" transform=\"rotate({} {} {})\"", mm(at.rotation), mm(cx), mm(cy)) } else { String::new() };
    let mut out = String::new();
    let _ = write!(
        out,
        "<g id=\"{id}\"{rot}><clipPath id=\"clip-{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath><g clip-path=\"url(#clip-{id})\">{body_svg}</g>",
        mm(at.x),
        mm(at.y),
        mm(at.w),
        mm(at.h)
    );
    out.push_str(&labels);
    if let Some(b) = f.border {
        let _ = write!(
            out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
            mm(at.x),
            mm(at.y),
            mm(at.w),
            mm(at.h),
            svg::color(&b.color, "#000000"),
            mm(b.width)
        );
    }
    out.push_str("</g>");
    Ok(FrameOutput { svg: out, warnings, drawn, extent, factors, basemap, labels: placed_labels, dropped_label_edits: dropped })
}
