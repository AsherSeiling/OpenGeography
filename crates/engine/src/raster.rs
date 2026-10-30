//! Raster layers (GeoTIFF, IMG, ASCII grid, …) through GDAL: import into a
//! raster store with overviews, map tiles (reprojected and colored on the
//! fly), and processing: contours, hillshade, slope, clip.
//!
//! Processing calls GDAL's own utilities (gdalwarp, gdal_translate, gdaldem,
//! gdal_contour) through the C API, so results match those tools exactly.

use anyhow::{anyhow, bail, Context, Result};
use gdal::cpl::CslStringList;
use gdal::Dataset;
use crate::raster_style::{RasterStyle, Renderer};
use serde::{Deserialize, Serialize};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RasterInfo {
    /// File in the raster store.
    pub path: String,
    pub width: usize,
    pub height: usize,
    pub bands: usize,
    pub data_type: String,
    pub nodata: Option<f64>,
    pub min: f64,
    pub max: f64,
    /// "elevation" (one band, colored + shaded) or "rgb" (imagery).
    pub kind: String,
    /// Pixel size in the raster's own units.
    pub pixel_size: f64,
    /// True when every cell is NoData: there is nothing to draw or compute (min and max are 0).
    #[serde(default)]
    pub empty: bool,
}

pub const EXTENSIONS: &[&str] = &["tif", "tiff", "img", "asc", "vrt", "dem", "hgt", "jp2", "nc", "bil", "flt", "adf", "grd"];

pub fn is_raster_path(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).map(|e| EXTENSIONS.contains(&e.to_lowercase().as_str())).unwrap_or(false)
}

/// Where imported and generated rasters live.
pub fn store_dir() -> PathBuf {
    let base = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = if cfg!(target_os = "macos") {
        base.join("Library/Application Support/GIS Studio/rasters")
    } else if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or(base).join("GIS Studio/rasters")
    } else {
        base.join(".local/share/gis-studio/rasters")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A new, unique file path in the raster store.
pub fn new_store_path(stem: &str, ext: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let safe: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    // The clock can repeat on two threads at once; the counter keeps names unique.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let k = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    store_dir().join(format!("{safe}_{n:x}_{k:x}.{ext}"))
}

fn cstr(s: &str) -> Result<CString> {
    Ok(CString::new(s)?)
}

/// `argv` for the other raster modules.
pub(crate) fn argv_pub(args: &[String]) -> Result<CslStringList> {
    argv(args)
}

fn argv(args: &[String]) -> Result<CslStringList> {
    let mut l = CslStringList::new();
    for a in args {
        l.add_string(a)?;
    }
    Ok(l)
}

/// GDAL progress callback: returning 0 makes GDAL stop ("User terminated"),
/// which is how a model run's Cancel interrupts a raster step.
pub(crate) unsafe extern "C" fn gdal_progress(_done: f64, _msg: *const std::ffi::c_char, arg: *mut std::ffi::c_void) -> std::ffi::c_int {
    // `arg` is the running model's cancel flag (progress::cancel_arg), or null.
    let cancelled = !arg.is_null() && (*(arg as *const std::sync::atomic::AtomicBool)).load(std::sync::atomic::Ordering::Relaxed);
    i32::from(!cancelled)
}

/// Remove a partly written output after a failed or cancelled GDAL step.
fn discard(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}.aux.xml", path.display()));
}

fn last_error(what: &str) -> anyhow::Error {
    let msg = unsafe { std::ffi::CStr::from_ptr(gdal_sys::CPLGetLastErrorMsg()) }.to_string_lossy().to_string();
    anyhow!("{what} failed{}", if msg.is_empty() { String::new() } else { format!(": {msg}") })
}

/// gdal_translate.
pub fn translate(src: &Dataset, dest: &Path, args: &[String]) -> Result<()> {
    let list = argv(args)?;
    let d = cstr(dest.to_str().ok_or_else(|| anyhow!("bad path"))?)?;
    unsafe {
        let opts = gdal_sys::GDALTranslateOptionsNew(list.as_ptr() as *mut *mut _, null_mut());
        if opts.is_null() {
            return Err(last_error("Raster copy options"));
        }
        gdal_sys::GDALTranslateOptionsSetProgress(opts, Some(gdal_progress), crate::progress::cancel_arg());
        let mut usage = 0;
        let out = gdal_sys::GDALTranslate(d.as_ptr(), src.c_dataset(), opts, &mut usage);
        gdal_sys::GDALTranslateOptionsFree(opts);
        if out.is_null() {
            discard(dest);
            return Err(last_error("Raster copy"));
        }
        gdal_sys::GDALClose(out);
    }
    Ok(())
}

