//! Tiled 3DEP download (phase D0, bug D-1, docs/TEST_PLAN.md §10A): a local server stands in for the ImageServer,
//! so tiling, retries, resume, cancel and failures are tested without the network.

use anyhow::anyhow;
use engine::tnm::{self, ElevationService};
use engine::Engine;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
enum B {
    Ok,
    Status(u16),
    JsonError200,
    Truncated,
    WrongType,
    WrongSize,
    NotATiff,
    Slow(u64),
}

type Rule = Arc<dyn Fn(&str, usize) -> B + Send + Sync>;

struct Mock {
    url: String,
    /// bbox string of every exportImage request, in order.
    hits: Arc<Mutex<Vec<String>>>,
    limit: u32,
}

impl Mock {
    fn service(&self) -> ElevationService {
        ElevationService { url: self.url.clone(), backoff_ms: 1, timeout_s: 20 }
    }
    fn hits_for(&self, bbox: &str) -> usize {
        self.hits.lock().unwrap().iter().filter(|h| h.as_str() == bbox).count()
    }
}

const PLANE: (f64, f64) = (0.001, 0.002);

/// A GeoTIFF of the plane z = a·x + b·y at the cell centres of `bbox` with `cols × rows` cells.
fn plane_tiff(bbox: [f64; 4], cols: usize, rows: usize) -> Vec<u8> {
    use gdal::raster::Buffer;
    let path = std::env::temp_dir().join(format!("mock-tile-{}-{:?}.tif", std::process::id(), std::thread::current().id()));
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&path, cols, rows, 1).unwrap();
        let (cx, cy) = ((bbox[2] - bbox[0]) / cols as f64, (bbox[3] - bbox[1]) / rows as f64);
        ds.set_geo_transform(&[bbox[0], cx, 0.0, bbox[3], 0.0, -cy]).unwrap();
        let mut data = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let (x, y) = (bbox[0] + (c as f64 + 0.5) * cx, bbox[3] - (r as f64 + 0.5) * cy);
                data.push((PLANE.0 * x + PLANE.1 * y) as f32);
            }
        }
        let mut band = ds.rasterband(1).unwrap();
        band.set_no_data_value(Some(tnm::NO_DATA)).unwrap();
        band.write((0, 0), (cols, rows), &mut Buffer::new((cols, rows), data)).unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    bytes
}

fn start(limit: u32, rule: Rule) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let counts: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let h2 = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let (hits, counts, rule) = (h2.clone(), counts.clone(), rule.clone());
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req).lines().next().unwrap_or("").to_string();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let send = |s: &mut std::net::TcpStream, status: &str, ctype: &str, body: &[u8], claim: usize| {
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {claim}\r\nConnection: close\r\n\r\n");
                    let _ = s.write_all(body);
                };
                if !path.contains("exportImage") {
                    let body = format!("{{\"maxImageWidth\": {limit}, \"maxImageHeight\": {limit}}}");
                    send(&mut s, "200 OK", "application/json", body.as_bytes(), body.len());
                    return;
                }
                let q: HashMap<String, String> = path.split_once('?').map(|x| x.1).unwrap_or("").split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
                let bbox_s = q.get("bbox").cloned().unwrap_or_default();
                let bbox: Vec<f64> = bbox_s.split(',').filter_map(|v| v.parse().ok()).collect();
                let size: Vec<usize> = q.get("size").map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
                hits.lock().unwrap().push(bbox_s.clone());
                let n = {
                    let mut c = counts.lock().unwrap();
                    let e = c.entry(bbox_s.clone()).or_insert(0);
                    *e += 1;
                    *e
                };
                match rule(&bbox_s, n) {
                    B::Ok => {
                        let b = plane_tiff([bbox[0], bbox[1], bbox[2], bbox[3]], size[0], size[1]);
                        send(&mut s, "200 OK", "image/tiff", &b, b.len());
                    }
                    B::Status(c) => send(&mut s, &format!("{c} Error"), "text/plain", b"upstream trouble", 16),
                    B::JsonError200 => {
                        let b = br#"{"error":{"code":400,"message":"Unable to complete operation."}}"#;
                        send(&mut s, "200 OK", "application/json", b, b.len());
                    }
                    B::Truncated => {
                        let b = plane_tiff([bbox[0], bbox[1], bbox[2], bbox[3]], size[0], size[1]);
                        send(&mut s, "200 OK", "image/tiff", &b[..b.len() / 2], b.len());
                    }
                    B::WrongType => send(&mut s, "200 OK", "text/html", b"<html>maintenance</html>", 24),
                    B::NotATiff => send(&mut s, "200 OK", "image/tiff", b"this is not a raster", 20),
                    B::WrongSize => {
                        let b = plane_tiff([bbox[0], bbox[1], bbox[2], bbox[3]], size[0].saturating_sub(1).max(1), size[1]);
                        send(&mut s, "200 OK", "image/tiff", &b, b.len());
                    }
                    B::Slow(ms) => {
                        std::thread::sleep(Duration::from_millis(ms));
                        let b = plane_tiff([bbox[0], bbox[1], bbox[2], bbox[3]], size[0], size[1]);
                        send(&mut s, "200 OK", "image/tiff", &b, b.len());
                    }
                }
            });
        }
    });
    Mock { url: format!("http://127.0.0.1:{port}/ImageServer"), hits, limit }
}

