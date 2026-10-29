//! Raster foundation tools (phase E1, E2): calculator, reclassify, NoData tools, band select, resample, align, mosaic, rasterize.
//! GDAL does what it already does well (translate, warp, fill-nodata, rasterize); the calculator evaluates expressions with the MIT-licensed
//! `fasteval` crate over one block of rows at a time, so a large raster never has to fit in memory. Every tool writes a compressed, tiled
//! GeoTIFF with overviews, keeps the input's coordinate system and checks for cancel between blocks.

use crate::raster::{build_overviews, new_store_path, translate, warp};
use anyhow::{anyhow, bail, Result};
use fasteval::Compiler;
use gdal::raster::{Buffer, RasterCreationOptions};
use gdal::vector::LayerAccess;
use gdal::Dataset;
use std::path::{Path, PathBuf};

/// Float32 no-data value written by the tools that create new values.
pub const OUT_NODATA: f64 = -3.402_823_466_385_288_6e38;
/// Rows read and written at a time.
const BLOCK_ROWS: usize = 128;
/// Most cells an output may have (a guard against a mistyped cell size).
pub const MAX_CELLS: u64 = 400_000_000;

pub(crate) fn cancelled() -> Result<()> {
    if crate::progress::run_cancelled() {
        bail!(crate::progress::CANCELLED);
    }
    Ok(())
}

/// Size, georeferencing and CRS of a raster.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub gt: [f64; 6],
    pub wkt: String,
    pub bands: usize,
}

pub fn grid_of(path: &Path) -> Result<Grid> {
    let ds = Dataset::open(path).map_err(|e| anyhow!("Could not open the raster: {e}"))?;
    let (cols, rows) = ds.raster_size();
    let gt = ds.geo_transform().map_err(|_| anyhow!("The raster has no georeferencing"))?;
    let wkt = ds.spatial_ref().ok().and_then(|s| s.to_wkt().ok()).unwrap_or_default();
    Ok(Grid { cols, rows, gt, wkt, bands: ds.raster_count() })
}

/// The tools that combine rasters cell by cell need them on one grid.
fn same_grid(a: &Grid, b: &Grid, what: &str) -> Result<()> {
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0);
    if a.cols != b.cols || a.rows != b.rows || !a.gt.iter().zip(&b.gt).all(|(x, y)| close(*x, *y)) {
        bail!(
            "{what}: the rasters are not on the same grid ({} × {} cells of {} against {} × {} cells of {}). Use Align Raster first to put one on the other's grid.",
            a.cols, a.rows, a.gt[1].abs(), b.cols, b.rows, b.gt[1].abs()
        );
    }
    if !crate::crs::same_opt((!a.wkt.is_empty()).then_some(a.wkt.as_str()), (!b.wkt.is_empty()).then_some(b.wkt.as_str())) {
        bail!("{what}: the rasters are in different coordinate systems. Use Align Raster first.");
    }
    Ok(())
}

pub(crate) fn create_like(out: &Path, g: &Grid, bands: usize, nodata: Option<f64>) -> Result<Dataset> {
    let opts = RasterCreationOptions::from_iter(["TILED=YES", "COMPRESS=DEFLATE", "PREDICTOR=3", "BIGTIFF=IF_SAFER"]);
    let drv = gdal::DriverManager::get_driver_by_name("GTiff")?;
    let mut ds = drv
        .create_with_band_type_with_options::<f32, _>(out, g.cols, g.rows, bands, &opts)
        .map_err(|e| anyhow!("Could not create the output raster ({e}). Is there enough free disk space?"))?;
    ds.set_geo_transform(&g.gt)?;
    if !g.wkt.is_empty() {
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_wkt(&g.wkt)?)?;
    }
    if let Some(nd) = nodata {
        for b in 1..=bands {
            ds.rasterband(b)?.set_no_data_value(Some(nd))?;
        }
    }
    Ok(ds)
}

