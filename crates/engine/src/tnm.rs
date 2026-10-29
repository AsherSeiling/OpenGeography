//! USGS The National Map downloads.
//!
//! Elevation comes from the 3DEP elevation image service, which returns a
//! GeoTIFF for any box at a requested resolution (the best available 3DEP
//! data, resampled). A box up to one UTM zone wide is requested in that zone so pixel sizes and
//! slopes are in meters; a wider box is requested in geographic coordinates (EPSG:4326, cells in degrees).
//! Large areas are cut into tiles of at most the service's image size on one cell grid, fetched one by one (retry,
//! resume, cancel) and written into a single GeoTIFF without holding more than one tile in memory (phase D0, docs/DEM_DOWNLOAD.md).
//! Elevation values are meters (NAVD88).

use anyhow::{anyhow, bail, Result};
use std::io::Read;
use std::path::PathBuf;


const SERVICE: &str = "https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer";
/// Cell value the service is asked to use for no data.
pub const NO_DATA: f64 = -999999.0;
/// Largest tile edge requested whatever the service allows: 4000 × 4000 F32 cells is 64 MB per tile.
const TILE_PX: u32 = 4000;
/// Coarsest cell size a download may fall back to. Beyond this a global DEM is the better source.
const MAX_RES_M: f64 = 1000.0;
/// A download is cut in one UTM zone when it is at most this wide (degrees of longitude), else it is geographic.
const MAX_UTM_WIDTH_DEG: f64 = 6.0;
/// UTM is not defined beyond this latitude.
const MAX_UTM_LAT: f64 = 84.0;
/// The most cells one download may have (about 4 GB of F32); a larger area is coarsened.
pub const MAX_CELLS: u64 = 1_000_000_000;
/// Above this many bytes the user must confirm.
pub const CONFIRM_BYTES: u64 = 500 * 1_000_000;
/// Times one tile is tried before the download stops.
const ATTEMPTS: u32 = 3;

/// The service to download from. Tests point `url` at a local server.
#[derive(Debug, Clone)]
pub struct ElevationService {
    /// The ImageServer URL (without `/exportImage`).
    pub url: String,
    /// Wait before the first retry; doubles for each further one.
    pub backoff_ms: u64,
    /// Seconds one request may take.
    pub timeout_s: u64,
}

impl Default for ElevationService {
    fn default() -> Self {
        ElevationService { url: SERVICE.into(), backoff_ms: 2000, timeout_s: 180 }
    }
}

/// The service's image size limits (case DEM-1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
}

impl Limits {
    /// The edge of the tiles to request: the service limit, never above `TILE_PX`.
    pub fn tile_px(&self) -> u32 {
        self.max_width.min(self.max_height).clamp(256, TILE_PX)
    }
}

/// Read `maxImageWidth` / `maxImageHeight` from the ImageServer's `?f=json` description.
pub fn parse_limits(json: &str) -> Result<Limits> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| anyhow!("The 3DEP service description isn't JSON"))?;
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).filter(|x| *x > 0).map(|x| x as u32);
    match (n("maxImageWidth"), n("maxImageHeight")) {
        (Some(max_width), Some(max_height)) => Ok(Limits { max_width, max_height }),
        _ => bail!("The 3DEP service description has no image size limits"),
    }
}

/// Ask the service for its limits; when it can't be reached or answers oddly, use the documented 4000 px.
pub fn fetch_limits(service: &ElevationService) -> (Limits, Option<String>) {
    let fallback = Limits { max_width: TILE_PX, max_height: TILE_PX };
    let got = ureq::get(&format!("{}?f=json", service.url))
        .set("User-Agent", "GIS-Studio (open-source GIS)")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| anyhow!("{e}"))
        .and_then(|r| r.into_string().map_err(|e| anyhow!("{e}")))
        .and_then(|b| parse_limits(&b));
    match got {
        Ok(l) => (l, None),
        Err(e) => (fallback, Some(format!("The service's size limits could not be read ({e}); using {TILE_PX} × {TILE_PX} cells per request."))),
    }
}

