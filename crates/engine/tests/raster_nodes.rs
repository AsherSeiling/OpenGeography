//! The raster foundation tools as model nodes (phases E1, E2): parameters, ports, CRS carried through, readable errors in the node.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

const X0: f64 = 600_000.0;
const Y0: f64 = 3_900_000.0;

fn dem(path: &Path, size: usize, x0: f64, cell: f64, epsg: u32, nodata: Option<f64>, f: impl Fn(usize, usize) -> f32) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, size, size, 1).unwrap();
    ds.set_geo_transform(&[x0, cell, 0.0, Y0, 0.0, -cell]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
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

fn size(path: &str) -> (usize, usize) {
    gdal::Dataset::open(path).unwrap().raster_size()
}

fn run(e: &Engine, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap_or_else(|| panic!("no tool {tool}"));
    let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
    let mut edges = vec![];
    for (i, l) in inputs.iter().enumerate() {
        nodes.push(json!({"id": format!("in{i}"), "tool": "input_layer", "params": {"layer": l}}));
        edges.push(json!({"source": format!("in{i}"), "target": "t", "targetHandle": spec.inputs[i].id}));
    }
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn ok(e: &Engine, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let r = run(e, tool, params, inputs);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{tool}: {:?} {:?}", r.status, r.error);
    r
}

fn import(e: &Engine, p: &Path) -> String {
    e.import(p).unwrap().remove(0).id
}

#[test]
fn e1_nodes_calculate_reclassify_fill_and_select() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let a = dir.path().join("a.tif");
    let b = dir.path().join("b.tif");
    dem(&a, 20, X0, 30.0, 32614, Some(-9999.0), |r, c| if r == 5 && c == 5 { -9999.0 } else { (r * 100 + c) as f32 });
    dem(&b, 20, X0, 30.0, 32614, None, |_, _| 2.0);
    let (ia, ib) = (import(&e, &a), import(&e, &b));

    let feet = ok(&e, "raster_calc", json!({"expr": "A * 3.28084"}), &[&ia]);
    assert_eq!(feet.crs.as_deref(), Some("EPSG:32614"));
    let out = feet.output.as_ref().unwrap();
    assert!((pixel(out, 3, 2) as f64 - 203.0 * 3.28084).abs() < 1e-2);
    assert_eq!(pixel(out, 5, 5), engine::raster_ops::OUT_NODATA as f32);

    let two = ok(&e, "raster_calc2", json!({"expr": "A / B"}), &[&ia, &ib]);
    assert_eq!(pixel(two.output.as_ref().unwrap(), 4, 4), 202.0);

    // A bad expression is the node's error, in words.
    let bad = run(&e, "raster_calc", json!({"expr": "A +"}), &[&ia]);
    assert!(matches!(bad.status, Status::Error) && bad.error.as_deref().unwrap_or("").contains("is not valid"), "{:?}", bad.error);

    let rc = ok(&e, "reclassify", json!({"band": 1, "rules": "0 to 100 = 1\n100 to 1000 = 2\nnodata = 0\nelse = nodata"}), &[&ia]);
    let ro = rc.output.as_ref().unwrap();
    assert_eq!((pixel(ro, 3, 0), pixel(ro, 3, 2), pixel(ro, 5, 5)), (1.0, 2.0, 0.0));
    let bad = run(&e, "reclassify", json!({"band": 1, "rules": "0 to 10"}), &[&ia]);
    assert!(bad.error.as_deref().unwrap_or("").contains("equals sign"), "{:?}", bad.error);

    let filled = ok(&e, "nodata", json!({"mode": "fill", "distance": 5, "smoothing": 0}), &[&ia]);
    let v = pixel(filled.output.as_ref().unwrap(), 5, 5);
    assert!((v - 505.0).abs() < 30.0, "the hole is filled from its neighbours (inverse-distance interpolation is not exact on a slope): {v}");
    let declared = ok(&e, "nodata", json!({"mode": "declare", "value": 0}), &[&ib]);
    assert!(declared.output.is_some());
    let replaced = ok(&e, "nodata", json!({"mode": "replace", "value": 0}), &[&ia]);
    assert_eq!(pixel(replaced.output.as_ref().unwrap(), 5, 5), 0.0);

    let sel = ok(&e, "select_bands", json!({"bands": "1"}), &[&ia]);
    assert_eq!(size(sel.output.as_ref().unwrap()), (20, 20));
    assert!(run(&e, "select_bands", json!({"bands": "2"}), &[&ia]).error.unwrap_or_default().contains("band 2 does not exist"));
}

#[test]
fn e2_nodes_resample_align_mosaic() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let a = dir.path().join("a.tif");
    let right = dir.path().join("right.tif");
    let coarse = dir.path().join("coarse.tif");
    dem(&a, 20, X0, 30.0, 32614, Some(-9999.0), |_, _| 1.0);
    dem(&right, 20, X0 + 600.0, 30.0, 32614, Some(-9999.0), |_, _| 2.0);
    dem(&coarse, 10, X0, 60.0, 32614, None, |_, _| 5.0);
    let (ia, ir, ic) = (import(&e, &a), import(&e, &right), import(&e, &coarse));

    let rs = ok(&e, "resample_raster", json!({"cell": 15, "resampling": "near"}), &[&ia]);
    assert_eq!(size(rs.output.as_ref().unwrap()), (40, 40));
    assert_eq!(rs.crs.as_deref(), Some("EPSG:32614"));
    assert!(run(&e, "resample_raster", json!({"cell": 0, "resampling": "near"}), &[&ia]).error.unwrap_or_default().contains("greater than 0"));

    let al = ok(&e, "align_raster", json!({"resampling": "near"}), &[&ic, &ia]);
    assert_eq!(size(al.output.as_ref().unwrap()), (20, 20), "the coarse raster is on the 30 m grid of the reference");
    assert_eq!(pixel(al.output.as_ref().unwrap(), 4, 4), 5.0);

    let m = ok(&e, "mosaic_rasters", json!({"resampling": "near"}), &[&ia, &ir]);
    let mo = m.output.as_ref().unwrap();
    assert_eq!(size(mo), (40, 20));
    assert_eq!((pixel(mo, 0, 0), pixel(mo, 39, 19)), (1.0, 2.0));
}

