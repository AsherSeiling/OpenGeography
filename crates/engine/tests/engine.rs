use engine::graph::{Graph, Status};
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::json;
use std::path::Path;

/// Write a fixture file with DuckDB's GDAL writer.
fn write_fixture(path: &Path, select: &str, driver: &str, srs: Option<&str>) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    c.execute_batch("INSTALL spatial; LOAD spatial; SET geometry_always_xy = true;").unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs})", path.display())).unwrap();
}

fn graph(v: serde_json::Value) -> Graph {
    serde_json::from_value(v).unwrap()
}

fn first_value(e: &Engine, layer: &str, field: &str) -> f64 {
    let info = e.layer(layer).unwrap();
    let idx = info.fields.iter().position(|f| f.name == field).unwrap();
    let page = e.rows(layer, &RowsQuery { limit: 1, ..Default::default() }).unwrap();
    page.rows[0][idx].as_ref().unwrap().parse().unwrap()
}

#[test]
fn imports_crs_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("utm.gpkg");
    write_fixture(&p, "SELECT 1 AS id, ST_Point(636548.88, 3896253.14) AS geom", "GPKG", Some("EPSG:26914"));
    let e = Engine::new().unwrap();
    let l = &e.import(&p).unwrap()[0];
    assert_eq!(l.crs.as_deref(), Some("EPSG:26914"));
    assert_eq!(l.geom_type, "Point");
    let ext = l.extent.unwrap();
    assert!((ext[0] + 97.5).abs() < 1e-4 && (ext[1] - 35.2).abs() < 1e-4, "extent {ext:?}");
}

#[test]
fn shapefile_without_prj_is_flagged_not_guessed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("noprj.shp");
    write_fixture(&p, "SELECT 1 AS id, ST_Point(-97.5, 35.2) AS geom", "ESRI Shapefile", None);
    let e = Engine::new().unwrap();
    let l = &e.import(&p).unwrap()[0];
    assert!(l.crs.is_none());
    assert!(!l.warnings.is_empty());
    assert!(e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty(), "undefined CRS must not be drawn");
    assert_eq!(e.suggest_crs(&l.id).unwrap()[0].crs, "EPSG:4326");
    let l = e.define_projection(&l.id, "EPSG:4269", Some("NAD83".into())).unwrap();
    assert!(l.warnings.is_empty());
    assert!(!e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty());
}

#[test]
fn buffer_is_true_ground_distance_on_geographic_data() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("pt.geojson");
    write_fixture(&p, "SELECT 1 AS id, ST_Point(-97.5, 35.2) AS geom", "GeoJSON", None);
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "buf", "tool": "buffer", "params": {"distance": 1, "units": "km"}},
            {"id": "area", "tool": "calculate_geometry", "params": {"field": "AREA", "property": "area", "area_units": "sqkm"}},
            {"id": "out", "tool": "add_to_map", "params": {"name": "Buffered"}}
        ],
        "edges": [
            {"source": "in", "target": "buf", "targetHandle": "in"},
            {"source": "buf", "target": "area", "targetHandle": "in"},
            {"source": "area", "target": "out", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert!(res.iter().all(|r| r.status == Status::Done), "{res:#?}");
    let out = res.iter().find(|r| r.id == "out").unwrap().layer.clone().unwrap();
    let area = first_value(&e, &out.id, "AREA");
    let expected = std::f64::consts::PI;
    assert!((area - expected).abs() / expected < 0.005, "area {area} vs {expected}");
}

#[test]
fn clip_keeps_only_inside_and_reprojects_clip_layer() {
    let dir = tempfile::tempdir().unwrap();
    let pts = dir.path().join("pts.gpkg");
    let area = dir.path().join("area.gpkg");
    // 10x10 grid of points across [-98,-97] x [35,36] in WGS84.
    write_fixture(&pts, "SELECT x*10+y AS id, ST_Point(-98 + x*0.1 + 0.05, 35 + y*0.1 + 0.05) AS geom FROM range(10) a(x), range(10) b(y)", "GPKG", Some("EPSG:4326"));
    // Clip square covering the west half, stored in Web Mercator.
    write_fixture(&area, "SELECT 1 AS id, ST_Transform(ST_MakeEnvelope(-98, 35, -97.5, 36), 'EPSG:4326', 'EPSG:3857', true) AS geom", "GPKG", Some("EPSG:3857"));
    let e = Engine::new().unwrap();
    let a = e.import(&pts).unwrap().remove(0);
    let c = e.import(&area).unwrap().remove(0);
    let g = graph(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": a.id}},
            {"id": "c", "tool": "input_layer", "params": {"layer": c.id}},
            {"id": "clip", "tool": "clip", "params": {}},
            {"id": "out", "tool": "add_to_map", "params": {"name": "Clipped"}}
        ],
        "edges": [
            {"source": "a", "target": "clip", "targetHandle": "in"},
            {"source": "c", "target": "clip", "targetHandle": "clip"},
            {"source": "clip", "target": "out", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let clip = res.iter().find(|r| r.id == "clip").unwrap();
    assert_eq!(clip.status, Status::Done, "{clip:?}");
    assert!(clip.reprojected, "clip layer should have been reprojected to the input CRS");
    assert_eq!(clip.rows, Some(50));
    let out = res.iter().find(|r| r.id == "out").unwrap().layer.clone().unwrap();
    let ext = out.extent.unwrap();
    assert!(ext[2] <= -97.5 + 1e-9, "clipped extent {ext:?}");
}

#[test]
fn model_reuses_cached_steps() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 1000, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let mk = |d: f64| graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "f", "tool": "filter", "params": {"where": "class < 5"}},
            {"id": "buf", "tool": "buffer", "params": {"distance": d, "units": "m"}}
        ],
        "edges": [
            {"source": "in", "target": "f", "targetHandle": "in"},
            {"source": "f", "target": "buf", "targetHandle": "in"}
        ]
    }));
    e.run_graph(&mk(100.0), None, &mut |_| {}).unwrap();
    let res = e.run_graph(&mk(200.0), None, &mut |_| {}).unwrap();
    let st = |id: &str| res.iter().find(|r| r.id == id).unwrap().status.clone();
    assert_eq!(st("f"), Status::Cached);
    assert_eq!(st("buf"), Status::Done);
}

