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

fn surface_layer(e: &Engine, dir: &Path, epsg: &str) -> String {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.join(format!("s_{epsg}.gpkg").replace(':', "_"));
    let mut rows = vec![];
    for i in 0..9 {
        for j in 0..9 {
            let (x, y) = (600_000.0 + i as f64 * 100.0 + ((i * 7 + j * 3) % 5) as f64, 3_900_000.0 + j as f64 * 100.0 + ((i * 2 + j * 11) % 7) as f64);
            rows.push(format!("({}::DOUBLE, ST_GeomFromText('POINT({x} {y})'))", 10.0 + 0.01 * (x - 600_000.0) + 0.02 * (y - 3_900_000.0)));
        }
    }
    c.execute_batch(&format!("COPY (SELECT * FROM (VALUES {}) t(val, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'p', SRS '{epsg}', USE_TMP_FILE false)", rows.join(","), gpkg.display())).unwrap();
    e.import(&gpkg).unwrap().remove(0).id
}

#[test]
fn h2_kriging_node_gives_prediction_and_variance_bands_in_the_layers_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = surface_layer(&e, dir.path(), "EPSG:32614");
    let r = run(&e, "kriging", json!({"field": "val", "cell": 50, "model": "auto"}), &id);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let ds = gdal::Dataset::open(r.output.as_ref().unwrap()).unwrap();
    assert_eq!(ds.raster_count(), 2);
    let (cols, rows) = ds.raster_size();
    assert!(cols >= 16 && rows >= 16, "{cols} x {rows}");
    // The middle of the grid: x = 600_400, y = 3_900_400 → z = 10 + 4 + 8 = 22 (the surface is a plane).
    let (c, rw) = (((600_400.0f64 - ds.geo_transform().unwrap()[0]) / 50.0) as isize, ((ds.geo_transform().unwrap()[3] - 3_900_400.0) / 50.0) as isize);
    let z = ds.rasterband(1).unwrap().read_as::<f32>((c, rw), (1, 1), (1, 1), None).unwrap().data()[0];
    let var = ds.rasterband(2).unwrap().read_as::<f32>((c, rw), (1, 1), (1, 1), None).unwrap().data()[0];
    assert!((z - 22.0).abs() < 0.3, "{z}");
    assert!(var >= 0.0 && var.is_finite());
    use gdal::Metadata;
    assert!(ds.metadata_item("VARIOGRAM", "").unwrap_or_default().contains("range="));
}

#[test]
fn h2_kriging_node_refuses_geographic_layers_and_partial_variograms_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let geo = grid_layer(&e, dir.path(), "EPSG:4326", false, 0.1, -100.0, 35.0);
    let r = run(&e, "kriging", json!({"field": "val", "cell": 0.05}), &geo);
    assert!(r.error.as_deref().unwrap_or("").contains("projected coordinate system"), "{:?}", r.error);
    let utm = surface_layer(&e, dir.path(), "EPSG:32614");
    let r = run(&e, "kriging", json!({"field": "val", "cell": 50, "nugget": 0.1}), &utm);
    assert!(r.error.as_deref().unwrap_or("").contains("all of nugget, sill and range"), "{:?}", r.error);
    let r = run(&e, "kriging", json!({"field": "val", "cell": 50, "nugget": 0.0, "sill": 4.0, "range": 300.0, "neighbours": 3}), &utm);
    assert!(r.error.as_deref().unwrap_or("").contains("between 8 and 32"), "{:?}", r.error);
    let ok = run(&e, "kriging", json!({"field": "val", "cell": 50, "model": "exponential", "nugget": 0.0, "sill": 4.0, "range": 300.0}), &utm);
    assert!(matches!(ok.status, Status::Done | Status::Cached), "{:?}", ok.error);
}

#[test]
fn c3_analysis_tools_warn_when_the_projection_stretches_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // The same grid of points in Web Mercator near 45°N (lengths stretched 1.4 times) and in UTM near its central meridian.
    let merc = grid_layer(&e, dir.path(), "EPSG:3857", false, 1000.0, -10_400_000.0, 5_600_000.0);
    let r = run(&e, "morans_i", json!({"field": "val", "k": 8}), &merc);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    assert!(r.message.as_deref().unwrap_or("").contains("Warning, Moran's I measures in the map units"), "{:?}", r.message);
    let utm = grid_layer(&e, dir.path(), "EPSG:32615", false, 100.0, 500_000.0, 4_400_000.0);
    let r = run(&e, "hot_spots", json!({"field": "val", "k": 8}), &utm);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    assert!(!r.message.as_deref().unwrap_or("").contains("map units"), "{:?}", r.message);
}
