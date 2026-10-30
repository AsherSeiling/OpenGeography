//! Raster tools on tiny synthetic grids with analytically known answers:
//! slope/aspect/hillshade of a plane, contours, clip, warp, polygonize,
//! sampling and zonal statistics with NoData (docs/TEST_PLAN.md §Raster).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

const X0: f64 = 600_000.0;
const Y0: f64 = 3_900_000.0;

/// `size`×`size` GeoTIFF with 30 m pixels, top-left at (X0, Y0), UTM 14N unless `epsg` is None.
fn dem(path: &Path, size: usize, nodata: Option<f64>, epsg: Option<u32>, f: impl Fn(usize, usize) -> f32) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, size, size, 1).unwrap();
    ds.set_geo_transform(&[X0, 30.0, 0.0, Y0, 0.0, -30.0]).unwrap();
    if let Some(code) = epsg {
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(code).unwrap()).unwrap();
    }
    let mut band = ds.rasterband(1).unwrap();
    if let Some(nd) = nodata {
        band.set_no_data_value(Some(nd)).unwrap();
    }
    let data: Vec<f32> = (0..size * size).map(|i| f(i / size, i % size)).collect();
    band.write((0, 0), (size, size), &mut gdal::raster::Buffer::new((size, size), data)).unwrap();
}

fn pixel(path: &str, col: isize, row: isize) -> f32 {
    let ds = gdal::Dataset::open(path).unwrap();
    ds.rasterband(1).unwrap().read_as::<f32>((col, row), (1, 1), (1, 1), None).unwrap().data()[0]
}

fn graph(v: Value) -> Graph {
    serde_json::from_value(v).unwrap()
}

fn run(e: &Engine, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap();
    let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
    let mut edges = vec![];
    for (i, l) in inputs.iter().enumerate() {
        nodes.push(json!({"id": format!("in{i}"), "tool": "input_layer", "params": {"layer": l}}));
        edges.push(json!({"source": format!("in{i}"), "target": "t", "targetHandle": spec.inputs[i].id}));
    }
    let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
    res.into_iter().find(|r| r.id == "t").unwrap()
}

fn ok(e: &Engine, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let r = run(e, tool, params, inputs);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{tool}: {:?} {:?}", r.status, r.error);
    r
}

fn import(e: &Engine, path: &Path) -> String {
    e.import(path).unwrap().remove(0).id
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (±{tol})");
}

#[test]
fn slope_aspect_and_hillshade_of_a_plane_match_the_formulas() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // z rises 1 m per pixel eastwards: rise/run = 1/30, the surface faces west.
    let p = dir.path().join("plane.tif");
    dem(&p, 20, None, Some(32614), |_, c| c as f32);
    let id = import(&e, &p);

    let deg = ok(&e, "slope", json!({"units": "degrees"}), &[&id]);
    assert_eq!(deg.crs.as_deref(), Some("EPSG:32614"), "raster results keep the CRS");
    close(pixel(deg.output.as_ref().unwrap(), 10, 10) as f64, (1.0f64 / 30.0).atan().to_degrees(), 0.01);
    let pct = ok(&e, "slope", json!({"units": "percent"}), &[&id]);
    close(pixel(pct.output.as_ref().unwrap(), 10, 10) as f64, 100.0 / 30.0, 0.01);

    let asp = ok(&e, "aspect", json!({}), &[&id]);
    close(pixel(asp.output.as_ref().unwrap(), 10, 10) as f64, 270.0, 0.5);

    // Hillshade of a flat surface at 45° sun altitude is 255·sin(45°) ≈ 180, whatever the azimuth.
    let flat = dir.path().join("flat.tif");
    dem(&flat, 20, None, Some(32614), |_, _| 100.0);
    let fid = import(&e, &flat);
    for az in [0.0, 135.0, 315.0] {
        let h = ok(&e, "hillshade", json!({"azimuth": az, "altitude": 45, "z": 1}), &[&fid]);
        close(pixel(h.output.as_ref().unwrap(), 10, 10) as f64, 255.0 * 45f64.to_radians().sin(), 2.0);
    }
    // Vertical exaggeration makes the same plane steeper.
    let s1 = ok(&e, "slope", json!({"units": "degrees"}), &[&id]);
    let hs_flat = pixel(ok(&e, "hillshade", json!({"azimuth": 270, "altitude": 45, "z": 1}), &[&id]).output.as_ref().unwrap(), 10, 10);
    let hs_exag = pixel(ok(&e, "hillshade", json!({"azimuth": 270, "altitude": 45, "z": 10}), &[&id]).output.as_ref().unwrap(), 10, 10);
    assert!(hs_exag != hs_flat, "z factor must change the shading ({hs_flat} vs {hs_exag})");
    let _ = s1;

    // Ruggedness of a perfect plane/flat surface is zero.
    let r = ok(&e, "ruggedness", json!({"mode": "TRI"}), &[&fid]);
    close(pixel(r.output.as_ref().unwrap(), 10, 10) as f64, 0.0, 1e-6);
    let tpi = ok(&e, "ruggedness", json!({"mode": "TPI"}), &[&id]);
    close(pixel(tpi.output.as_ref().unwrap(), 10, 10) as f64, 0.0, 1e-6);
}

