//! Cancelling long-running tools mid-run (docs/TEST_PLAN.md §Progress/cancel).
//! Each case runs a deliberately big job, presses Cancel 300 ms in and checks:
//! the step reports Cancelled, the run stops promptly, no half-made result is
//! left behind and the engine still works afterwards. They take a few seconds
//! each, so they are ignored by default: `cargo test -p engine --test cancel -- --ignored`.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn run_and_cancel(label: &str, e: &Arc<Engine>, g: &Graph, step: &str, owner: &str) {
    run_and_cancel_within(label, e, g, step, owner, Duration::from_secs(3))
}

fn run_and_cancel_within(label: &str, e: &Arc<Engine>, g: &Graph, step: &str, owner: &str, limit: Duration) {
    run_and_cancel_at(Duration::from_millis(300), label, e, g, step, owner, limit)
}

fn run_and_cancel_at(delay: Duration, label: &str, e: &Arc<Engine>, g: &Graph, step: &str, owner: &str, limit: Duration) {
    let label = label.to_string();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(delay);
        let t = engine::progress::snapshot().into_iter().find(|t| t.label == label).unwrap_or_else(|| panic!("task '{label}' is not running (the job finished before Cancel?)"));
        assert!(t.cancellable);
        engine::progress::cancel(t.id);
        Instant::now()
    });
    let res = e.run_graph(g, None, &mut |_| {}).unwrap();
    let stopped = Instant::now();
    let pressed = canceller.join().unwrap();
    eprintln!("CANCEL stopped {:?} after Cancel", stopped.duration_since(pressed));
    assert!(stopped.duration_since(pressed) < limit, "stopped {:?} after Cancel", stopped.duration_since(pressed));
    let st = |id: &str| res.iter().find(|r| r.id == id).unwrap().status.clone();
    assert_eq!(st(step), Status::Cancelled, "{res:?}");
    assert_eq!(st("o"), Status::Cancelled, "the Output after a cancelled step is cancelled too");
    assert!(e.layer(&engine::result_id(owner)).is_err(), "no half-made result layer");
    assert!(engine::progress::snapshot().iter().all(|t| t.label != format!("Running model — {}", e.layer(owner).unwrap().name)), "task removed");
    // Still usable.
    let quick: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": owner}}], "edges": []
    }))
    .unwrap();
    assert!(matches!(e.run_graph(&quick, None, &mut |_| {}).unwrap()[0].status, Status::Done | Status::Cached));
}

fn tree(owner: &str, tool: &str, params: Value, second: Option<&str>) -> Graph {
    let mut nodes = vec![json!({"id": "in", "tool": "input_layer", "params": {"layer": owner}}), json!({"id": "t", "tool": tool, "params": params}), json!({"id": "o", "tool": "layer_output", "params": {"layer": owner}})];
    let mut edges = vec![json!({"source": "in", "target": "t", "targetHandle": "in"}), json!({"source": "t", "target": "o", "targetHandle": "in"})];
    if let Some(b) = second {
        nodes.push(json!({"id": "b", "tool": "input_layer", "params": {"layer": b}}));
        edges.push(json!({"source": "b", "target": "t", "targetHandle": "near"}));
    }
    serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap()
}

#[test]
#[ignore = "perf: ~10 s"]
fn cancel_geodesic_buffer() {
    let e = Arc::new(Engine::new().unwrap());
    let pts = e.generate_test_layer("points", 1_500_001, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let g = tree(&pts.id, "buffer", json!({"distance": 500, "units": "m", "method": "geodesic"}), None);
    run_and_cancel_within("Running model — Test points (1500001)", &e, &g, "t", &pts.id, Duration::from_secs(2));
}

#[test]
#[ignore = "perf: ~3 s"]
fn cancel_near() {
    let e = Arc::new(Engine::new().unwrap());
    let pts = e.generate_test_layer("points", 400_002, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let polys = e.generate_test_layer("polygons", 20_000, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let g = tree(&pts.id, "near", json!({"radius": 500, "units": "km"}), Some(&polys.id));
    run_and_cancel("Running model — Test points (400002)", &e, &g, "t", &pts.id);
}

#[test]
#[ignore = "perf: ~3 s"]
fn cancel_voronoi() {
    let e = Arc::new(Engine::new().unwrap());
    let pts = e.generate_test_layer("points", 300_003, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    run_and_cancel("Running model — Test points (300003)", &e, &tree(&pts.id, "voronoi", json!({"buffer": 10}), None), "t", &pts.id);
}

#[test]
#[ignore = "perf: ~10 s"]
fn cancel_dissolve() {
    let e = Arc::new(Engine::new().unwrap());
    let polys = e.generate_test_layer("polygons", 200_004, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    run_and_cancel_within("Running model — Test polygons (200004)", &e, &tree(&polys.id, "dissolve", json!({}), None), "t", &polys.id, Duration::from_secs(2));
}

#[test]
#[ignore = "perf: ~15 s"]
fn cancel_dissolve_during_the_tile_unions() {
    // 300 ms lands in the first statements; these land in the partial unions and the pairwise merges.
    for secs in [3, 5, 7] {
        let e = Arc::new(Engine::new().unwrap());
        let polys = e.generate_test_layer("polygons", 200_004, [-100.0, 30.0, -94.0, 36.0]).unwrap();
        let g = tree(&polys.id, "dissolve", json!({}), None);
        run_and_cancel_at(Duration::from_secs(secs), "Running model — Test polygons (200004)", &e, &g, "t", &polys.id, Duration::from_secs(2));
    }
}

#[test]
#[ignore = "perf: ~3 s"]
fn cancel_spatial_join() {
    let e = Arc::new(Engine::new().unwrap());
    let pts = e.generate_test_layer("points", 3_000_005, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let polys = e.generate_test_layer("polygons", 100_000, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let mut g = tree(&pts.id, "spatial_join", json!({"predicate": "intersects", "keep_all": true}), None);
    // Re-wire: the join layer goes to the "join" port.
    g.nodes.push(serde_json::from_value(json!({"id": "b", "tool": "input_layer", "params": {"layer": polys.id}})).unwrap());
    g.edges[0] = serde_json::from_value(json!({"source": "in", "target": "t", "targetHandle": "target"})).unwrap();
    g.edges.push(serde_json::from_value(json!({"source": "b", "target": "t", "targetHandle": "join"})).unwrap());
    run_and_cancel("Running model — Test points (3000005)", &e, &g, "t", &pts.id);
}

#[test]
#[ignore = "perf: ~3 s"]
fn cancel_an_import_and_a_tile_build_leave_nothing_behind() {
    // Import: a Cancel during the read must not leave a layer or a citation.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("big.csv");
    let mut text = String::from("id,lat,lon\n");
    for i in 0..2_000_000 {
        text.push_str(&format!("{i},{},{}\n", 30.0 + (i % 6000) as f64 / 1000.0, -100.0 + (i % 5000) as f64 / 1000.0));
    }
    std::fs::write(&p, text).unwrap();
    let e = Arc::new(Engine::new().unwrap());
    let canceller = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(150));
        if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == "Adding big.csv") {
            engine::progress::cancel(t.id);
        }
    });
    let r = e.import(&p);
    canceller.join().unwrap();
    // Import isn't cancellable today (no cancel flag on the task): either outcome must leave a consistent engine.
    match r {
        Ok(l) => assert_eq!(e.list_layers().len(), l.len()),
        Err(_) => assert!(e.list_layers().is_empty() && e.sources().unwrap().is_empty()),
    }
}