#[test]
fn e2_rasterize_node_burns_a_field_in_the_layers_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.path().join("z.gpkg");
    c.execute_batch(&format!(
        "COPY (SELECT * FROM (VALUES (1, 4.5, ST_GeomFromText('POLYGON((600000 3899000, 600100 3899000, 600100 3899100, 600000 3899100, 600000 3899000))')),
                                     (2, 9.5, ST_GeomFromText('POLYGON((600100 3899000, 600200 3899000, 600200 3899100, 600100 3899100, 600100 3899000))'))) t(id, val, geom))
         TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'z', SRS 'EPSG:32614', USE_TMP_FILE false)",
        gpkg.display()
    ))
    .unwrap();
    let id = import(&e, &gpkg);
    let r = ok(&e, "rasterize", json!({"field": "val", "cell": 20}), &[&id]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let out = r.output.as_ref().unwrap();
    assert_eq!(size(out), (10, 5));
    assert_eq!((pixel(out, 1, 1), pixel(out, 8, 1)), (4.5, 9.5));
    let ones = ok(&e, "rasterize", json!({"cell": 50}), &[&id]);
    assert_eq!(pixel(ones.output.as_ref().unwrap(), 0, 0), 1.0);
}

fn points_layer(e: &Engine, dir: &Path, rows: &[(f64, f64, f64)]) -> String {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.join("pts.gpkg");
    let values: Vec<String> = rows.iter().map(|(x, y, v)| format!("({v}, ST_GeomFromText('POINT({x} {y})'))")).collect();
    c.execute_batch(&format!("COPY (SELECT * FROM (VALUES {}) t(val, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'p', SRS 'EPSG:32614', USE_TMP_FILE false)", values.join(","), gpkg.display())).unwrap();
    import(e, &gpkg)
}

#[test]
fn h1_interpolate_recovers_a_plane_and_keeps_the_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // z = x/100 + 2*y/100 on a 5 x 5 lattice of points 100 m apart.
    let mut rows = vec![];
    for i in 0..5 {
        for j in 0..5 {
            let (x, y) = (600_000.0 + i as f64 * 100.0, 3_900_000.0 + j as f64 * 100.0);
            rows.push((x, y, (x - 600_000.0) / 100.0 + 2.0 * (y - 3_900_000.0) / 100.0));
        }
    }
    let id = points_layer(&e, dir.path(), &rows);
    let r = ok(&e, "interpolate", json!({"field": "val", "method": "linear", "cell": 20}), &[&id]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let out = r.output.as_ref().unwrap();
    let (cols, rws) = size(out);
    assert_eq!((cols, rws), (20, 20));
    // Cell (10, 10) from the top: centre x = 600_210, y = 3_900_190 → z = 2.1 + 3.8 = 5.9 (a plane is exact for a TIN).
    assert!((pixel(out, 10, 10) - 5.9).abs() < 1e-3, "{}", pixel(out, 10, 10));
    let idw = ok(&e, "interpolate", json!({"field": "val", "method": "idw", "cell": 50, "power": 2}), &[&id]);
    let o = idw.output.as_ref().unwrap();
    assert!(pixel(o, 0, 0).is_finite() && pixel(o, 0, 0) > -1e30);
}

#[test]
fn h1_interpolate_reports_readable_errors() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = points_layer(&e, dir.path(), &[(600_000.0, 3_900_000.0, 1.0), (600_100.0, 3_900_000.0, 2.0)]);
    let r = run(&e, "interpolate", json!({"field": "val", "method": "linear", "cell": 10}), &[&id]);
    assert!(r.error.as_deref().unwrap_or("").contains("at least 3 points"), "{:?}", r.error);
    let r = run(&e, "interpolate", json!({"field": "val", "method": "average", "cell": 10, "radius": 0}), &[&id]);
    assert!(r.error.as_deref().unwrap_or("").contains("search radius"), "{:?}", r.error);
    let r = run(&e, "interpolate", json!({"field": "val", "method": "idw", "cell": 0}), &[&id]);
    assert!(r.error.as_deref().unwrap_or("").contains("cell size"), "{:?}", r.error);
}

