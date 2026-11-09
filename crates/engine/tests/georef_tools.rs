//! Georeferencing as Model nodes (GR4): the control point set is chosen by name, a missing set is a readable error in the node,
//! editing a set re-runs the node, and the results match the windows' (docs/TEST_PLAN.md §Georeferencing).

use engine::georef::{ControlPoint, ControlSet};
use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use gdal::raster::Buffer;
use serde_json::{json, Value};
use std::path::Path;

const UTM: &str = "EPSG:32614";

fn pt(id: u32, pixel: f64, line: f64, x: f64, y: f64) -> ControlPoint {
    ControlPoint { id, pixel, line, x, y, enabled: true }
}

fn set(name: &str, crs: &str, points: Vec<ControlPoint>) -> ControlSet {
    ControlSet { id: format!("id-{name}"), name: name.into(), target_crs: crs.into(), image: None, points }
}

/// Three corners shifted by (dx, dy): an affine pure translation.
fn shift(name: &str, dx: f64, dy: f64) -> ControlSet {
    let f = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0), (600_200.0, 3_900_200.0)];
    set(name, UTM, f.iter().enumerate().map(|(i, p)| pt(i as u32 + 1, p.0, p.1, p.0 + dx, p.1 + dy)).collect())
}

fn run(e: &Engine, tool: &str, params: Value, input: &str) -> NodeResult {
    let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap_or_else(|| panic!("no tool {tool}"));
    let nodes = vec![json!({"id": "t", "tool": tool, "params": params}), json!({"id": "in0", "tool": "input_layer", "params": {"layer": input}})];
    let edges = vec![json!({"source": "in0", "target": "t", "targetHandle": spec.inputs[0].id})];
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn vector_fixture() -> (Engine, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("in.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, 'a' AS name, ST_MakeEnvelope(600000, 3900000, 600100, 3900100) AS geom UNION ALL SELECT 2, 'b', ST_MakeEnvelope(600100, 3900000, 600200, 3900100) UNION ALL SELECT 3, 'c', ST_MakeEnvelope(600000, 3900100, 600100, 3900200)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{UTM}', USE_TMP_FILE false)",
        f.display()
    ))
    .unwrap();
    let e = Engine::new().unwrap();
    let id = e.import(&f).unwrap().remove(0).id;
    (e, dir, id)
}

fn first_corner(e: &Engine, r: &NodeResult) -> (f64, f64, String) {
    let t = format!("\"{}\"", r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT min(ST_XMin(geom)), min(ST_YMin(geom)), string_agg(name, ',' ORDER BY id) FROM {t}"), [], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?)))?)).unwrap()
}

#[test]
fn adjust_node_moves_the_shapes_with_the_named_set_and_keeps_attributes_and_crs() {
    let (e, _d, layer) = vector_fixture();
    e.save_control_set(&shift("My Shift", 10.0, 20.0)).unwrap();
    // The name is matched ignoring case.
    let r = run(&e, "adjust_shapes", json!({"set": "my shift", "method": "affine"}), &layer);
    assert!(matches!(r.status, Status::Done), "{:?} {:?}", r.status, r.error);
    assert_eq!(r.crs.as_deref(), Some(UTM));
    assert_eq!(r.rows, Some(3));
    let (x, y, names) = first_corner(&e, &r);
    assert!((x - 600_010.0).abs() < 1e-6 && (y - 3_900_020.0).abs() < 1e-6, "({x}, {y})");
    assert_eq!(names, "a,b,c");
    // The scratch tables are gone.
    let left: i64 = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM duckdb_tables() WHERE table_name LIKE '__adjn_%'", [], |row| row.get(0))?)).unwrap();
    assert_eq!(left, 0);
}

#[test]
fn editing_the_set_runs_the_node_again_instead_of_returning_the_old_result() {
    let (e, _d, layer) = vector_fixture();
    e.save_control_set(&shift("S", 10.0, 0.0)).unwrap();
    let a = run(&e, "adjust_shapes", json!({"set": "S"}), &layer);
    assert!((first_corner(&e, &a).0 - 600_010.0).abs() < 1e-6);
    let again = run(&e, "adjust_shapes", json!({"set": "S"}), &layer);
    assert!(matches!(again.status, Status::Cached), "an unchanged set is cached: {:?}", again.status);
    e.save_control_set(&shift("S", 50.0, 0.0)).unwrap();
    let b = run(&e, "adjust_shapes", json!({"set": "S"}), &layer);
    assert!(matches!(b.status, Status::Done), "{:?}", b.status);
    assert!((first_corner(&e, &b).0 - 600_050.0).abs() < 1e-6);
}

#[test]
fn a_missing_or_unusable_set_is_a_readable_error_in_the_node() {
    let (e, _d, layer) = vector_fixture();
    // No sets at all.
    let r = run(&e, "adjust_shapes", json!({"set": "Nope"}), &layer);
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.as_deref().unwrap().contains("no control point set named 'Nope'") && r.error.as_deref().unwrap().contains("none yet"), "{:?}", r.error);
    // Another set exists: it is named.
    e.save_control_set(&shift("Real", 1.0, 1.0)).unwrap();
    let r = run(&e, "adjust_shapes", json!({"set": "Nope"}), &layer);
    assert!(r.error.as_deref().unwrap().contains("Sets in this project: Real"), "{:?}", r.error);
    // Nothing typed.
    let r = run(&e, "adjust_shapes", json!({"set": ""}), &layer);
    assert!(r.error.as_deref().unwrap().to_lowercase().contains("control point set"), "{:?}", r.error);
    // A set with too few points for the method (similarity needs 2).
    let mut small = shift("Small", 1.0, 1.0);
    small.points.truncate(1);
    e.save_control_set(&small).unwrap_or(());
    let r = run(&e, "adjust_shapes", json!({"set": "Small", "method": "similarity"}), &layer);
    assert!(matches!(r.status, Status::Error), "{:?}", r.status);
    // A bad method is refused by the settings check.
    let r = run(&e, "adjust_shapes", json!({"set": "Real", "method": "magic"}), &layer);
    assert!(matches!(r.status, Status::Error));
}

