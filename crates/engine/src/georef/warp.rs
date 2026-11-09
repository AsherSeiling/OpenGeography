//! Raster georeferencing (GR2): fit control points with GDAL's own GCP polynomial or thin plate spline transformer, report the
//! errors from that fit, and warp the image with it.
//!
//! There is one transformer object. [`fit`] builds it (a GDAL "GenImgProj" transformer on a VRT that carries the enabled points as
//! GCPs, with `METHOD=GCP_POLYNOMIAL` and `MAX_GCP_ORDER` or `METHOD=GCP_TPS`). The ground figures (dx, dy, length, metres, RMS) are
//! computed by calling it forward (image to ground); that forward fit also fixes the output extent and cell size. The pixels are moved
//! through the *reverse* direction (ground to image), which GDAL fits as a separate polynomial of the same order (for a spline, a
//! separate spline); `dpixel`, `dline` and `length_px` measure that reverse fit and nothing else. The warp sets its output grid on this
//! same object, so the figures come from the transformer the warp uses, but the ground errors describe the forward fit and the pixel
//! errors the reverse fit. No fitting maths is written here.
//!
//! The source image is opened read-only with GDAL's PAM sidecar writing switched off for this thread, so no `.aux.xml` or other file
//! appears beside it. All working files are in GDAL's in-memory file system or the output location.

use super::{ControlPoint, ControlSet};
use crate::progress::{self, TaskHandle};
use anyhow::{anyhow, bail, Context, Result};
use gdal::Dataset;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde::{Deserialize, Serialize};
use std::ffi::{c_int, c_void, CString};
use std::path::Path;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// How the control points are turned into a transform.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Method {
    /// Least-squares polynomial of order 1 (affine), 2 or 3.
    Polynomial { order: u8 },
    /// Thin plate spline: passes through every enabled point.
    Tps,
}

impl Method {
    /// Fewest enabled points the method needs (the polynomial's coefficient count; 3 for a spline).
    pub fn min_points(self) -> usize {
        match self {
            Method::Polynomial { order: 1 } => 3,
            Method::Polynomial { order: 2 } => 6,
            Method::Polynomial { .. } => 10,
            Method::Tps => 3,
        }
    }

    pub fn label(self) -> String {
        match self {
            Method::Polynomial { order: 1 } => "1st-order polynomial (affine)".into(),
            Method::Polynomial { order: 2 } => "2nd-order polynomial".into(),
            Method::Polynomial { order } => format!("{order}rd-order polynomial"),
            Method::Tps => "thin plate spline".into(),
        }
    }

    fn validate(self) -> Result<()> {
        match self {
            Method::Polynomial { order } if !(1..=3).contains(&order) => bail!("Polynomial order must be 1, 2 or 3 (not {order})."),
            _ => Ok(()),
        }
    }
}

/// The error of one control point against the fitted transform.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PointError {
    pub id: u32,
    pub enabled: bool,
    /// True for a point that was left out of the fit: its error is an independent accuracy figure.
    pub check: bool,
    /// Where the transform puts the point minus where it was said to be, in target CRS units (degrees when geographic).
    pub dx: f64,
    pub dy: f64,
    /// sqrt(dx² + dy²) in target CRS units.
    pub length: f64,
    /// Metres: geodesic distance on the CRS's ellipsoid when the target is geographic; the grid distance times the unit's metres
    /// per unit when it is projected (so it carries the projection's scale error).
    pub length_m: f64,
    /// Where the transform puts the ground position back in the image minus where the point was clicked, in source pixels.
    pub dpixel: f64,
    pub dline: f64,
    pub length_px: f64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReport {
    pub method: Method,
    pub method_label: String,
    pub min_points: usize,
    pub enabled_count: usize,
    pub points: Vec<PointError>,
    /// sqrt(sum(dx² + dy²) / n) over the enabled points, in target CRS units. Zero is shown as 0, never as "not available".
    pub rms: f64,
    /// RMS of the metre lengths, and of the source-pixel lengths, over the enabled points.
    pub rms_m: f64,
    pub rms_px: f64,
    /// The same three figures over the points that are turned off (check points); None when there are none.
    pub check_rms: Option<f64>,
    pub check_rms_m: Option<f64>,
    pub check_rms_px: Option<f64>,
    /// "degree", "metre", "US survey foot" …
    pub unit: String,
    /// "geodesic" (target is geographic) or "grid" (projected: grid distance in metres, not corrected for scale).
    pub metres_kind: String,
    /// The RMS formula, for the tooltip.
    pub formula: String,
    /// Things to know about reading these figures, such as the spline's zero error at its own points.
    pub warnings: Vec<String>,
}

