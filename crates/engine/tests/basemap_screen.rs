//! UR-19: raster tile basemaps warped by the engine to any map CRS. A small local tile server stands in for the provider: every tile of
//! every level is white with a dark disc of fixed ground size at one known place, so where that disc lands in a warped tile proves the
//! placement. No network.

use engine::layout::basemap::screen_tile;
use engine::Engine;
use std::io::{Read, Write};
use std::net::TcpListener;

const WORLD: f64 = 20037508.342789244;

fn mercator(lon: f64, lat: f64) -> (f64, f64) {
    let x = lon.to_radians() * 6378137.0;
    let y = (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * 6378137.0;
    (x, y)
}

/// A 256 px RGBA PNG of tile (z, x, y): white, a black disc of `radius` metres centred on the Web Mercator point `at`.
fn tile_png(z: u32, x: u32, y: u32, at: (f64, f64), radius: f64) -> Vec<u8> {
    let span = 2.0 * WORLD / 2f64.powi(z as i32);
    let (x0, y1) = (-WORLD + x as f64 * span, WORLD - y as f64 * span);
    let res = span / 256.0;
    let mut px = vec![255u8; 256 * 256 * 4];
    for r in 0..256 {
        for c in 0..256 {
            let (mx, my) = (x0 + (c as f64 + 0.5) * res, y1 - (r as f64 + 0.5) * res);
            if ((mx - at.0).powi(2) + (my - at.1).powi(2)).sqrt() <= radius.max(res * 0.7) {
                let i = (r * 256 + c) * 4;
                px[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 256, 256);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px).unwrap();
    out
}

/// Serve `/{z}/{x}/{y}.png`; tiles whose centre lies west of the Web Mercator x `missing_west_of` answer 404 (the provider has no tile there).
fn serve(at: (f64, f64), radius: f64, missing_west_of: f64) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("").trim_start_matches('/').trim_end_matches(".png").to_string();
                let p: Vec<u32> = path.split('/').filter_map(|s| s.parse().ok()).collect();
                let centre_x = if p.len() == 3 { -WORLD + (p[1] as f64 + 0.5) * (2.0 * WORLD / 2f64.powi(p[0] as i32)) } else { 0.0 };
                if p.len() == 3 && centre_x >= missing_west_of {
                    let body = tile_png(p[0], p[1], p[2], at, radius);
                    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = stream.write_all(&body);
                } else {
                    let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            });
        }
    });
    format!("http://127.0.0.1:{port}/${{z}}/${{x}}/${{y}}.png")
}

fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut buf = vec![0u8; dec.output_buffer_size().unwrap()];
    let info = dec.next_frame(&mut buf).unwrap();
    (info.width, info.height, buf)
}

/// Centroid (x, y in pixels) and count of the opaque dark pixels, plus how many pixels are fully transparent.
fn disc(img: &(u32, u32, Vec<u8>)) -> (Option<(f64, f64)>, usize, usize) {
    let (w, h, d) = img;
    let (mut sx, mut sy, mut n, mut clear) = (0.0, 0.0, 0usize, 0usize);
    for r in 0..*h {
        for c in 0..*w {
            let i = ((r * w + c) * 4) as usize;
            if d[i + 3] == 0 {
                clear += 1;
            } else if d[i + 3] > 200 && d[i] < 100 && d[i + 1] < 100 && d[i + 2] < 100 {
                sx += c as f64 + 0.5;
                sy += r as f64 + 0.5;
                n += 1;
            }
        }
    }
    (if n > 0 { Some((sx / n as f64, sy / n as f64)) } else { None }, n, clear)
}

struct Case {
    crs: &'static str,
    what: &'static str,
    lon: f64,
    lat: f64,
    /// Half the width of the 256 px tile, in the CRS's units.
    half: f64,
    /// Where in the tile (pixels) the point is put.
    at_px: (f64, f64),
    radius_m: f64,
}

