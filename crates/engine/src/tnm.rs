//! USGS The National Map downloads.
//!
//! Elevation comes from the 3DEP elevation image service, which returns a
//! GeoTIFF for any box at a requested resolution (the best available 3DEP
//! data, resampled). We request it in the local UTM zone so pixel sizes and
//! slopes are in meters. Elevation values are meters (NAVD88).

use anyhow::{anyhow, bail, Result};
use std::io::Read;
use std::path::PathBuf;

const ELEVATION: &str = "https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer/exportImage";
/// The service's maximum image size per side.
const MAX_PX: f64 = 8000.0;

/// Coarsest cell size one download may fall back to; a larger area needs the tiled download (phase D0).
const MAX_RES_M: f64 = 90.0;
/// One download is cut in one UTM zone, so it may be at most this wide (degrees of longitude).
const MAX_WIDTH_DEG: f64 = 6.0;
/// UTM is not defined beyond this latitude.
const MAX_LAT: f64 = 84.0;

/// What a 3DEP download of a box will be (bug D-1, case DEM-2): sizes on the WGS84 ellipsoid, the resolution
/// actually used, and whether it was coarsened.
#[derive(Debug, Clone, PartialEq)]
pub struct ElevationPlan {
    pub requested_res: f64,
    pub used_res: f64,
    pub cols: u32,
    pub rows: u32,
    /// Size of the F32 image the service is asked for.
    pub bytes: u64,
    pub coarsened: bool,
    pub width_m: f64,
    pub height_m: f64,
}

/// Metres per degree of latitude at `lat` degrees (WGS84 series).
fn m_per_deg_lat(lat: f64) -> f64 {
    let p = lat.to_radians();
    111_132.954 - 559.822 * (2.0 * p).cos() + 1.175 * (4.0 * p).cos()
}

/// Metres per degree of longitude at `lat` degrees (WGS84 series).
fn m_per_deg_lon(lat: f64) -> f64 {
    let p = lat.to_radians();
    111_412.84 * p.cos() - 93.5 * (3.0 * p).cos() + 0.118 * (5.0 * p).cos()
}

/// Size and check a download before any request. Refuses what one download cannot do correctly, in plain words.
pub fn plan_elevation(bbox: [f64; 4], resolution: f64) -> Result<ElevationPlan> {
    let [w, s, e, n] = bbox;
    if bbox.iter().any(|v| !v.is_finite()) || !(w < e && s < n) {
        bail!("The area is empty");
    }
    if w < -180.0 || e > 180.0 || s < -90.0 || n > 90.0 {
        bail!("The area is outside longitude -180…180 and latitude -90…90");
    }
    if !(resolution.is_finite() && resolution > 0.0) {
        bail!("The resolution must be a number of metres greater than 0");
    }
    if s < -MAX_LAT || n > MAX_LAT {
        bail!("3DEP elevation is downloaded in UTM, which does not reach beyond {MAX_LAT}° latitude. Choose an area between {MAX_LAT}° S and {MAX_LAT}° N.");
    }
    if e - w > MAX_WIDTH_DEG {
        bail!(
            "The area is {:.1}° wide. One elevation download covers at most {MAX_WIDTH_DEG}° (one UTM zone) so the cells stay square and true; choose a narrower area, or download it in pieces.",
            e - w
        );
    }
    // The widest part of the box is at the edge nearest the equator.
    let near_equator = if s <= 0.0 && n >= 0.0 { 0.0 } else { s.abs().min(n.abs()) };
    let width_m = (e - w) * m_per_deg_lon(near_equator);
    let height_m = (n - s) * m_per_deg_lat((s + n) / 2.0);
    let requested_res = resolution.max(1.0);
    let longest = width_m.max(height_m);
    let used_res = if longest / requested_res > MAX_PX { (longest / MAX_PX).ceil() } else { requested_res };
    if used_res > MAX_RES_M {
        bail!(
            "This area ({:.0} × {:.0} km) is too large for one download: it would need cells of {used_res:.0} m, coarser than the {MAX_RES_M:.0} m limit. Choose a smaller area (about {:.0} km on a side at most).",
            width_m / 1000.0,
            height_m / 1000.0,
            MAX_PX * MAX_RES_M / 1000.0
        );
    }
    let cols = (width_m / used_res).ceil().max(1.0) as u32;
    let rows = (height_m / used_res).ceil().max(1.0) as u32;
    Ok(ElevationPlan { requested_res, used_res, cols, rows, bytes: cols as u64 * rows as u64 * 4, coarsened: used_res > requested_res, width_m, height_m })
}

/// Download 3DEP elevation for a lon/lat box at `resolution` meters.
/// Returns the GeoTIFF path and the resolution actually used.
pub fn download_elevation(bbox: [f64; 4], resolution: f64, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<(PathBuf, f64)> {
    let plan = plan_elevation(bbox, resolution)?;
    let [w, s, e, n] = bbox;
    let (res, cols, rows) = (plan.used_res, plan.cols, plan.rows);
    let lat = (s + n) / 2.0;
    let zone = (((((w + e) / 2.0) + 180.0) / 6.0).floor() as i32).clamp(0, 59) + 1;
    let wkid = if lat >= 0.0 { 32600 + zone } else { 32700 + zone };
    let url = format!(
        "{ELEVATION}?bbox={w},{s},{e},{n}&bboxSR=4326&imageSR={wkid}&size={cols},{rows}&format=tiff&pixelType=F32&noData=-999999&interpolation=RSP_BilinearInterpolation&f=image"
    );
    let note = if plan.coarsened { format!(" (cells of {res:.0} m: the area is too large for {:.0} m)", plan.requested_res) } else { String::new() };
    report(0.0, &format!("Waiting for the 3DEP service to cut {cols} × {rows} cells of elevation{note}…"))?;
    let resp = ureq::get(&url)
        .set("User-Agent", "GIS-Studio (open-source GIS)")
        .timeout(std::time::Duration::from_secs(300))
        .call()
        .map_err(|e| anyhow!("Could not reach the USGS 3DEP elevation service: {e}"))?;
    if !resp.content_type().contains("tiff") {
        let mut body = String::new();
        resp.into_reader().take(2000).read_to_string(&mut body).ok();
        bail!("The 3DEP service didn't return elevation data: {}", body.chars().take(300).collect::<String>());
    }
    let tmp = crate::raster::new_store_path("3dep_download", "tif");
    let mut f = std::fs::File::create(&tmp)?;
    // Stream to disk; the service sends a length, so progress is real.
    let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
    let mut reader = resp.into_reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut got: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut f, &buf[..n])?;
        got += n as u64;
        let mb = got as f64 / 1e6;
        match total {
            Some(t) if t > 0 => report(0.8 * got as f64 / t as f64, &format!("Downloaded {mb:.1} of {:.1} MB", t as f64 / 1e6))?,
            _ => report(0.4, &format!("Downloaded {mb:.1} MB"))?,
        }
    }
    drop(f);
    report(0.85, "Building overviews…")?;
    // Mark the service's no-data value, then store with overviews.
    {
        let ds = gdal::Dataset::open_ex(&tmp, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
        ds.rasterband(1)?.set_no_data_value(Some(-999999.0))?;
    }
    let stored = crate::raster::import(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok((stored, res))
}