fn ok_rule() -> Rule {
    Arc::new(|_, _| B::Ok)
}

fn noop(_: f64, _: &str) -> anyhow::Result<()> {
    Ok(())
}

/// A box about 21 km square at 36°N: 700 cells at 30 m, so 3 × 3 tiles of 256.
const BOX: [f64; 4] = [-100.0, 36.0, -99.78, 36.19];

#[test]
fn dem_1_limits_are_read_from_the_service_and_fall_back_when_unreadable() {
    assert_eq!(tnm::parse_limits(r#"{"maxImageWidth": 4100, "maxImageHeight": 15000, "other": 1}"#).unwrap(), tnm::Limits { max_width: 4100, max_height: 15000 });
    assert!(tnm::parse_limits("{}").unwrap_err().to_string().contains("no image size limits"));
    assert!(tnm::parse_limits("<html>").unwrap_err().to_string().contains("isn't JSON"));
    assert_eq!(tnm::Limits { max_width: 4100, max_height: 15000 }.tile_px(), 4000, "never above 4000");
    assert_eq!(tnm::Limits { max_width: 10, max_height: 10 }.tile_px(), 256);
    let m = start(512, ok_rule());
    let (l, note) = tnm::fetch_limits(&m.service());
    assert_eq!((l.max_width, note), (512, None));
    let dead = ElevationService { url: "http://127.0.0.1:1/ImageServer".into(), backoff_ms: 1, timeout_s: 2 };
    let (l, note) = tnm::fetch_limits(&dead);
    assert_eq!(l.max_width, 4000);
    assert!(note.unwrap().contains("could not be read"));
}

#[test]
fn dem_2_plans_size_tile_and_choose_the_output_crs() {
    use tnm::plan_elevation;
    // One UTM zone: UTM output, metre cells on a grid snapped to the cell size.
    let p = plan_elevation([-100.0, 40.0, -99.8829, 40.0903], 30.0).unwrap();
    assert!(!p.coarsened && p.used_res == 30.0 && p.crs == "EPSG:32614", "{p:?}");
    assert!((p.height_m - 10_000.0).abs() < 60.0 && (p.width_m - 10_000.0).abs() < 100.0, "{p:?}");
    assert_eq!(p.bytes, p.cols * p.rows * 4);
    assert!((p.x0 / 30.0).fract().abs() < 1e-9 && (p.y0 / 30.0).fract().abs() < 1e-9, "grid snapped to the cell size");
    assert_eq!(p.requests(), 1);
    // The grid covers the whole lon/lat box.
    assert!(p.cols as f64 * p.cell >= p.width_m * 0.99 && p.rows as f64 * p.cell >= p.height_m * 0.99);
    // 3° × 3° at 10 m is 9 × 4000-cell tiles or so, not coarsened any more.
    let c = plan_elevation([-100.0, 39.0, -97.0, 42.0], 10.0).unwrap();
    assert!(!c.coarsened && c.used_res == 10.0, "{c:?}");
    assert!(c.requests() >= 9 && c.needs_confirm, "{c:?}");
    // Every tile is within the request limit and the tiles cover the grid exactly.
    for (bbox, res) in [([0.0, 0.0, 6.0, 6.0], 30.0), ([-120.0, -60.0, -114.5, -55.0], 30.0), ([170.0, 10.0, 176.0, 12.0], 30.0)] {
        let p = plan_elevation(bbox, res).unwrap();
        let (mut cells, mut last_c, mut last_r) = (0u64, 0, 0);
        for ty in 0..p.tiles_y {
            for tx in 0..p.tiles_x {
                let (c0, r0, w, h) = p.tile(tx, ty);
                assert!(w <= p.tile_px && h <= p.tile_px && w > 0 && h > 0, "{bbox:?}");
                cells += w as u64 * h as u64;
                last_c = c0 + w as u64;
                last_r = r0 + h as u64;
            }
        }
        assert_eq!((cells, last_c, last_r), (p.cols * p.rows, p.cols, p.rows), "{bbox:?}");
    }
    // Neighbouring tiles share an edge exactly.
    let p = tnm::plan_elevation_with(BOX, 30.0, 256).unwrap();
    let (a, b) = (p.tile_bbox(0, 0), p.tile_bbox(1, 0));
    assert_eq!(a[2], b[0]);
    // 1 degree of latitude is 110.6 km at the equator.
    let eq = plan_elevation([0.0, -0.5, 0.1, 0.5], 90.0).unwrap();
    assert!((eq.height_m - 110_574.0).abs() < 300.0, "{eq:?}");
}

#[test]
fn dem_5_wide_and_polar_areas_are_geographic_and_continental_ones_are_coarsened_with_a_note() {
    use tnm::plan_elevation;
    for (bbox, why) in [([-100.0, 30.0, -90.0, 31.0], "more than one UTM zone"), ([-100.0, 80.0, -99.0, 85.0], "beyond 84"), ([-100.0, -86.0, -99.0, -80.0], "beyond 84"), ([170.0, -10.0, 180.0, 10.0], "more than one UTM zone")] {
        let p = plan_elevation(bbox, 1000.0).unwrap();
        assert_eq!(p.crs, "EPSG:4326", "{bbox:?}");
        assert!(p.notes.iter().any(|n| n.contains(why)), "{bbox:?}: {:?}", p.notes);
        assert!((p.x0 / p.cell).fract().abs() < 1e-6 || (p.x0 / p.cell).fract().abs() > 1.0 - 1e-6);
    }
    // The contiguous US at 1 m would be hundreds of billions of cells: coarsened to fit, and says so.
    let us = plan_elevation([-125.0, 25.0, -66.0, 49.0], 1.0).unwrap();
    assert!(us.coarsened && us.cols * us.rows <= tnm::MAX_CELLS && us.used_res <= 1000.0, "{us:?}");
    assert!(us.notes.iter().any(|n| n.contains("too large for 1 m cells")));
    // The whole globe fits at about 1 km.
    let globe = plan_elevation([-180.0, -90.0, 180.0, 90.0], 1.0).unwrap();
    assert!(globe.used_res <= 1000.0 && globe.cols * globe.rows <= tnm::MAX_CELLS, "{globe:?}");
    // Across the equator and the zone edge.
    assert_eq!(plan_elevation([-96.5, -1.0, -95.5, 1.0], 30.0).unwrap().crs.starts_with("EPSG:326"), true);
}

#[test]
fn dem_2_guard_refuses_in_plain_words_before_any_request() {
    use tnm::plan_elevation;
    for (bbox, res, text) in [([-181.0, 0.0, -180.0, 1.0], 30.0, "outside longitude"), ([0.0, 0.0, 1.0, 1.0], 0.0, "resolution"), ([0.0, 0.0, 1.0, 1.0], f64::NAN, "resolution"), ([5.0, 5.0, 5.0, 6.0], 30.0, "empty")] {
        let err = plan_elevation(bbox, res).unwrap_err().to_string();
        assert!(err.contains(text), "{bbox:?}: {err}");
        let m = start(512, ok_rule());
        assert_eq!(tnm::download_elevation_from(&m.service(), bbox, res, true, &noop).unwrap_err().to_string(), err);
        assert!(m.hits.lock().unwrap().is_empty(), "no request for a refused area");
    }
}

#[test]
fn dem_4_tiles_are_seamless_and_equal_the_plane() {
    let m = start(256, ok_rule());
    let (path, plan) = tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &noop).unwrap();
    assert_eq!((plan.tiles_x, plan.tiles_y), (3, 3), "{plan:?}");
    assert_eq!(m.hits.lock().unwrap().len(), 9);
    let ds = gdal::Dataset::open(&path).unwrap();
    let (cols, rows) = ds.raster_size();
    assert_eq!((cols as u64, rows as u64), (plan.cols, plan.rows));
    let data = ds.rasterband(1).unwrap().read_as::<f32>((0, 0), (cols, rows), (cols, rows), None).unwrap();
    let mut worst = 0.0f64;
    let mut nodata = 0;
    for r in 0..rows {
        for c in 0..cols {
            let v = data.data()[r * cols + c] as f64;
            if (v - tnm::NO_DATA).abs() < 1.0 {
                nodata += 1;
                continue;
            }
            let (x, y) = (plan.x0 + (c as f64 + 0.5) * plan.cell, plan.y0 - (r as f64 + 0.5) * plan.cell);
            worst = worst.max((v - (PLANE.0 * x + PLANE.1 * y)).abs());
        }
    }
    assert_eq!(nodata, 0, "no gap at tile edges");
    assert!(worst < 2e-3, "max difference from the plane {worst} m (F32 rounding only)");
    // The job folder is gone after success.
    assert!(!std::path::Path::new(&path).starts_with(std::env::temp_dir().join("dem_jobs")));
}

#[test]
fn dem_3_each_service_failure_gives_a_readable_message_after_retries() {
    for (mode, text) in [
        (B::Status(500), "HTTP 500"),
        (B::Status(504), "took too long"),
        (B::JsonError200, "didn't return elevation data"),
        (B::Truncated, "stopped sending the tile"),
        (B::WrongType, "didn't return elevation data"),
        (B::NotATiff, "isn't a readable raster"),
        (B::WrongSize, "should be"),
    ] {
        let mode2 = mode.clone();
        let m = start(256, Arc::new(move |_, _| mode2.clone()));
        // A single tile: 2 km box.
        let err = tnm::download_elevation_from(&m.service(), [-100.0, 36.0, -99.98, 36.018], 30.0, true, &noop).unwrap_err().to_string();
        assert!(err.contains(text), "{mode:?}: {err}");
        assert!(err.contains("failed after 3 tries") && err.contains("start the same download again"), "{mode:?}: {err}");
        assert_eq!(m.hits.lock().unwrap().len(), 3, "{mode:?}: three tries");
    }
    // Nobody home: a clear "could not reach".
    let dead = ElevationService { url: "http://127.0.0.1:1/ImageServer".into(), backoff_ms: 1, timeout_s: 2 };
    let err = tnm::download_elevation_from(&dead, [-100.0, 36.0, -99.98, 36.018], 30.0, true, &noop).unwrap_err().to_string();
    assert!(err.contains("Could not reach the USGS 3DEP"), "{err}");
}

#[test]
fn dem_6_cancel_stops_during_a_tile_and_during_the_wait() {
    // While the service is still cutting tile 1.
    let m = start(256, Arc::new(|_, _| B::Slow(8000)));
    let flag = Arc::new(AtomicBool::new(false));
    let f2 = flag.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        f2.store(true, Ordering::SeqCst);
    });
    let t = Instant::now();
    let err = tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &|_, _| if flag.load(Ordering::SeqCst) { Err(anyhow!("Cancelled")) } else { Ok(()) }).unwrap_err().to_string();
    assert!(err.contains("Cancelled"), "{err}");
    assert!(t.elapsed() < Duration::from_secs(2), "stopped in {:?}", t.elapsed());
    // After tile 3 of 9: the earlier tiles are kept for a resume, nothing is joined.
    let m = start(256, ok_rule());
    let seen = std::cell::Cell::new(0);
    let err = tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &|_, msg| {
        if msg.starts_with("Downloaded tile 3 of 9") {
            seen.set(1);
        }
        if seen.get() == 1 { Err(anyhow!("Cancelled")) } else { Ok(()) }
    })
    .unwrap_err()
    .to_string();
    assert!(err.contains("Cancelled"));
    assert_eq!(m.hits.lock().unwrap().len(), 3);
}

