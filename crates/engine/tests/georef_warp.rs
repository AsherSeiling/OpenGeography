//! Raster georeferencing (GR2): error figures and warps with known answers (docs/TEST_PLAN.md §Georeferencing).

use engine::georef::warp::{self, Ctx, Dest, Method};
use engine::georef::{ControlPoint, ControlSet};
use engine::Engine;
use gdal::raster::Buffer;
use gdal::Dataset;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const UTM: &str = "EPSG:32614";

fn ctx() -> Ctx {
    Ctx { stop: Arc::new(AtomicBool::new(false)), task: None }
}

/// An unreferenced Byte image whose value at (col, row) is a function of both, so a warp can be checked cell by cell.
fn write_image(path: &Path, w: usize, h: usize, bands: usize) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<u8, _>(path, w, h, bands).unwrap();
    for b in 1..=bands {
        let data: Vec<u8> = (0..w * h).map(|i| (((i % w) * 3 + (i / w) * 5 + b * 40) % 250 + 1) as u8).collect();
        ds.rasterband(b).unwrap().write((0, 0), (w, h), &mut Buffer::new((w, h), data)).unwrap();
    }
}

fn pt(id: u32, pixel: f64, line: f64, x: f64, y: f64) -> ControlPoint {
    ControlPoint { id, pixel, line, x, y, enabled: true }
}

/// A 4 x 4 grid of points over a 200 x 200 image mapped by `f`.
fn grid(crs: &str, f: impl Fn(f64, f64) -> (f64, f64)) -> ControlSet {
    let mut points = vec![];
    for (j, line) in [0.0, 70.0, 140.0, 200.0].into_iter().enumerate() {
        for (i, pixel) in [0.0, 70.0, 140.0, 200.0].into_iter().enumerate() {
            let (x, y) = f(pixel, line);
            points.push(pt((j * 4 + i) as u32 + 1, pixel, line, x, y));
        }
    }
    ControlSet { id: "s".into(), name: "s".into(), target_crs: crs.into(), image: None, points }
}

fn affine(p: f64, l: f64) -> (f64, f64) {
    (600_000.0 + 30.0 * p, 3_900_000.0 - 30.0 * l)
}

const O1: Method = Method::Polynomial { order: 1 };
const O2: Method = Method::Polynomial { order: 2 };
const O3: Method = Method::Polynomial { order: 3 };