#[test]
fn a_known_point_lands_where_it_should_in_web_mercator_utm_state_plane_polar_and_world_crs() {
    let e = Engine::new().unwrap();
    let cases = [
        Case { crs: "EPSG:3857", what: "Web Mercator", lon: -97.5, lat: 35.5, half: 20_000.0, at_px: (128.0, 128.0), radius_m: 1500.0 },
        Case { crs: "EPSG:3857", what: "Web Mercator, off centre", lon: -97.5, lat: 35.5, half: 20_000.0, at_px: (64.0, 192.0), radius_m: 1500.0 },
        Case { crs: "EPSG:26915", what: "UTM 15N", lon: -93.0, lat: 41.5, half: 20_000.0, at_px: (128.0, 128.0), radius_m: 1500.0 },
        Case { crs: "EPSG:26915", what: "UTM 15N far outside its zone (MM-3: Arizona)", lon: -111.0, lat: 33.5, half: 20_000.0, at_px: (200.0, 60.0), radius_m: 1500.0 },
        Case { crs: "EPSG:2277", what: "State Plane Texas Central (US feet)", lon: -97.74, lat: 30.27, half: 70_000.0, at_px: (100.0, 150.0), radius_m: 5000.0 },
        Case { crs: "EPSG:3413", what: "NSIDC polar stereographic north (Greenland)", lon: -45.0, lat: 72.0, half: 60_000.0, at_px: (128.0, 128.0), radius_m: 4000.0 },
        Case { crs: "EPSG:3031", what: "Antarctic polar stereographic (McMurdo)", lon: 166.67, lat: -77.85, half: 60_000.0, at_px: (180.0, 90.0), radius_m: 4000.0 },
        Case { crs: "EPSG:8857", what: "Equal Earth world projection", lon: 10.0, lat: 50.0, half: 1_000_000.0, at_px: (128.0, 128.0), radius_m: 90_000.0 },
        Case { crs: "EPSG:4326", what: "geographic WGS 84", lon: 2.35, lat: 48.85, half: 0.2, at_px: (128.0, 128.0), radius_m: 1500.0 },
    ];
    for c in &cases {
        let (mx, my) = mercator(c.lon, c.lat);
        let url = serve((mx, my), c.radius_m, f64::NEG_INFINITY);
        let (px, py) = e.transform_point(c.lon, c.lat, "EPSG:4326", c.crs).unwrap();
        // The tile's rectangle puts the point at `at_px` (pixel rows run down).
        let u = 2.0 * c.half / 256.0;
        let x0 = px - c.at_px.0 * u;
        let y1 = py + c.at_px.1 * u;
        let bounds = [x0, y1 - 256.0 * u, x0 + 256.0 * u, y1];
        let png = screen_tile(&url, 16, c.crs, bounds, 256).unwrap_or_else(|er| panic!("{}: {er:#}", c.what));
        let img = decode(&png);
        let (centroid, n, _) = disc(&img);
        let (cx, cy) = centroid.unwrap_or_else(|| panic!("{}: no dark disc found in the tile", c.what));
        assert!(n >= 4, "{}: disc has only {n} pixels", c.what);
        assert!((cx - c.at_px.0).abs() < 1.6 && (cy - c.at_px.1).abs() < 1.6, "{}: the point should be at {:?} px but its centre is at ({cx:.1}, {cy:.1})", c.what, c.at_px);
    }
}

#[test]
fn blank_where_the_crs_or_the_tiles_cannot_cover_and_never_black() {
    let e = Engine::new().unwrap();
    // The north pole is past Web Mercator's last latitude: a polar tile there is entirely transparent, not black.
    let url = serve(mercator(0.0, 60.0), 5000.0, f64::NEG_INFINITY);
    let img = decode(&screen_tile(&url, 16, "EPSG:3413", [-100_000.0, -100_000.0, 100_000.0, 100_000.0], 64).unwrap());
    let (found, _, clear) = disc(&img);
    assert!(found.is_none());
    assert_eq!(clear, 64 * 64, "past 85.05° N there is nothing to draw");
    // A provider that has no tile for part of the area: tiles west of the prime meridian are missing. That half is transparent (not black),
    // the east half is drawn, and the disc just east of the meridian is where it belongs.
    let (_, my) = mercator(0.0, 40.0);
    let url = serve((20_000.0, my), 3000.0, 0.0);
    let span = 80_000.0;
    let both = decode(&screen_tile(&url, 12, "EPSG:3857", [-span, my - span, span, my + span], 128).unwrap());
    let (found, n, clear_px) = disc(&both);
    let (w, h, d) = &both;
    let alpha_at = |c: u32, r: u32| d[((r * w + c) * 4 + 3) as usize];
    assert_eq!(alpha_at(10, 64), 0, "west of the meridian: no tile, so transparent");
    assert_eq!(alpha_at(118, 10), 255, "east of the meridian: drawn");
    assert!(clear_px > 128 * 128 / 3 && clear_px < 128 * 128 * 2 / 3, "about half is clear: {clear_px}");
    let (cx, cy) = found.expect("the disc east of the meridian");
    assert!(n >= 4 && (cx - (64.0 + 20_000.0 / (2.0 * span / 128.0))).abs() < 1.6 && (cy - 64.0).abs() < 1.6, "disc at ({cx:.1}, {cy:.1})");
    let _ = h;
}

