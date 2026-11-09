//! GDAL raster utilities as model nodes (Wave 1, package T3): Proximity (`GDALComputeProximity`, as in gdal_proximity.py) and Sieve
//! (`GDALSieveFilter`, as in gdal_sieve.py). GDAL is MIT. Terrain Ruggedness, TPI and Roughness already exist as the `ruggedness` node
//! (gdaldem); GDAL's viewshed was not needed because the WhiteboxTools Viewshed fits (`whitebox_tools.rs`).
//!
//! Both nodes write a new GeoTIFF on the input's grid and CRS. Proximity measures straight lines in the raster's map units, so it refuses a
//! raster in degrees (a geodesic version is not implemented) and tells the user to project it first.

use super::*;
use crate::raster;
use gdal::cpl::CslStringList;
use std::path::{Path, PathBuf};

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(Proximity), Box::new(Sieve)]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "proximity" => &["distance to", "distance raster", "how far", "nearest", "buffer raster"],
        "sieve" => &["remove small patches", "clean classification", "despeckle", "minimum mapping unit", "remove islands"],
        _ => &[],
    }
}

fn err(what: &str) -> anyhow::Error {
    let msg = unsafe { std::ffi::CStr::from_ptr(gdal_sys::CPLGetLastErrorMsg()) }.to_string_lossy().to_string();
    anyhow!("{what} failed{}", if msg.is_empty() { String::new() } else { format!(": {msg}") })
}

fn cancelled_or(what: &str) -> anyhow::Error {
    if crate::progress::run_cancelled() { anyhow!("{}", crate::progress::CANCELLED) } else { err(what) }
}

fn discard(p: &Path) {
    let _ = std::fs::remove_file(p);
    let _ = std::fs::remove_file(format!("{}.aux.xml", p.display()));
}

fn restore_crs(out: &Path, crs: &str) -> Result<()> {
    let mut ds = gdal::Dataset::open_ex(out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
    let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_wkt(&wkt)?)?;
    Ok(())
}

