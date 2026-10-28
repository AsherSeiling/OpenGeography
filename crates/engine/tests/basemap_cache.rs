//! C3.0: basemap tiles are cached on disk, so editing a layout doesn't fetch them again.
//! A tiny local HTTP server stands in for the USGS tile service and counts requests.

use engine::layout::basemap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn tile_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 256, 256);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&vec![180u8; 256 * 256 * 3]).unwrap();
    }
    out
}

/// Serves the same tile for every URL; returns (url template, request counter).
fn tile_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let png = tile_png();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let (png, counter) = (png.clone(), counter.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                if String::from_utf8_lossy(&buf[..n]).starts_with("GET /tile/") {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", png.len());
                    let _ = s.write_all(&png);
                } else {
                    let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            });
        }
    });
    (format!("http://127.0.0.1:{port}/tile/${{z}}/${{y}}/${{x}}"), hits)
}

const EXTENT: [f64; 4] = [-5000.0, -5000.0, 5000.0, 5000.0];

#[test]
fn c3_0_a_second_render_reads_tiles_from_the_disk_cache_not_the_network() {
    let dir = tempfile::tempdir().unwrap();
    let (url, hits) = tile_server();

    let ds = basemap::open_url(&url, Some(dir.path())).unwrap();
    let first = basemap::render(&ds, "EPSG:3857", EXTENT, 128, 128).unwrap();
    drop(ds);
    let fetched = hits.load(Ordering::SeqCst);
    assert!(fetched > 0, "the first render fetched tiles");
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_some(), "tiles were written to the cache folder");

    // A new dataset (another preview after an edit) over the same cache: no new requests, same picture.
    let ds = basemap::open_url(&url, Some(dir.path())).unwrap();
    let second = basemap::render(&ds, "EPSG:3857", EXTENT, 128, 128).unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), fetched, "no network requests for cached tiles");
    assert_eq!(first, second);
}

#[test]
fn c3_0_without_a_cache_every_open_fetches_again() {
    let (url, hits) = tile_server();
    for _ in 0..2 {
        let ds = basemap::open_url(&url, None).unwrap();
        basemap::render(&ds, "EPSG:3857", EXTENT, 128, 128).unwrap();
    }
    let n = hits.load(Ordering::SeqCst);
    assert!(n >= 2 && n % 2 == 0, "two uncached renders fetch the same tiles twice ({n} requests)");
}

#[test]
fn c3_0_the_cache_folder_is_per_user_and_can_be_overridden() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_CACHE", dir.path().join("t"));
    let d = engine::resources::tile_cache_dir().unwrap();
    assert!(d.is_dir() && d.ends_with("t"));
    std::env::remove_var("GIS_STUDIO_CACHE");
}