#[test]
fn the_reproject_tool_is_no_longer_found_by_georeference_and_the_new_tools_are() {
    let (e, _d, _l) = vector_fixture();
    let specs = e.tool_specs();
    let words = |id: &str| specs.iter().find(|s| s.id == id).unwrap().keywords.to_vec();
    assert!(!words("warp").contains(&"georeference"));
    assert!(words("georef_image").contains(&"georeference"));
    assert!(words("adjust_shapes").contains(&"rubber sheet"));
}

fn write_dem(path: &Path, size: usize) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<u8, _>(path, size, size, 1).unwrap();
    ds.set_geo_transform(&[0.0, 1.0, 0.0, 0.0, 0.0, -1.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    let data: Vec<u8> = (0..size * size).map(|i| ((i % size) % 250 + 1) as u8).collect();
    ds.rasterband(1).unwrap().write((0, 0), (size, size), &mut Buffer::new((size, size), data)).unwrap();
}

#[test]
fn georef_image_node_warps_with_the_named_set() {
    let dir = tempfile::tempdir().unwrap();
    let img = dir.path().join("scan.tif");
    write_dem(&img, 100);
    let e = Engine::new().unwrap();
    let layer = e.import(&img).unwrap().remove(0).id;
    // 100 x 100 pixels become 5 m cells with the origin at (600000, 3900500).
    let pts = [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)];
    e.save_control_set(&set("Scan", UTM, pts.iter().enumerate().map(|(i, p)| pt(i as u32 + 1, p.0, p.1, 600_000.0 + 5.0 * p.0, 3_900_500.0 - 5.0 * p.1)).collect())).unwrap();
    let r = run(&e, "georef_image", json!({"set": "Scan", "method": "affine", "resampling": "nearest"}), &layer);
    assert!(matches!(r.status, Status::Done), "{:?} {:?}", r.status, r.error);
    assert_eq!(r.crs.as_deref(), Some(UTM));
    let out = gdal::Dataset::open(r.output.as_ref().unwrap()).unwrap();
    let gt = out.geo_transform().unwrap();
    assert!((gt[1] - 5.0).abs() < 1e-6 && (gt[0] - 600_000.0).abs() < 1e-6 && (gt[3] - 3_900_500.0).abs() < 1e-6, "{gt:?}");
    // A missing set is the node's error.
    let bad = run(&e, "georef_image", json!({"set": "Missing"}), &layer);
    assert!(matches!(bad.status, Status::Error) && bad.error.as_deref().unwrap().contains("no control point set named 'Missing'"), "{:?}", bad.error);
}