#[test]
fn contours_follow_the_interval_base_and_index_settings() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 40, None, Some(32614), |_, c| c as f32); // 0…39 m west to east
    let id = import(&e, &p);
    let c = ok(&e, "contours", json!({"interval": 10, "base": 0, "index_every": 2}), &[&id]);
    assert_eq!(c.crs.as_deref(), Some("EPSG:32614"));
    assert_eq!(c.rows, Some(3), "10, 20 and 30 m");
    let layer = e.preview_output(c.output.as_ref().unwrap(), c.crs.clone(), "c").unwrap();
    assert!(layer.fields.iter().any(|f| f.name.eq_ignore_ascii_case("elev")), "{:?}", layer.fields);
    assert_eq!(layer.geom_type, "Line");
    let shifted = ok(&e, "contours", json!({"interval": 10, "base": 5, "index_every": 0}), &[&id]);
    assert_eq!(shifted.rows, Some(4), "5, 15, 25 and 35 m");
    assert_eq!(ok(&e, "contours", json!({"interval": 100}), &[&id]).rows, Some(0), "interval above the relief: no lines");
    for bad in [0.0, -5.0] {
        assert_eq!(run(&e, "contours", json!({"interval": bad}), &[&id]).status, Status::Error, "interval {bad}");
    }
}

#[test]
fn clip_and_warp_keep_the_grid_honest() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 40, None, Some(32614), |_, c| c as f32);
    let id = import(&e, &p);
    // Clip to a 300 m box (10 px) in the middle: columns 10–19 hold 10…19.
    let box_path = dir.path().join("box.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, ST_MakeEnvelope({}, {}, {}, {}) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:32614', USE_TMP_FILE false)",
        X0 + 300.0, Y0 - 600.0, X0 + 600.0, Y0 - 300.0, box_path.display()
    ))
    .unwrap();
    let bx = import(&e, &box_path);
    let r = ok(&e, "clip_raster", json!({}), &[&id, &bx]);
    let out = r.output.unwrap();
    let ds = gdal::Dataset::open(&out).unwrap();
    let (w, h) = ds.raster_size();
    assert!((9..=12).contains(&w) && (9..=12).contains(&h), "{w}×{h}");
    let gt = ds.geo_transform().unwrap();
    close(gt[0], X0 + 300.0, 31.0);
    close(gt[1], 30.0, 1e-9);
    assert!(pixel(&out, 0, 0) >= 9.0 && pixel(&out, 0, 0) <= 11.0);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));

    // Warp to geographic: new CRS, pixel size in degrees, values preserved at the centre.
    let wr = ok(&e, "warp", json!({"crs": "EPSG:4326", "resampling": "bilinear"}), &[&id]);
    assert_eq!(wr.crs.as_deref(), Some("EPSG:4326"));
    let wp = wr.output.unwrap();
    let wds = gdal::Dataset::open(&wp).unwrap();
    assert!(wds.geo_transform().unwrap()[1] < 0.01, "pixel size is in degrees");
    let (ww, wh) = wds.raster_size();
    close(pixel(&wp, ww as isize / 2, wh as isize / 2) as f64, 20.0, 3.0);
}

