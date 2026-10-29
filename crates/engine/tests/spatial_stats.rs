//! Spatial statistics nodes (phase H3): Moran's I and hot spots on a layer, CRS handling and refusals.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn grid_layer(e: &Engine, dir: &Path, epsg: &str, hot: bool, scale: f64, x0: f64, y0: f64) -> String {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.join(format!("g_{epsg}.gpkg").replace(':', "_"));
    let mut rows = vec![];
    for i in 0..10 {
        for j in 0..10 {
            let v = if hot { if i < 5 { 10.0 } else { 0.0 } } else { ((i * 7 + j * 13) % 10) as f64 };
            rows.push(format!("({v}, ST_GeomFromText('POINT({} {})'))", x0 + i as f64 * scale, y0 + j as f64 * scale));
        }
    }
    c.execute_batch(&format!("COPY (SELECT * FROM (VALUES {}) t(val, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'p', SRS '{epsg}', USE_TMP_FILE false)", rows.join(","), gpkg.display())).unwrap();
    e.import(&gpkg).unwrap().remove(0).id
}

fn run(e: &Engine, tool: &str, params: Value, layer: &str) -> NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in0", "tool": "input_layer", "params": {"layer": layer}}, {"id": "t", "tool": tool, "params": params}],
        "edges": [{"source": "in0", "target": "t", "targetHandle": "in"}]
    })).unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

#[test]
fn h3_morans_i_node_reports_a_clustered_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = grid_layer(&e, dir.path(), "EPSG:32614", true, 100.0, 600_000.0, 3_900_000.0);
    let r = run(&e, "morans_i", json!({"field": "val", "k": 8}), &id);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    let sql = format!("SELECT pattern || '|' || morans_i::VARCHAR || '|' || p_value::VARCHAR FROM \"{}\"", r.output.as_ref().unwrap());
    let text: String = e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| row.get::<_, String>(0))?)).unwrap();
    assert!(text.starts_with("clustered|"), "{text}");
}

#[test]
fn h3_hot_spots_node_adds_columns_and_keeps_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = grid_layer(&e, dir.path(), "EPSG:32614", true, 100.0, 600_000.0, 3_900_000.0);
    let r = run(&e, "hot_spots", json!({"field": "val", "k": 8}), &id);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    assert_eq!(r.rows, Some(100));
    let sql = format!("SELECT count(*) FILTER (WHERE hotspot = 3), count(*) FILTER (WHERE hotspot = -3), count(*) FILTER (WHERE gi_z IS NULL) FROM \"{}\"", r.output.as_ref().unwrap());
    let (hot, cold, empty): (i64, i64, i64) = e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?)).unwrap();
    assert!(hot > 0 && cold > 0 && empty == 0, "{hot} {cold} {empty}");
}

#[test]
fn h3_nodes_refuse_geographic_layers_and_bad_input_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let geo = grid_layer(&e, dir.path(), "EPSG:4326", false, 0.1, -100.0, 35.0);
    for tool in ["morans_i", "hot_spots"] {
        let r = run(&e, tool, json!({"field": "val", "k": 8}), &geo);
        assert!(r.error.as_deref().unwrap_or("").contains("projected coordinate system"), "{tool}: {:?}", r.error);
    }
    let utm = grid_layer(&e, dir.path(), "EPSG:32614", false, 100.0, 600_000.0, 3_900_000.0);
    let r = run(&e, "hot_spots", json!({"field": "val", "k": 0}), &utm);
    assert!(r.error.as_deref().unwrap_or("").contains("whole number"), "{:?}", r.error);
}