pub const RMS_FORMULA: &str = "RMS = sqrt( sum(dx² + dy²) / n ) over the n enabled points, where (dx, dy) is where the fitted transform puts a control point minus where you said it is, in the target coordinate system. Points turned off are not in the sum; their errors are shown as check points.";

fn spline_warning() -> String {
    "A thin plate spline passes exactly through every enabled control point, so their error is zero whatever the quality of the picking. That figure says nothing about accuracy at those points. Turn a few points off and read them as check points: that is the honest error.".into()
}

fn rms(v: impl Iterator<Item = f64>) -> Option<f64> {
    let (mut s, mut n) = (0.0, 0usize);
    for x in v {
        s += x * x;
        n += 1;
    }
    (n > 0).then(|| (s / n as f64).sqrt())
}

/// True when the (x, y) positions all lie on one straight line (or one point): the smaller spread is a negligible part of the larger.
fn collinear(p: &[(f64, f64)]) -> bool {
    let n = p.len() as f64;
    let (mx, my) = (p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for q in p {
        let (dx, dy) = (q.0 - mx, q.1 - my);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let tr = sxx + syy;
    let det = sxx * syy - sxy * sxy;
    if tr <= 0.0 {
        return true;
    }
    let disc = (tr * tr / 4.0 - det).max(0.0).sqrt();
    let (l1, l2) = (tr / 2.0 + disc, (tr / 2.0 - disc).max(0.0));
    l2 / l1 < 1e-9
}

/// The enabled points of a set, after the checks every fit needs; errors say what to do.
fn usable<'a>(set: &'a ControlSet, method: Method) -> Result<Vec<&'a ControlPoint>> {
    method.validate()?;
    set.validate()?;
    let pts: Vec<&ControlPoint> = set.points.iter().filter(|p| p.enabled).collect();
    let need = method.min_points();
    if pts.len() < need {
        bail!("A {} needs at least {need} enabled control points; this set has {}. Add {} more (or turn some back on).", method.label(), pts.len(), need - pts.len());
    }
    if crate::crs::is_geographic(&set.target_crs) {
        super::longitude_span_ok(&pts)?;
    }
    let img: Vec<(f64, f64)> = pts.iter().map(|p| (p.pixel, p.line)).collect();
    let gnd: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
    if collinear(&img) {
        bail!("The enabled control points all lie on one straight line in the image, so they can't fix a position across the picture. Spread them over the image, especially near the corners.");
    }
    if collinear(&gnd) {
        bail!("The enabled control points all lie on one straight line on the ground (in the target coordinates), so they can't fix a position across the picture. Check the coordinates (one point may be very far from the others), and spread the points over the area.");
    }
    for (i, a) in img.iter().enumerate() {
        if let Some(j) = img[..i].iter().position(|b| (b.0 - a.0).abs() < 1e-9 && (b.1 - a.1).abs() < 1e-9) {
            bail!("Control points {} and {} are at the same place in the image.", pts[j].id, pts[i].id);
        }
    }
    Ok(pts)
}

// ---------- the transformer ----------

static VRT_SEQ: AtomicU64 = AtomicU64::new(0);

/// A GDAL image-to-ground transformer built from the control points. Destroyed (and its in-memory VRT removed) on drop.
pub(crate) struct Transformer {
    h: *mut c_void,
    vrt: Option<Dataset>,
    vrt_path: String,
}

// The handle is used by one thread at a time (a fit or a warp owns its transformer).
unsafe impl Send for Transformer {}

impl Drop for Transformer {
    fn drop(&mut self) {
        if !self.h.is_null() {
            unsafe { gdal_sys::GDALDestroyGenImgProjTransformer(self.h) };
        }
        self.vrt.take();
        let _ = gdal::vsi::unlink_mem_file(&self.vrt_path);
    }
}

fn cstr(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| anyhow!("A text value held a null character."))
}

fn gdal_msg() -> String {
    unsafe { std::ffi::CStr::from_ptr(gdal_sys::CPLGetLastErrorMsg()) }.to_string_lossy().trim().to_string()
}