/// What a 3DEP download of a box will be (bug D-1, cases DEM-2, DEM-5): the output grid, how it is cut into requests,
/// the resolution actually used, and whether it was coarsened.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationPlan {
    pub requested_res: f64,
    /// Cell size in metres (UTM output) or the metre equivalent at the box's mid-latitude (geographic output).
    pub used_res: f64,
    pub cols: u64,
    pub rows: u64,
    /// Size of the whole F32 image.
    pub bytes: u64,
    pub coarsened: bool,
    pub width_m: f64,
    pub height_m: f64,
    /// "EPSG:326xx" (one UTM zone) or "EPSG:4326" (wider).
    pub crs: String,
    /// Output grid: west, north corner and the cell size in CRS units (metres, or degrees for EPSG:4326).
    pub x0: f64,
    pub y0: f64,
    pub cell: f64,
    /// Edge of one request in cells, and the tiles across and down.
    pub tile_px: u32,
    pub tiles_x: u32,
    pub tiles_y: u32,
    /// The user must confirm before a download this large.
    pub needs_confirm: bool,
    /// Sentences about how the area is handled (coarsened, geographic output …).
    pub notes: Vec<String>,
}

impl ElevationPlan {
    pub fn requests(&self) -> u32 {
        self.tiles_x * self.tiles_y
    }
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

fn utm_wkid(lon: f64, lat: f64) -> u32 {
    let zone = (((lon + 180.0) / 6.0).floor() as i32).clamp(0, 59) as u32 + 1;
    if lat >= 0.0 {
        32600 + zone
    } else {
        32700 + zone
    }
}

/// Bounding box in UTM `wkid` of a lon/lat box (the edges are sampled, since they curve).
fn utm_extent(bbox: [f64; 4], wkid: u32) -> Result<[f64; 4]> {
    use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
    let mut a = SpatialRef::from_epsg(4326)?;
    a.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let mut b = SpatialRef::from_epsg(wkid)?;
    b.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let t = CoordTransform::new(&a, &b)?;
    let [w, s, e, n] = bbox;
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for i in 0..=20 {
        let f = i as f64 / 20.0;
        for (x, y) in [(w + (e - w) * f, s), (w + (e - w) * f, n), (w, s + (n - s) * f), (e, s + (n - s) * f)] {
            xs.push(x);
            ys.push(y);
        }
    }
    t.transform_coords(&mut xs, &mut ys, &mut [])?;
    let f = |v: &[f64], m: fn(f64, f64) -> f64, init: f64| v.iter().fold(init, |a, b| m(a, *b));
    Ok([f(&xs, f64::min, f64::INFINITY), f(&ys, f64::min, f64::INFINITY), f(&xs, f64::max, f64::NEG_INFINITY), f(&ys, f64::max, f64::NEG_INFINITY)])
}

/// Size and check a download before any request, with the documented 4000-cell tiles.
pub fn plan_elevation(bbox: [f64; 4], resolution: f64) -> Result<ElevationPlan> {
    plan_elevation_with(bbox, resolution, TILE_PX)
}

/// `plan_elevation` for a service whose request limit is `tile_px` cells.
pub fn plan_elevation_with(bbox: [f64; 4], resolution: f64, tile_px: u32) -> Result<ElevationPlan> {
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
    let tile_px = tile_px.clamp(256, TILE_PX);
    let requested_res = resolution.max(1.0);
    let mid_lat = (s + n) / 2.0;
    // The widest part of the box is at the edge nearest the equator.
    let near_equator = if s <= 0.0 && n >= 0.0 { 0.0 } else { s.abs().min(n.abs()) };
    let width_m = (e - w) * m_per_deg_lon(near_equator);
    let height_m = (n - s) * m_per_deg_lat(mid_lat);
    let mut notes = Vec::new();
    let utm = e - w <= MAX_UTM_WIDTH_DEG && s >= -MAX_UTM_LAT && n <= MAX_UTM_LAT;
    // The output grid for a cell size: UTM (metre cells snapped to the size) or geographic (square degree cells).
    let grid = |res: f64| -> Result<(String, f64, f64, f64, u64, u64)> {
        if utm {
            let wkid = utm_wkid((w + e) / 2.0, mid_lat);
            let [xmin, ymin, xmax, ymax] = utm_extent(bbox, wkid)?;
            // Snap to the cell grid so tiles meet exactly and a repeated download gives the same cells.
            let (xs, ys) = ((xmin / res).floor() * res, (ymax / res).ceil() * res);
            Ok((format!("EPSG:{wkid}"), xs, ys, res, ((xmax - xs) / res).ceil().max(1.0) as u64, ((ys - ymin) / res).ceil().max(1.0) as u64))
        } else {
            // Square in degrees: `res` metres along a meridian at the box's mid-latitude.
            let cell = res / m_per_deg_lat(mid_lat);
            let (xs, ys) = ((w / cell).floor() * cell, (n / cell).ceil() * cell);
            Ok(("EPSG:4326".to_string(), xs, ys, cell, ((e - xs) / cell).ceil().max(1.0) as u64, ((ys - s) / cell).ceil().max(1.0) as u64))
        }
    };
    // The cell size is chosen so the whole image has at most MAX_CELLS cells.
    let mut used_res = requested_res;
    let mut g = grid(used_res)?;
    while g.4 * g.5 > MAX_CELLS {
        used_res = (used_res * 1.05).ceil().max(used_res + 1.0);
        if used_res > MAX_RES_M {
            bail!(
                "This area ({:.0} × {:.0} km) is too large to download as one elevation raster: it would need cells coarser than the {MAX_RES_M:.0} m limit. Choose a smaller area, or use a global elevation model.",
                width_m / 1000.0,
                height_m / 1000.0
            );
        }
        g = grid(used_res)?;
    }
    let coarsened = used_res > requested_res;
    if coarsened {
        notes.push(format!("The area is too large for {requested_res:.0} m cells (over {} million cells); {used_res:.0} m cells are used.", MAX_CELLS / 1_000_000));
    }
    let (crs, x0, y0, cell, cols, rows) = g;
    if crs == "EPSG:4326" {
        notes.push(if e - w > MAX_UTM_WIDTH_DEG {
            format!("The area is {:.1}° wide, more than one UTM zone, so the elevation is kept in geographic coordinates (EPSG:4326, cells of {cell:.6}°). East–west cells get narrower toward the poles.", e - w)
        } else {
            format!("The area reaches beyond {MAX_UTM_LAT}° latitude, where UTM is not defined, so the elevation is kept in geographic coordinates (EPSG:4326, cells of {cell:.6}°).")
        });
    }
    let bytes = cols * rows * 4;
    let tp = tile_px as u64;
    Ok(ElevationPlan {
        requested_res,
        used_res,
        cols,
        rows,
        bytes,
        coarsened,
        width_m,
        height_m,
        crs,
        x0,
        y0,
        cell,
        tile_px,
        tiles_x: cols.div_ceil(tp) as u32,
        tiles_y: rows.div_ceil(tp) as u32,
        needs_confirm: bytes > CONFIRM_BYTES,
        notes,
    })
}

impl ElevationPlan {
    /// Cell window of tile (tx, ty): column and row offset, width and height in cells.
    pub fn tile(&self, tx: u32, ty: u32) -> (u64, u64, u32, u32) {
        let tp = self.tile_px as u64;
        let (c0, r0) = (tx as u64 * tp, ty as u64 * tp);
        (c0, r0, (self.cols - c0).min(tp) as u32, (self.rows - r0).min(tp) as u32)
    }