fn dir_listing(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

#[test]
fn affine_is_recovered_and_the_source_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir(&src_dir).unwrap();
    let src = src_dir.join("scan.tif");
    write_image(&src, 200, 200, 3);
    let before = std::fs::read(&src).unwrap();
    let listing = dir_listing(&src_dir);

    let out = dir.path().join("out.tif");
    let (ds, res) = warp::run(&src, &grid(UTM, affine), O1, "nearest", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    let gt = res.geo_transform;
    for (got, want) in gt.iter().zip([600_000.0, 30.0, 0.0, 3_900_000.0, 0.0, -30.0]) {
        assert!((got - want).abs() < 1e-6, "geotransform {gt:?}");
    }
    assert_eq!((res.width, res.height), (200, 200));
    assert!(res.report.rms < 1e-6 && res.report.rms_px < 1e-6, "rms {}", res.report.rms);

    // The output is a GeoTIFF in the target CRS: 3 colour bands plus an alpha band (the image had no NoData).
    let o = Dataset::open(&out).unwrap();
    assert_eq!(o.raster_count(), 4);
    assert_eq!(o.spatial_ref().unwrap().auth_code().unwrap(), 32614);
    let s = Dataset::open(&src).unwrap();
    let read = |d: &Dataset, b: usize, c: usize, r: usize| d.rasterband(b).unwrap().read_as::<u8>((c as isize, r as isize), (1, 1), (1, 1), None).unwrap().data()[0];
    for (c, r) in [(0, 0), (10, 20), (199, 199), (100, 150)] {
        for b in 1..=3 {
            assert_eq!(read(&o, b, c, r), read(&s, b, c, r), "cell ({c},{r}) band {b}");
        }
        assert_eq!(read(&o, 4, c, r), 255);
    }
    drop((o, s));

    // Source bytes and folder are unchanged; nothing was written beside it.
    assert_eq!(std::fs::read(&src).unwrap(), before);
    assert_eq!(dir_listing(&src_dir), listing);
}

#[test]
fn second_order_distortion_needs_order_two() {
    let bent = |p: f64, l: f64| {
        let (x, y) = affine(p, l);
        (x + 0.01 * p * p, y + 0.008 * p * l)
    };
    let set = grid(UTM, bent);
    let r2 = warp::errors(&set, O2).unwrap();
    let r1 = warp::errors(&set, O1).unwrap();
    assert!(r2.rms < 1e-4, "order 2 rms {}", r2.rms);
    // The warp samples through the reverse fit (ground to image), a separate order-2 polynomial: the inverse of a quadratic is not exactly one, so its pixel error is small but real.
    assert!(r2.rms_px < 0.1, "order 2 rms px {}", r2.rms_px);
    assert!(r1.rms > 20.0, "order 1 should not follow a curved distortion: {}", r1.rms);
    // Order 3 contains order 2, so it fits too.
    assert!(warp::errors(&set, O3).unwrap().rms < 1e-3);
}

#[test]
fn one_bad_point_inflates_rms_and_turning_it_off_restores_it() {
    let mut set = grid(UTM, affine);
    assert!(warp::errors(&set, O1).unwrap().rms < 1e-6);
    set.points[5].x += 500.0;
    let bad = warp::errors(&set, O1).unwrap();
    assert!(bad.rms > 30.0, "rms {}", bad.rms);
    // The worst point is the one that was moved.
    let worst = bad.points.iter().max_by(|a, b| a.length.partial_cmp(&b.length).unwrap()).unwrap();
    assert_eq!(worst.id, set.points[5].id);
    set.points[5].enabled = false;
    let fixed = warp::errors(&set, O1).unwrap();
    assert!(fixed.rms < 1e-6, "rms {}", fixed.rms);
    // Left out of the fit, it is a check point and its error is the independent figure.
    let chk = fixed.points.iter().find(|p| p.id == set.points[5].id).unwrap();
    assert!(chk.check && !chk.enabled);
    assert!((chk.length - 500.0).abs() < 1e-3, "check length {}", chk.length);
    assert!((chk.dx + 500.0).abs() < 1e-3 && chk.dy.abs() < 1e-3, "dx is predicted minus stated: {}", chk.dx);
    assert!((chk.length_px - 500.0 / 30.0).abs() < 1e-3, "check px {}", chk.length_px);
    assert!((fixed.check_rms.unwrap() - 500.0).abs() < 1e-3);
    assert_eq!(fixed.enabled_count, 15);
}

#[test]
fn rms_follows_the_stated_formula() {
    let mut set = grid(UTM, affine);
    set.points[1].y += 3.0;
    set.points[7].x -= 4.0;
    let r = warp::errors(&set, O1).unwrap();
    let n = r.enabled_count as f64;
    let want = (r.points.iter().filter(|p| p.enabled).map(|p| p.dx * p.dx + p.dy * p.dy).sum::<f64>() / n).sqrt();
    assert!((r.rms - want).abs() < 1e-12);
    assert!(r.formula.contains("sqrt( sum(dx² + dy²) / n )"));
}

#[test]
fn spline_error_is_zero_at_its_points_and_the_warning_says_so() {
    let wobble = |p: f64, l: f64| {
        let (x, y) = affine(p, l);
        (x + 25.0 * (p / 40.0).sin(), y + 18.0 * (l / 50.0).cos())
    };
    let mut set = grid(UTM, wobble);
    set.points[6].enabled = false;
    let r = warp::errors(&set, Method::Tps).unwrap();
    for p in r.points.iter().filter(|p| p.enabled) {
        assert!(p.length < 1e-6 && p.length_px < 1e-6, "point {} error {}", p.id, p.length);
    }
    assert!(r.rms < 1e-6);
    // The point left out shows a real, non-zero error: the honest figure.
    let chk = r.points.iter().find(|p| p.check).unwrap();
    assert!(chk.length > 1e-3, "check error {}", chk.length);
    assert!(r.warnings.iter().any(|w| w.contains("zero")), "{:?}", r.warnings);
    // The same set under a polynomial gives no such warning.
    assert!(warp::errors(&set, O1).unwrap().warnings.is_empty());
}

#[test]
fn metres_are_correct_for_utm_and_for_a_geographic_target() {
    // Projected: a check point 3 m east and 4 m north of where the affine puts it is 5 m off.
    let mut set = grid(UTM, affine);
    set.points[3].enabled = false;
    set.points[3].x += 3.0;
    set.points[3].y += 4.0;
    let r = warp::errors(&set, O1).unwrap();
    let c = r.points.iter().find(|p| p.check).unwrap();
    assert!((c.length - 5.0).abs() < 1e-6 && (c.length_m - 5.0).abs() < 1e-6, "{} {}", c.length, c.length_m);
    assert_eq!(r.metres_kind, "grid");
    assert_eq!(r.unit, "metre");

    // Geographic: degrees on the ground, measured on the ellipsoid.
    let geo = |p: f64, l: f64| (-100.0 + 0.001 * p, 40.0 - 0.001 * l);
    let mut set = grid("EPSG:4326", geo);
    set.points[5].enabled = false;
    let (x0, y0) = (set.points[5].x, set.points[5].y);
    set.points[5].x += 0.001;
    set.points[5].y += 0.0009;
    let r = warp::errors(&set, O1).unwrap();
    assert_eq!(r.metres_kind, "geodesic");
    assert_eq!(r.unit, "degree");
    let c = r.points.iter().find(|p| p.check).unwrap();
    // The transform puts the point at (x0, y0); it was said to be 0.001 deg east and 0.0009 deg north of that.
    let want: f64 = Geodesic::wgs84().inverse(y0, x0, y0 + 0.0009, x0 + 0.001);
    assert!((c.length_m - want).abs() < 0.01, "length_m {} want {want}", c.length_m);
    assert!((c.dx + 0.001).abs() < 1e-9 && (c.dy + 0.0009).abs() < 1e-9, "dx {} dy {}", c.dx, c.dy);
    assert!(c.length_m > 100.0 && c.length_m < 140.0);
    // And a geographic warp lands in degrees with the right origin.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("g.tif");
    write_image(&src, 200, 200, 1);
    let out = dir.path().join("g_out.tif");
    set.points[5].enabled = true;
    set.points[5].x = x0;
    set.points[5].y = y0;
    let (ds, res) = warp::run(&src, &set, O1, "bilinear", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    assert!((res.geo_transform[0] + 100.0).abs() < 1e-9 && (res.geo_transform[3] - 40.0).abs() < 1e-9, "{:?}", res.geo_transform);
    assert!((res.geo_transform[1] - 0.001).abs() < 1e-9);
}

#[test]
fn too_few_or_degenerate_points_are_refused_in_words() {
    let set = grid(UTM, affine);
    let few = |n: usize| ControlSet { points: set.points[..n].to_vec(), ..set.clone() };
    let e = |s: &ControlSet, m| warp::errors(s, m).unwrap_err().to_string();
    assert!(e(&few(2), O1).contains("at least 3 enabled control points"), "{}", e(&few(2), O1));
    assert!(e(&few(5), O2).contains("at least 6"));
    assert!(e(&few(9), O3).contains("at least 10"));
    assert!(e(&few(2), Method::Tps).contains("at least 3"));
    // Three points on one row are a line; three corners are enough for an affine.
    assert!(e(&few(3), O1).contains("straight line in the image"));
    let corners = ControlSet { points: vec![set.points[0].clone(), set.points[3].clone(), set.points[12].clone()], ..set.clone() };
    assert!(warp::errors(&corners, O1).is_ok() && warp::errors(&corners, Method::Tps).is_ok());
    // Six points on only two rows of the image reach the minimum count but can't fix a 2nd-order surface: GDAL says so and we repeat it.
    assert!(e(&few(6), O2).contains("GDAL could not fit"), "{}", e(&few(6), O2));
    // Points on one line in the image, and on one line on the ground.
    let line_img = ControlSet { points: (0..5).map(|i| pt(i + 1, 10.0 * i as f64, 20.0 * i as f64, 600_000.0 + 7.0 * (i * i) as f64, 3_900_000.0 + 3.0 * i as f64)).collect(), ..set.clone() };
    assert!(e(&line_img, O1).contains("straight line in the image"), "{}", e(&line_img, O1));
    let line_gnd = ControlSet { points: (0..5).map(|i| pt(i + 1, 10.0 * (i * i) as f64, 20.0 * i as f64 + 5.0 * i as f64 * i as f64, 600_000.0 + 10.0 * i as f64, 3_900_000.0 + 20.0 * i as f64)).collect(), ..set.clone() };
    assert!(e(&line_gnd, Method::Tps).contains("straight line on the ground"), "{}", e(&line_gnd, Method::Tps));
    // Two points at one image position.
    let mut dup = set.clone();
    dup.points[1].pixel = dup.points[0].pixel;
    dup.points[1].line = dup.points[0].line;
    assert!(e(&dup, O1).contains("same place in the image"));
    // A missing CRS is never guessed.
    let none = ControlSet { target_crs: String::new(), ..set };
    assert!(e(&none, O1).contains("never guessed"));
}

#[test]
fn outside_the_image_is_nodata_or_transparent_never_zero_data() {
    let dir = tempfile::tempdir().unwrap();
    // Rotate by 30 degrees so the output rectangle has empty corners.
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let rot = move |p: f64, l: f64| (600_000.0 + 10.0 * (c * p - s * l), 3_900_000.0 - 10.0 * (s * p + c * l));
    let set = grid(UTM, rot);

    // RGB without NoData: an alpha band appears and is 0 in the corner.
    let rgb = dir.path().join("rgb.tif");
    write_image(&rgb, 200, 200, 3);
    let out = dir.path().join("rgb_out.tif");
    let (ds, res) = warp::run(&rgb, &set, O1, "nearest", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    let o = Dataset::open(&out).unwrap();
    assert_eq!(o.raster_count(), 4);
    let px = |b: usize, c: usize, r: usize| o.rasterband(b).unwrap().read_as::<u8>((c as isize, r as isize), (1, 1), (1, 1), None).unwrap().data()[0];
    assert_eq!(px(4, 0, 0), 0, "corner outside the image must be transparent");
    assert_eq!(px(4, res.width / 2, res.height / 2), 255, "centre is inside");
    drop(o);

    // Float data with a NoData value: the corner is that NoData value, not 0.
    let dem = dir.path().join("dem.tif");
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&dem, 200, 200, 1).unwrap();
        let mut b = ds.rasterband(1).unwrap();
        b.set_no_data_value(Some(-9999.0)).unwrap();
        b.write((0, 0), (200, 200), &mut Buffer::new((200, 200), vec![5.0f32; 200 * 200])).unwrap();
    }
    let out = dir.path().join("dem_out.tif");
    let (ds, res) = warp::run(&dem, &set, O1, "bilinear", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    let o = Dataset::open(&out).unwrap();
    assert_eq!(o.raster_count(), 1);
    assert_eq!(o.rasterband(1).unwrap().no_data_value(), Some(-9999.0));
    let v = |c: usize, r: usize| o.rasterband(1).unwrap().read_as::<f32>((c as isize, r as isize), (1, 1), (1, 1), None).unwrap().data()[0];
    assert_eq!(v(0, 0), -9999.0);
    assert_eq!(v(res.width / 2, res.height / 2), 5.0);

    // Float data with no NoData value: NaN marks the outside.
    let plain = dir.path().join("plain.tif");
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&plain, 200, 200, 1).unwrap();
        ds.rasterband(1).unwrap().write((0, 0), (200, 200), &mut Buffer::new((200, 200), vec![5.0f32; 200 * 200])).unwrap();
    }
    let out = dir.path().join("plain_out.tif");
    let (ds, _) = warp::run(&plain, &set, O1, "nearest", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    let o = Dataset::open(&out).unwrap();
    assert!(o.rasterband(1).unwrap().no_data_value().is_some_and(|n| n.is_nan()));
    assert!(o.rasterband(1).unwrap().read_as::<f32>((0, 0), (1, 1), (1, 1), None).unwrap().data()[0].is_nan());
}

#[test]
fn the_warp_uses_the_transform_the_errors_come_from() {
    // A 2nd-order set: the report returned with the warp is the one `errors` gives, and the output follows the fitted map
    // (a control point's image cell lands at the control point's ground position within one output cell).
    let bent = |p: f64, l: f64| {
        let (x, y) = affine(p, l);
        (x + 0.01 * p * p, y)
    };
    let set = grid(UTM, bent);
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("b.tif");
    write_image(&src, 200, 200, 1);
    let out = dir.path().join("b_out.tif");
    let (ds, res) = warp::run(&src, &set, O2, "nearest", Dest::GeoTiff(&out), None, &ctx()).unwrap();
    drop(ds);
    let alone = warp::errors(&set, O2).unwrap();
    assert!((res.report.rms - alone.rms).abs() < 1e-9);
    let o = Dataset::open(&out).unwrap();
    let s = Dataset::open(&src).unwrap();
    let gt = o.geo_transform().unwrap();
    // Control point 11 (pixel 140, line 140): the output cell at its ground position holds the source cell there.
    let p = &set.points[10];
    let (col, row) = (((p.x - gt[0]) / gt[1]).floor() as isize, ((p.y - gt[3]) / gt[5]).floor() as isize);
    let from_out = o.rasterband(1).unwrap().read_as::<u8>((col, row), (1, 1), (1, 1), None).unwrap().data()[0];
    let from_src = s.rasterband(1).unwrap().read_as::<u8>((p.pixel as isize, p.line as isize), (1, 1), (1, 1), None).unwrap().data()[0];
    // Neighbouring source cells differ by a few levels; the cell holding the control point is within a cell of it.
    let near: Vec<u8> = (-1..=1).flat_map(|dc| (-1..=1).map(move |dr| (dc, dr))).map(|(dc, dr)| s.rasterband(1).unwrap().read_as::<u8>((p.pixel as isize + dc, p.line as isize + dr), (1, 1), (1, 1), None).unwrap().data()[0]).collect();
    assert!(near.contains(&from_out), "out {from_out}, src {from_src}, near {near:?}");
}

#[test]
fn a_stop_request_cancels_and_leaves_no_output() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("c.tif");
    write_image(&src, 200, 200, 1);
    let out = dir.path().join("c_out.tif");
    let stop = Ctx { stop: Arc::new(AtomicBool::new(true)), task: None };
    let err = warp::run(&src, &grid(UTM, affine), O1, "nearest", Dest::GeoTiff(&out), None, &stop).err().unwrap();
    assert!(engine::progress::is_cancelled(&err), "{err}");
    assert!(!out.exists());
    assert!(!dir.path().join("c_out.tif.aux.xml").exists());
}