#[test]
fn warp_to_an_invalid_crs_is_an_error_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 10, None, Some(32614), |_, c| c as f32);
    let id = import(&e, &p);
    for bad in ["EPSG:99999999", "nonsense", ""] {
        assert_eq!(run(&e, "warp", json!({"crs": bad, "resampling": "near"}), &[&id]).status, Status::Error, "{bad:?}");
    }
}

#[test]
fn raster_helpers_report_bad_gdal_options_instead_of_crashing() {
    // GDAL's *OptionsNew returns NULL for a bad option; the helpers must not use it.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 10, None, Some(32614), |_, c| c as f32);
    let ds = gdal::Dataset::open(&p).unwrap();
    let bad = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(engine::raster::warp(&ds, "", &bad(&["-of", "MEM", "-t_srs", "EPSG:99999999"])).is_err());
    assert!(engine::raster::translate(&ds, &dir.path().join("t.tif"), &bad(&["-of", "GTiff", "-projwin_srs", "EPSG:99999999", "-projwin", "0", "1", "1", "0", "-notanoption"])).is_err());
    assert!(engine::raster::dem(&ds, &dir.path().join("d.tif"), "slope", &bad(&["-notanoption"])).is_err());
}

#[test]
fn polygonize_sampling_and_zonal_statistics_handle_nodata() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // Left half 1, right half 2; a 3×3 NoData hole in the right half.
    let p = dir.path().join("classes.tif");
    dem(&p, 20, Some(-9999.0), Some(32614), |r, c| if (8..11).contains(&r) && (14..17).contains(&c) { -9999.0 } else if c < 10 { 1.0 } else { 2.0 });
    let id = import(&e, &p);
    let poly = ok(&e, "polygonize", json!({}), &[&id]);
    assert_eq!(poly.rows, Some(2), "two classes → two polygons; the NoData hole is a hole, not a class");
    assert_eq!(poly.crs.as_deref(), Some("EPSG:32614"));

    let pts = dir.path().join("pts.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, ST_Point({a}, {b}) AS geom UNION ALL SELECT 2, ST_Point({c}, {b}) UNION ALL SELECT 3, ST_Point({d}, {b}) UNION ALL SELECT 4, ST_Point({e}, {b}))
         TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:32614', USE_TMP_FILE false)",
        pts.display(),
        a = X0 + 45.0,  // col 1, class 1
        c = X0 + 585.0, // col 19, class 2
        d = X0 + 465.0, // col 15 → inside the NoData hole (rows 8–10)
        e = X0 + 99999.0, // far outside the raster
        b = Y0 - 285.0  // row 9
    ))
    .unwrap();
    let pid = import(&e, &pts);
    let s = ok(&e, "sample_raster", json!({"field": "V"}), &[&pid, &id]);
    assert_eq!(s.rows, Some(4), "no point is dropped");
    let get = |fid: i64| -> Option<f64> {
        e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT V FROM \"{}\" WHERE id = {fid}", s.output.as_ref().unwrap()), [], |r| r.get::<_, Option<f64>>(0))?)).unwrap()
    };
    assert_eq!(get(1), Some(1.0));
    assert_eq!(get(2), Some(2.0));
    assert_eq!(get(3), None, "a point on NoData samples as NULL, not as -9999");
    assert_eq!(get(4), None, "a point outside the raster samples as NULL");

    // Zonal statistics: one zone over the whole raster, one inside the NoData hole.
    let z = dir.path().join("zones.gpkg");
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, ST_MakeEnvelope({}, {}, {}, {}) AS geom UNION ALL SELECT 2, ST_MakeEnvelope({}, {}, {}, {}))
         TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:32614', USE_TMP_FILE false)",
        X0, Y0 - 600.0, X0 + 600.0, Y0,
        X0 + 435.0, Y0 - 315.0, X0 + 495.0, Y0 - 255.0,
        z.display()
    ))
    .unwrap();
    let zid = import(&e, &z);
    let zr = ok(&e, "zonal_statistics", json!({"prefix": "Z_"}), &[&zid, &id]);
    assert_eq!(zr.rows, Some(2), "every zone keeps its row");
    let layer = e.preview_output(zr.output.as_ref().unwrap(), zr.crs.clone(), "z").unwrap();
    let names: Vec<_> = layer.fields.iter().map(|f| f.name.clone()).collect();
    assert!(names.iter().any(|n| n.starts_with("Z_")), "{names:?}");
    let mean = |zone: i64| -> Option<f64> {
        let col = names.iter().find(|n| n.to_lowercase().contains("mean")).unwrap();
        e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT \"{col}\" FROM \"{}\" WHERE id = {zone}", zr.output.as_ref().unwrap()), [], |r| r.get::<_, Option<f64>>(0))?)).unwrap()
    };
    // 400 pixels: 200 of class 1 and 191 of class 2 (9 NoData) → mean ignores NoData.
    let m = mean(1).unwrap();
    close(m, (200.0 * 1.0 + 191.0 * 2.0) / 391.0, 0.01);
    assert!(mean(2).map_or(true, |v| v.is_nan()), "a zone that only covers NoData has no mean (NULL/NaN), not -9999: {:?}", mean(2));
}