/// gdalwarp. With an empty `dest` and "-of MEM", returns the in-memory result.
pub fn warp(src: &Dataset, dest: &str, args: &[String]) -> Result<Dataset> {
    let list = argv(args)?;
    let d = cstr(dest)?;
    unsafe {
        let opts = gdal_sys::GDALWarpAppOptionsNew(list.as_ptr() as *mut *mut _, null_mut());
        if opts.is_null() {
            return Err(last_error("Raster warp options"));
        }
        gdal_sys::GDALWarpAppOptionsSetProgress(opts, Some(gdal_progress), crate::progress::cancel_arg());
        let mut srcs = [src.c_dataset()];
        let mut usage = 0;
        let out = gdal_sys::GDALWarp(d.as_ptr(), null_mut(), 1, srcs.as_mut_ptr(), opts, &mut usage);
        gdal_sys::GDALWarpAppOptionsFree(opts);
        if out.is_null() {
            if !dest.is_empty() {
                discard(Path::new(dest));
            }
            return Err(last_error("Raster warp"));
        }
        Ok(Dataset::from_c_dataset(out))
    }
}

/// gdaldem (hillshade, slope, aspect, …).
///
/// gdaldem takes one scale for both axes, but a degree of longitude is `cos(latitude)` shorter than a degree of latitude: on a
/// geographic DEM a slope that runs east–west would come out several degrees too gentle (bug G11, R5). Hillshade, slope and aspect
/// on a geographic raster therefore run on a copy whose pixels have their true size in metres at the raster's centre latitude, and
/// the result gets the original degrees georeferencing back.
pub fn dem(src: &Dataset, dest: &Path, mode: &str, args: &[String]) -> Result<()> {
    if geographic(src) && matches!(mode, "hillshade" | "slope" | "aspect") {
        return dem_geographic(src, dest, mode, args);
    }
    dem_plain(src, dest, mode, args)
}

fn dem_geographic(src: &Dataset, dest: &Path, mode: &str, args: &[String]) -> Result<()> {
    let gt = src.geo_transform()?;
    if gt[2] != 0.0 || gt[4] != 0.0 {
        return Err(anyhow!("This raster is rotated; terrain tools on a rotated geographic raster aren't supported. Warp it to a projected coordinate system first."));
    }
    let (cols, rows) = src.raster_size();
    let lat = gt[3] + gt[5] * rows as f64 / 2.0;
    let p = lat.to_radians();
    // Metres per degree on the WGS 84 ellipsoid at this latitude.
    let per_lon = 111_412.84 * p.cos() - 93.5 * (3.0 * p).cos() + 0.118 * (5.0 * p).cos();
    let per_lat = 111_132.954 - 559.822 * (2.0 * p).cos() + 1.175 * (4.0 * p).cos();
    let (w_m, h_m) = (cols as f64 * gt[1].abs() * per_lon, rows as f64 * gt[5].abs() * per_lat);
    // Callers add a single "-s" scale for geographic rasters; it is replaced by the true pixel size.
    let mut kept: Vec<String> = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if a == "-s" {
            skip = true;
        } else {
            kept.push(a.clone());
        }
    }
    let metric = new_store_path("metric_view", "tif");
    let result = new_store_path("metric_result", "tif");
    let run = (|| -> Result<()> {
        translate(src, &metric, &["-of", "GTiff", "-a_srs", "LOCAL_CS[\"metric\",UNIT[\"metre\",1]]", "-a_ullr", "0", &h_m.to_string(), &w_m.to_string(), "0"].map(String::from))?;
        let m = Dataset::open(&metric)?;
        dem_plain(&m, &result, mode, &kept)?;
        let r = Dataset::open(&result)?;
        let srs = src.spatial_ref()?.to_wkt()?;
        let (ulx, uly) = (gt[0], gt[3]);
        let (lrx, lry) = (gt[0] + cols as f64 * gt[1], gt[3] + rows as f64 * gt[5]);
        translate(&r, dest, &["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-a_srs", &srs, "-a_ullr", &ulx.to_string(), &uly.to_string(), &lrx.to_string(), &lry.to_string()].map(String::from))
    })();
    let _ = std::fs::remove_file(&metric);
    let _ = std::fs::remove_file(&result);
    run
}

fn dem_plain(src: &Dataset, dest: &Path, mode: &str, args: &[String]) -> Result<()> {
    let list = argv(args)?;
    let d = cstr(dest.to_str().ok_or_else(|| anyhow!("bad path"))?)?;
    let m = cstr(mode)?;
    unsafe {
        let opts = gdal_sys::GDALDEMProcessingOptionsNew(list.as_ptr() as *mut *mut _, null_mut());
        if opts.is_null() {
            return Err(last_error(&format!("gdaldem {mode} options")));
        }
        gdal_sys::GDALDEMProcessingOptionsSetProgress(opts, Some(gdal_progress), crate::progress::cancel_arg());
        let mut usage = 0;
        let out = gdal_sys::GDALDEMProcessing(d.as_ptr(), src.c_dataset(), m.as_ptr(), null(), opts, &mut usage);
        gdal_sys::GDALDEMProcessingOptionsFree(opts);
        if out.is_null() {
            discard(dest);
            return Err(last_error(&format!("gdaldem {mode}")));
        }
        gdal_sys::GDALClose(out);
    }
    Ok(())
}

/// True when `sr` is the CRS its own authority code names (same datum, projection, parameters and units).
fn crs_is_its_code(sr: &gdal::spatial_ref::SpatialRef, code: &str) -> bool {
    match (crate::crs::canonical_json(sr), crate::crs::srs(&crate::crs::resolve(code)).and_then(|s| crate::crs::canonical_json(&s))) {
        (Ok(a), Ok(b)) => a == b,
        // If either can't be described, keep the old behaviour: trust the code.
        _ => true,
    }
}