#[test]
fn dem_7_a_tile_is_retried_and_a_stopped_download_resumes_without_asking_again() {
    let plan = tnm::plan_elevation_with(BOX, 30.0, 256).unwrap();
    let t5 = plan.tile_bbox(1, 1); // tile 5 of 9
    let key = format!("{},{},{},{}", t5[0], t5[1], t5[2], t5[3]);
    // Fails twice, then succeeds: 3 requests for that tile, 1 for each other.
    let k2 = key.clone();
    let m = start(256, Arc::new(move |b, n| if b == k2 && n <= 2 { B::Status(503) } else { B::Ok }));
    tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &noop).unwrap();
    assert_eq!(m.hits_for(&key), 3);
    assert_eq!(m.hits.lock().unwrap().len(), 11);
    // A permanent failure names the tile and keeps the finished ones; a second run asks only for what is left.
    let k2 = key.clone();
    let fixed = Arc::new(AtomicBool::new(false));
    let f2 = fixed.clone();
    let m = start(256, Arc::new(move |b, _| if b == k2 && !f2.load(Ordering::SeqCst) { B::Status(500) } else { B::Ok }));
    let err = tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &noop).unwrap_err().to_string();
    assert!(err.contains("Tile 5 of 9") && err.contains("4 tiles downloaded so far are saved"), "{err}");
    let before = m.hits.lock().unwrap().len();
    assert_eq!(before, 4 + 3);
    fixed.store(true, Ordering::SeqCst);
    tnm::download_elevation_from(&m.service(), BOX, 30.0, true, &noop).unwrap();
    let after: Vec<String> = m.hits.lock().unwrap()[before..].to_vec();
    assert_eq!(after.len(), 5, "tiles 5–9 only: {after:?}");
    for done in 0..4 {
        let (tx, ty) = (done % 3, done / 3);
        let b = plan.tile_bbox(tx as u32, ty as u32);
        assert!(!after.contains(&format!("{},{},{},{}", b[0], b[1], b[2], b[3])), "tile {} was asked for again", done + 1);
    }
}