impl Transformer {
    /// Fit `pts` (the enabled points) with `method`, on a VRT over `src` that carries them as GCPs. `src` is only read.
    fn new(src: &Dataset, set: &ControlSet, pts: &[&ControlPoint], method: Method, extra_translate: &[&str]) -> Result<Transformer> {
        let wkt = crate::crs::srs(&crate::crs::resolve(&set.target_crs))?.to_wkt()?;
        let n = VRT_SEQ.fetch_add(1, Ordering::Relaxed);
        let vrt_path = format!("/vsimem/georef_gcp_{}_{n}.vrt", std::process::id());
        let mut args: Vec<String> = vec!["-of".into(), "VRT".into()];
        args.extend(extra_translate.iter().map(|s| s.to_string()));
        for p in pts {
            args.extend(["-gcp".into(), p.pixel.to_string(), p.line.to_string(), p.x.to_string(), p.y.to_string()]);
        }
        args.extend(["-a_srs".into(), wkt.clone()]);
        crate::raster::translate(src, Path::new(&vrt_path), &args).context("Could not attach the control points to the image")?;
        let vrt = Dataset::open(&vrt_path).context("Could not read back the image with its control points")?;
        let mut opts = gdal::cpl::CslStringList::new();
        match method {
            Method::Polynomial { order } => {
                opts.add_string("METHOD=GCP_POLYNOMIAL")?;
                opts.add_string(&format!("MAX_GCP_ORDER={order}"))?;
            }
            Method::Tps => opts.add_string("METHOD=GCP_TPS")?,
        }
        // Both ends are the target CRS: the ground positions are used as given, never reprojected here.
        opts.add_string(&format!("SRC_SRS={wkt}"))?;
        opts.add_string(&format!("DST_SRS={wkt}"))?;
        let h = unsafe { gdal_sys::GDALCreateGenImgProjTransformer2(vrt.c_dataset(), null_mut(), opts.as_ptr()) };
        if h.is_null() {
            let m = gdal_msg();
            let _ = gdal::vsi::unlink_mem_file(&vrt_path);
            bail!("GDAL could not fit a {} to these {} control points{}. They may be too close to a pattern (a line or a circle) for this method; spread them over the image or try a lower order.", method.label(), pts.len(), if m.is_empty() { String::new() } else { format!(" ({m})") });
        }
        Ok(Transformer { h, vrt: Some(vrt), vrt_path })
    }

    fn apply(&self, to_src: bool, pts: &mut [(f64, f64)]) -> Result<()> {
        let mut xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let mut ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        let mut ok = vec![0 as c_int; pts.len()];
        let r = unsafe { gdal_sys::GDALGenImgProjTransform(self.h, to_src as c_int, pts.len() as c_int, xs.as_mut_ptr(), ys.as_mut_ptr(), null_mut(), ok.as_mut_ptr()) };
        if r == 0 || ok.iter().any(|&s| s == 0) || xs.iter().chain(ys.iter()).any(|v| !v.is_finite()) {
            bail!("The fitted transform could not be applied to every control point. The points may be badly placed for this method.");
        }
        for (p, (x, y)) in pts.iter_mut().zip(xs.into_iter().zip(ys)) {
            *p = (x, y);
        }
        Ok(())
    }

    /// Image (pixel, line) to ground (x, y) in the target CRS. Valid before the output grid is set on the transformer.
    fn forward(&self, pts: &mut [(f64, f64)]) -> Result<()> {
        self.apply(false, pts)
    }

    /// Ground (x, y) to image (pixel, line).
    fn backward(&self, pts: &mut [(f64, f64)]) -> Result<()> {
        self.apply(true, pts)
    }
}

/// Ellipsoid (a, f) and unit of a CRS for the metre figures.
struct Units {
    geographic: bool,
    unit: String,
    metres_per_unit: f64,
    geod: Option<Geodesic>,
}

fn units_of(crs: &str) -> Result<Units> {
    let info = crate::crs::info(crs)?;
    let s = crate::crs::srs(&crate::crs::resolve(crs))?;
    if info.geographic {
        let (a, b) = (s.semi_major()?, s.semi_minor()?);
        let f = if a > 0.0 { (a - b) / a } else { 0.0 };
        Ok(Units { geographic: true, unit: info.unit, metres_per_unit: 1.0, geod: Some(Geodesic::new(a, f)) })
    } else {
        Ok(Units { geographic: false, unit: info.unit, metres_per_unit: s.linear_units(), geod: None })
    }
}