/// WKT1 without the AUTHORITY node of the whole CRS (the last one), so a driver can't replace the definition by its code.
fn strip_top_authority(wkt: &str) -> String {
    let w = wkt.trim();
    if let Some(i) = w.rfind(",AUTHORITY[") {
        let tail = &w[i..];
        if tail.matches('[').count() == 1 && tail.ends_with("]]") {
            return format!("{}]", &w[..i]);
        }
    }
    w.to_string()
}

/// CRS of a raster as "AUTH:CODE" when it has one, else WKT.
pub fn crs_of(ds: &Dataset) -> (Option<String>, Option<String>) {
    match ds.spatial_ref() {
        Ok(sr) => {
            let name = sr.name();
            // The authority code only when it really is that CRS: a file can carry "EPSG:26915" on a definition whose parameters were
            // changed, and the code would then hide the change (bug G6). Otherwise the file's own definition is kept.
            let id = match (sr.auth_name(), sr.auth_code()) {
                (Some(a), Ok(c)) if crs_is_its_code(&sr, &format!("{a}:{c}")) => Some(format!("{a}:{c}")),
                _ => sr.to_wkt().ok(),
            };
            (id, name)
        }
        Err(_) => (None, None),
    }
}

/// Lon/lat bounds of a raster.
pub fn extent_lonlat(ds: &Dataset) -> Result<[f64; 4]> {
    let (w, h) = ds.raster_size();
    let gt = ds.geo_transform()?;
    let corners = [(0.0, 0.0), (w as f64, 0.0), (0.0, h as f64), (w as f64, h as f64)];
    let src = ds.spatial_ref()?;
    let mut dst = gdal::spatial_ref::SpatialRef::from_epsg(4326)?;
    dst.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let mut src = src;
    src.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let tr = gdal::spatial_ref::CoordTransform::new(&src, &dst)?;
    let mut xs: Vec<f64> = corners.iter().map(|(c, r)| gt[0] + c * gt[1] + r * gt[2]).collect();
    let mut ys: Vec<f64> = corners.iter().map(|(c, r)| gt[3] + c * gt[4] + r * gt[5]).collect();
    tr.transform_coords(&mut xs, &mut ys, &mut [])?;
    let f = |v: &[f64], max: bool| v.iter().cloned().fold(if max { f64::MIN } else { f64::MAX }, if max { f64::max } else { f64::min });
    Ok([f(&xs, false), f(&ys, false), f(&xs, true), f(&ys, true)])
}

/// Read size, bands, statistics etc. of a raster file.
pub fn describe(path: &Path) -> Result<RasterInfo> {
    let ds = Dataset::open(path).with_context(|| format!("GDAL could not open {}", path.display()))?;
    let (w, h) = ds.raster_size();
    let n = ds.raster_count();
    if n == 0 {
        bail!("{} has no raster bands", path.display());
    }
    let b = ds.rasterband(1)?;
    // GDAL names the 8-bit unsigned type "Byte" (older bindings printed "UInt8"; accept both).
    let dt = b.band_type().name();
    // A raster with no valid cell has no statistics: GDAL calls that an error. It is still a raster the user may want to see listed.
    let (min, max, empty) = match b.get_statistics(true, true) {
        Ok(Some(s)) => (s.min, s.max, false),
        Ok(None) => (0.0, 0.0, true),
        Err(e) if e.to_string().contains("no valid pixels") => (0.0, 0.0, true),
        Err(e) => return Err(e.into()),
    };
    let gt = ds.geo_transform()?;
    let rgb = n >= 3 && (dt == "Byte" || dt == "UInt8");
    Ok(RasterInfo {
        path: path.display().to_string(),
        width: w,
        height: h,
        bands: n,
        data_type: dt,
        nodata: b.no_data_value(),
        min,
        max,
        kind: if rgb { "rgb".into() } else { "elevation".into() },
        pixel_size: gt[1].abs(),
        empty,
    })
}

/// Copy a raster into the store as a tiled, compressed GeoTIFF with overviews.
pub fn import(src_path: &Path) -> Result<PathBuf> {
    let src = Dataset::open(src_path).with_context(|| format!("GDAL could not open {}", src_path.display()))?;
    let stem = src_path.file_stem().and_then(|s| s.to_str()).unwrap_or("raster");
    let dest = new_store_path(stem, "tif");
    let mut args: Vec<String> = ["-of", "GTiff", "-co", "TILED=YES", "-co", "COMPRESS=DEFLATE", "-co", "BIGTIFF=IF_SAFER"].map(String::from).to_vec();
    // A definition that carries an EPSG label but differs from that EPSG CRS (a changed false easting, say) would be written to the
    // GeoTIFF as the bare code, and the raster would silently land somewhere else (bug C-7). Its own definition, without the label, is
    // written instead, so the stored file means what the source meant.
    if let Ok(sr) = src.spatial_ref() {
        if let (Some(a), Ok(c)) = (sr.auth_name(), sr.auth_code()) {
            if !crs_is_its_code(&sr, &format!("{a}:{c}")) {
                if let Ok(w) = sr.to_wkt() {
                    args.extend(["-a_srs".to_string(), strip_top_authority(&w)]);
                }
            }
        }
    }
    translate(&src, &dest, &args)?;
    build_overviews(&dest)?;
    Ok(dest)
}

