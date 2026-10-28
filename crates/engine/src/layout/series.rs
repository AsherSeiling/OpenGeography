//! Map series (C3.2, docs/LAYOUTS_PLAN.md): one page per feature of an index layer. A map frame that follows the series
//! is moved to each feature (fitted, or at a fixed scale); text tokens `{page_*}` carry the feature's name and fields.
//! Export writes one multi-page PDF or one PNG per page, all or nothing, with progress per page and Cancel between pages.

use super::geo::{self, Body, FrameCrs};
use super::model::{Element, Layout, PageCtx, RenderRequest};
use super::{export, write_atomic, ExportFormat, LayoutSvg};
use crate::db::{ident, lit};
use crate::progress::Task;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Most pages one series may have.
pub const MAX_PAGES: usize = 500;
/// Fallback scale when a feature has no extent (a point) and the series gives none.
const POINT_SCALE: f64 = 10_000.0;

/// `Layout.series`: stored as JSON (camelCase).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeriesSpec {
    /// Layer whose features are the pages.
    pub index_layer: String,
    /// Field shown by `{page_name}` and used in file names.
    pub name_field: String,
    /// Field the pages are ordered by (default: the name field).
    #[serde(default)]
    pub sort_field: Option<String>,
    /// "fit": the scale that shows the feature with `margin_pct` around it; "fixed_scale": `scale` for every page.
    #[serde(default = "fit")]
    pub extent: String,
    /// Space around the feature as a percentage of its size, each side (fit only).
    #[serde(default = "ten")]
    pub margin_pct: f64,
    #[serde(default)]
    pub scale: Option<f64>,
}

fn fit() -> String {
    "fit".into()
}
fn ten() -> f64 {
    10.0
}

impl SeriesSpec {
    /// Read and check `Layout.series`.
    pub fn parse(v: &serde_json::Value) -> Result<SeriesSpec> {
        let s: SeriesSpec = serde_json::from_value(v.clone()).map_err(|e| anyhow!("Invalid map series: {e}"))?;
        if s.index_layer.trim().is_empty() {
            bail!("The map series needs an index layer");
        }
        if s.name_field.trim().is_empty() {
            bail!("The map series needs a name field");
        }
        match s.extent.as_str() {
            "fit" => {}
            "fixed_scale" => {
                if !s.scale.is_some_and(|x| x.is_finite() && x > 0.0) {
                    bail!("A map series at a fixed scale needs a scale greater than 0");
                }
            }
            other => bail!("Unknown series extent '{other}' (use fit or fixed_scale)"),
        }
        if !(s.margin_pct.is_finite() && (0.0..=50.0).contains(&s.margin_pct)) {
            bail!("The series margin must be between 0 and 50 percent");
        }
        Ok(s)
    }
}

/// One page of a series.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeriesPage {
    /// 0-based position in the series.
    pub index: usize,
    pub fid: i64,
    pub name: String,
    #[serde(skip)]
    pub attrs: BTreeMap<String, String>,
}

/// A text safe to use in a file name.
fn safe(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).collect();
    let t = t.trim().chars().take(60).collect::<String>();
    if t.is_empty() { "page".into() } else { t }
}

fn is_numeric(ty: &str) -> bool {
    let t = ty.to_uppercase();
    ["INT", "DOUBLE", "FLOAT", "DECIMAL", "REAL", "NUMERIC"].iter().any(|k| t.contains(k))
}