/// Build the transformer for `set`/`method` and compute every error figure from it.
fn fit(src: &Dataset, set: &ControlSet, method: Method, extra_translate: &[&str]) -> Result<(Transformer, ErrorReport)> {
    let pts = usable(set, method)?;
    let t = Transformer::new(src, set, &pts, method, extra_translate)?;
    let units = units_of(&set.target_crs)?;
    let mut fwd: Vec<(f64, f64)> = set.points.iter().map(|p| (p.pixel, p.line)).collect();
    t.forward(&mut fwd)?;
    let mut back: Vec<(f64, f64)> = set.points.iter().map(|p| (p.x, p.y)).collect();
    t.backward(&mut back)?;
    let mut points = Vec::with_capacity(set.points.len());
    let mut off_globe: Vec<u32> = Vec::new();
    for (i, p) in set.points.iter().enumerate() {
        let (dx, dy) = (fwd[i].0 - p.x, fwd[i].1 - p.y);
        let length = dx.hypot(dy);
        let length_m = match &units.geod {
            Some(g) => {
                let (m, off) = super::geodesic_m(g, (p.x, p.y), fwd[i]);
                if off {
                    off_globe.push(p.id);
                }
                m
            }
            None => length * units.metres_per_unit,
        };
        let (dpixel, dline) = (back[i].0 - p.pixel, back[i].1 - p.line);
        points.push(PointError { id: p.id, enabled: p.enabled, check: !p.enabled, dx, dy, length, length_m, dpixel, dline, length_px: dpixel.hypot(dline) });
    }
    let on = || points.iter().filter(|e| e.enabled);
    let off = || points.iter().filter(|e| !e.enabled);
    let mut warnings = Vec::new();
    if method == Method::Tps {
        warnings.push(spline_warning());
    }
    if let Some(w) = super::exact_fit_warning(pts.len(), method.min_points()) {
        warnings.push(w);
    }
    if !off_globe.is_empty() {
        let ids: Vec<String> = off_globe.iter().map(|i| i.to_string()).collect();
        warnings.push(format!("The fit puts control point{} {} off the globe (a latitude beyond 90°). Its metre figure is a lower bound, measured to the pole. Check that point, or use a lower order.", if off_globe.len() == 1 { "" } else { "s" }, ids.join(", ")));
    }
    if method == Method::Tps && off().next().is_none() {
        warnings.push("No check points yet: every point is in the fit, so no honest accuracy figure exists.".into());
    }
    let report = ErrorReport {
        method,
        method_label: method.label(),
        min_points: method.min_points(),
        enabled_count: pts.len(),
        rms: rms(on().map(|e| e.length)).unwrap_or(0.0),
        rms_m: rms(on().map(|e| e.length_m)).unwrap_or(0.0),
        rms_px: rms(on().map(|e| e.length_px)).unwrap_or(0.0),
        check_rms: rms(off().map(|e| e.length)),
        check_rms_m: rms(off().map(|e| e.length_m)),
        check_rms_px: rms(off().map(|e| e.length_px)),
        points,
        unit: units.unit,
        metres_kind: if units.geographic { "geodesic" } else { "grid" }.into(),
        formula: RMS_FORMULA.into(),
        warnings,
    };
    Ok((t, report))
}

/// A 1 x 1 stand-in image in GDAL's memory file system: the fit needs only the points, not the picture. It is a file (not a MEM
/// dataset) so the VRT that carries the points can be read back.
struct Dummy {
    path: String,
    ds: Option<Dataset>,
}

impl Dummy {
    fn new() -> Result<Dummy> {
        let path = format!("/vsimem/georef_dummy_{}_{}.tif", std::process::id(), VRT_SEQ.fetch_add(1, Ordering::Relaxed));
        let drv = gdal::DriverManager::get_driver_by_name("GTiff")?;
        drop(drv.create_with_band_type::<u8, _>(&path, 1, 1, 1)?);
        let ds = Dataset::open(&path)?;
        Ok(Dummy { path, ds: Some(ds) })
    }
}

impl Drop for Dummy {
    fn drop(&mut self) {
        self.ds.take();
        let _ = gdal::vsi::unlink_mem_file(&self.path);
    }
}

/// Per-point and overall errors for a control point set and method, without reading any image. The numbers come from the same
/// transformer construction the warp uses ([`fit`]).
pub fn errors(set: &ControlSet, method: Method) -> Result<ErrorReport> {
    let _pam = PamOff::new();
    let dummy = Dummy::new()?;
    Ok(fit(dummy.ds.as_ref().expect("dummy image"), set, method, &[])?.1)
}

