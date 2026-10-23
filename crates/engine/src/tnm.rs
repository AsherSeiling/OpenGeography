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

/// Download 3DEP elevation for a lon/lat box at `resolution` meters.
/// Returns the GeoTIFF path and the resolution actually used.
pub fn download_elevation(bbox: [f64; 4], resolution: f64, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<(PathBuf, f64)> {
    let [w, s, e, n] = bbox;
    if !(w < e && s < n) {
        bail!("The area is empty");
    }
    let lat = (s + n) / 2.0;
    let width_m = (e - w) * 111_320.0 * lat.to_radians().cos();
    let height_m = (n - s) * 110_950.0;
    let mut res = resolution.max(1.0);
    if width_m.max(height_m) / res > MAX_PX {
        res = (width_m.max(height_m) / MAX_PX).ceil();
    }
    let (cols, rows) = ((width_m / res).ceil().max(1.0) as u32, (height_m / res).ceil().max(1.0) as u32);
    let zone = (((((w + e) / 2.0) + 180.0) / 6.0).floor() as i32).clamp(0, 59) + 1;
    let wkid = if lat >= 0.0 { 32600 + zone } else { 32700 + zone };
    let url = format!(
        "{ELEVATION}?bbox={w},{s},{e},{n}&bboxSR=4326&imageSR={wkid}&size={cols},{rows}&format=tiff&pixelType=F32&noData=-999999&interpolation=RSP_BilinearInterpolation&f=image"
    );
    report(0.0, "Waiting for the 3DEP service to cut the elevation…")?;
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
