//! Rasters in every map CRS (docs/MAP_PROJECTION_PLAN.md M4, P14/P15/P17): a raster tile on a CRS's own grid is the file warped to
//! that tile's rectangle, so it lines up with vectors drawn in the same CRS, shows across a pole, and is safe under many threads.

use engine::Engine;
use std::path::Path;

/// A one-band Float32 GeoTIFF: `cols`×`rows` cells of `px`×`py` (py positive = cell height), top-left at (x0, y0).
fn tif(path: &Path, epsg: u32, cols: usize, rows: usize, x0: f64, y0: f64, px: f64, py: f64, f: impl Fn(usize, usize) -> f32) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, cols, rows, 1).unwrap();
    ds.set_geo_transform(&[x0, px, 0.0, y0, 0.0, -py]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    let mut band = ds.rasterband(1).unwrap();
    band.set_no_data_value(Some(-9999.0)).unwrap();
    let data: Vec<f32> = (0..cols * rows).map(|i| f(i / cols, i % cols)).collect();
    band.write((0, 0), (cols, rows), &mut gdal::raster::Buffer::new((cols, rows), data)).unwrap();
}

/// RGBA of a PNG tile: (width, height, bytes).
fn decode(png: &[u8], dir: &Path) -> (usize, usize, Vec<u8>) {
    assert!(png.starts_with(b"\x89PNG"), "not a PNG ({} bytes)", png.len());
    let p = dir.join("tile.png");
    std::fs::write(&p, png).unwrap();
    let ds = gdal::Dataset::open(&p).unwrap();
    let (w, h) = ds.raster_size();
    let mut out = vec![0u8; w * h * 4];
    for b in 1..=4usize {
        let band = ds.rasterband(b).unwrap().read_as::<u8>((0, 0), (w, h), (w, h), None).unwrap();
        for (i, v) in band.data().iter().enumerate() {
            out[i * 4 + b - 1] = *v;
        }
    }
    (w, h, out)
}

fn alpha(t: &(usize, usize, Vec<u8>), x: usize, y: usize) -> u8 {
    t.2[(y * t.0 + x) * 4 + 3]
}

/// The tile (z, x, y) of the grid that holds the CRS point (px, py), at the first zoom whose tile is at least `min_span` wide.
fn tile_at(di: &engine::display::DisplayInfo, px: f64, py: f64, min_span: f64) -> (u32, u32, u32) {
    let z = di.grid.resolutions.iter().position(|r| r * di.grid.tile_size as f64 <= min_span * 2.0 && r * di.grid.tile_size as f64 >= min_span).expect("a zoom with a tile that wide") as u32;
    let span = di.grid.resolutions[z as usize] * di.grid.tile_size as f64;
    (z, ((px - di.grid.origin[0]) / span).floor() as u32, ((di.grid.origin[1] - py) / span).floor() as u32)
}

#[test]
fn a_raster_shows_across_the_south_pole_in_polar_stereographic() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // Everything south of 80°S, in plain lon/lat: the pole is a line along the bottom of the file.
    let p = dir.path().join("antarctic.tif");
    tif(&p, 4326, 720, 100, -180.0, -80.0, 0.5, 0.1, |_, c| 1000.0 + c as f32);
    let layer = e.import(&p).unwrap().remove(0);
    let di = e.crs_display_info("EPSG:3031", None).unwrap();
    let (z, x, y) = tile_at(&di, 0.0, 0.0, 2_500_000.0);
    let png = e.tile_in_crs(&layer.id, &di.crs_key, z, x, y, &[]).unwrap();
    let t = decode(&png, dir.path());
    let b = di.grid.tile_bounds(z, x, y).unwrap();
    let px = |cx: f64, cy: f64| (((cx - b[0]) / (b[2] - b[0]) * t.0 as f64) as usize, ((b[3] - cy) / (b[3] - b[1]) * t.1 as f64) as usize);
    // The pole itself, and a point 500 km from it, are drawn; a point 1 500 km out (past 80°S, 1 111 km) is not.
    let (cx, cy) = px(0.0, 0.0);
    assert_eq!(alpha(&t, cx, cy), 255, "the South Pole is drawn");
    let (nx, ny) = px(500_000.0, 300_000.0);
    assert_eq!(alpha(&t, nx, ny), 255, "500 km from the pole is drawn");
    let (fx, fy) = px(1_500_000.0, 0.0);
    if fx < t.0 {
        assert_eq!(alpha(&t, fx, fy), 0, "beyond 80°S there is no raster");
    }
}