/// Switches GDAL's sidecar (`.aux.xml`) writing off for this thread while alive.
struct PamOff;
impl PamOff {
    fn new() -> PamOff {
        let _ = gdal::config::set_thread_local_config_option("GDAL_PAM_ENABLED", "NO");
        PamOff
    }
}
impl Drop for PamOff {
    fn drop(&mut self) {
        let _ = gdal::config::clear_thread_local_config_option("GDAL_PAM_ENABLED");
    }
}

// ---------- the warp ----------

/// Cancel and progress for one GDAL call: an explicit stop flag and, for a saved warp, a progress task.
pub struct Ctx {
    pub stop: Arc<AtomicBool>,
    pub task: Option<TaskHandle>,
}

unsafe extern "C" fn warp_progress(done: f64, _msg: *const std::ffi::c_char, arg: *mut c_void) -> c_int {
    let ctx = &*(arg as *const Ctx);
    if ctx.stop.load(Ordering::Relaxed) {
        return 0;
    }
    if let Some(t) = &ctx.task {
        if t.check().is_err() {
            return 0;
        }
        t.report(done, "Warping".into());
    }
    1
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WarpOutput {
    pub width: usize,
    pub height: usize,
    /// GDAL geotransform of the output, in the target CRS.
    pub geo_transform: [f64; 6],
    /// The same transform's errors (computed before the warp from the object the warp used).
    pub report: ErrorReport,
}

pub fn resampling_alg(name: &str) -> Result<gdal_sys::GDALResampleAlg::Type> {
    use gdal_sys::GDALResampleAlg::*;
    Ok(match name.trim().to_ascii_lowercase().as_str() {
        "" | "nearest" => GRA_NearestNeighbour,
        "bilinear" => GRA_Bilinear,
        "cubic" => GRA_Cubic,
        "cubicspline" => GRA_CubicSpline,
        "lanczos" => GRA_Lanczos,
        other => bail!("'{other}' is not a resampling method. Use nearest, bilinear, cubic, cubicspline or lanczos."),
    })
}

/// Where a warp writes.
pub enum Dest<'a> {
    /// A new GeoTIFF at this path.
    GeoTiff(&'a Path),
    /// In memory (for the preview picture).
    Memory,
}

const MAX_SIDE: usize = 200_000;
const MAX_PIXELS: usize = 1_500_000_000;

/// Open an image read-only for georeferencing.
pub fn open_image(path: &Path) -> Result<Dataset> {
    Dataset::open_ex(path, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_RASTER | gdal::GdalOpenFlags::GDAL_OF_READONLY, ..Default::default() })
        .with_context(|| format!("GDAL could not open the image {}", path.display()))
}