#[test]
fn dem_10_large_downloads_need_a_confirmation_before_any_request() {
    let m = start(4000, ok_rule());
    let big = [-100.0, 36.0, -95.5, 40.0]; // about 400 × 440 km at 30 m: ~200 million cells, 800 MB
    let plan = tnm::plan_elevation(big, 30.0).unwrap();
    assert!(plan.needs_confirm && plan.bytes > tnm::CONFIRM_BYTES, "{plan:?}");
    let err = tnm::download_elevation_from(&m.service(), big, 30.0, false, &noop).unwrap_err().to_string();
    assert!(err.contains("Confirm to continue") && err.contains("MB"), "{err}");
    assert!(m.hits.lock().unwrap().is_empty(), "no tile requested before the user confirms");
    // The engine says the same.
    let e = Engine::new().unwrap();
    assert!(e.download_elevation_from(&m.service(), big, 30.0, "Big", false).unwrap_err().to_string().contains("Confirm to continue"));
}

#[test]
fn dem_13_the_result_states_its_crs_and_is_cited_once_per_job() {
    let m = start(256, ok_rule());
    let e = Engine::new().unwrap();
    let l = e.download_elevation_from(&m.service(), BOX, 30.0, "Tiled test area", true).unwrap();
    assert_eq!(m.hits.lock().unwrap().len(), 9);
    assert!(l.name.contains("3DEP") && l.name.contains("Tiled test area") && l.name.contains("30 m"), "{}", l.name);
    assert!(l.crs.as_deref().is_some_and(|c| c.contains("32614")), "{:?}", l.crs);
    let srcs = e.sources().unwrap();
    let cited: Vec<_> = srcs.iter().filter(|s| s.citation.title.to_lowercase().contains("3dep") || s.citation.title.to_lowercase().contains("elevation")).collect();
    assert_eq!(cited.len(), 1);
    assert_eq!(cited[0].uses.len(), 1, "one use for the job, not one per tile");
    let r = l.raster.unwrap();
    assert!(r.min > 0.0 && (r.pixel_size - 30.0).abs() < 1e-6);
    // A wide area is geographic and says so.
    let m = start(256, ok_rule());
    let w = e.download_elevation_from(&m.service(), [-100.0, 30.0, -90.0, 30.3], 1000.0, "Wide test", true).unwrap();
    assert!(w.crs.as_deref().is_some_and(|c| c.contains("4326")), "{:?}", w.crs);
    assert!(w.name.contains("~1000 m"), "{}", w.name);
}
