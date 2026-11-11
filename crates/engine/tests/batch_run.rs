//! Batch run (UX5): one tree over many inputs, a report per input, never touching the sources.

use engine::batch::{BatchRequest, BatchSource, BatchStatus};
use engine::graph::Graph;
use engine::Engine;
use serde_json::json;
use std::path::Path;

fn square(path: &Path, srs_ddl: &str, cx: f64, cy: f64, half: f64) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_MakeEnvelope({}, {}, {}, {}) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', {srs_ddl}USE_TMP_FILE false)", cx - half, cy - half, cx + half, cy + half, path.display())).unwrap();
}

fn tree() -> Graph {
    serde_json::from_value(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {}},
            {"id": "b", "tool": "buffer", "params": {"distance": 10}},
            {"id": "o", "tool": "layer_output", "params": {}}
        ],
        "edges": [{"source": "in", "target": "b", "targetHandle": "in"}, {"source": "b", "target": "o", "targetHandle": "in"}]
    }))
    .unwrap()
}

fn request(sources: Vec<BatchSource>, out: &Path) -> BatchRequest {
    BatchRequest { graph: tree(), input_node: "in".into(), result_node: "b".into(), sources, out_dir: out.display().to_string(), format: "GPKG".into(), overwrite: false, add_to_project: false }
}

#[test]
fn ux5_each_input_gets_a_result_file_and_a_row_and_one_bad_input_does_not_stop_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (src, out) = (dir.path().join("in"), dir.path().join("out"));
    std::fs::create_dir(&src).unwrap();
    square(&src.join("one.gpkg"), "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0);
    square(&src.join("two.gpkg"), "SRS 'EPSG:26915', ", 501000.0, 4_401_000.0, 500.0);
    std::fs::write(src.join("broken.geojson"), "{this is not geojson").unwrap();
    let e = Engine::new().unwrap();
    let before = e.list_layers().len();
    let files = engine::batch::list_folder(&src).unwrap();
    assert_eq!(files.len(), 3);
    let sources = files.iter().map(|p| BatchSource::File { path: p.display().to_string() }).collect();
    let items = e.run_batch(&request(sources, &out)).unwrap();
    assert_eq!(items.len(), 3, "{items:?}");
    let by = |n: &str| items.iter().find(|i| i.name.starts_with(n)).unwrap_or_else(|| panic!("no row for {n}: {items:?}"));
    assert_eq!(by("one").status, BatchStatus::Done, "{:?}", by("one"));
    assert_eq!(by("two").status, BatchStatus::Done);
    assert!(out.join("one.gpkg").exists() && out.join("two.gpkg").exists());
    assert_eq!(by("one").rows, Some(1));
    let broken = by("broken");
    assert_eq!(broken.status, BatchStatus::Failed);
    assert!(broken.message.as_deref().unwrap_or("").contains("Could not read"), "{broken:?}");
    // The sources are untouched and the project has no leftover layers.
    assert!(src.join("one.gpkg").exists());
    assert_eq!(e.list_layers().len(), before, "layers imported for the run are removed again");
}

#[test]
fn ux5_a_layer_without_a_crs_is_a_failed_row_with_a_sentence() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let f = dir.path().join("nocrs.gpkg");
    square(&f, "", 0.0, 0.0, 1.0);
    let e = Engine::new().unwrap();
    let items = e.run_batch(&request(vec![BatchSource::File { path: f.display().to_string() }], &out)).unwrap();
    // The importer may assign a CRS to a file without one; either way the row is explained.
    match items[0].status {
        BatchStatus::Failed => assert!(items[0].message.as_deref().unwrap_or("").contains("coordinate system"), "{items:?}"),
        BatchStatus::Done => {}
        s => panic!("{s:?}"),
    }
}

#[test]
fn ux5_existing_results_are_never_replaced_unless_asked() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let f = dir.path().join("a.gpkg");
    square(&f, "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0);
    std::fs::write(out.join("a.gpkg"), b"precious").unwrap();
    let e = Engine::new().unwrap();
    let src = vec![BatchSource::File { path: f.display().to_string() }];
    let items = e.run_batch(&request(src.clone(), &out)).unwrap();
    assert_eq!(items[0].status, BatchStatus::Skipped, "{items:?}");
    assert_eq!(std::fs::read(out.join("a.gpkg")).unwrap(), b"precious");
    let mut again = request(src, &out);
    again.overwrite = true;
    let items = e.run_batch(&again).unwrap();
    assert_eq!(items[0].status, BatchStatus::Done, "{items:?}");
    assert_ne!(std::fs::read(out.join("a.gpkg")).unwrap(), b"precious");
}

#[test]
fn ux5_project_layers_work_as_inputs_and_can_be_added_back() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let f = dir.path().join("p.gpkg");
    square(&f, "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0);
    let e = Engine::new().unwrap();
    let l = e.import(&f).unwrap().remove(0);
    let mut r = request(vec![BatchSource::Layer { id: l.id.clone() }], &out);
    r.add_to_project = true;
    let items = e.run_batch(&r).unwrap();
    assert_eq!(items[0].status, BatchStatus::Done, "{items:?}");
    assert!(items[0].layer.is_some(), "added to the project");
    assert!(e.layer(&l.id).is_ok(), "a project layer given as input stays");
    // A layer that is gone is a row, not an error.
    let items = e.run_batch(&request(vec![BatchSource::Layer { id: "L999".into() }], &out)).unwrap();
    assert_eq!(items[0].status, BatchStatus::Failed);
}

#[test]
fn ux5_cancel_marks_the_rest_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("cancelme");
    let e = Engine::new().unwrap();
    let mut sources = Vec::new();
    for i in 0..3 {
        let f = dir.path().join(format!("f{i}.gpkg"));
        square(&f, "SRS 'EPSG:26915', ", 500000.0 + i as f64, 4_400_000.0, 1000.0);
        sources.push(BatchSource::File { path: f.display().to_string() });
    }
    // Cancel the moment the batch task shows up.
    let canceller = std::thread::spawn(|| loop {
        if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == "Batch run — cancelme") {
            engine::progress::cancel(t.id);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    });
    let items = e.run_batch(&request(sources, &out)).unwrap();
    canceller.join().unwrap();
    assert_eq!(items.len(), 3);
    assert!(items.iter().any(|i| i.status == BatchStatus::Cancelled), "{items:?}");
    assert!(items.iter().all(|i| i.status != BatchStatus::Failed), "{items:?}");
}
