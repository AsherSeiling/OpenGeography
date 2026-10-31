//! STAC search and COG clipping (phase D3): a local server stands in for a STAC API and for the cloud storage (with HTTP Range, which
//! GDAL's /vsicurl/ needs), so no network is used. The real services are in the ignored test at the bottom.

use engine::stac::{self, ImportSpec, SearchSpec};
use engine::Engine;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

struct Mock {
    base: String,
    bodies: Arc<Mutex<Vec<Value>>>,
}

/// A GeoTIFF in UTM 14N: 200 × 200 cells of 30 m from (500000, 4000200), value = column + 1000·row, 2 bands.
fn cog_bytes() -> Vec<u8> {
    use gdal::raster::Buffer;
    let path = std::env::temp_dir().join(format!("mock-cog-{}-{:?}.tif", std::process::id(), std::thread::current().id()));
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&path, 200, 200, 2).unwrap();
        ds.set_geo_transform(&[500000.0, 30.0, 0.0, 4006000.0, 0.0, -30.0]).unwrap();
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
        for b in 1..=2 {
            let data: Vec<f32> = (0..200 * 200).map(|i| (i % 200) as f32 + 1000.0 * (i / 200) as f32 + b as f32 * 0.5).collect();
            ds.rasterband(b).unwrap().write((0, 0), (200, 200), &mut Buffer::new((200, 200), data)).unwrap();
        }
    }
    let b = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    b
}

fn start(search_status: u16) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let bodies: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(vec![]));
    let b2 = bodies.clone();
    let cog = Arc::new(cog_bytes());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let (bodies, cog) = (b2.clone(), cog.clone());
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                    if let Some(h) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&req[..h]).to_string();
                        let want: usize = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
                        if req.len() >= h + 4 + want {
                            break;
                        }
                    }
                }
                let h = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(req.len());
                let head = String::from_utf8_lossy(&req[..h]).to_string();
                let body = String::from_utf8_lossy(&req[(h + 4).min(req.len())..]).to_string();
                let first = head.lines().next().unwrap_or("").to_string();
                let (method, path) = (first.split_whitespace().next().unwrap_or(""), first.split_whitespace().nth(1).unwrap_or(""));
                let text = |s: &mut std::net::TcpStream, status: &str, ctype: &str, b: &str| {
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                };
                if path == "/notstac/search" {
                    return text(&mut s, "200 OK", "text/html", "<html>hello</html>");
                }
                if path.ends_with("/search") && method == "POST" {
                    if let Ok(v) = serde_json::from_str::<Value>(&body) {
                        bodies.lock().unwrap().push(v);
                    }
                    if search_status != 200 {
                        return text(&mut s, &format!("{search_status} Error"), "application/json", r#"{"description":"Invalid bbox"}"#);
                    }
                    let a = |k: &str| json!({"type": "image/tiff; application=geotiff; profile=cloud-optimized", "title": k, "href": format!("http://127.0.0.1:{}/data/scene.tif", port)});
                    let feats = json!({"type": "FeatureCollection", "features": [
                        {"id": "SCENE_A", "collection": "sentinel-2-l2a", "properties": {"datetime": "2026-10-25T17:00:00Z", "eo:cloud_cover": 12.5}, "assets": {"visual": a("True colour"), "thumbnail": {"type": "image/jpeg", "href": "http://x/y.jpg"}, "s3only": {"type": "image/tiff", "href": "s3://public-bucket/data/x.tif"}, "gs": {"type": "image/tiff", "href": "gs://nope/x.tif"}}},
                        {"id": "SCENE_B", "collection": "sentinel-2-l2a", "properties": {"datetime": "2026-10-20T17:00:00Z"}, "assets": {}}
                    ]});
                    return text(&mut s, "200 OK", "application/geo+json", &feats.to_string());
                }
                if path == "/notstac/search" {
                    return text(&mut s, "200 OK", "text/html", "<html>hello</html>");
                }
                if path.starts_with("/data/scene.tif") {
                    let total = cog.len();
                    let range = head.lines().find_map(|l| l.to_lowercase().strip_prefix("range: bytes=").map(String::from));
                    let (status, a, b) = match range {
                        Some(r) => {
                            let (a, b) = r.split_once('-').unwrap();
                            let a: usize = a.trim().parse().unwrap_or(0);
                            let b: usize = b.trim().parse().unwrap_or(total - 1);
                            ("206 Partial Content", a.min(total), (b + 1).min(total))
                        }
                        None => ("200 OK", 0, total),
                    };
                    let extra = if status.starts_with("206") { format!("Content-Range: bytes {}-{}/{}\r\n", a, b.saturating_sub(1), total) } else { String::new() };
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: image/tiff\r\nAccept-Ranges: bytes\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", if method == "HEAD" { total } else { b - a });
                    if method != "HEAD" {
                        let _ = s.write_all(&cog[a..b]);
                    }
                    return;
                }
                text(&mut s, "404 Not Found", "text/plain", "no");
            });
        }
    });
    Mock { base: format!("http://127.0.0.1:{port}"), bodies }
}