#[test]
fn h5_spectral_index_and_classification_nodes_keep_grid_and_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // 20 × 20, two bands: red (1) low on the left, high on the right; near infrared (2) the opposite.
    let p = dir.path().join("img.tif");
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(&p, 20, 20, 2).unwrap();
    ds.set_geo_transform(&[X0, 30.0, 0.0, Y0, 0.0, -30.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    for (b, (lo, hi)) in [(0.05f32, 0.4f32), (0.5, 0.1)].into_iter().enumerate() {
        let data: Vec<f32> = (0..400).map(|i| if i % 20 < 10 { lo } else { hi }).collect();
        ds.rasterband(b + 1).unwrap().write((0, 0), (20, 20), &mut gdal::raster::Buffer::new((20, 20), data)).unwrap();
    }
    drop(ds);
    let id = import(&e, &p);
    let r = ok(&e, "spectral_index", json!({"index": "ndvi", "band_a": 2, "band_b": 1}), &[&id]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let out = r.output.as_ref().unwrap();
    assert_eq!(size(out), (20, 20));
    assert!((pixel(out, 2, 2) - (0.5 - 0.05) / (0.5 + 0.05)).abs() < 1e-5);
    assert!(pixel(out, 15, 2) < 0.0);
    let c = ok(&e, "classify_kmeans", json!({"classes": 2, "sample": 1000}), &[&id]);
    let co = c.output.as_ref().unwrap();
    assert_eq!(size(co), (20, 20));
    assert_ne!(pixel(co, 2, 2), pixel(co, 15, 2));
    assert_eq!(c.crs.as_deref(), Some("EPSG:32614"));
    let bad = run(&e, "spectral_index", json!({"index": "ndvi", "band_a": 1, "band_b": 1}), &[&id]);
    assert!(bad.error.as_deref().unwrap_or("").contains("different"), "{:?}", bad.error);
    let bad = run(&e, "classify_kmeans", json!({"classes": 1}), &[&id]);
    assert!(bad.error.as_deref().unwrap_or("").contains("between 2 and 20"), "{:?}", bad.error);
}

#[test]
fn c1_c2_vertical_datums_are_recorded_inherited_and_mixing_is_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let (a, b) = (dir.path().join("a.tif"), dir.path().join("b.tif"));
    dem(&a, 10, X0, 30.0, 32614, None, |r, c| (r + c) as f32);
    dem(&b, 10, X0, 30.0, 32614, None, |r, c| (r * c) as f32);
    let (ia, ib) = (import(&e, &a), import(&e, &b));

    // Define: a copy that carries the datum; the values and the grid are unchanged.
    let da = ok(&e, "define_vertical_datum", json!({"datum": "NAVD88"}), &[&ia]);
    let out_a = da.output.as_ref().unwrap();
    assert_eq!(engine::vertical::get(Path::new(out_a)).as_deref(), Some("NAVD88"));
    assert_eq!((pixel(out_a, 3, 4), size(out_a)), (7.0, (10, 10)));
    assert_eq!(da.crs.as_deref(), Some("EPSG:32614"));

    // Combining NAVD88 (defined above) with EGM2008 (recorded in the second file) warns, and the result carries no datum.
    engine::vertical::set(Path::new(&b.display().to_string()), "EGM2008").unwrap();
    let imported_b = import(&e, &b);
    // Two layers are needed on the graph: NAVD88 copy and EGM2008 file.
    let g: engine::graph::Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "i1", "tool": "input_layer", "params": {"layer": ia}},
            {"id": "d1", "tool": "define_vertical_datum", "params": {"datum": "NAVD88"}},
            {"id": "i2", "tool": "input_layer", "params": {"layer": imported_b}},
            {"id": "c", "tool": "raster_calc2", "params": {"expr": "A - B"}}
        ],
        "edges": [
            {"source": "i1", "target": "d1", "targetHandle": "in"},
            {"source": "d1", "target": "c", "targetHandle": "a"},
            {"source": "i2", "target": "c", "targetHandle": "b"}
        ]
    })).unwrap();
    let r = e.run_graph(&g, Some("c"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "c").unwrap();
    assert!(matches!(r.status, engine::graph::Status::Done | engine::graph::Status::Cached), "{:?}", r.error);
    let msg = r.message.unwrap_or_default();
    assert!(msg.contains("Warning, the Raster Calculator: the rasters use different vertical datums (EGM2008 and NAVD88)"), "{msg}");
    assert_eq!(engine::vertical::get(Path::new(r.output.as_ref().unwrap())), None, "different datums are not inherited");
}
