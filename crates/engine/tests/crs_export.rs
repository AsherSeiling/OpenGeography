//! CRS on export (docs/CRS_PLAN.md R3, RD-7): GeoJSON is longitude/latitude on WGS 84. Case R-E3.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn write_utm_point(path: &Path) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_Point(500000.0, 4400000.0) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:26915', USE_TMP_FILE false)", path.display())).unwrap();
}

fn export(e: &Engine, layer: &str, out: &Path, extra: Value) -> engine::graph::NodeResult {
    let mut params = json!({ "path": out.to_str().unwrap(), "format": "GeoJSON" });
    for (k, v) in extra.as_object().unwrap() {
        params[k] = v.clone();
    }
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "x", "tool": "export", "params": params}],
        "edges": [{"source": "in", "target": "x", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let x = res.into_iter().find(|r| r.id == "x").unwrap();
    assert_eq!(x.status, Status::Done, "{x:?}");
    x
}

fn first_coordinates(path: &Path) -> (f64, f64) {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let c = &v["features"][0]["geometry"]["coordinates"];
    (c[0].as_f64().unwrap(), c[1].as_f64().unwrap())
}

#[test]
fn r_e3_utm_layer_to_geojson_is_reprojected_to_wgs84_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("utm.gpkg");
    write_utm_point(&src);
    let e = Engine::new().unwrap();
    let layer = e.import(&src).unwrap().remove(0);

    // Default: longitude/latitude (cs2cs: 500000, 4400000 in UTM 15N is about -93.0, 39.75).
    let out = dir.path().join("wgs.geojson");
    let r = export(&e, &layer.id, &out, json!({}));
    let (lon, lat) = first_coordinates(&out);
    // (NAD83 → WGS 84 moves it by about a metre, so 1e-5° is the right size of agreement.)
    assert!((lon + 93.0).abs() < 1e-5 && (lat - 39.75).abs() < 0.01, "got ({lon}, {lat}) instead of degrees");
    assert!(r.message.as_deref().unwrap_or("").contains("Reprojected to WGS 84"), "{:?}", r.message);
    // It reads back as WGS 84 and goes back to the same UTM position to 1 mm.
    let back = Engine::new().unwrap().import(&out).unwrap().remove(0);
    assert!(engine::crs::same(back.crs.as_deref().unwrap(), "EPSG:4326"));
    let (x, y) = e.transform_point(lon, lat, "EPSG:4326", "EPSG:26915").unwrap();
    assert!((x - 500000.0).abs() < 1e-3 && (y - 4_400_000.0).abs() < 1e-3, "round trip: ({x}, {y})");

    // The older style keeps the layer's coordinates (and a crs member).
    let keep = dir.path().join("utm.geojson");
    let r = export(&e, &layer.id, &keep, json!({ "geojson_crs": "layer" }));
    let (x, y) = first_coordinates(&keep);
    assert!((x - 500000.0).abs() < 1e-3 && (y - 4_400_000.0).abs() < 1e-3, "({x}, {y})");
    assert!(!r.message.as_deref().unwrap_or("").contains("Reprojected"), "{:?}", r.message);
    let text = std::fs::read_to_string(&keep).unwrap();
    assert!(text.contains("\"crs\""), "the legacy crs member is written");
    let back = Engine::new().unwrap().import(&keep).unwrap().remove(0);
    assert!(engine::crs::same(back.crs.as_deref().expect("CRS kept"), "EPSG:26915"));
}

#[test]
fn r_e3_a_wgs84_layer_is_written_as_is_without_a_message() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("ll.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_Point(-93.0, 39.0) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", src.display())).unwrap();
    let e = Engine::new().unwrap();
    let layer = e.import(&src).unwrap().remove(0);
    let out = dir.path().join("ll.geojson");
    let r = export(&e, &layer.id, &out, json!({}));
    assert_eq!(first_coordinates(&out), (-93.0, 39.0));
    assert!(!r.message.as_deref().unwrap_or("").contains("Reprojected"), "{:?}", r.message);
}