pub fn build_overviews(path: &Path) -> Result<()> {
    let ds = Dataset::open_ex(path, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
    let (w, h) = ds.raster_size();
    let mut levels: Vec<i32> = Vec::new();
    let mut f = 2;
    while w.max(h) / f >= 256 {
        levels.push(f as i32);
        f *= 2;
    }
    if levels.is_empty() {
        return Ok(());
    }
    // Through the C API so a model run's Cancel stops it between blocks (E3): the gdal crate's wrapper has no progress hook.
    let method = cstr("AVERAGE")?;
    // SAFETY: `ds` outlives the call; `levels` holds `levels.len()` ints.
    let rc = unsafe { gdal_sys::GDALBuildOverviews(ds.c_dataset(), method.as_ptr(), levels.len() as i32, levels.as_mut_ptr(), 0, null_mut(), Some(gdal_progress), crate::progress::cancel_arg()) };
    if rc != gdal_sys::CPLErr::CE_None {
        return Err(last_error("Building overviews"));
    }
    Ok(())
}

// ---------- map tiles ----------

/// Hypsometric color ramp (low → high).
const RAMP: [[f32; 3]; 6] = [
    [70.0, 120.0, 80.0],
    [118.0, 166.0, 98.0],
    [204.0, 206.0, 138.0],
    [214.0, 178.0, 120.0],
    [170.0, 130.0, 100.0],
    [245.0, 245.0, 245.0],
];

pub(crate) fn ramp(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32;
    let i = (t.floor() as usize).min(RAMP.len() - 2);
    let f = t - i as f32;
    let (a, b) = (RAMP[i], RAMP[i + 1]);
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
}

/// One 256×256 Web Mercator PNG tile of a raster.
pub fn render_tile(info: &RasterInfo, z: u32, x: u32, y: u32, style: &RasterStyle) -> Result<Vec<u8>> {
    let world = 20037508.342789244;
    let size = 2.0 * world / 2f64.powi(z as i32);
    let (x0, y1) = (-world + x as f64 * size, world - y as f64 * size);
    render_extent_styled(info, style, crate::crs::WEB_MERCATOR, [x0, y1 - size, x0 + size, y1], 256, 256)
}

/// Concurrent map tiles each warp their own copy of the raster; this keeps the number running at once to the machine's cores, so a
/// busy map cannot pile up dozens of warps. Held while a tile is drawn, released when it drops.
pub struct WarpSlot;

static WARP_SLOTS: std::sync::OnceLock<(parking_lot::Mutex<usize>, parking_lot::Condvar)> = std::sync::OnceLock::new();

impl WarpSlot {
    pub fn take() -> WarpSlot {
        let (m, cv) = WARP_SLOTS.get_or_init(|| (parking_lot::Mutex::new(0), parking_lot::Condvar::new()));
        let cap = std::thread::available_parallelism().map_or(2, |n| n.get());
        let mut running = m.lock();
        while *running >= cap {
            cv.wait(&mut running);
        }
        *running += 1;
        WarpSlot
    }
}

impl Drop for WarpSlot {
    fn drop(&mut self) {
        if let Some((m, cv)) = WARP_SLOTS.get() {
            *m.lock() -= 1;
            cv.notify_one();
        }
    }
}

/// A PNG of `extent` (`[xmin, ymin, xmax, ymax]` in `crs`) at `w_px` × `h_px`:
/// the raster warped to that grid and coloured like the map (elevation ramp
/// with hillshade, or RGB). Pixels with no data are transparent. Map tiles
/// (`render_tile`), layouts and the projected map view all draw through this.
pub fn render_extent(info: &RasterInfo, crs: &str, extent: [f64; 4], w_px: u32, h_px: u32) -> Result<Vec<u8>> {
    render_extent_styled(info, &RasterStyle::default(), crs, extent, w_px, h_px)
}

/// [`render_extent`] with a raster style (`LayerStyle.raster`); the default style draws as before.
pub fn render_extent_styled(info: &RasterInfo, style: &RasterStyle, crs: &str, extent: [f64; 4], w_px: u32, h_px: u32) -> Result<Vec<u8>> {
    let custom = style.renderer != Renderer::Default;
    if custom {
        style.validate(info)?;
    }
    let (w, h) = (w_px as usize, h_px as usize);
    if w == 0 || h == 0 || w.saturating_mul(h) > 100_000_000 {
        bail!("Raster image size {w_px}×{h_px} px is not supported");
    }
    let [x0, y0, x1, y1] = extent;
    let target = crate::crs::resolve(crs);
    let src = Dataset::open(&info.path)?;
    let rgb = info.kind == "rgb" && !custom;
    let mut args: Vec<String> = ["-of", "MEM", "-t_srs", target.as_str(), "-r", match (style.renderer, style.resampling.as_str()) { (Renderer::Paletted, _) | (_, "nearest") => "near", (_, "cubic") => "cubic", _ => "bilinear" }, "-dstalpha"].map(String::from).to_vec();
    args.extend(["-te".into(), x0.to_string(), y0.to_string(), x1.to_string(), y1.to_string(), "-ts".into(), w.to_string(), h.to_string()]);
    if rgb {
        args.extend(["-b", "1", "-b", "2", "-b", "3"].map(String::from));
    } else if style.renderer == Renderer::Multiband {
        for b in style.rgb_bands {
            args.extend(["-b".to_string(), b.to_string()]);
        }
        args.extend(["-ot".into(), "Float32".into()]);
    } else {
        args.extend(["-b".to_string(), if custom { style.band.to_string() } else { "1".into() }, "-ot".into(), "Float32".into()]);
    }
    let tile = warp(&src, "", &args)?;
    let nb = tile.raster_count();
    let alpha: Vec<f32> = tile.rasterband(nb)?.read_as::<f32>((0, 0), (w, h), (w, h), None)?.data().to_vec();
    let mut rgba = vec![0u8; w * h * 4];
    if style.renderer == Renderer::Multiband {
        let ch: Vec<Vec<f32>> = (1..=3).map(|i| tile.rasterband(i).and_then(|b| b.read_as::<f32>((0, 0), (w, h), (w, h), None)).map(|b| b.data().to_vec())).collect::<Result<_, _>>()?;
        let ranges = [crate::raster_style::channel_range(info, style, 0)?, crate::raster_style::channel_range(info, style, 1)?, crate::raster_style::channel_range(info, style, 2)?];
        rgba = crate::raster_style::colorize_rgb([&ch[0], &ch[1], &ch[2]], &alpha, info, style, ranges)?;
    } else if custom {
        let v: Vec<f32> = tile.rasterband(1)?.read_as::<f32>((0, 0), (w, h), (w, h), None)?.data().to_vec();
        let range = if style.renderer == Renderer::Paletted { (0.0, 1.0) } else { crate::raster_style::stretch_range(info, style)? };
        rgba = crate::raster_style::colorize(&v, &alpha, info, style, range)?;
        if let (Some(sh), Renderer::Gray | Renderer::Pseudocolor) = (&style.shading, style.renderer) {
            let nd = info.nodata.map(|n| n as f32);
            let ok: Vec<bool> = (0..w * h).map(|i| alpha[i] >= 1.0 && v[i].is_finite() && nd.map_or(true, |n| (v[i] - n).abs() > 1e-6)).collect();
            let (pxx, pxy) = ground_pixel_m(&target, extent, w, h)?;
            let shade = crate::raster_style::shade_factors(&v, &ok, w, h, pxx, pxy, sh);
            crate::raster_style::apply_shade(&mut rgba, &shade, sh.strength);
        }
    } else if rgb {
        let bands: Vec<Vec<f32>> = (1..=3).map(|i| tile.rasterband(i).and_then(|b| b.read_as::<f32>((0, 0), (w, h), (w, h), None)).map(|b| b.data().to_vec())).collect::<Result<_, _>>()?;
        for i in 0..w * h {
            for c in 0..3 {
                rgba[i * 4 + c] = bands[c][i].clamp(0.0, 255.0) as u8;
            }
            rgba[i * 4 + 3] = alpha[i].clamp(0.0, 255.0) as u8;
        }
    } else {
        let v: Vec<f32> = tile.rasterband(1)?.read_as::<f32>((0, 0), (w, h), (w, h), None)?.data().to_vec();
        let (lo, hi) = (info.min as f32, info.max as f32);
        let span = (hi - lo).max(1e-6);
        let (pxx, pxy) = ground_pixel_m(&target, extent, w, h)?;
        let at = |r: isize, c: isize| v[(r.clamp(0, h as isize - 1) as usize) * w + c.clamp(0, w as isize - 1) as usize];
        let (az, alt) = (315f32.to_radians(), 45f32.to_radians());
        for r in 0..h as isize {
            for c in 0..w as isize {
                let i = r as usize * w + c as usize;
                if alpha[i] < 1.0 || (info.nodata.map(|nd| (v[i] - nd as f32).abs() < 1e-6).unwrap_or(false)) {
                    continue;
                }
                // Horn slope/aspect for hillshading.
                let dzdx = ((at(r - 1, c + 1) + 2.0 * at(r, c + 1) + at(r + 1, c + 1)) - (at(r - 1, c - 1) + 2.0 * at(r, c - 1) + at(r + 1, c - 1))) / (8.0 * pxx);
                let dzdy = ((at(r + 1, c - 1) + 2.0 * at(r + 1, c) + at(r + 1, c + 1)) - (at(r - 1, c - 1) + 2.0 * at(r - 1, c) + at(r - 1, c + 1))) / (8.0 * pxy);
                let slope = (dzdx * dzdx + dzdy * dzdy).sqrt().atan();
                let aspect = dzdy.atan2(-dzdx);
                let shade = (alt.sin() * slope.cos() + alt.cos() * slope.sin() * (az - std::f32::consts::FRAC_PI_2 - aspect).cos()).max(0.0);
                let col = ramp((v[i] - lo) / span);
                let k = 0.55 + 0.6 * shade;
                for ch in 0..3 {
                    rgba[i * 4 + ch] = (col[ch] * k).clamp(0.0, 255.0) as u8;
                }
                rgba[i * 4 + 3] = 255;
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w_px, h_px);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header()?;
        wr.write_image_data(&rgba)?;
    }
    Ok(out)
}

/// Ground size in metres of one output pixel (east, north) for hillshading.
/// Web Mercator pixels shrink by cos(latitude) on the ground; other
/// projected CRSs use their linear unit (the scale factor is within ~0.1%
/// over a frame); geographic CRSs convert degrees on the CRS's ellipsoid.
fn ground_pixel_m(target: &str, e: [f64; 4], w: usize, h: usize) -> Result<(f32, f32)> {
    let (dx, dy) = ((e[2] - e[0]) / w as f64, (e[3] - e[1]) / h as f64);
    if target == crate::crs::WEB_MERCATOR {
        let lat = (2.0 * ((e[1] + e[3]) / 2.0 / 6378137.0).exp().atan() - std::f64::consts::FRAC_PI_2).cos();
        return Ok(((dx * lat) as f32, (dy * lat) as f32));
    }
    let srs = gdal::spatial_ref::SpatialRef::from_definition(target).map_err(|e| anyhow!("Unknown coordinate system '{target}': {e}"))?;
    if srs.is_geographic() {
        let a = srs.semi_major().map_err(|e| anyhow!("{e}"))?;
        let m_per_deg = a * std::f64::consts::PI / 180.0;
        let lat = ((e[1] + e[3]) / 2.0).to_radians().cos().max(1e-6);
        Ok(((dx * m_per_deg * lat) as f32, (dy * m_per_deg) as f32))
    } else {
        let u = srs.linear_units();
        Ok(((dx * u) as f32, (dy * u) as f32))
    }
}

/// Value of band 1 at a lon/lat (for identify).
pub fn sample(info: &RasterInfo, lon: f64, lat: f64) -> Result<Option<f64>> {
    let ds = Dataset::open(&info.path)?;
    let mut src = gdal::spatial_ref::SpatialRef::from_epsg(4326)?;
    src.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let mut dst = ds.spatial_ref()?;
    dst.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let tr = gdal::spatial_ref::CoordTransform::new(&src, &dst)?;
    let (mut xs, mut ys) = ([lon], [lat]);
    // A point outside the raster CRS's domain of validity has no value there.
    if tr.transform_coords(&mut xs, &mut ys, &mut []).is_err() || !xs[0].is_finite() || !ys[0].is_finite() {
        return Ok(None);
    }
    let gt = ds.geo_transform()?;
    let col = ((xs[0] - gt[0]) / gt[1]).floor();
    let row = ((ys[0] - gt[3]) / gt[5]).floor();
    let (w, h) = ds.raster_size();
    if col < 0.0 || row < 0.0 || col >= w as f64 || row >= h as f64 {
        return Ok(None);
    }
    let v = ds.rasterband(1)?.read_as::<f64>((col as isize, row as isize), (1, 1), (1, 1), None)?.data()[0];
    Ok(if info.nodata.map(|nd| (v - nd).abs() < 1e-9).unwrap_or(false) { None } else { Some(v) })
}

// ---------- processing ----------

/// Contour lines (gdal_contour) written to a GeoPackage; returns its path.
/// Lines get ID and ELEV fields; ELEV is in the raster's value units.
pub fn contours(src_path: &Path, interval: f64, base: f64) -> Result<PathBuf> {
    if interval <= 0.0 {
        bail!("Contour interval must be greater than zero");
    }
    let ds = Dataset::open(src_path)?;
    let band = ds.rasterband(1)?;
    let nodata = band.no_data_value();
    let out = new_store_path("contours", "gpkg");
    unsafe {
        let drv = gdal_sys::GDALGetDriverByName(cstr("GPKG")?.as_ptr());
        let p = cstr(out.to_str().unwrap())?;
        let vds = gdal_sys::GDALCreate(drv, p.as_ptr(), 0, 0, 0, gdal_sys::GDALDataType::GDT_Unknown, null_mut());
        if vds.is_null() {
            return Err(last_error("Creating contour output"));
        }
        let srs = gdal_sys::OSRNewSpatialReference(gdal_sys::GDALGetProjectionRef(ds.c_dataset()));
        let layer = gdal_sys::GDALDatasetCreateLayer(vds, cstr("contours")?.as_ptr(), srs, gdal_sys::OGRwkbGeometryType::wkbLineString, null_mut());
        for (name, ty) in [("ID", gdal_sys::OGRFieldType::OFTInteger), ("ELEV", gdal_sys::OGRFieldType::OFTReal)] {
            let f = gdal_sys::OGR_Fld_Create(cstr(name)?.as_ptr(), ty);
            gdal_sys::OGR_L_CreateField(layer, f, 1);
            gdal_sys::OGR_Fld_Destroy(f);
        }
        let mut opts = vec![format!("LEVEL_INTERVAL={interval}"), format!("LEVEL_BASE={base}"), "ID_FIELD=0".into(), "ELEV_FIELD=1".into()];
        if let Some(nd) = nodata {
            opts.push(format!("NODATA={nd}"));
        }
        let list = argv(&opts)?;
        let err = gdal_sys::GDALContourGenerateEx(band.c_rasterband(), layer, list.as_ptr(), Some(gdal_progress), crate::progress::cancel_arg());
        gdal_sys::OSRDestroySpatialReference(srs);
        gdal_sys::GDALClose(vds);
        if err != gdal_sys::CPLErr::CE_None {
            let e = last_error("Contour generation");
            discard(&out); // cancelled or failed: no partial output
            return Err(e);
        }
    }
    Ok(out)
}

/// Hillshade (gdaldem hillshade) as a new raster.
pub fn hillshade(src_path: &Path, azimuth: f64, altitude: f64, z_factor: f64, multidirectional: bool) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let out = new_store_path("hillshade", "tif");
    let mut args: Vec<String> = vec!["-of".into(), "GTiff".into(), "-co".into(), "COMPRESS=DEFLATE".into(), "-z".into(), z_factor.to_string(), "-compute_edges".into()];
    if multidirectional {
        args.push("-multidirectional".into());
    } else {
        args.extend(["-az".into(), azimuth.to_string(), "-alt".into(), altitude.to_string()]);
    }
    if geographic(&ds) {
        args.extend(["-s".into(), "111120".into()]); // degrees → meters
    }
    dem(&ds, &out, "hillshade", &args)?;
    build_overviews(&out)?;
    Ok(out)
}

/// Slope (gdaldem slope) in degrees or percent.
pub fn slope(src_path: &Path, percent: bool) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let out = new_store_path("slope", "tif");
    let mut args: Vec<String> = vec!["-of".into(), "GTiff".into(), "-co".into(), "COMPRESS=DEFLATE".into(), "-compute_edges".into()];
    if percent {
        args.push("-p".into());
    }
    if geographic(&ds) {
        args.extend(["-s".into(), "111120".into()]);
    }
    dem(&ds, &out, "slope", &args)?;
    build_overviews(&out)?;
    Ok(out)
}