#[test]
fn warping_a_raster_of_unknown_crs_is_refused_not_guessed() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("nocrs.tif");
    dem(&p, 10, None, None, |_, c| c as f32);
    let l = e.import(&p).unwrap().remove(0);
    assert!(l.crs.is_none());
    let w = run(&e, "warp", json!({"crs": "EPSG:4326", "resampling": "near"}), &[&l.id]);
    assert_eq!(w.status, Status::Error, "reprojecting a raster of unknown CRS must not guess");
    assert!(w.error.is_some_and(|m| !m.is_empty()));
}

#[test]
fn raster_tools_that_compare_with_vectors_refuse_a_raster_of_unknown_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("nocrs.tif");
    dem(&p, 10, None, None, |_, c| c as f32);
    let r = import(&e, &p);
    let pts = e.generate_test_layer("points", 5, [-98.0, 35.0, -97.0, 36.0]).unwrap().id;
    let polys = e.generate_test_layer("polygons", 3, [-98.0, 35.0, -97.0, 36.0]).unwrap().id;
    for (tool, params, inputs) in [
        ("clip_raster", json!({}), vec![&r, &pts]),
        ("sample_raster", json!({"field": "V"}), vec![&pts, &r]),
        ("zonal_statistics", json!({"prefix": "Z_"}), vec![&polys, &r]),
    ] {
        let res = run(&e, tool, params, &inputs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        assert_eq!(res.status, Status::Error, "{tool}");
        assert!(res.error.is_some_and(|m| m.contains("coordinate system")), "{tool}");
    }
}