/// Apply `f` to every cell of band `band`, block by block. `f` gets the value (NaN for no data) and returns the new value (NaN → no data).
fn map_cells(src: &Path, band: usize, out: &Path, mut f: impl FnMut(f32) -> f32) -> Result<()> {
    let ds = Dataset::open(src)?;
    let g = grid_of(src)?;
    if band == 0 || band > g.bands {
        bail!("The raster has {} band{}; band {band} does not exist", g.bands, if g.bands == 1 { "" } else { "s" });
    }
    let nd = ds.rasterband(band)?.no_data_value();
    let dst = create_like(out, &g, 1, Some(OUT_NODATA))?;
    let mut row = 0;
    while row < g.rows {
        cancelled()?;
        let h = BLOCK_ROWS.min(g.rows - row);
        let data = ds.rasterband(band)?.read_as::<f32>((0, row as isize), (g.cols, h), (g.cols, h), None)?;
        let vals: Vec<f32> = data
            .data()
            .iter()
            .map(|&v| {
                let missing = v.is_nan() || nd.is_some_and(|n| (v as f64 - n).abs() <= 1e-6 * n.abs().max(1.0) || v as f64 == n);
                let r = f(if missing { f32::NAN } else { v });
                if r.is_finite() { r } else { OUT_NODATA as f32 }
            })
            .collect();
        dst.rasterband(1)?.write((0, row as isize), (g.cols, h), &mut Buffer::new((g.cols, h), vals))?;
        row += h;
    }
    drop(dst);
    build_overviews(out)?;
    Ok(())
}

// ---------------------------------------------------------------- E1

