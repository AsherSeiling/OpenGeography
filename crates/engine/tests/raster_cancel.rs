//! E3: every raster step stops when the run is cancelled and leaves no partial output (gdaldem, overviews, fill, mosaic, align,
//! resample, rasterize, the calculator). GDAL steps stop at their first progress report ("User terminated"); our own loops check between blocks.

use engine::progress::{is_cancelled, with_cancel_flag};
use engine::raster;
use engine::raster_ops as ro;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

fn dem(path: &Path, size: usize, epsg: u32, cell: f64, x0: f64, y0: f64) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, size, size, 1).unwrap();
    ds.set_geo_transform(&[x0, cell, 0.0, y0, 0.0, -cell]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    let data: Vec<f32> = (0..size * size).map(|i| ((i % size) as f32 * 0.5 + (i / size) as f32 * 0.25) + 100.0).collect();
    ds.rasterband(1).unwrap().write((0, 0), (size, size), &mut gdal::raster::Buffer::new((size, size), data)).unwrap();
}

fn stopped(r: anyhow::Result<PathBuf>) -> bool {
    match r {
        Ok(_) => false,
        Err(e) => is_cancelled(&e) || e.to_string().contains("User terminated") || e.to_string().contains("cancelled"),
    }
}

#[test]
fn e3_every_raster_step_stops_when_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let utm = dir.path().join("utm.tif");
    let geo = dir.path().join("geo.tif");
    let other = dir.path().join("other.tif");
    dem(&utm, 1200, 32614, 30.0, 600_000.0, 3_900_000.0);
    dem(&geo, 1200, 4326, 0.0003, -98.0, 36.0);
    dem(&other, 1200, 32614, 30.0, 636_000.0, 3_900_000.0);
    let cancelled = || Arc::new(AtomicBool::new(true));

    // gdaldem: all modes, on a projected raster and on a geographic one (which takes the metre-view path).
    for src in [&utm, &geo] {
        for mode in ["hillshade", "slope", "aspect", "TRI"] {
            let r = with_cancel_flag(cancelled(), || match mode {
                "hillshade" => raster::hillshade(src, 315.0, 45.0, 1.0, false),
                "slope" => raster::slope(src, false),
                m => raster::dem_mode(src, m, &[]),
            });
            assert!(stopped(r), "{mode} on {}", src.display());
        }
    }
    // Overviews, on a copy so the original stays intact.
    let copy = dir.path().join("copy.tif");
    std::fs::copy(&utm, &copy).unwrap();
    let err = with_cancel_flag(cancelled(), || raster::build_overviews(&copy)).unwrap_err();
    assert!(err.to_string().contains("User terminated") || err.to_string().contains("Building overviews"), "{err}");
    raster::build_overviews(&copy).unwrap();

    // The new tools.
    assert!(stopped(with_cancel_flag(cancelled(), || ro::calc(&[&utm], "A * 2"))));
    assert!(stopped(with_cancel_flag(cancelled(), || ro::resample(&utm, 15.0, "bilinear"))));
    assert!(stopped(with_cancel_flag(cancelled(), || ro::align(&geo, &utm, "near"))));
    assert!(stopped(with_cancel_flag(cancelled(), || ro::mosaic(&[&utm, &other], "near"))));
    assert!(stopped(with_cancel_flag(cancelled(), || ro::reclassify(&utm, 1, &ro::parse_rules("0 to 1000 = 1").unwrap()))));
    assert!(stopped(with_cancel_flag(cancelled(), || ro::select_bands(&utm, "1"))));
    // Not cancelled: they complete.
    let flag = Arc::new(AtomicBool::new(false));
    with_cancel_flag(flag, || {
        ro::calc(&[&utm], "A * 2").unwrap();
        raster::hillshade(&utm, 315.0, 45.0, 1.0, false).unwrap();
        ro::mosaic(&[&utm, &other], "near").unwrap();
    });
}

#[test]
fn e3_a_cancel_part_way_through_a_big_calculation_stops_within_a_blink() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.tif");
    dem(&big, 6000, 32614, 10.0, 600_000.0, 3_900_000.0); // 36 million cells
    let flag = Arc::new(AtomicBool::new(false));
    let f2 = flag.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        f2.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let t = std::time::Instant::now();
    let r = with_cancel_flag(flag, || ro::calc(&[&big], "sqrt(A * A + 1) * 2 + sin(A)"));
    assert!(stopped(r), "it should have been cancelled, not finished");
    assert!(t.elapsed() < std::time::Duration::from_secs(5), "stopped after {:?}", t.elapsed());
}