/// Crop a raster to a lon/lat box.
pub fn clip_to_extent(src_path: &Path, bbox: [f64; 4]) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let out = new_store_path("clip", "tif");
    let args: Vec<String> = vec![
        "-of".into(), "GTiff".into(), "-co".into(), "COMPRESS=DEFLATE".into(), "-co".into(), "TILED=YES".into(),
        "-projwin_srs".into(), "EPSG:4326".into(), "-projwin".into(),
        bbox[0].to_string(), bbox[3].to_string(), bbox[2].to_string(), bbox[1].to_string(),
    ];
    translate(&ds, &out, &args)?;
    build_overviews(&out)?;
    Ok(out)
}

fn geographic(ds: &Dataset) -> bool {
    ds.spatial_ref().map(|s| s.is_geographic()).unwrap_or(false)
}

/// Other gdaldem modes: "aspect", "TRI", "TPI", "roughness".
pub fn dem_mode(src_path: &Path, mode: &str, extra: &[String]) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let out = new_store_path(&mode.to_lowercase(), "tif");
    let mut args: Vec<String> = vec!["-of".into(), "GTiff".into(), "-co".into(), "COMPRESS=DEFLATE".into(), "-compute_edges".into()];
    args.extend(extra.iter().cloned());
    dem(&ds, &out, mode, &args)?;
    build_overviews(&out)?;
    Ok(out)
}