#[test]
fn a_raster_lines_up_with_the_same_rectangle_in_utm_within_one_pixel() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // A 1 km × 1 km square of data (UTM 15N, 10 m cells) in nodata, at E 500 000–501 000, N 4 400 000–4 401 000.
    let p = dir.path().join("square.tif");
    tif(&p, 26915, 300, 300, 499_900.0, 4_401_100.0, 10.0, 10.0, |r, c| if (10..110).contains(&c) && (10..110).contains(&r) { 100.0 } else { -9999.0 });
    let layer = e.import(&p).unwrap().remove(0);
    let di = e.crs_display_info("EPSG:26915", None).unwrap();
    // The data is columns 10..110, rows 10..110: E 500 000–501 000, N 4 400 000–4 401 000.
    let (z, x, y) = tile_at(&di, 500_500.0, 4_400_500.0, 1500.0);
    let png = e.tile_in_crs(&layer.id, &di.crs_key, z, x, y, &[]).unwrap();
    let t = decode(&png, dir.path());
    let b = di.grid.tile_bounds(z, x, y).unwrap();
    let one_px = (b[2] - b[0]) / t.0 as f64;
    // Find the drawn square's edges and compare with the rectangle's real edges.
    let (mut minx, mut maxx, mut miny, mut maxy) = (usize::MAX, 0, usize::MAX, 0);
    for yy in 0..t.1 {
        for xx in 0..t.0 {
            if alpha(&t, xx, yy) == 255 {
                minx = minx.min(xx);
                maxx = maxx.max(xx);
                miny = miny.min(yy);
                maxy = maxy.max(yy);
            }
        }
    }
    assert!(minx != usize::MAX, "something is drawn");
    let (west, east) = (b[0] + minx as f64 * one_px, b[0] + (maxx + 1) as f64 * one_px);
    let (north, south) = (b[3] - miny as f64 * one_px, b[3] - (maxy + 1) as f64 * one_px);
    for (got, want, name) in [(west, 500_000.0, "west"), (east, 501_000.0, "east"), (south, 4_400_000.0, "south"), (north, 4_401_000.0, "north")] {
        assert!((got - want).abs() <= one_px + 1e-9, "{name} edge {got} vs {want} (one pixel is {one_px:.2} m)");
    }
}

#[test]
fn many_threads_drawing_tiles_of_one_raster_agree_and_do_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    tif(&p, 26915, 200, 200, 500_000.0, 4_402_000.0, 10.0, 10.0, |r, c| (r + c) as f32);
    let layer = e.import(&p).unwrap().remove(0);
    let di = e.crs_display_info("EPSG:26915", None).unwrap();
    let (z, x, y) = tile_at(&di, 501_000.0, 4_401_000.0, 1500.0);
    let one = e.tile_in_crs(&layer.id, &di.crs_key, z, x, y, &[]).unwrap();
    // 32 threads, 5 rounds, each asking for neighbouring tiles too (a cold cache every time: the version is the same, so clear by asking others).
    std::thread::scope(|s| {
        for t in 0..32u32 {
            let (e, layer, key, one) = (&e, &layer, &di.crs_key, &one);
            s.spawn(move || {
                for round in 0..5 {
                    let png = e.tile_in_crs(&layer.id, key, z, x, y, &[]).unwrap();
                    assert_eq!(png.as_slice(), one.as_slice(), "thread {t} round {round}");
                    // A tile no one asked for yet forces a real warp under the cap.
                    let _ = e.tile_in_crs(&layer.id, key, z, x + 1 + t % 3, y + round % 2, &[]).unwrap();
                }
            });
        }
    });
}