#[test]
fn missing_input_and_bad_expression_report_errors() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 10, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "f", "tool": "filter", "params": {"where": "no_such_field > 1"}},
            {"id": "clip", "tool": "clip", "params": {}},
            {"id": "down", "tool": "buffer", "params": {"distance": 1}}
        ],
        "edges": [
            {"source": "in", "target": "f", "targetHandle": "in"},
            {"source": "f", "target": "down", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let get = |id: &str| res.iter().find(|r| r.id == id).unwrap();
    assert_eq!(get("f").status, Status::Error);
    assert!(!get("f").error.as_ref().unwrap().contains("__node_"));
    assert_eq!(get("clip").status, Status::Error);
    assert_eq!(get("down").status, Status::Skipped);
}

#[test]
fn nad27_to_nad83_shift_matches_nadcon_magnitude() {
    let e = Engine::new().unwrap();
    // San Francisco: published NAD27→NAD83 shift is roughly 90–95 m, mostly westward.
    let (lon, lat) = e.transform_point(-122.4, 37.8, "EPSG:4267", "EPSG:4269").unwrap();
    let dx = (lon + 122.4) * 111_320.0 * 37.8f64.to_radians().cos();
    let dy = (lat - 37.8) * 110_950.0;
    let d = (dx * dx + dy * dy).sqrt();
    assert!(dx < 0.0 && (80.0..105.0).contains(&d), "shift dx={dx:.1} dy={dy:.1} d={d:.1}");
}

#[test]
fn utm_round_trip() {
    let e = Engine::new().unwrap();
    let (x, y) = e.transform_point(-97.5, 35.2, "EPSG:4326", "EPSG:32614").unwrap();
    let (lon, lat) = e.transform_point(x, y, "EPSG:32614", "EPSG:4326").unwrap();
    assert!((lon + 97.5).abs() < 1e-9 && (lat - 35.2).abs() < 1e-9);
}

#[test]
fn dense_tiles_stay_small() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("polygons", 200_000, [-106.0, 26.0, -94.0, 36.0]).unwrap();
    // z5 tile over Texas.
    let t = e.tile(&l.id, 5, 7, 13, &[]).unwrap();
    assert!(!t.is_empty());
    assert!(t.len() < 1_500_000, "tile is {} bytes", t.len());
}

#[test]
fn save_and_reopen_project() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("test.gproj");
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 100, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e.save_project(&proj, r#"{"hello":1}"#).unwrap();
    let e2 = Engine::new().unwrap();
    let ui = e2.open_project(&proj).unwrap();
    assert_eq!(ui, r#"{"hello":1}"#);
    assert_eq!(e2.layer(&l.id).unwrap().feature_count, 100);
    assert!(!e2.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty());
}

#[test]
fn export_handles_gdal_reserved_id_columns_and_keeps_crs() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("roads.geojson");
    write_fixture(&src, "SELECT 1 AS id, ST_MakeLine(ST_Point(-97, 35), ST_Point(-96, 36)) AS geom", "GeoJSON", None);
    let e = Engine::new().unwrap();
    let l = e.import(&src).unwrap().remove(0);
    assert!(l.fields.iter().any(|f| f.name.eq_ignore_ascii_case("ogc_fid") || f.name == "id"));
    for (name, fmt) in [("out.gpkg", "GPKG"), ("out.shp", "ESRI Shapefile"), ("out.geojson", "GeoJSON")] {
        let out = dir.path().join(name);
        let g = graph(json!({
            "nodes": [
                {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
                {"id": "x", "tool": "export", "params": {"path": out.to_str().unwrap(), "format": fmt}}
            ],
            "edges": [{"source": "in", "target": "x", "targetHandle": "in"}]
        }));
        let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
        let x = res.iter().find(|r| r.id == "x").unwrap();
        assert_eq!(x.status, Status::Done, "{fmt}: {x:?}");
        let back = Engine::new().unwrap().import(&out).unwrap().remove(0);
        assert_eq!(back.feature_count, 1, "{fmt}");
        assert!(back.crs.is_some(), "{fmt} lost its CRS");
    }
}