/// Raster calculator: `expr` over band 1 of A (and B). Cells where any input has no data are no data in the result, as are results that
/// are not numbers (division by zero, square root of a negative). Operators `+ - * / ^ %`, comparisons (1 when true, 0 when not: `(A > 5) * 10`),
/// `&&`, `||`, `if(condition, then, else)`, and functions `abs sqrt exp ln pow log(base, x) min max floor ceil round sin cos tan atan2`.
pub fn calc(inputs: &[&Path], expr: &str) -> Result<PathBuf> {
    if inputs.is_empty() || inputs.len() > 2 {
        bail!("The calculator takes one or two rasters");
    }
    if expr.trim().is_empty() {
        bail!("Type an expression, for example (A - B) / (A + B)");
    }
    let grids: Vec<Grid> = inputs.iter().map(|p| grid_of(p)).collect::<Result<_>>()?;
    if grids.len() == 2 {
        same_grid(&grids[0], &grids[1], "Raster calculator")?;
    }
    let dss: Vec<Dataset> = inputs.iter().map(|p| Dataset::open(p)).collect::<std::result::Result<_, _>>()?;
    let nds: Vec<Option<f64>> = dss.iter().map(|d| d.rasterband(1).ok().and_then(|b| b.no_data_value())).collect();

    // Parse once; probe once so a bad expression is reported before any work.
    let mut slab = fasteval::Slab::new();
    let parsed = fasteval::Parser::new().parse(expr, &mut slab.ps).map_err(|e| anyhow!("The expression '{}' is not valid: {e}", expr.trim()))?;
    let compiled = parsed.from(&slab.ps).compile(&slab.ps, &mut slab.cs);
    let vals = std::cell::Cell::new([1.0f64, 1.0f64]);
    let n_in = inputs.len();
    let mut ns = |name: &str, args: Vec<f64>| -> Option<f64> {
        match (name, args.as_slice()) {
            ("A", []) => Some(vals.get()[0]),
            ("B", []) if n_in == 2 => Some(vals.get()[1]),
            // Functions fasteval does not have: the ones people expect from a raster calculator.
            ("sqrt", [x]) => Some(x.sqrt()),
            ("exp", [x]) => Some(x.exp()),
            ("ln", [x]) => Some(x.ln()),
            ("pow", [x, y]) => Some(x.powf(*y)),
            ("atan2", [y, x]) => Some(y.atan2(*x)),
            ("if", [c, t, e]) => Some(if *c != 0.0 { *t } else { *e }),
            _ => None,
        }
    };
    {
        use fasteval::Evaler;
        compiled.eval(&slab, &mut ns).map_err(|e| anyhow!("The expression '{}' cannot be evaluated: {e}. Use A{} for the raster{}; functions are abs, sqrt, exp, ln, pow, log(base, x), min, max, floor, ceil, round, sin, cos, tan, atan2, if(condition, then, else); a comparison such as (A > 5) is 1 or 0.", expr.trim(), if n_in == 2 { " and B" } else { "" }, if n_in == 2 { "s" } else { "" }))?;
    }

    let out = new_store_path("calc", "tif");
    let g = &grids[0];
    let dst = create_like(&out, g, 1, Some(OUT_NODATA))?;
    let run = (|| -> Result<()> {
        use fasteval::Evaler;
        let mut row = 0;
        while row < g.rows {
            cancelled()?;
            let h = BLOCK_ROWS.min(g.rows - row);
            let blocks: Vec<_> = dss.iter().map(|d| d.rasterband(1).and_then(|b| b.read_as::<f32>((0, row as isize), (g.cols, h), (g.cols, h), None))).collect::<std::result::Result<_, _>>()?;
            let mut outv = vec![OUT_NODATA as f32; g.cols * h];
            for i in 0..g.cols * h {
                let mut v = [0.0f64; 2];
                let mut bad = false;
                for (k, b) in blocks.iter().enumerate() {
                    let x = b.data()[i];
                    if x.is_nan() || nds[k].is_some_and(|n| x as f64 == n) {
                        bad = true;
                        break;
                    }
                    v[k] = x as f64;
                }
                if bad {
                    continue;
                }
                vals.set(v);
                if let Ok(r) = compiled.eval(&slab, &mut ns) {
                    if r.is_finite() && r.abs() < f32::MAX as f64 {
                        outv[i] = r as f32;
                    }
                }
            }
            dst.rasterband(1)?.write((0, row as isize), (g.cols, h), &mut Buffer::new((g.cols, h), outv))?;
            row += h;
        }
        Ok(())
    })();
    drop(dst);
    if let Err(e) = run {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    build_overviews(&out)?;
    Ok(out)
}

/// One reclassify rule.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    /// `lo <= x < hi`
    Range(f64, f64, f64),
    Exact(f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reclass {
    pub rules: Vec<Rule>,
    /// What cells with no data become (None: stay no data).
    pub nodata: Option<f64>,
    /// What cells no rule matched become (None: no data).
    pub otherwise: Option<f64>,
}

fn number(s: &str) -> Result<f64> {
    match s.trim().to_lowercase().as_str() {
        "inf" | "+inf" | "infinity" => Ok(f64::INFINITY),
        "-inf" | "-infinity" => Ok(f64::NEG_INFINITY),
        t => t.parse::<f64>().ok().filter(|v| !v.is_nan()).ok_or_else(|| anyhow!("'{}' is not a number", s.trim())),
    }
}

/// Rules, one per line or separated by `;`: `0 to 10 = 1` (from 0 up to but not including 10), `5 = 2` (exactly 5), `20 to inf = 3`,
/// `nodata = 0` (no-data cells become 0), `else = 9` or `else = nodata` (cells no rule matches).
pub fn parse_rules(text: &str) -> Result<Reclass> {
    let mut r = Reclass { rules: vec![], nodata: None, otherwise: None };
    for line in text.split(['\n', ';']).map(str::trim).filter(|l| !l.is_empty()) {
        let (lhs, rhs) = line.split_once('=').ok_or_else(|| anyhow!("The rule '{line}' needs an equals sign, like 0 to 10 = 1"))?;
        let lhs = lhs.trim().to_lowercase();
        let val = |s: &str| -> Result<Option<f64>> {
            if s.trim().eq_ignore_ascii_case("nodata") { Ok(None) } else { number(s).map(Some).map_err(|e| anyhow!("In '{line}': {e}")) }
        };
        match lhs.as_str() {
            "nodata" => r.nodata = val(rhs)?,
            "else" => r.otherwise = val(rhs)?,
            _ => {
                let to = val(rhs)?.ok_or_else(|| anyhow!("In '{line}': a range cannot become nodata; leave it out and use 'else = nodata'"))?;
                match lhs.split_once(" to ") {
                    Some((a, b)) => {
                        let (lo, hi) = (number(a).map_err(|e| anyhow!("In '{line}': {e}"))?, number(b).map_err(|e| anyhow!("In '{line}': {e}"))?);
                        if lo >= hi {
                            bail!("In '{line}': the start must be less than the end");
                        }
                        r.rules.push(Rule::Range(lo, hi, to));
                    }
                    None => r.rules.push(Rule::Exact(number(&lhs).map_err(|e| anyhow!("In '{line}': {e}"))?, to)),
                }
            }
        }
    }
    if r.rules.is_empty() && r.nodata.is_none() && r.otherwise.is_none() {
        bail!("Type at least one rule, for example: 0 to 10 = 1");
    }
    // Overlapping ranges would make the result depend on order: refuse.
    let ranges: Vec<(f64, f64)> = r.rules.iter().filter_map(|x| if let Rule::Range(a, b, _) = x { Some((*a, *b)) } else { None }).collect();
    for (i, (a, b)) in ranges.iter().enumerate() {
        for (c, d) in ranges.iter().skip(i + 1) {
            if a < d && c < b {
                bail!("The ranges {a} to {b} and {c} to {d} overlap. Each cell value may fall in only one range.");
            }
        }
    }
    Ok(r)
}

/// Reclassify band `band` by the rules.
pub fn reclassify(src: &Path, band: usize, rules: &Reclass) -> Result<PathBuf> {
    let out = new_store_path("reclass", "tif");
    let run = map_cells(src, band, &out, |v| {
        if v.is_nan() {
            return rules.nodata.map_or(f32::NAN, |x| x as f32);
        }
        let x = v as f64;
        for r in &rules.rules {
            match r {
                Rule::Exact(a, to) if x == *a => return *to as f32,
                Rule::Range(lo, hi, to) if x >= *lo && x < *hi => return *to as f32,
                _ => {}
            }
        }
        rules.otherwise.map_or(f32::NAN, |x| x as f32)
    });
    if let Err(e) = run {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    Ok(out)
}

/// What a NoData tool does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoDataMode {
    /// Declare `value` as the no-data value (cells holding it become no data); nothing else changes.
    Declare(f64),
    /// Give cells with no data this value; the result has no no-data value.
    Replace(f64),
    /// Fill no-data cells by interpolating from the cells around them, up to `max_distance` cells away.
    Fill { max_distance: f64, smoothing: u32 },
}

pub fn nodata_tool(src: &Path, mode: NoDataMode) -> Result<PathBuf> {
    let out = new_store_path("nodata", "tif");
    match mode {
        NoDataMode::Declare(v) => {
            let ds = Dataset::open(src)?;
            let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-a_nodata"].map(String::from).into_iter().chain([v.to_string()]).collect();
            translate(&ds, &out, &args)?;
            build_overviews(&out)?;
        }
        NoDataMode::Replace(v) => {
            let ds = Dataset::open(src)?;
            if ds.rasterband(1)?.no_data_value().is_none() {
                bail!("This raster has no no-data value declared, so there is nothing to replace. Use Set NoData Value first.");
            }
            drop(ds);
            let run = map_cells(src, 1, &out, |x| if x.is_nan() { v as f32 } else { x });
            if let Err(e) = run {
                let _ = std::fs::remove_file(&out);
                return Err(e);
            }
            // The values are all real now: drop the no-data flag the helper wrote.
            let ds = Dataset::open_ex(&out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
            ds.rasterband(1)?.set_no_data_value(None)?;
        }
        NoDataMode::Fill { max_distance, smoothing } => {
            if !(1.0..=1000.0).contains(&max_distance) {
                bail!("The search distance must be between 1 and 1000 cells");
            }
            let ds = Dataset::open(src)?;
            if ds.rasterband(1)?.no_data_value().is_none() {
                bail!("This raster has no no-data value declared, so there is nothing to fill. Use Set NoData Value first.");
            }
            let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES"].map(String::from).to_vec();
            translate(&ds, &out, &args)?;
            drop(ds);
            let ds = Dataset::open_ex(&out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
            let band = ds.rasterband(1)?;
            // SAFETY: the band handle belongs to `ds`, which outlives the call.
            let rc = unsafe { gdal_sys::GDALFillNodata(band.c_rasterband(), std::ptr::null_mut(), max_distance, 0, smoothing as i32, std::ptr::null_mut(), Some(crate::raster::gdal_progress), crate::progress::cancel_arg()) };
            drop(band);
            drop(ds);
            if rc != 0 {
                let _ = std::fs::remove_file(&out);
                bail!("Filling no-data cells failed or was cancelled");
            }
            build_overviews(&out)?;
        }
    }
    Ok(out)
}

/// Keep some bands ("1,3" or "2").
pub fn select_bands(src: &Path, bands: &str) -> Result<PathBuf> {
    let g = grid_of(src)?;
    let mut list = Vec::new();
    for t in bands.split([',', ' ']).filter(|t| !t.trim().is_empty()) {
        let b: usize = t.trim().parse().map_err(|_| anyhow!("'{}' is not a band number. Type numbers like 1,3", t.trim()))?;
        if b == 0 || b > g.bands {
            bail!("The raster has {} band{}; band {b} does not exist", g.bands, if g.bands == 1 { "" } else { "s" });
        }
        list.push(b);
    }
    if list.is_empty() {
        bail!("Type the band numbers to keep, like 1,3");
    }
    let ds = Dataset::open(src)?;
    let out = new_store_path("bands", "tif");
    let mut args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES"].map(String::from).to_vec();
    for b in &list {
        args.extend(["-b".into(), b.to_string()]);
    }
    translate(&ds, &out, &args)?;
    build_overviews(&out)?;
    Ok(out)
}

// ---------------------------------------------------------------- E2

pub(crate) fn check_cells(cols: f64, rows: f64) -> Result<()> {
    if !(cols.is_finite() && rows.is_finite()) || cols * rows > MAX_CELLS as f64 {
        bail!("That would make a raster of {:.0} million cells ({} × {}); the limit is {} million. Use a larger cell size.", cols * rows / 1e6, cols as u64, rows as u64, MAX_CELLS / 1_000_000);
    }
    Ok(())
}

fn method(m: &str) -> Result<&'static str> {
    Ok(match m {
        "near" => "near",
        "bilinear" => "bilinear",
        "cubic" => "cubic",
        "average" => "average",
        "mode" => "mode",
        other => bail!("Unknown resampling method '{other}'"),
    })
}

/// Change the cell size (in the raster's own units).
pub fn resample(src: &Path, cell: f64, resampling: &str) -> Result<PathBuf> {
    let g = grid_of(src)?;
    if !(cell.is_finite() && cell > 0.0) {
        bail!("The cell size must be a number greater than 0");
    }
    check_cells(g.cols as f64 * g.gt[1].abs() / cell, g.rows as f64 * g.gt[5].abs() / cell)?;
    let ds = Dataset::open(src)?;
    let out = new_store_path("resample", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-r", method(resampling)?, "-tr"].map(String::from).into_iter().chain([cell.to_string(), cell.to_string()]).collect();
    translate(&ds, &out, &args)?;
    build_overviews(&out)?;
    Ok(out)
}

/// Put `src` on the grid of `reference`: its coordinate system, extent and cell size.
pub fn align(src: &Path, reference: &Path, resampling: &str) -> Result<PathBuf> {
    let g = grid_of(reference)?;
    if g.wkt.is_empty() {
        bail!("The reference raster has no coordinate system");
    }
    let s = grid_of(src)?;
    if s.wkt.is_empty() {
        bail!("The raster has no coordinate system. Use Define Projection on the source layer first.");
    }
    let (x0, y1) = (g.gt[0], g.gt[3]);
    let (x1, y0) = (g.gt[0] + g.gt[1] * g.cols as f64, g.gt[3] + g.gt[5] * g.rows as f64);
    check_cells(g.cols as f64, g.rows as f64)?;
    let ds = Dataset::open(src)?;
    let out = new_store_path("align", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-r", method(resampling)?, "-t_srs"]
        .map(String::from)
        .into_iter()
        .chain([g.wkt.clone(), "-te".into(), x0.min(x1).to_string(), y0.min(y1).to_string(), x0.max(x1).to_string(), y0.max(y1).to_string(), "-ts".into(), g.cols.to_string(), g.rows.to_string()])
        .collect();
    let r = warp(&ds, out.to_str().unwrap(), &args);
    r?;
    build_overviews(&out)?;
    Ok(out)
}

/// Join rasters into one (GDAL warp with several sources: reprojects to the first one's CRS, later rasters win where they overlap).
pub fn mosaic(paths: &[&Path], resampling: &str) -> Result<PathBuf> {
    if paths.len() < 2 {
        bail!("A mosaic needs at least two rasters");
    }
    let grids: Vec<Grid> = paths.iter().map(|p| grid_of(p)).collect::<Result<_>>()?;
    if grids.iter().any(|g| g.wkt.is_empty()) {
        bail!("Every raster in a mosaic needs a coordinate system. Use Define Projection first.");
    }
    let dss: Vec<Dataset> = paths.iter().map(|p| Dataset::open(p)).collect::<std::result::Result<_, _>>()?;
    let out = new_store_path("mosaic", "tif");
    let list = crate::raster::argv_pub(&["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER", "-r", method(resampling)?, "-t_srs", &grids[0].wkt].map(String::from))?;
    let dest = std::ffi::CString::new(out.to_str().unwrap())?;
    // SAFETY: the datasets outlive the call; the options are freed after it.
    let ok = unsafe {
        let opts = gdal_sys::GDALWarpAppOptionsNew(list.as_ptr() as *mut *mut _, std::ptr::null_mut());
        if opts.is_null() {
            bail!("Mosaic options were not accepted");
        }
        gdal_sys::GDALWarpAppOptionsSetProgress(opts, Some(crate::raster::gdal_progress), crate::progress::cancel_arg());
        let mut srcs: Vec<_> = dss.iter().map(|d| d.c_dataset()).collect();
        let mut usage = 0;
        let h = gdal_sys::GDALWarp(dest.as_ptr(), std::ptr::null_mut(), srcs.len() as i32, srcs.as_mut_ptr(), opts, &mut usage);
        gdal_sys::GDALWarpAppOptionsFree(opts);
        if h.is_null() {
            false
        } else {
            gdal_sys::GDALClose(h);
            true
        }
    };
    if !ok {
        let _ = std::fs::remove_file(&out);
        bail!("The mosaic failed or was cancelled");
    }
    build_overviews(&out)?;
    Ok(out)
}

/// Burn vector features into a raster. `gpkg` holds the features in the CRS `wkt`; `field` gives the value (None: burn 1).
pub fn rasterize(gpkg: &Path, field: Option<&str>, cell: f64) -> Result<PathBuf> {
    if !(cell.is_finite() && cell > 0.0) {
        bail!("The cell size must be a number greater than 0");
    }
    let src = Dataset::open(gpkg).map_err(|e| anyhow!("Could not read the features: {e}"))?;
    let layer = src.layer(0)?;
    let ext = layer.try_get_extent().ok().flatten().ok_or_else(|| anyhow!("The features have no extent (is the layer empty?)"))?;
    check_cells((ext.MaxX - ext.MinX) / cell, (ext.MaxY - ext.MinY) / cell)?;
    drop(layer);
    let out = new_store_path("rasterize", "tif");
    let nd = OUT_NODATA.to_string();
    let c = cell.to_string();
    // The grid starts on a multiple of the cell size, so the same data and cell size always give the same cells.
    let (x0, y0) = ((ext.MinX / cell).floor() * cell, (ext.MinY / cell).floor() * cell);
    let (x1, y1) = ((ext.MaxX / cell).ceil() * cell, (ext.MaxY / cell).ceil() * cell);
    let mut args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-ot", "Float32", "-a_nodata", &nd, "-init", &nd, "-tr", &c, &c, "-te"].map(String::from).into_iter().chain([x0, y0, x1, y1].map(|v| v.to_string())).collect();
    match field {
        Some(f) => args.extend(["-a".into(), f.to_string()]),
        None => args.extend(["-burn".into(), "1".into()]),
    }
    let list = crate::raster::argv_pub(&args)?;
    let dest = std::ffi::CString::new(out.to_str().unwrap())?;
    // SAFETY: `src` outlives the call; the options are freed after it.
    let ok = unsafe {
        let opts = gdal_sys::GDALRasterizeOptionsNew(list.as_ptr() as *mut *mut _, std::ptr::null_mut());
        if opts.is_null() {
            bail!("Rasterize options were not accepted");
        }
        gdal_sys::GDALRasterizeOptionsSetProgress(opts, Some(crate::raster::gdal_progress), crate::progress::cancel_arg());
        let mut usage = 0;
        let h = gdal_sys::GDALRasterize(dest.as_ptr(), std::ptr::null_mut(), src.c_dataset(), opts, &mut usage);
        gdal_sys::GDALRasterizeOptionsFree(opts);
        if h.is_null() {
            false
        } else {
            gdal_sys::GDALClose(h);
            true
        }
    };
    if !ok {
        let _ = std::fs::remove_file(&out);
        bail!("Rasterizing failed or was cancelled. Check the value field holds numbers.");
    }
    build_overviews(&out)?;
    Ok(out)
}

/// Interpolation methods of GDAL's grid algorithms (`gdal_grid`, MIT/X): inverse distance, linear (TIN), nearest, moving average.
pub fn interpolation_algorithm(method: &str, power: f64, radius: f64) -> Result<String> {
    let nd = OUT_NODATA;
    let radius_opt = if radius > 0.0 { format!(":radius1={radius}:radius2={radius}") } else { String::new() };
    Ok(match method {
        "idw" => {
            if !(power.is_finite() && power > 0.0) {
                bail!("The power must be a number greater than 0 (2 is usual)");
            }
            format!("invdist:power={power}:smoothing=0{radius_opt}:nodata={nd}")
        }
        "linear" => format!("linear:radius=-1:nodata={nd}"),
        "nearest" => format!("nearest{radius_opt}:nodata={nd}"),
        "average" => {
            if radius <= 0.0 {
                bail!("Moving average needs a search radius greater than 0");
            }
            format!("average{radius_opt}:nodata={nd}")
        }
        other => bail!("Unknown interpolation method \"{other}\""),
    })
}

/// Interpolate a surface from points. `gpkg` holds point features with a numeric field `v` in the CRS being used; `cell` and `radius` are in that CRS's units.
/// IDW and nearest cover the whole extent (a radius limits them); linear is no data outside the points' convex hull.
pub fn interpolate(gpkg: &Path, method: &str, cell: f64, power: f64, radius: f64) -> Result<PathBuf> {
    if !(cell.is_finite() && cell > 0.0) {
        bail!("The cell size must be a number greater than 0");
    }
    if !radius.is_finite() || radius < 0.0 {
        bail!("The search radius must be 0 (no limit) or greater");
    }
    let algo = interpolation_algorithm(method, power, radius)?;
    let src = Dataset::open(gpkg).map_err(|e| anyhow!("Could not read the points: {e}"))?;
    let layer = src.layer(0)?;
    let n = layer.feature_count();
    if n == 0 {
        bail!("There are no points with a number in the value field to interpolate from");
    }
    if method == "linear" && n < 3 {
        bail!("Linear interpolation needs at least 3 points ({n} given)");
    }
    let ext = layer.try_get_extent().ok().flatten().ok_or_else(|| anyhow!("The points have no extent"))?;
    drop(layer);
    // The grid starts on a multiple of the cell size, so the same data and cell size always give the same cells.
    let (x0, y0) = ((ext.MinX / cell).floor() * cell, (ext.MinY / cell).floor() * cell);
    let cols = (((ext.MaxX - x0) / cell).ceil()).max(1.0);
    let rows = (((ext.MaxY - y0) / cell).ceil()).max(1.0);
    check_cells(cols, rows)?;
    let (x1, y1) = (x0 + cols * cell, y0 + rows * cell);
    let out = new_store_path("interpolate", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-ot", "Float32", "-zfield", "v", "-txe"]
        .iter().map(|s| s.to_string())
        .chain([x0.to_string(), x1.to_string(), "-tye".into(), y0.to_string(), y1.to_string(), "-outsize".into(), (cols as u64).to_string(), (rows as u64).to_string(), "-a".into(), algo])
        .collect();
    let list = crate::raster::argv_pub(&args)?;
    let dest = std::ffi::CString::new(out.to_str().unwrap())?;
    // SAFETY: `src` outlives the call; the options are freed after it.
    let ok = unsafe {
        let opts = gdal_sys::GDALGridOptionsNew(list.as_ptr() as *mut *mut _, std::ptr::null_mut());
        if opts.is_null() {
            bail!("Interpolation options were not accepted");
        }
        gdal_sys::GDALGridOptionsSetProgress(opts, Some(crate::raster::gdal_progress), crate::progress::cancel_arg());
        let mut usage = 0;
        let h = gdal_sys::GDALGrid(dest.as_ptr(), src.c_dataset(), opts, &mut usage);
        gdal_sys::GDALGridOptionsFree(opts);
        if h.is_null() {
            false
        } else {
            gdal_sys::GDALClose(h);
            true
        }
    };
    if !ok {
        let _ = std::fs::remove_file(&out);
        bail!("Interpolation failed or was cancelled. Check the value field holds numbers.");
    }
    // gdal_grid names no-data in the algorithm only; record it on the band so every tool treats those cells as no data.
    {
        let ds = Dataset::open_ex(&out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
        ds.rasterband(1)?.set_no_data_value(Some(OUT_NODATA))?;
    }
    build_overviews(&out)?;
    Ok(out)
}