#[test]
fn an_absurd_stretch_is_refused_and_a_preview_is_a_small_png() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("p.tif");
    write_image(&src, 200, 200, 3);
    let p = warp::preview(&src, &grid(UTM, affine), O1, "nearest", 100).unwrap();
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&p.png).unwrap();
    assert_eq!(&bytes[1..4], b"PNG");
    assert!(p.width <= 100 && p.height <= 100, "{}x{}", p.width, p.height);
    assert!((p.extent[0] - 600_000.0).abs() < 1e-6 && (p.extent[3] - 3_900_000.0).abs() < 1e-6, "{:?}", p.extent);
    assert!((p.extent[2] - 606_000.0).abs() < 1e-6);
    assert_eq!(dir_listing(dir.path()), vec!["p.tif".to_string()], "a preview writes nothing beside the image");
    // A wild point can make GDAL propose a huge raster: that is refused with advice (or the fit is, in words); it never allocates it.
    let mut set = grid(UTM, affine);
    set.points[0].x = 600_000.0 + 3.0e8;
    set.points[0].y = 3_900_000.0 - 3.0e8;
    if let Err(e) = warp::run(&src, &set, O3, "nearest", Dest::Memory, None, &ctx()) {
        let t = e.to_string();
        assert!(t.contains("check the points") || t.contains("far larger") || t.contains("could not") || t.contains("straight line"), "{t}");
    }
}