pub struct Proximity;
impl Tool for Proximity {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "proximity", name: "Proximity (Distance to Cells)", category: "Raster › Distance", role: Role::Tool,
            description: "Distance from every cell to the nearest target cell, measured between cell centres in a straight line, in the raster's map unit (metres for a metric CRS, feet for a foot CRS) (GDAL: GDALComputeProximity). Targets are the cells whose value is in the list, or every cell with a non-zero value when the list is empty. Cells farther than the maximum distance, and every cell when there is no target, get no data. Needs a projected raster: in degrees the result would be wrong, so project it first.",
            inputs: vec![Port { id: "in", label: "Raster with target cells", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("values", "Target values (comma-separated, empty = every non-zero cell)", ParamKind::Text).optional(),
                Param::new("max_distance", "Maximum distance (map units, 0 = no limit)", ParamKind::Number).default(0),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src_path = Path::new(need_raster(i)?);
        let crs = need_raster_crs(i)?;
        if crate::crs::is_geographic(crs) {
            bail!("Proximity measures straight-line distance in the raster's map units, and this raster is in degrees, so the result would be wrong. Project the raster to a metre-based coordinate system first (Warp or Project).");
        }
        let maxd = p.f64("max_distance").unwrap_or(0.0);
        if !maxd.is_finite() || maxd < 0.0 {
            bail!("'Maximum distance' must be 0 (no limit) or a positive number");
        }
        let values: Option<Vec<f64>> = match p.str("values") {
            None => None,
            Some(t) => Some(t.split(',').map(|v| v.trim().parse::<f64>().map_err(|_| anyhow!("'{}' in the target values is not a number", v.trim()))).collect::<Result<Vec<_>>>()?),
        };
        let grid = crate::raster_ops::grid_of(src_path)?;
        let out = raster::new_store_path("proximity", "tif");
        let nodata = -9999.0;
        {
            let dst = crate::raster_ops::create_like(&out, &grid, 1, Some(nodata))?;
            let src = gdal::Dataset::open(src_path)?;
            let mut opts = CslStringList::new();
            opts.add_string("DISTUNITS=GEO")?;
            opts.add_string(&format!("NODATA={nodata}"))?;
            if let Some(v) = &values {
                opts.add_string(&format!("VALUES={}", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")))?;
            }
            if maxd > 0.0 {
                opts.add_string(&format!("MAXDIST={maxd}"))?;
            }
            let r = unsafe {
                gdal_sys::GDALComputeProximity(src.rasterband(1)?.c_rasterband(), dst.rasterband(1)?.c_rasterband(), opts.as_ptr(), Some(crate::raster::gdal_progress), crate::progress::cancel_arg())
            };
            if r != gdal_sys::CPLErr::CE_None {
                let e = cancelled_or("Proximity");
                drop(dst);
                discard(&out);
                return Err(e);
            }
        }
        restore_crs(&out, crs)?;
        raster::build_overviews(&out)?;
        // How many cells were left without a distance: nothing within the maximum distance, or no target at all.
        if let Some((n, bad)) = count_where(&out, |v, nd| v == nd) {
            if bad == n {
                ctx.add_note("Proximity: no cell matched the target values, so every cell has no data.".into());
            } else if bad > 0 {
                ctx.add_note(format!("Proximity: {bad} of {n} cells have no data because no target lies within the maximum distance."));
            }
        }
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

/// (cells, cells for which `f(value, nodata)` holds) in band 1.
fn count_where(path: &Path, f: impl Fn(f64, f64) -> bool) -> Option<(u64, u64)> {
    let ds = gdal::Dataset::open(path).ok()?;
    let band = ds.rasterband(1).ok()?;
    let nd = band.no_data_value().unwrap_or(f64::NAN);
    let (cols, rows) = ds.raster_size();
    let step = (4_000_000 / cols.max(1)).clamp(1, rows.max(1));
    let (mut n, mut hit, mut r) = (0u64, 0u64, 0);
    while r < rows {
        let h = step.min(rows - r);
        let buf = band.read_as::<f64>((0, r as isize), (cols, h), (cols, h), None).ok()?;
        for v in buf.data() {
            n += 1;
            if f(*v, nd) {
                hit += 1;
            }
        }
        r += h;
    }
    Some((n, hit))
}

pub struct Sieve;
impl Tool for Sieve {
    fn result_look(&self) -> ResultLook {
        ResultLook::Source
    }
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "sieve", name: "Sieve (Remove Small Patches)", category: "Raster › Management", role: Role::Tool,
            description: "Merge every patch of connected cells with the same value that has fewer cells than the minimum into its largest neighbouring patch (GDAL: GDALSieveFilter). Cleans speckle from a classified raster. The minimum is in cells: with 30 m cells, 11 cells is about one hectare. Takes a whole-number raster (8-bit, 16-bit or 32-bit integer); no-data cells stay no data. The result says how many cells changed.",
            inputs: vec![Port { id: "in", label: "Classified raster (whole numbers)", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("min_cells", "Smallest patch to keep (cells)", ParamKind::Number).default(4),
                Param::new("connectedness", "Cells count as connected", ParamKind::Enum).default("4").options(&[("4", "Sharing an edge (4 neighbours)"), ("8", "Sharing an edge or a corner (8 neighbours)")]),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src_path = Path::new(need_raster(i)?);
        let crs = need_raster_crs(i)?;
        let min = p.f64("min_cells").unwrap_or(f64::NAN);
        if !(min.is_finite() && min >= 1.0 && min.fract() == 0.0 && min <= i32::MAX as f64) {
            bail!("'Smallest patch to keep' must be a whole number of cells, 1 or more");
        }
        let conn = if p.str("connectedness").as_deref() == Some("8") { 8 } else { 4 };
        let src = gdal::Dataset::open(src_path)?;
        let ty = src.rasterband(1)?.band_type();
        if !matches!(ty, gdal::raster::GdalDataType::UInt8 | gdal::raster::GdalDataType::Int16 | gdal::raster::GdalDataType::UInt16 | gdal::raster::GdalDataType::Int32) {
            bail!("Sieve works on whole-number rasters (8-bit, 16-bit or 32-bit integer), and this raster holds {ty:?} values. Use Reclassify or the raster calculator to make classes first.");
        }
        let out: PathBuf = raster::new_store_path("sieve", "tif");
        let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER"].map(String::from).to_vec();
        raster::translate(&src, &out, &args)?;
        {
            let dst = gdal::Dataset::open_ex(&out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
            let r = unsafe {
                let sb = src.rasterband(1)?.c_rasterband();
                gdal_sys::GDALSieveFilter(sb, gdal_sys::GDALGetMaskBand(sb), dst.rasterband(1)?.c_rasterband(), min as i32, conn, std::ptr::null_mut(), Some(crate::raster::gdal_progress), crate::progress::cancel_arg())
            };
            if r != gdal_sys::CPLErr::CE_None {
                let e = cancelled_or("Sieve");
                drop(dst);
                discard(&out);
                return Err(e);
            }
        }
        restore_crs(&out, crs)?;
        raster::build_overviews(&out)?;
        crate::vertical::inherit(&out, &[src_path]);
        let changed = changed_cells(src_path, &out);
        ctx.add_note(match changed {
            Some((n, c)) => format!("Sieve: {c} of {n} cells changed value (patches under {min} cells, {conn}-connected, merged into their largest neighbour)."),
            None => "Sieve finished.".into(),
        });
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

fn changed_cells(a: &Path, b: &Path) -> Option<(u64, u64)> {
    let (da, db) = (gdal::Dataset::open(a).ok()?, gdal::Dataset::open(b).ok()?);
    let (ba, bb) = (da.rasterband(1).ok()?, db.rasterband(1).ok()?);
    let (cols, rows) = da.raster_size();
    let step = (2_000_000 / cols.max(1)).clamp(1, rows.max(1));
    let (mut n, mut c, mut r) = (0u64, 0u64, 0);
    while r < rows {
        let h = step.min(rows - r);
        let x = ba.read_as::<f64>((0, r as isize), (cols, h), (cols, h), None).ok()?;
        let y = bb.read_as::<f64>((0, r as isize), (cols, h), (cols, h), None).ok()?;
        for (u, v) in x.data().iter().zip(y.data()) {
            n += 1;
            if u != v {
                c += 1;
            }
        }
        r += h;
    }
    Some((n, c))
}