/// Fit, then warp `image` into `dest` in the target CRS. With `max_side`, the output is shrunk so its longer side has at most that
/// many pixels (previews). Returns the dataset (for the preview) and the output facts.
pub fn run(image: &Path, set: &ControlSet, method: Method, resampling: &str, dest: Dest, max_side: Option<usize>, ctx: &Ctx) -> Result<(Dataset, WarpOutput)> {
    let _pam = PamOff::new();
    let alg = resampling_alg(resampling)?;
    let src = open_image(image)?;
    let (sw, sh) = src.raster_size();
    if src.raster_count() == 0 {
        bail!("{} has no raster bands.", image.display());
    }
    // A palette image is expanded to colours first: resampling palette indexes would invent colours.
    let palette = src.rasterband(1).map(|b| b.color_interpretation() == gdal::raster::ColorInterpretation::PaletteIndex).unwrap_or(false);
    let extra: Vec<&str> = if palette { vec!["-expand", "rgba"] } else { vec![] };
    let (t, report) = fit(&src, set, method, &extra)?;
    let vrt = t.vrt.as_ref().expect("the transformer holds its dataset");

    // Source bands: everything except an alpha band, which the warper handles as alpha.
    let nb = vrt.raster_count();
    let last_alpha = vrt.rasterband(nb)?.color_interpretation() == gdal::raster::ColorInterpretation::AlphaBand;
    let data_bands = if last_alpha { nb - 1 } else { nb };
    if data_bands == 0 {
        bail!("{} has only an alpha band.", image.display());
    }
    let b1 = vrt.rasterband(1)?;
    let dtype = b1.band_type();
    let float = matches!(dtype.name().as_str(), "Float32" | "Float64");
    let nodata = if last_alpha { None } else { b1.no_data_value() };
    // Outside the image is NoData or transparent: never a false 0. An image with neither gets an alpha band; float data gets NaN.
    let (dst_nodata, add_alpha) = match nodata {
        Some(nd) => (Some(nd), false),
        None if !last_alpha && float => (Some(f64::NAN), false),
        None => (None, !last_alpha),
    };
    let dst_bands = data_bands + usize::from(last_alpha || add_alpha);
    let dst_alpha = last_alpha || add_alpha;

    // Output grid: GDAL's suggested extent and resolution from this same transformer (the transformer still maps to ground units).
    let mut gt = [0.0; 6];
    let (mut nx, mut ny): (c_int, c_int) = (0, 0);
    let r = unsafe { gdal_sys::GDALSuggestedWarpOutput(vrt.c_dataset(), Some(gdal_sys::GDALGenImgProjTransform), t.h, gt.as_mut_ptr(), &mut nx, &mut ny) };
    if r != gdal_sys::CPLErr::CE_None || nx <= 0 || ny <= 0 {
        bail!("GDAL could not work out where this image lands with these control points. Check the points: {}", gdal_msg());
    }
    let (mut nx, mut ny) = (nx as usize, ny as usize);
    if let Some(m) = max_side {
        let big = nx.max(ny);
        if big > m {
            let k = m as f64 / big as f64;
            gt[1] /= k;
            gt[5] /= k;
            nx = ((nx as f64 * k).round() as usize).max(1);
            ny = ((ny as f64 * k).round() as usize).max(1);
        }
    }
    if nx > MAX_SIDE || ny > MAX_SIDE || nx.saturating_mul(ny) > MAX_PIXELS {
        bail!("With these control points the image would be stretched to {nx} by {ny} pixels, which is far larger than the picture ({sw} by {sh}). A point is probably wrong; check the points with the largest errors.");
    }

    // Create the destination.
    let (drv_name, path_str) = match &dest {
        Dest::GeoTiff(p) => ("GTiff", p.to_str().ok_or_else(|| anyhow!("The output path is not text."))?.to_string()),
        Dest::Memory => ("MEM", String::new()),
    };
    let drv = gdal::DriverManager::get_driver_by_name(drv_name)?;
    let mut co = gdal::cpl::CslStringList::new();
    if matches!(dest, Dest::GeoTiff(_)) {
        for o in ["TILED=YES", "COMPRESS=DEFLATE", "BIGTIFF=IF_SAFER"] {
            co.add_string(o)?;
        }
        if add_alpha || last_alpha {
            co.add_string("ALPHA=YES")?;
        }
    }
    let cpath = cstr(&path_str)?;
    let dst_h = unsafe { gdal_sys::GDALCreate(drv.c_driver(), cpath.as_ptr(), nx as c_int, ny as c_int, dst_bands as c_int, dtype as gdal_sys::GDALDataType::Type, co.as_ptr()) };
    if dst_h.is_null() {
        bail!("Could not create the output image ({nx} by {ny} pixels): {}", gdal_msg());
    }
    let mut dst = unsafe { Dataset::from_c_dataset(dst_h) };
    let cleanup = |dst: Dataset| {
        drop(dst);
        if let Dest::GeoTiff(p) = &dest {
            let _ = std::fs::remove_file(p);
            let _ = std::fs::remove_file(format!("{}.aux.xml", p.display()));
        }
    };
    let wkt = crate::crs::srs(&crate::crs::resolve(&set.target_crs))?.to_wkt()?;
    dst.set_geo_transform(&gt)?;
    dst.set_projection(&wkt)?;
    for i in 1..=data_bands {
        let mut b = dst.rasterband(i)?;
        if let Ok(ci) = vrt.rasterband(i).map(|b| b.color_interpretation()) {
            let _ = b.set_color_interpretation(ci);
        }
        if let Some(nd) = dst_nodata {
            b.set_no_data_value(Some(nd))?;
        }
    }
    if dst_alpha {
        let mut a = dst.rasterband(dst_bands)?;
        a.set_color_interpretation(gdal::raster::ColorInterpretation::AlphaBand)?;
    }
    // The output grid goes on the same transformer: from here it maps output pixels to image pixels.
    unsafe { gdal_sys::GDALSetGenImgProjTransformerDstGeoTransform(t.h, gt.as_ptr()) };

    let status = unsafe {
        let o = gdal_sys::GDALCreateWarpOptions();
        (*o).hSrcDS = vrt.c_dataset();
        (*o).hDstDS = dst.c_dataset();
        (*o).nBandCount = data_bands as c_int;
        let bands = gdal_sys::CPLMalloc(4 * data_bands) as *mut c_int;
        let bands_d = gdal_sys::CPLMalloc(4 * data_bands) as *mut c_int;
        for i in 0..data_bands {
            *bands.add(i) = i as c_int + 1;
            *bands_d.add(i) = i as c_int + 1;
        }
        (*o).panSrcBands = bands;
        (*o).panDstBands = bands_d;
        (*o).nSrcAlphaBand = if last_alpha { nb as c_int } else { 0 };
        (*o).nDstAlphaBand = if dst_alpha { dst_bands as c_int } else { 0 };
        (*o).eResampleAlg = alg;
        (*o).pfnTransformer = Some(gdal_sys::GDALGenImgProjTransform);
        (*o).pTransformerArg = t.h;
        (*o).pfnProgress = Some(warp_progress);
        (*o).pProgressArg = ctx as *const Ctx as *mut c_void;
        let init = if dst_nodata.is_some() { "NO_DATA" } else { "0" };
        (*o).papszWarpOptions = gdal_sys::CSLSetNameValue(null_mut(), cstr("INIT_DEST")?.as_ptr(), cstr(init)?.as_ptr());
        if let (Some(nd_dst), true) = (dst_nodata, true) {
            let alloc = |v: f64| {
                let a = gdal_sys::CPLMalloc(8 * data_bands) as *mut f64;
                for i in 0..data_bands {
                    *a.add(i) = v;
                }
                a
            };
            (*o).padfDstNoDataReal = alloc(nd_dst);
            (*o).padfDstNoDataImag = alloc(0.0);
            if let Some(nd_src) = nodata {
                (*o).padfSrcNoDataReal = alloc(nd_src);
                (*o).padfSrcNoDataImag = alloc(0.0);
            }
        }
        let op = gdal_sys::GDALCreateWarpOperation(o);
        let rc = if op.is_null() { gdal_sys::CPLErr::CE_Failure } else { gdal_sys::GDALChunkAndWarpImage(op, 0, 0, nx as c_int, ny as c_int) };
        if !op.is_null() {
            gdal_sys::GDALDestroyWarpOperation(op);
        }
        let msg = gdal_msg();
        gdal_sys::GDALDestroyWarpOptions(o);
        (rc, msg)
    };
    if status.0 != gdal_sys::CPLErr::CE_None {
        let cancelled = ctx.stop.load(Ordering::Relaxed) || ctx.task.as_ref().is_some_and(|t| t.check().is_err());
        cleanup(dst);
        if cancelled {
            bail!("{}", progress::CANCELLED);
        }
        bail!("The warp failed: {}", if status.1.is_empty() { "GDAL gave no reason".into() } else { status.1 });
    }
    drop(t);
    Ok((dst, WarpOutput { width: nx, height: ny, geo_transform: gt, report }))
}