/// Reproject a raster to another CRS (gdalwarp).
pub fn reproject(src_path: &Path, crs: &str, resampling: &str) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let out = new_store_path("warp", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-t_srs", crs, "-r", resampling].map(String::from).to_vec();
    warp(&ds, out.to_str().unwrap(), &args)?;
    build_overviews(&out)?;
    Ok(out)
}

/// Polygons of connected cells with the same value (gdal_polygonize), with a DN field.
pub fn polygonize(src_path: &Path) -> Result<PathBuf> {
    let ds = Dataset::open(src_path)?;
    let band = ds.rasterband(1)?;
    let out = new_store_path("polygonize", "gpkg");
    unsafe {
        let drv = gdal_sys::GDALGetDriverByName(cstr("GPKG")?.as_ptr());
        let p = cstr(out.to_str().unwrap())?;
        let vds = gdal_sys::GDALCreate(drv, p.as_ptr(), 0, 0, 0, gdal_sys::GDALDataType::GDT_Unknown, null_mut());
        if vds.is_null() {
            return Err(last_error("Creating polygonize output"));
        }
        let srs = gdal_sys::OSRNewSpatialReference(gdal_sys::GDALGetProjectionRef(ds.c_dataset()));
        let layer = gdal_sys::GDALDatasetCreateLayer(vds, cstr("polygons")?.as_ptr(), srs, gdal_sys::OGRwkbGeometryType::wkbPolygon, null_mut());
        let f = gdal_sys::OGR_Fld_Create(cstr("DN")?.as_ptr(), gdal_sys::OGRFieldType::OFTReal);
        gdal_sys::OGR_L_CreateField(layer, f, 1);
        gdal_sys::OGR_Fld_Destroy(f);
        let mask = gdal_sys::GDALGetMaskBand(band.c_rasterband());
        let err = gdal_sys::GDALFPolygonize(band.c_rasterband(), mask, layer, 0, null_mut(), Some(gdal_progress), crate::progress::cancel_arg());
        gdal_sys::OSRDestroySpatialReference(srs);
        gdal_sys::GDALClose(vds);
        if err != gdal_sys::CPLErr::CE_None {
            let e = last_error("Polygonize");
            discard(&out); // cancelled or failed: no partial output
            return Err(e);
        }
    }
    Ok(out)
}