    /// Bounding box of tile (tx, ty) in the output CRS: xmin, ymin, xmax, ymax.
    pub fn tile_bbox(&self, tx: u32, ty: u32) -> [f64; 4] {
        let (c0, r0, w, h) = self.tile(tx, ty);
        let (xmin, ymax) = (self.x0 + c0 as f64 * self.cell, self.y0 - r0 as f64 * self.cell);
        [xmin, ymax - h as f64 * self.cell, xmin + w as f64 * self.cell, ymax]
    }
}

fn wkid_of(crs: &str) -> &str {
    crs.strip_prefix("EPSG:").unwrap_or(crs)
}

/// Why a tile request failed, as a sentence.
fn http_message(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let mut body = String::new();
            resp.into_reader().take(500).read_to_string(&mut body).ok();
            let hint = match code {
                500 | 502 | 503 => "the service is having trouble",
                504 => "the service took too long",
                429 => "too many requests",
                _ => "the request was refused",
            };
            format!("USGS 3DEP service error (HTTP {code}: {hint}){}", if body.trim().is_empty() { String::new() } else { format!(": {}", body.trim().chars().take(200).collect::<String>()) })
        }
        ureq::Error::Transport(t) => format!("Could not reach the USGS 3DEP elevation service: {t}"),
    }
}