// ---------- preview: only the latest request keeps running ----------

fn latest_preview() -> &'static parking_lot::Mutex<Option<Arc<AtomicBool>>> {
    static L: std::sync::OnceLock<parking_lot::Mutex<Option<Arc<AtomicBool>>>> = std::sync::OnceLock::new();
    L.get_or_init(Default::default)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// PNG, base64.
    pub png: String,
    /// [xmin, ymin, xmax, ymax] of the picture in the target CRS.
    pub extent: [f64; 4],
    pub width: usize,
    pub height: usize,
    pub crs: String,
    pub report: ErrorReport,
}

/// A low-resolution picture of the warped image, made in memory (nothing is written to the project or beside the image). Asking for a
/// new preview stops the one still running: that one returns the error "Cancelled".
pub fn preview(image: &Path, set: &ControlSet, method: Method, resampling: &str, max_side: usize) -> Result<Preview> {
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(old) = latest_preview().lock().replace(stop.clone()) {
        old.store(true, Ordering::Relaxed);
    }
    let ctx = Ctx { stop: stop.clone(), task: None };
    let (mem, out) = run(image, set, method, resampling, Dest::Memory, Some(max_side.clamp(64, 2048)), &ctx)?;
    if stop.load(Ordering::Relaxed) {
        bail!("{}", progress::CANCELLED);
    }
    let _pam = PamOff::new();
    let png_path = format!("/vsimem/georef_preview_{}_{}.png", std::process::id(), VRT_SEQ.fetch_add(1, Ordering::Relaxed));
    let float_or_wide = !matches!(mem.rasterband(1)?.band_type().name().as_str(), "Byte" | "UInt8");
    let mut args: Vec<String> = vec!["-of".into(), "PNG".into()];
    if float_or_wide {
        args.extend(["-ot".into(), "Byte".into(), "-scale".into()]);
    }
    // PNG holds 1 to 4 bands; keep the first colour bands and the alpha.
    if mem.raster_count() > 4 {
        args.extend(["-b", "1", "-b", "2", "-b", "3"].map(String::from));
    }
    crate::raster::translate(&mem, Path::new(&png_path), &args).context("Could not make the preview picture")?;
    let bytes = gdal::vsi::get_vsi_mem_file_bytes_owned(&png_path)?;
    let _ = gdal::vsi::unlink_mem_file(&png_path);
    use base64::Engine as _;
    let gt = out.geo_transform;
    let (x1, y1) = (gt[0] + gt[1] * out.width as f64, gt[3] + gt[5] * out.height as f64);
    Ok(Preview {
        png: base64::engine::general_purpose::STANDARD.encode(bytes),
        extent: [gt[0].min(x1), gt[3].min(y1), gt[0].max(x1), gt[3].max(y1)],
        width: out.width,
        height: out.height,
        crs: set.target_crs.clone(),
        report: out.report,
    })
}