/// The raster's CRS as WKT (for transforming other data into it).
pub fn wkt_of(src_path: &Path) -> Result<String> {
    Ok(Dataset::open(src_path)?.spatial_ref()?.to_wkt()?)
}

/// Band-1 values at points given in the raster's CRS (None outside or no-data).
pub fn sample_points(src_path: &Path, pts: &[(f64, f64)]) -> Result<Vec<Option<f64>>> {
    let ds = Dataset::open(src_path)?;
    let (w, h) = ds.raster_size();
    let gt = ds.geo_transform()?;
    let band = ds.rasterband(1)?;
    let nodata = band.no_data_value();
    // Small rasters: read once. Large ones: read per point.
    let whole: Option<Vec<f64>> = if w * h <= 64_000_000 { Some(band.read_as::<f64>((0, 0), (w, h), (w, h), None)?.data().to_vec()) } else { None };
    pts.iter()
        .map(|&(x, y)| {
            let c = ((x - gt[0]) / gt[1]).floor();
            let r = ((y - gt[3]) / gt[5]).floor();
            if c < 0.0 || r < 0.0 || c >= w as f64 || r >= h as f64 {
                return Ok(None);
            }
            let v = match &whole {
                Some(a) => a[r as usize * w + c as usize],
                None => band.read_as::<f64>((c as isize, r as isize), (1, 1), (1, 1), None)?.data()[0],
            };
            Ok(if nodata.map(|nd| (v - nd).abs() < 1e-9).unwrap_or(false) || v.is_nan() { None } else { Some(v) })
        })
        .collect()
}

