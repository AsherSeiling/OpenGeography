//! Hydrology nodes (phase H4), run by the WhiteboxTools program. They skip, saying so, when the program is not installed
//! (set GIS_STUDIO_WHITEBOX to a whitebox_tools build, or use the packaged app's resources).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

const X0: f64 = 600_000.0;
const Y0: f64 = 3_900_300.0;
const N: usize = 30;

fn have() -> bool {
    let ok = engine::whitebox::available();
    if !ok {
        eprintln!("SKIPPED: WhiteboxTools is not installed (set GIS_STUDIO_WHITEBOX)");
    }
    ok
}

/// A 30 × 30 plane falling 0.5 per cell to the east and 0.2 per cell to the south, with a 3 × 3 pit at columns/rows 10–12.
fn dem(path: &Path, epsg: u32, pit: bool) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, N, N, 1).unwrap();
    ds.set_geo_transform(&[X0, 10.0, 0.0, Y0, 0.0, -10.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    let mut band = ds.rasterband(1).unwrap();
    band.set_no_data_value(Some(-9999.0)).unwrap();
    let data: Vec<f32> = (0..N * N)
        .map(|i| {
            let (r, c) = (i / N, i % N);
            let z = 100.0 - 0.5 * c as f32 - 0.2 * r as f32;
            if pit && (10..13).contains(&r) && (10..13).contains(&c) { z - 8.0 } else { z }
        })
        .collect();
    band.write((0, 0), (N, N), &mut gdal::raster::Buffer::new((N, N), data)).unwrap();
}

fn px(path: &str, col: isize, row: isize) -> f32 {
    gdal::Dataset::open(path).unwrap().rasterband(1).unwrap().read_as::<f32>((col, row), (1, 1), (1, 1), None).unwrap().data()[0]
}

fn import(e: &Engine, p: &Path) -> String {
    e.import(p).unwrap().remove(0).id
}

fn run(e: &Engine, nodes: Vec<Value>, edges: Vec<Value>, last: &str) -> NodeResult {
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    e.run_graph(&g, Some(last), &mut |_| {}).unwrap().into_iter().find(|r| r.id == last).unwrap()
}

/// input → chain of tools (each takes the previous output on port "in"); returns every step's result.
fn chain(e: &Engine, layer: &str, steps: &[(&str, Value)]) -> Vec<NodeResult> {
    let mut nodes = vec![json!({"id": "in0", "tool": "input_layer", "params": {"layer": layer}})];
    let mut edges = vec![];
    let mut prev = "in0".to_string();
    for (i, (tool, params)) in steps.iter().enumerate() {
        let id = format!("s{i}");
        nodes.push(json!({"id": id, "tool": tool, "params": params}));
        edges.push(json!({"source": prev, "target": id, "targetHandle": "in"}));
        prev = id;
    }
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    let all = e.run_graph(&g, Some(&prev), &mut |_| {}).unwrap();
    (0..steps.len()).map(|i| all.iter().find(|r| r.id == format!("s{i}")).unwrap().clone()).collect()
}

fn done(r: &NodeResult) {
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?} {:?}", r.status, r.error);
}

#[test]
fn h4_fill_depressions_raises_only_the_pit_and_keeps_the_crs() {
    if !have() { return; }
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("dem.tif");
    dem(&p, 32614, true);
    let id = import(&e, &p);
    let r = &chain(&e, &id, &[("fill_depressions", json!({"fix_flats": true}))])[0];
    done(r);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let out = r.output.as_ref().unwrap();
    let f = out;
    // Away from the pit the surface is unchanged; the pit centre (col 11, row 11) was 8 below the plane and is now at least its lowest neighbour on the rim.
    assert!((px(f, 25, 25) - (100.0 - 12.5 - 5.0)).abs() < 1e-3);
    let plane = 100.0 - 0.5 * 11.0 - 0.2 * 11.0;
    assert!(px(f, 11, 11) > plane - 8.0 + 1.0, "pit not filled: {}", px(f, 11, 11));
}