impl Engine {
    /// The pages of a layout's series in order (sort field, then feature id). Errors read as sentences.
    pub fn series_pages(&self, layout: &Layout) -> Result<Vec<SeriesPage>> {
        let spec = SeriesSpec::parse(layout.series.as_ref().ok_or_else(|| anyhow!("This layout has no map series. Set one up first (Layout tab → Map Series)."))?)?;
        let info = self.layer(&spec.index_layer).map_err(|_| anyhow!("The series' index layer is no longer in the project"))?;
        if info.is_raster() || !info.has_geometry() {
            bail!("'{}' has no features with geometry, so it can't be a series index", info.name);
        }
        if info.crs.is_none() {
            bail!("'{}' has no coordinate system; define one before using it as a series index", info.name);
        }
        let field = |n: &str| info.fields.iter().find(|f| f.name == n).ok_or_else(|| anyhow!("'{}' has no field called '{n}'", info.name));
        field(&spec.name_field)?;
        let sort = field(spec.sort_field.as_deref().unwrap_or(&spec.name_field))?;
        let sort_expr = if is_numeric(&sort.ty) { ident(&sort.name) } else { format!("lower(CAST({} AS VARCHAR))", ident(&sort.name)) };
        let cols: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
        let select = cols.iter().map(|c| format!("CAST({} AS VARCHAR)", ident(c))).collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT __fid, {select} FROM {t} WHERE __g IS NOT NULL{def} ORDER BY {sort_expr} NULLS LAST, __fid LIMIT {limit}",
            t = ident(&info.table),
            def = info.def_and(),
            limit = MAX_PAGES + 1
        );
        let name_at = cols.iter().position(|c| *c == spec.name_field).unwrap_or(0);
        let pages: Vec<SeriesPage> = self.with_conn(|c| {
            let mut st = c.prepare(&sql)?;
            let mut rows = st.query([])?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                let fid: i64 = r.get(0)?;
                let mut attrs = BTreeMap::new();
                for (i, name) in cols.iter().enumerate() {
                    let v: Option<String> = r.get(i + 1)?;
                    attrs.insert((*name).to_string(), v.unwrap_or_default());
                }
                let name = attrs.get(cols[name_at]).cloned().unwrap_or_default();
                out.push(SeriesPage { index: out.len(), fid, name, attrs });
            }
            Ok(out)
        })?;
        if pages.len() > MAX_PAGES {
            bail!("The series would have more than {MAX_PAGES} pages; the limit is {MAX_PAGES}. Use a definition query on '{}' to split it up.", info.name);
        }
        if pages.is_empty() {
            bail!("'{}' has no features to make pages from", info.name);
        }
        Ok(pages)
    }

    /// The request with page `page` of the series applied: followed frames are moved to the page's feature and the text
    /// tokens know the page. The page list is returned too.
    pub fn apply_series_page(&self, req: &RenderRequest, page: usize) -> Result<(RenderRequest, SeriesPage, usize)> {
        let layout = &req.layout;
        let spec = SeriesSpec::parse(layout.series.as_ref().ok_or_else(|| anyhow!("This layout has no map series"))?)?;
        let pages = self.series_pages(layout)?;
        let count = pages.len();
        let p = pages.get(page).cloned().ok_or_else(|| anyhow!("The series has {count} page{}; there is no page {}", if count == 1 { "" } else { "s" }, page + 1))?;
        let info = self.layer(&spec.index_layer)?;
        let body = Body::get(&layout.body)?;
        let mut out = req.clone();
        out.series_page = None;
        out.page = Some(PageCtx { number: page + 1, count, name: p.name.clone(), attrs: p.attrs.clone() });
        let mut followed = 0;
        for e in out.layout.elements.iter_mut() {
            let Element::MapFrame { at, crs, center, scale, follows_series: true, .. } = e else { continue };
            followed += 1;
            if crs.trim().is_empty() {
                bail!("Set a coordinate system for the map frame '{}' before using the series", at.id);
            }
            let fc = FrameCrs::new(crs, &body)?;
            let bbox = self.with_conn(|c| feature_bbox(c, &info, p.fid, crs))?;
            let cx = [(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0];
            let (bw, bh) = (bbox[2] - bbox[0], bbox[3] - bbox[1]);
            *center = cx;
            *scale = match spec.extent.as_str() {
                "fixed_scale" => spec.scale.unwrap_or(POINT_SCALE),
                _ if bw <= 0.0 && bh <= 0.0 => spec.scale.unwrap_or(POINT_SCALE),
                _ => fit_scale(&body, &fc, cx, [bw, bh], at.w, at.h, spec.margin_pct)?,
            };
        }
        if followed == 0 {
            bail!("No map frame follows the series. Select a map frame and tick \"Follows the map series\" in its properties.");
        }
        Ok((out, p, count))
    }

    /// Export the whole series: PDF = one multi-page file at `path`; PNG = one file per page in the folder `path`
    /// (`<layout>_<NNN>_<name>.png`). All or nothing: nothing is left behind after an error or Cancel. Returns the files
    /// written and the render warnings, each tagged with its page.
    pub fn export_series(&self, req: &RenderRequest, format: ExportFormat, dpi: f64, path: &Path) -> Result<SeriesExport> {
        if format == ExportFormat::Svg {
            bail!("A map series is exported as one PDF or as PNG files; choose one of those");
        }
        let pages = self.series_pages(&req.layout)?;
        let n = pages.len();
        let task = Task::start(format!("Exporting map series — {}", req.layout.name)).cancellable();
        let stage = (format == ExportFormat::Png).then(|| path.join(format!(".{}-series.part", safe(&req.layout.name))));
        if let Some(s) = &stage {
            std::fs::create_dir_all(s).map_err(|e| anyhow!("Could not create {}: {e}", s.display()))?;
        }
        let cleanup = |stage: &Option<PathBuf>| {
            if let Some(s) = stage {
                let _ = std::fs::remove_dir_all(s);
            }
        };
        let run = (|| -> Result<(Vec<String>, Vec<String>)> {
            let mut svgs: Vec<String> = Vec::new();
            let mut warnings = Vec::new();
            for (i, page) in pages.iter().enumerate() {
                task.check()?;
                task.set(i as f64 / n as f64 * 0.95, Some(format!("Page {} of {n}: {}", i + 1, page.name)));
                let mut r = req.clone();
                r.series_page = Some(i);
                r.raster_dpi.get_or_insert(dpi);
                let out: LayoutSvg = self.render_layout_svg(&r, &|| task.check()).map_err(|e| anyhow!("Page {} ({}): {e:#}", i + 1, page.name))?;
                warnings.extend(out.warnings.iter().map(|w| format!("Page {} ({}): {w}", i + 1, page.name)));
                match &stage {
                    Some(dir) => std::fs::write(dir.join(format!("{:03}.png", i + 1)), export::svg_to_png(&out.svg, dpi)?)?,
                    None => svgs.push(out.svg),
                }
            }
            task.check()?;
            task.set(0.97, Some("Writing the files"));
            Ok((svgs, warnings))
        })();
        let (svgs, warnings) = match run {
            Ok(v) => v,
            Err(e) => {
                cleanup(&stage);
                return Err(e);
            }
        };
        let mut files = Vec::new();
        match &stage {
            None => {
                let pdf = export::svgs_to_pdf(&svgs)?;
                write_atomic(path, &pdf)?;
                files.push(path.to_path_buf());
            }
            Some(dir) => {
                let mut moved: Vec<PathBuf> = Vec::new();
                for (i, page) in pages.iter().enumerate() {
                    let to = path.join(format!("{}_{:03}_{}.png", safe(&req.layout.name), i + 1, safe(&page.name)));
                    if let Err(e) = std::fs::rename(dir.join(format!("{:03}.png", i + 1)), &to) {
                        for f in &moved {
                            let _ = std::fs::remove_file(f);
                        }
                        cleanup(&stage);
                        return Err(anyhow!("Could not write {}: {e}", to.display()));
                    }
                    moved.push(to);
                }
                cleanup(&stage);
                files = moved;
            }
        }
        Ok(SeriesExport { pages: n, files, warnings })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesExport {
    pub pages: usize,
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

/// Bounding box `[xmin, ymin, xmax, ymax]` of one feature in `crs`.
fn feature_bbox(conn: &duckdb::Connection, info: &crate::layers::LayerInfo, fid: i64, crs: &str) -> Result<[f64; 4]> {
    let (src, dst) = (crate::crs::resolve(info.crs.as_deref().unwrap_or("")), crate::crs::resolve(crs));
    let g = if src == dst { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, true)", lit(&src), lit(&dst)) };
    let sql = format!("SELECT ST_XMin(g), ST_YMin(g), ST_XMax(g), ST_YMax(g) FROM (SELECT {g} AS g FROM {} WHERE __fid = {fid})", ident(&info.table));
    let b: [Option<f64>; 4] = conn.query_row(&sql, [], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?])).map_err(|_| anyhow!("The feature has no geometry that can be shown in the frame's coordinate system"))?;
    match b {
        [Some(a), Some(b2), Some(c), Some(d)] if [a, b2, c, d].iter().all(|v| v.is_finite()) => Ok([a, b2, c, d]),
        _ => bail!("The feature's extent can't be worked out in the frame's coordinate system"),
    }
}

/// The scale at which a frame of `w_mm` × `h_mm` shows a feature of `size` (CRS units) with `margin_pct` on each side,
/// rounded up to a whole number.
fn fit_scale(body: &Body, crs: &FrameCrs, center: [f64; 2], size: [f64; 2], w_mm: f64, h_mm: f64, margin_pct: f64) -> Result<f64> {
    let k = 1.0 + 2.0 * margin_pct / 100.0;
    let (bw, bh) = (size[0] * k, size[1] * k);
    let e = geo::frame_extent(body, crs, center, 1_000_000.0, w_mm, h_mm)?;
    let (w0, h0) = (e[2] - e[0], e[3] - e[1]);
    let mut s = (1_000_000.0 * (bw / w0).max(bh / h0)).max(1.0).ceil();
    // Geographic frames aren't exactly linear in scale: grow until the padded feature fits.
    for _ in 0..30 {
        let e = geo::frame_extent(body, crs, center, s, w_mm, h_mm)?;
        if e[2] - e[0] >= bw && e[3] - e[1] >= bh {
            break;
        }
        s = (s * 1.03).ceil();
    }
    Ok(s)
}