/// Per-zone statistics: `zones_gpkg` holds polygons (layer "zones", integer
/// field ZONE, in the raster's CRS). Returns zone → (count, sum, min, max).
pub fn zonal(src_path: &Path, zones_gpkg: &Path) -> Result<std::collections::HashMap<i64, (i64, f64, f64, f64)>> {
    let ds = Dataset::open(src_path)?;
    let (w, h) = ds.raster_size();
    if w * h > 200_000_000 {
        bail!("Raster too large for zonal statistics ({}×{} cells); clip it first", w, h);
    }
    let gt = ds.geo_transform()?;
    let (x0, y1) = (gt[0], gt[3]);
    let (x1, y0) = (gt[0] + gt[1] * w as f64, gt[3] + gt[5] * h as f64);
    let zones_ds = Dataset::open(zones_gpkg)?;
    let args: Vec<String> = vec![
        "-of".into(), "MEM".into(), "-a".into(), "ZONE".into(), "-ot".into(), "Int32".into(), "-init".into(), "0".into(),
        "-te".into(), x0.to_string(), y0.min(y1).to_string(), x1.to_string(), y0.max(y1).to_string(),
        "-ts".into(), w.to_string(), h.to_string(), "-l".into(), "zones".into(),
    ];
    let list = argv(&args)?;
    let zr = unsafe {
        let opts = gdal_sys::GDALRasterizeOptionsNew(list.as_ptr() as *mut *mut _, null_mut());
        if opts.is_null() {
            return Err(last_error("Rasterizing zones options"));
        }
        gdal_sys::GDALRasterizeOptionsSetProgress(opts, Some(gdal_progress), crate::progress::cancel_arg());
        let mut usage = 0;
        let out = gdal_sys::GDALRasterize(cstr("")?.as_ptr(), null_mut(), zones_ds.c_dataset(), opts, &mut usage);
        gdal_sys::GDALRasterizeOptionsFree(opts);
        if out.is_null() {
            return Err(last_error("Rasterizing zones"));
        }
        Dataset::from_c_dataset(out)
    };
    let zones = zr.rasterband(1)?.read_as::<i32>((0, 0), (w, h), (w, h), None)?.data().to_vec();
    let band = ds.rasterband(1)?;
    let nodata = band.no_data_value();
    let vals = band.read_as::<f64>((0, 0), (w, h), (w, h), None)?.data().to_vec();
    let mut out: std::collections::HashMap<i64, (i64, f64, f64, f64)> = Default::default();
    for (z, v) in zones.iter().zip(vals.iter()) {
        if *z == 0 || v.is_nan() || nodata.map(|nd| (v - nd).abs() < 1e-9).unwrap_or(false) {
            continue;
        }
        let e = out.entry(*z as i64).or_insert((0, 0.0, f64::MAX, f64::MIN));
        e.0 += 1;
        e.1 += v;
        e.2 = e.2.min(*v);
        e.3 = e.3.max(*v);
    }
    Ok(out)
}
