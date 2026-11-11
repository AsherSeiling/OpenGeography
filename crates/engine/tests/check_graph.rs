//! Check before running (UX7): the CRS predicted before a run is the CRS the run produces, and mismatches and missing CRS are
//! reported without running anything.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;
use std::path::Path;

fn square(path: &Path, srs_ddl: &str, cx: f64, cy: f64, half: f64) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_MakeEnvelope({}, {}, {}, {}) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', {srs_ddl}USE_TMP_FILE false)", cx - half, cy - half, cx + half, cy + half, path.display())).unwrap();
}

fn pair(e: &Engine, dir: &Path) -> (engine::layers::LayerInfo, engine::layers::LayerInfo) {
    let (pa, pb) = (dir.join("a.gpkg"), dir.join("b.gpkg"));
    square(&pa, "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0);
    square(&pb, "SRS 'EPSG:4326', ", -93.0, 39.75, 0.002);
    (e.import(&pa).unwrap().remove(0), e.import(&pb).unwrap().remove(0))
}

#[test]
fn ux7_mismatch_is_reported_before_running_and_the_run_reprojects_as_said() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let (a, b) = pair(&e, dir.path());
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": a.id}},
            {"id": "b", "tool": "input_layer", "params": {"layer": b.id}},
            {"id": "t", "tool": "erase", "params": {}}
        ],
        "edges": [{"source": "a", "target": "t", "targetHandle": "in"}, {"source": "b", "target": "t", "targetHandle": "erase"}]
    }))
    .unwrap();
    let problems = e.check_graph(&g).unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].node, "t");
    assert_eq!(problems[0].problem.code, "crs-mismatch");
    let fix = problems[0].problem.fix.clone().unwrap();
    assert_eq!(fix.port, "erase");
    assert!(engine::crs::same(&fix.crs, a.crs.as_deref().unwrap()));
    let run = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let t = run.iter().find(|r| r.id == "t").unwrap();
    assert_eq!(t.status, Status::Done);
    assert!(t.reprojected, "the run does what the check said");
    assert!(engine::crs::same(t.crs.as_deref().unwrap(), a.crs.as_deref().unwrap()));
}

#[test]
fn ux7_the_predicted_crs_matches_the_real_one_through_project_and_first_input_tools() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let (a, b) = pair(&e, dir.path());
    // b -> Project to 26915 -> Erase's second input: no mismatch is reported, and the run does not reproject.
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": a.id}},
            {"id": "b", "tool": "input_layer", "params": {"layer": b.id}},
            {"id": "p", "tool": "project", "params": {"crs": "EPSG:26915"}},
            {"id": "t", "tool": "erase", "params": {}}
        ],
        "edges": [
            {"source": "b", "target": "p", "targetHandle": "in"},
            {"source": "a", "target": "t", "targetHandle": "in"},
            {"source": "p", "target": "t", "targetHandle": "erase"}
        ]
    }))
    .unwrap();
    assert!(e.check_graph(&g).unwrap().is_empty());
    let run = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert!(!run.iter().find(|r| r.id == "t").unwrap().reprojected);
    // A first-input tool (buffer) in front keeps the CRS the check predicted.
    let g2: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": a.id}},
            {"id": "buf", "tool": "buffer", "params": {"distance": 10}},
            {"id": "b", "tool": "input_layer", "params": {"layer": b.id}},
            {"id": "t", "tool": "erase", "params": {}}
        ],
        "edges": [
            {"source": "a", "target": "buf", "targetHandle": "in"},
            {"source": "buf", "target": "t", "targetHandle": "in"},
            {"source": "b", "target": "t", "targetHandle": "erase"}
        ]
    }))
    .unwrap();
    let probs = e.check_graph(&g2).unwrap();
    assert_eq!(probs.len(), 1, "{probs:?}");
    assert!(engine::crs::same(&probs[0].problem.fix.clone().unwrap().crs, a.crs.as_deref().unwrap()));
    let run = e.run_graph(&g2, None, &mut |_| {}).unwrap();
    let buf = run.iter().find(|r| r.id == "buf").unwrap();
    assert!(engine::crs::same(buf.crs.as_deref().unwrap(), a.crs.as_deref().unwrap()), "buffer keeps its input's CRS: {buf:?}");
}

#[test]
fn ux7_a_layer_without_a_crs_is_an_error_and_has_no_fix() {
    let dir = tempfile::tempdir().unwrap();
    let pa = dir.path().join("a.gpkg");
    square(&pa, "", 0.0, 0.0, 1.0);
    let e = Engine::new().unwrap();
    let a = e.import(&pa).unwrap().remove(0);
    if a.crs.is_some() {
        return; // the importer assigned one; nothing to test here
    }
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "a", "tool": "input_layer", "params": {"layer": a.id}}, {"id": "t", "tool": "simplify", "params": {"tolerance": 1}}],
        "edges": [{"source": "a", "target": "t", "targetHandle": "in"}]
    }))
    .unwrap();
    let p = e.check_graph(&g).unwrap();
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].problem.code, "crs-missing");
    assert!(p[0].problem.fix.is_none());
}