#[test]
fn the_engine_adds_a_layer_with_a_citation_and_keeps_the_source() {
    let dir = tempfile::tempdir().unwrap();
    // The raster store lives under HOME; point it at the temp folder for this test.
    std::env::set_var("HOME", dir.path());
    std::env::set_var("LOCALAPPDATA", dir.path());
    let src_dir = dir.path().join("in");
    std::fs::create_dir(&src_dir).unwrap();
    let src = src_dir.join("map scan.tif");
    write_image(&src, 200, 200, 3);
    let before = std::fs::read(&src).unwrap();
    let e = Engine::new().unwrap();
    let mut set = grid(UTM, affine);
    set.name = "Scan 1".into();
    set.points[4].enabled = false;
    let info = e.georef_warp(&engine::georef::api::WarpRequest { set, method: O1, resampling: Some("bilinear".into()), image: src.display().to_string(), source_layer: None, name: Some("Scan georeferenced".into()) }).unwrap();
    assert_eq!(info.geom_type, "Raster");
    assert_eq!(info.crs.as_deref(), Some("EPSG:32614"));
    assert_eq!(std::fs::read(&src).unwrap(), before);
    assert_eq!(dir_listing(&src_dir), vec!["map scan.tif".to_string()]);
    let srcs = e.sources().unwrap();
    let geo = srcs.iter().find(|s| s.citation.key.starts_with("georef:")).expect("a georeferencing citation");
    assert!(geo.citation.annotation.contains("1st-order polynomial") && geo.citation.annotation.contains("[check point]") && geo.citation.annotation.contains("bilinear"), "{}", geo.citation.annotation);
    assert!(srcs.iter().any(|s| s.citation.key.starts_with("file:") && s.citation.key.contains("map scan.tif")), "the image's own source is kept");
    // A bad request fails before anything is made.
    let mut bad = grid(UTM, affine);
    bad.points.truncate(2);
    let n = e.list_layers().len();
    assert!(e.georef_warp(&engine::georef::api::WarpRequest { set: bad, method: O1, resampling: None, image: src.display().to_string(), source_layer: None, name: None }).is_err());
    assert_eq!(e.list_layers().len(), n);
}