#[test]
fn the_engine_serves_these_tiles_by_id_and_the_provider_list_matches_the_map_view_list() {
    // The map view's list of tile URLs (ui/src/map/basemaps.ts) and the engine's name the same services at the same finest level.
    let ts = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/map/basemaps.ts")).unwrap();
    for (id, url, level) in engine::layout::basemap::SCREEN_RASTERS {
        let at = ts.find(&format!("id: \"{id}\"")).unwrap_or_else(|| panic!("{id} is not in basemaps.ts"));
        let line = &ts[at..(at + 600).min(ts.len())];
        let tail = url.rsplit("/MapServer/tile/").next().unwrap_or(url);
        let host_and_path = url.split("//").nth(1).unwrap().replace("${z}", "{z}").replace("${x}", "{x}").replace("${y}", "{y}");
        let is_usgs = url.contains("basemap.nationalmap.gov");
        let svc = url.split("/services/").nth(1).and_then(|s| s.split('/').next()).unwrap_or("");
        assert!(line.contains(&format!("maxzoom: {level}")), "{id}: finest level {level} not in {line}");
        if is_usgs {
            assert!(line.contains(&format!("USGS(\"{svc}\")")), "{id}: {svc} not in {line}");
            let _ = tail;
        } else {
            // OpenTopoMap lists a, b, c servers in the UI; the engine uses one of them.
            let path = host_and_path.split_once('/').unwrap().1;
            assert!(ts.contains(path) || ts.contains(&path.replace("{z}", "{z}")), "{id}: {path} not in basemaps.ts");
        }
    }
}