#[test]
fn a_vector_layer_in_a_raster_port_fails_with_a_reason() {
    let e = Engine::new().unwrap();
    let v = e.generate_test_layer("points", 5, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let r = run(&e, "slope", json!({"units": "degrees"}), &[&v.id]);
    assert_eq!(r.status, Status::Error);
    assert!(r.error.is_some_and(|m| !m.is_empty()));
}

#[test]
fn identify_far_from_a_projected_raster_is_just_empty() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 10, None, Some(32614), |_, c| c as f32);
    let l = e.import(&p).unwrap().remove(0);
    assert_eq!(e.raster_value(&l.id, 0.0, 0.0).unwrap(), None);
    assert_eq!(e.raster_value(&l.id, 100.0, -80.0).unwrap(), None);
}

#[test]
fn raster_layers_report_stats_and_draw_tiles() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 40, Some(-9999.0), Some(32614), |r, c| if r == 0 && c == 0 { -9999.0 } else { c as f32 });
    let l = e.import(&p).unwrap().remove(0);
    let info = l.raster.unwrap();
    assert_eq!((info.width, info.height, info.bands), (40, 40, 1));
    assert_eq!(info.nodata, Some(-9999.0));
    close(info.min, 0.0, 1e-6);
    close(info.max, 39.0, 1e-6);
    close(info.pixel_size, 30.0, 1e-9);
    assert_eq!(info.kind, "elevation");
    // Sampling at the NoData pixel is None; elsewhere the value.
    let (lon, lat) = e.transform_point(X0 + 15.0, Y0 - 15.0, "EPSG:32614", "EPSG:4326").unwrap();
    assert_eq!(e.raster_value(&l.id, lon, lat).unwrap(), None);
    let (lon, lat) = e.transform_point(X0 + 15.0 + 30.0 * 7.0, Y0 - 15.0, "EPSG:32614", "EPSG:4326").unwrap();
    close(e.raster_value(&l.id, lon, lat).unwrap().unwrap(), 7.0, 1e-4);
    let (lon, lat) = e.transform_point(X0 + 5000.0, Y0 - 15.0, "EPSG:32614", "EPSG:4326").unwrap();
    assert_eq!(e.raster_value(&l.id, lon, lat).unwrap(), None, "outside the raster");
    // Importing a non-raster as a raster is an error, not a bogus layer.
    let junk = dir.path().join("junk.tif");
    std::fs::write(&junk, b"not a tiff").unwrap();
    assert!(e.import(&junk).is_err());
}

#[test]
fn lw9_topo_data_step_makes_contours_with_index_lines_and_a_hillshade_from_one_dem() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("ramp.tif");
    dem(&p, 40, None, Some(32614), |_, c| c as f32); // 0…39 m west to east
    let id = import(&e, &p);
    let made = e.topo_prepare_data(&id, 10.0, 2, "Test sheet").unwrap();
    // Contours: a line layer at 10, 20 and 30 m with ELEV and INDEX, named for the sheet.
    assert_eq!(made.contours.name, "Contours — Test sheet");
    assert_eq!(made.contours.geom_type, "Line");
    assert_eq!(made.contours.feature_count, 3, "{:?}", made.contours);
    for f in ["elev", "index"] {
        assert!(made.contours.fields.iter().any(|x| x.name.eq_ignore_ascii_case(f)), "{f} in {:?}", made.contours.fields);
    }
    // Hillshade: a raster layer in the same coordinate system.
    assert_eq!(made.hillshade.name, "Hillshade — Test sheet");
    assert!(made.hillshade.raster.is_some());
    assert_eq!(made.hillshade.crs.as_deref(), Some("EPSG:32614"));
    // Both are in the project.
    let all: Vec<String> = e.list_layers().into_iter().map(|l| l.id).collect();
    assert!(all.contains(&made.contours.id) && all.contains(&made.hillshade.id));
    // Refusals in words: a vector layer is not a DEM; the interval must be positive.
    let err = e.topo_prepare_data(&made.contours.id, 10.0, 5, "x").unwrap_err().to_string();
    assert!(err.contains("not an elevation raster"), "{err}");
    assert!(e.topo_prepare_data(&id, 0.0, 5, "x").unwrap_err().to_string().contains("greater than zero"));
}