#[test]
fn h4_flow_direction_accumulation_and_streams_follow_the_slope() {
    if !have() { return; }
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("dem.tif");
    dem(&p, 32614, false);
    let id = import(&e, &p);
    let dirs = chain(&e, &id, &[("flow_direction", json!({}))]);
    done(&dirs[0]);
    let f = dirs[0].output.as_ref().unwrap();
    // Steepest descent on this plane is due east (0.5 per cell beats the diagonal's 0.7 / 1.414 = 0.495): ESRI code 1.
    assert_eq!(px(f, 5, 5), 1.0);
    assert_eq!(px(f, 20, 15), 1.0);
    let acc = chain(&e, &id, &[("flow_accumulation", json!({"out_type": "cells"})), ("extract_streams", json!({"threshold": 20}))]);
    done(&acc[0]);
    done(&acc[1]);
    let a = acc[0].output.as_ref().unwrap();
    // Each row drains east on its own: the cell in column c has c + 1 cells upstream, itself included.
    let v = px(a, 24, 5);
    assert!(v == 25.0 || v == 24.0, "accumulation at column 24: {v}");
    let s = acc[1].output.as_ref().unwrap();
    assert_eq!(px(s, 28, 5), 1.0);
    // WhiteboxTools leaves non-stream cells as no data.
    assert!(px(s, 3, 5) != 1.0);
    assert_eq!(acc[1].crs.as_deref(), Some("EPSG:32614"));
}

#[test]
fn h4_watersheds_label_the_area_that_drains_to_each_outlet() {
    if !have() { return; }
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("dem.tif");
    dem(&p, 32614, false);
    let dem_id = import(&e, &p);
    // An outlet at the east end of row 5 (cell centre column 29, row 5).
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.path().join("outlet.gpkg");
    c.execute_batch(&format!("COPY (SELECT ST_GeomFromText('POINT({} {})') AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'o', SRS 'EPSG:32614', USE_TMP_FILE false)", X0 + 29.5 * 10.0, Y0 - 5.5 * 10.0, gpkg.display())).unwrap();
    let out_id = import(&e, &gpkg);
    let nodes = vec![
        json!({"id": "d", "tool": "input_layer", "params": {"layer": dem_id}}),
        json!({"id": "fd", "tool": "flow_direction", "params": {}}),
        json!({"id": "o", "tool": "input_layer", "params": {"layer": out_id}}),
        json!({"id": "w", "tool": "watersheds", "params": {}}),
    ];
    let edges = vec![
        json!({"source": "d", "target": "fd", "targetHandle": "in"}),
        json!({"source": "fd", "target": "w", "targetHandle": "pntr"}),
        json!({"source": "o", "target": "w", "targetHandle": "pour"}),
    ];
    let r = run(&e, nodes, edges, "w");
    done(&r);
    let w = r.output.as_ref().unwrap();
    assert_eq!(px(w, 0, 5), 1.0, "row 5 drains to the outlet");
    assert_eq!(px(w, 29, 5), 1.0);
    assert!(px(w, 10, 20) != 1.0, "another row does not");
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
}

#[test]
fn h4_refusals_are_readable() {
    if !have() { return; }
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("dem.tif");
    dem(&p, 32614, false);
    let id = import(&e, &p);
    let r = &chain(&e, &id, &[("extract_streams", json!({"threshold": 0}))])[0];
    assert!(r.error.as_deref().unwrap_or("").contains("greater than 0"), "{:?}", r.error);
}

#[test]
fn h4_fill_depressions_survives_repeated_runs() {
    if !have() { return; }
    // The program itself fails now and then (a thread race in v2.4.0); engine::whitebox::run retries that message.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("dem.tif");
    dem(&p, 32614, true);
    for k in 0..40 {
        let out = dir.path().join(format!("fill{k}.tif"));
        engine::whitebox::run("FillDepressions", &[("dem", p.display().to_string()), ("output", out.display().to_string()), ("fix_flats", "true".into())]).unwrap_or_else(|e| panic!("run {k}: {e}"));
        assert!(out.is_file());
    }
}