/// Fetch one tile into `dest`. The request runs on its own thread so a cancel is noticed within a fraction of a second even
/// while the service is still cutting the image; an abandoned request ends by itself.
fn fetch_tile(service: &ElevationService, plan: &ElevationPlan, tx: u32, ty: u32, dest: &std::path::Path, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<()> {
    let (_, _, w, h) = plan.tile(tx, ty);
    let [x0, y0, x1, y1] = plan.tile_bbox(tx, ty);
    let wkid = wkid_of(&plan.crs);
    let url = format!(
        "{}/exportImage?bbox={x0},{y0},{x1},{y1}&bboxSR={wkid}&imageSR={wkid}&size={w},{h}&format=tiff&pixelType=F32&noData={NO_DATA}&interpolation=RSP_BilinearInterpolation&f=image",
        service.url
    );
    let timeout = std::time::Duration::from_secs(service.timeout_s);
    let part = dest.with_extension("part");
    let (tx_done, rx_done) = std::sync::mpsc::channel::<Result<()>>();
    let part2 = part.clone();
    std::thread::spawn(move || {
        let run = || -> Result<()> {
            let resp = ureq::get(&url).set("User-Agent", "GIS-Studio (open-source GIS)").timeout(timeout).call().map_err(|e| anyhow!("{}", http_message(e)))?;
            if !resp.content_type().contains("tiff") {
                let mut body = String::new();
                resp.into_reader().take(2000).read_to_string(&mut body).ok();
                bail!("The 3DEP service didn't return elevation data: {}", body.chars().take(300).collect::<String>());
            }
            let total: Option<u64> = resp.header("Content-Length").and_then(|v| v.parse().ok());
            let mut f = std::fs::File::create(&part2)?;
            let mut reader = resp.into_reader();
            let mut buf = vec![0u8; 1 << 16];
            let mut got: u64 = 0;
            loop {
                let n = reader.read(&mut buf).map_err(|e| anyhow!("The 3DEP service stopped sending the tile ({e})"))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut f, &buf[..n]).map_err(|e| anyhow!("Could not write the tile to disk ({e}). Is the disk full?"))?;
                got += n as u64;
            }
            if let Some(t) = total {
                if got < t {
                    bail!("The 3DEP service sent only {got} of {t} bytes of the tile");
                }
            }
            Ok(())
        };
        let _ = tx_done.send(run());
    });
    let result = loop {
        report(-1.0, "")?; // cancel check; negative fraction means "no progress change"
        match rx_done.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(r) => break r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => bail!("The tile download stopped unexpectedly"),
        }
    };
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    // The tile must be a readable raster of the size asked for.
    let check = (|| -> Result<()> {
        let ds = gdal::Dataset::open(&part).map_err(|_| anyhow!("The 3DEP service sent a file that isn't a readable raster"))?;
        let (cols, rows) = ds.raster_size();
        if cols != w as usize || rows != h as usize {
            bail!("The 3DEP service returned {cols} × {rows} cells for a tile that should be {w} × {h}");
        }
        Ok(())
    })();
    if let Err(e) = check {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// A tile already on disk from an earlier run, if it has the right size.
fn tile_ready(path: &std::path::Path, w: u32, h: u32) -> bool {
    gdal::Dataset::open(path).map(|d| d.raster_size() == (w as usize, h as usize)).unwrap_or(false)
}

/// Sleep `ms` in steps, stopping early with the cancel error.
fn wait(ms: u64, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<()> {
    let mut left = ms;
    while left > 0 {
        report(-1.0, "")?;
        let step = left.min(100);
        std::thread::sleep(std::time::Duration::from_millis(step));
        left -= step;
    }
    Ok(())
}

fn job_dir(plan: &ElevationPlan, service: &ElevationService) -> PathBuf {
    let key = crate::crs::hash12(&format!("{}|{}|{}|{}|{}|{}|{}", service.url, plan.crs, plan.x0, plan.y0, plan.cell, plan.cols, plan.rows));
    crate::raster::store_dir().join("dem_jobs").join(key)
}

/// Download 3DEP elevation for a lon/lat box at `resolution` meters from the public service.
/// Returns the GeoTIFF path and the resolution actually used.
pub fn download_elevation(bbox: [f64; 4], resolution: f64, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<(PathBuf, f64)> {
    let (path, plan) = download_elevation_from(&ElevationService::default(), bbox, resolution, true, report)?;
    Ok((path, plan.used_res))
}

/// Download in tiles from `service`. `report(fraction, message)` is called often; a negative fraction only checks for cancel.
/// More than `CONFIRM_BYTES` needs `confirmed`. Finished tiles stay on disk when a tile fails for good, so running the same
/// download again continues from there; on success they are removed.
pub fn download_elevation_from(service: &ElevationService, bbox: [f64; 4], resolution: f64, confirmed: bool, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<(PathBuf, ElevationPlan)> {
    let (limits, limit_note) = fetch_limits(service);
    let plan = plan_elevation_with(bbox, resolution, limits.tile_px())?;
    if plan.needs_confirm && !confirmed {
        bail!("This download is {:.0} MB ({} × {} cells in {} requests). Confirm to continue.", plan.bytes as f64 / 1e6, plan.cols, plan.rows, plan.requests());
    }
    let dir = job_dir(&plan, service);
    std::fs::create_dir_all(&dir).map_err(|e| anyhow!("Could not create the download folder {}: {e}", dir.display()))?;
    let total = plan.requests() as usize;
    let note = if plan.coarsened { format!(" (cells of {:.0} m: the area is too large for {:.0} m)", plan.used_res, plan.requested_res) } else { String::new() };
    if let Some(n) = limit_note {
        report(0.0, &n)?;
    }
    report(0.0, &format!("{} × {} cells in {total} request{}{note}", plan.cols, plan.rows, if total == 1 { "" } else { "s" }))?;
    let mut done = 0usize;
    for ty in 0..plan.tiles_y {
        for tx in 0..plan.tiles_x {
            let (_, _, w, h) = plan.tile(tx, ty);
            let path = dir.join(format!("t_{ty}_{tx}.tif"));
            let frac = |d: usize| 0.9 * d as f64 / total as f64;
            if tile_ready(&path, w, h) {
                done += 1;
                report(frac(done), &format!("Tile {done} of {total} was already downloaded"))?;
                continue;
            }
            let mut last = String::new();
            for attempt in 1..=ATTEMPTS {
                report(frac(done), &format!("Waiting for tile {} of {total}{}…", done + 1, if attempt > 1 { format!(" (try {attempt} of {ATTEMPTS})") } else { String::new() }))?;
                match fetch_tile(service, &plan, tx, ty, &path, report) {
                    Ok(()) => {
                        last.clear();
                        break;
                    }
                    Err(e) => {
                        if e.to_string().to_lowercase().contains("cancel") {
                            return Err(e);
                        }
                        last = e.to_string();
                        if attempt < ATTEMPTS {
                            wait(service.backoff_ms << (attempt - 1), report)?;
                        }
                    }
                }
            }
            if !last.is_empty() {
                bail!(
                    "Tile {} of {total} (column {}, row {}) failed after {ATTEMPTS} tries: {last}. {done} tile{} downloaded so far {} saved; start the same download again to continue where it stopped.",
                    done + 1,
                    tx + 1,
                    ty + 1,
                    if done == 1 { "" } else { "s" },
                    if done == 1 { "is" } else { "are" }
                );
            }
            done += 1;
            report(frac(done), &format!("Downloaded tile {done} of {total}"))?;
        }
    }
    report(0.9, "Joining the tiles…")?;
    let joined = join_tiles(&plan, &dir, report)?;
    report(0.95, "Building overviews…")?;
    let stored = crate::raster::import(&joined)?;
    let _ = std::fs::remove_file(&joined);
    let _ = std::fs::remove_dir_all(&dir);
    Ok((stored, plan))
}

/// Write the tiles into one tiled, compressed GeoTIFF, one tile in memory at a time.
fn join_tiles(plan: &ElevationPlan, dir: &std::path::Path, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<PathBuf> {
    use gdal::raster::{Buffer, RasterCreationOptions};
    let out = crate::raster::new_store_path("3dep_download", "tif");
    let driver = gdal::DriverManager::get_driver_by_name("GTiff")?;
    let opts = RasterCreationOptions::from_iter(["TILED=YES", "COMPRESS=DEFLATE", "PREDICTOR=3", "BIGTIFF=IF_SAFER"]);
    let mut ds = driver
        .create_with_band_type_with_options::<f32, _>(&out, plan.cols as usize, plan.rows as usize, 1, &opts)
        .map_err(|e| anyhow!("Could not create the elevation file ({e}). Is there enough free disk space?"))?;
    ds.set_geo_transform(&[plan.x0, plan.cell, 0.0, plan.y0, 0.0, -plan.cell])?;
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(wkid_of(&plan.crs).parse()?)?)?;
    {
        let mut band = ds.rasterband(1)?;
        band.set_no_data_value(Some(NO_DATA))?;
        for ty in 0..plan.tiles_y {
            for tx in 0..plan.tiles_x {
                report(-1.0, "")?;
                let (c0, r0, w, h) = plan.tile(tx, ty);
                let src = gdal::Dataset::open(dir.join(format!("t_{ty}_{tx}.tif")))?;
                let mut data = src.rasterband(1)?.read_as::<f32>((0, 0), (w as usize, h as usize), (w as usize, h as usize), None)?;
                // The service may use its own no-data value; keep ours.
                if let Some(nd) = src.rasterband(1)?.no_data_value() {
                    if (nd - NO_DATA).abs() > 0.5 {
                        for v in data.data_mut() {
                            if (*v as f64 - nd).abs() < 0.5 || v.is_nan() {
                                *v = NO_DATA as f32;
                            }
                        }
                    }
                }
                let mut buf = Buffer::new((w as usize, h as usize), data.into_shape_and_vec().1);
                band.write((c0 as isize, r0 as isize), (w as usize, h as usize), &mut buf)?;
            }
        }
    }
    drop(ds);
    Ok(out)
}