/// An RGBA PNG covering the Web Mercator rectangle `merc`, `w` × `h` px: white, with a black disc of `radius` metres at `at`.
fn picture(merc: [f64; 4], w: u32, h: u32, at: (f64, f64), radius: f64) -> Vec<u8> {
    let (ux, uy) = ((merc[2] - merc[0]) / w as f64, (merc[3] - merc[1]) / h as f64);
    let mut px = vec![255u8; (w * h * 4) as usize];
    for r in 0..h {
        for c in 0..w {
            let (mx, my) = (merc[0] + (c as f64 + 0.5) * ux, merc[3] - (r as f64 + 0.5) * uy);
            if ((mx - at.0).powi(2) + (my - at.1).powi(2)).sqrt() <= radius {
                let i = ((r * w + c) * 4) as usize;
                px[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px).unwrap();
    out
}

/// The vector basemap path: the map view draws the style in Web Mercator over the rectangle the engine names, and the engine warps that
/// picture to the map CRS's rectangle. A known point must land where it belongs in the same six kinds of CRS.
#[test]
fn a_vector_basemap_drawn_in_web_mercator_is_placed_correctly_in_other_crs() {
    let e = Engine::new().unwrap();
    let cases = [
        ("EPSG:26915", "UTM 15N", -93.0, 41.5, 20_000.0, (128.0, 128.0)),
        ("EPSG:26915", "UTM 15N far outside its zone", -111.0, 33.5, 20_000.0, (200.0, 60.0)),
        ("EPSG:2277", "State Plane Texas Central (US feet)", -97.74, 30.27, 70_000.0, (100.0, 150.0)),
        ("EPSG:3413", "polar stereographic north", -45.0, 72.0, 60_000.0, (128.0, 128.0)),
        ("EPSG:3031", "Antarctic polar stereographic", 166.67, -77.85, 60_000.0, (180.0, 90.0)),
        ("EPSG:8857", "Equal Earth", 10.0, 50.0, 1_000_000.0, (128.0, 128.0)),
    ];
    for (crs, what, lon, lat, half, at_px) in cases {
        let key = e.crs_display_info(crs, None).unwrap().crs_key;
        let (px, py) = e.transform_point(lon, lat, "EPSG:4326", crs).unwrap();
        let u = 2.0 * half / 256.0;
        let x0 = px - at_px.0 * u;
        let y1 = py + at_px.1 * u;
        let extent = [x0, y1 - 256.0 * u, x0 + 256.0 * u, y1];
        let merc = e.basemap_mercator_extent(&key, extent).unwrap().unwrap_or_else(|| panic!("{what}: no Web Mercator extent"));
        // The named rectangle contains the point.
        let (mx, my) = mercator(lon, lat);
        assert!(merc[0] <= mx && mx <= merc[2] && merc[1] <= my && my <= merc[3], "{what}: {merc:?} does not contain the point");
        // Draw the picture the way the map view would: fixed pixels over `merc`.
        let (w, h) = (512u32, ((merc[3] - merc[1]) / (merc[2] - merc[0]) * 512.0).round().max(8.0) as u32);
        let ground_px = (merc[2] - merc[0]) / w as f64;
        let png = picture(merc, w, h, (mx, my), (half / 25.0 / lat.to_radians().cos()).max(ground_px * 1.5)); // Web Mercator metres are longer than ground metres away from the equator
        let warped = e.warp_basemap_picture(&png, merc, &key, extent, 256, 256).unwrap_or_else(|er| panic!("{what}: {er:#}"));
        let (centroid, n, _) = disc(&decode(&warped));
        let (cx, cy) = centroid.unwrap_or_else(|| panic!("{what}: no disc in the warped picture"));
        assert!(n >= 4, "{what}: {n} px");
        assert!((cx - at_px.0).abs() < 2.0 && (cy - at_px.1).abs() < 2.0, "{what}: expected {at_px:?}, got ({cx:.1}, {cy:.1})");
    }
}

#[test]
fn the_mercator_rectangle_for_a_map_view_is_none_where_nothing_can_be_placed_and_wide_at_a_pole() {
    let e = Engine::new().unwrap();
    let key = e.crs_display_info("EPSG:3413", None).unwrap().crs_key;
    // A square around the north pole: past Web Mercator's last latitude, so the whole width of the world, up to its top edge.
    let m = e.basemap_mercator_extent(&key, [-100_000.0, -100_000.0, 100_000.0, 100_000.0]).unwrap().unwrap();
    assert!((m[0] + WORLD).abs() < 1.0 && (m[2] - WORLD).abs() < 1.0 && m[3] > 19_000_000.0, "{m:?}");
    // Web Mercator itself is the identity.
    let w = e.crs_display_info("EPSG:3857", None).unwrap().crs_key;
    assert_eq!(e.basemap_mercator_extent(&w, [1.0, 2.0, 3.0, 4.0]).unwrap(), Some([1.0, 2.0, 3.0, 4.0]));
    // A bad extent is refused in words; a picture that isn't RGBA, too.
    assert!(e.basemap_mercator_extent(&key, [1.0, 1.0, 1.0, 1.0]).is_err());
    let gray = {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 2, 2);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[0, 0, 0, 0]).unwrap();
        out
    };
    assert!(e.warp_basemap_picture(&gray, [0.0, 0.0, 1.0, 1.0], &w, [0.0, 0.0, 1.0, 1.0], 2, 2).unwrap_err().to_string().contains("RGBA"));
    assert!(e.warp_basemap_picture(b"not a png", [0.0, 0.0, 1.0, 1.0], &w, [0.0, 0.0, 1.0, 1.0], 2, 2).unwrap_err().to_string().contains("PNG"));
}