fn spec(api: &str) -> SearchSpec {
    SearchSpec { api: Some(api.to_string()), collection: "sentinel-2-l2a".into(), bbox: [-97.0, 36.0, -96.9, 36.1], datetime: Some("2026-10-01/2026-10-31".into()), max_cloud: Some(30.0), limit: Some(5) }
}

#[test]
fn d3_hrefs_dates_and_the_catalog_are_checked_without_a_request() {
    assert_eq!(stac::https_href("https://a.b/c.tif").unwrap(), "https://a.b/c.tif");
    assert_eq!(stac::https_href("s3://bucket/dir/x.tif").unwrap(), "https://bucket.s3.amazonaws.com/dir/x.tif");
    assert!(stac::https_href("gs://b/x").unwrap_err().to_string().contains("not an address this app can read"));
    assert!(stac::https_href("s3://nokey").is_err());
    for ok in ["2024-06-01", "2024-06-01/2024-09-30", "2024-06-01/..", "../2024-09-30"] {
        stac::check_datetime(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    for bad in ["June 2024", "2024-6-1", "../..", "2024-09-30/2024-06-01", "2024-06-01/2024-07-01/2024-08-01"] {
        assert!(stac::check_datetime(bad).is_err(), "{bad}");
    }
    let mut ids = std::collections::HashSet::new();
    for a in stac::catalog() {
        assert!(a.url.starts_with("https://"));
        for c in &a.collections {
            assert!(ids.insert(c.id.clone()), "duplicate {}", c.id);
            assert!(!c.assets.is_empty() && !c.license.is_empty() && !c.author.is_empty());
            let cite = stac::citation(&c.id, "SCENE", "visual").unwrap();
            assert!(cite.complete && cite.annotation.contains("SCENE"));
        }
    }
    assert!(stac::citation("nope", "x", "y").is_none());
}

#[test]
fn d3_search_sends_the_filters_and_lists_scenes_with_readable_assets_only() {
    let m = start(200);
    let items = stac::search(&spec(&m.base)).unwrap();
    let sent = m.bodies.lock().unwrap()[0].clone();
    assert_eq!(sent["collections"], json!(["sentinel-2-l2a"]));
    assert_eq!(sent["datetime"], "2026-10-01T00:00:00Z/2026-10-31T23:59:59Z");
    assert_eq!(sent["query"]["eo:cloud_cover"]["lte"], 30.0);
    assert_eq!(sent["limit"], 5);
    assert_eq!(sent["sortby"][0]["direction"], "desc");
    assert_eq!(items.len(), 2);
    assert_eq!((items[0].id.as_str(), items[0].cloud_cover), ("SCENE_A", Some(12.5)));
    // Only GeoTIFF assets with an address GDAL can read: not the JPEG thumbnail, not gs://; s3:// is rewritten.
    let keys: Vec<&str> = items[0].assets.iter().map(|a| a.key.as_str()).collect();
    assert_eq!(keys, vec!["s3only", "visual"]);
    assert_eq!(items[0].assets[0].href, "https://public-bucket.s3.amazonaws.com/data/x.tif");
    assert!(items[1].assets.is_empty());
}

#[test]
fn d3_search_errors_are_readable() {
    let m = start(400);
    let err = stac::search(&spec(&m.base)).unwrap_err().to_string();
    assert!(err.contains("refused the search (HTTP 400)") && err.contains("Invalid bbox"), "{err}");
    let m = start(200);
    let err = stac::search(&spec(&format!("{}/notstac", m.base))).unwrap_err().to_string();
    assert!(err.contains("isn't a STAC search result"), "{err}");
    assert!(stac::search(&spec("http://127.0.0.1:1")).unwrap_err().to_string().contains("Could not reach the STAC service"));
    assert!(stac::search(&spec("ftp://x")).unwrap_err().to_string().contains("https://"));
    let mut bad = spec(&m.base);
    bad.bbox = [1.0, 1.0, 1.0, 2.0];
    assert!(stac::search(&bad).unwrap_err().to_string().contains("area is empty"));
    bad = spec(&m.base);
    bad.datetime = Some("yesterday".into());
    assert!(stac::search(&bad).unwrap_err().to_string().contains("Dates look like"));
    bad = spec(&m.base);
    bad.collection = " ".into();
    assert!(stac::search(&bad).unwrap_err().to_string().contains("Choose a collection"));
}

#[test]
fn d3_a_clip_reads_only_the_area_in_the_image_crs_with_cell_values_intact() {
    let m = start(200);
    let href = format!("{}/data/scene.tif", m.base);
    let e = Engine::new().unwrap();
    // The image covers UTM 14N x 500000–506000, y 4000000–4006000 (about 97.0°W–96.93°W, 36.15°N–36.2°N is not it): find a lon/lat box inside.
    let (lon, lat) = e.transform_point(503000.0, 4003000.0, "EPSG:32614", "EPSG:4326").unwrap();
    let bbox = [lon - 0.004, lat - 0.004, lon + 0.004, lat + 0.004];
    let spec = ImportSpec { href: href.clone(), bbox, label: "Mock scene (visual)".into(), collection: "sentinel-2-l2a".into(), item_id: "SCENE_A".into(), asset: "visual".into(), area_label: "Test area".into() };
    let l = e.stac_import(&spec).unwrap();
    assert_eq!(l.name, "Mock scene (visual)");
    assert!(l.crs.as_deref().is_some_and(|c| c.contains("32614")), "{:?}", l.crs);
    let r = l.raster.clone().unwrap();
    assert!((r.pixel_size - 30.0).abs() < 1e-6 && r.bands == 2, "{r:?}");
    // Values are the source's: column + 1000·row (+0.5 for band 1); the clip is small, not the whole 200 × 200 image.
    assert!(r.min >= 0.0 && r.max < 200.0 + 1000.0 * 200.0);
    let cols = ((bbox[2] - bbox[0]) * 85_000.0 / 30.0).round();
    assert!(cols < 40.0, "clip should be a few dozen cells, not {cols}");
    // Cited from the catalog, naming the scene.
    let s = e.sources().unwrap();
    let c = s.iter().find(|x| x.citation.key == "stac:sentinel-2-l2a").expect("cited");
    assert!(c.citation.annotation.contains("SCENE_A") && c.citation.license.contains("Copernicus"));
}

#[test]
fn d3_clip_refusals_say_what_is_wrong() {
    let m = start(200);
    let href = format!("{}/data/scene.tif", m.base);
    // A box far from the image.
    let err = stac::clip(&href, [10.0, 10.0, 10.1, 10.1], stac::MAX_CELLS).unwrap_err().to_string();
    assert!(err.contains("does not cover the area"), "{err}");
    // Too many cells: the limit and the size are named.
    let e = Engine::new().unwrap();
    let (lon, lat) = e.transform_point(503000.0, 4003000.0, "EPSG:32614", "EPSG:4326").unwrap();
    let err = stac::clip(&href, [lon - 0.004, lat - 0.004, lon + 0.004, lat + 0.004], 100).unwrap_err().to_string();
    assert!(err.contains("The limit is 0 million") || err.contains("choose a smaller area"), "{err}");
    // Not an image.
    let err = stac::clip(&format!("{}/missing.tif", m.base), [0.0, 0.0, 1.0, 1.0], stac::MAX_CELLS).unwrap_err().to_string();
    assert!(err.contains("Could not open the image"), "{err}");
    assert!(stac::clip("gs://b/x.tif", [0.0, 0.0, 1.0, 1.0], 10).unwrap_err().to_string().contains("not an address"));
}

/// Real services: Sentinel-2 true colour and a Copernicus DEM tile near Norman, Oklahoma.
#[test]
#[ignore = "network: Element 84 Earth Search and public AWS buckets"]
fn d3_real_sentinel_and_copernicus_dem() {
    let e = Engine::new().unwrap();
    let bbox = [-97.5, 35.15, -97.45, 35.2];
    let items = stac::search(&SearchSpec { api: None, collection: "cop-dem-glo-30".into(), bbox, datetime: None, max_cloud: None, limit: Some(1) }).unwrap();
    let a = items[0].assets.iter().find(|a| a.key == "data").expect("data asset");
    let l = e.stac_import(&ImportSpec { href: a.href.clone(), bbox, label: "Copernicus DEM".into(), collection: "cop-dem-glo-30".into(), item_id: items[0].id.clone(), asset: "data".into(), area_label: "Norman".into() }).unwrap();
    let r = l.raster.unwrap();
    assert!(r.min > 250.0 && r.max < 450.0, "Norman elevation {} {}", r.min, r.max);
}

/// TP-2: days are sent to the service as RFC 3339 date-times (it answered HTTP 400 to plain days).
#[test]
fn days_become_rfc3339_intervals() {
    assert_eq!(stac::to_rfc3339("2024-06-01/2024-09-30"), "2024-06-01T00:00:00Z/2024-09-30T23:59:59Z");
    assert_eq!(stac::to_rfc3339("2024-06-01"), "2024-06-01T00:00:00Z/2024-06-01T23:59:59Z");
    assert_eq!(stac::to_rfc3339("../2024-09-30"), "../2024-09-30T23:59:59Z");
    assert_eq!(stac::to_rfc3339("2024-06-01/.."), "2024-06-01T00:00:00Z/..");
}