// ---------- the image viewer ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub width: usize,
    pub height: usize,
    pub bands: usize,
    /// The file already carries georeferencing (a geotransform, GCPs or a CRS); the control points will replace it in the output.
    pub has_georef: bool,
    pub crs: Option<String>,
}

pub fn image_info(path: &Path) -> Result<ImageInfo> {
    let _pam = PamOff::new();
    let ds = open_image(path)?;
    let (w, h) = ds.raster_size();
    let gt = ds.geo_transform().ok();
    let default_gt = gt.map(|g| g == [0.0, 1.0, 0.0, 0.0, 0.0, 1.0]).unwrap_or(true);
    let (crs, _) = crate::raster::crs_of(&ds);
    let gcps = unsafe { gdal_sys::GDALGetGCPCount(ds.c_dataset()) } > 0;
    Ok(ImageInfo { width: w, height: h, bands: ds.raster_count(), has_georef: !default_gt || gcps || crs.is_some(), crs })
}

/// A PNG (base64) of the window `[x0, y0, w, h]` (image pixels) resampled to `out_w` by `out_h`, for the image pane.
pub fn image_region(path: &Path, x0: f64, y0: f64, w: f64, h: f64, out_w: usize, out_h: usize) -> Result<String> {
    let _pam = PamOff::new();
    let ds = open_image(path)?;
    let (iw, ih) = ds.raster_size();
    let (x0, y0) = (x0.floor().max(0.0), y0.floor().max(0.0));
    let (x1, y1) = ((x0 + w.ceil()).min(iw as f64), (y0 + h.ceil()).min(ih as f64));
    if x1 <= x0 || y1 <= y0 {
        bail!("That part of the image is outside it.");
    }
    let (out_w, out_h) = (out_w.clamp(1, 4096), out_h.clamp(1, 4096));
    let first = ds.rasterband(1)?;
    let mut args: Vec<String> = vec!["-of".into(), "PNG".into(), "-srcwin".into(), x0.to_string(), y0.to_string(), (x1 - x0).to_string(), (y1 - y0).to_string(), "-outsize".into(), out_w.to_string(), out_h.to_string()];
    args.extend(["-r".into(), if (out_w as f64) < (x1 - x0) { "average" } else { "nearest" }.into()]);
    if first.color_interpretation() == gdal::raster::ColorInterpretation::PaletteIndex {
        args.extend(["-expand".into(), "rgba".into()]);
    } else if !matches!(first.band_type().name().as_str(), "Byte" | "UInt8") {
        args.extend(["-ot".into(), "Byte".into(), "-scale".into()]);
    }
    if ds.raster_count() > 4 {
        args.extend(["-b", "1", "-b", "2", "-b", "3"].map(String::from));
    }
    let png_path = format!("/vsimem/georef_region_{}_{}.png", std::process::id(), VRT_SEQ.fetch_add(1, Ordering::Relaxed));
    crate::raster::translate(&ds, Path::new(&png_path), &args).context("Could not draw that part of the image")?;
    let bytes = gdal::vsi::get_vsi_mem_file_bytes_owned(&png_path)?;
    let _ = gdal::vsi::unlink_mem_file(&png_path);
    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

