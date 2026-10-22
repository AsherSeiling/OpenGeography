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

#[test]
fn layer_tree_output_replaces_display_and_can_be_saved_as_new_layer() {
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 100, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "f", "tool": "filter", "params": {"where": "class < 3"}},
            {"id": "out", "tool": "layer_output", "params": {"layer": l.id}}
        ],
        "edges": [
            {"source": "in", "target": "f", "targetHandle": "in"},
            {"source": "f", "target": "out", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "out").unwrap();
    assert_eq!(out.status, Status::Done, "{out:?}");
    let result = out.layer.clone().unwrap();
    assert_eq!(result.result_of.as_deref(), Some(l.id.as_str()));
    assert_eq!(result.name, l.name);
    assert!(result.feature_count < 100);
    // Original untouched.
    assert_eq!(e.layer(&l.id).unwrap().feature_count, 100);
    // Re-running replaces the same result layer.
    let again = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert_eq!(again.iter().find(|r| r.id == "out").unwrap().layer.as_ref().unwrap().id, result.id);
    // Save as a new independent layer.
    let copy = e.copy_layer(&result.id, "Filtered points").unwrap();
    assert!(copy.result_of.is_none());
    assert_eq!(copy.feature_count, result.feature_count);
    assert_eq!(copy.crs, l.crs);
    // Removing the layer removes its result too.
    e.remove_layer(&l.id).unwrap();
    assert!(e.layer(&result.id).is_err());
}

#[test]
fn spreadsheet_style_attribute_editing() {
    let e = Engine::new().unwrap();
    // Large enough to get overview tables, which must stay in sync.
    let l = e.generate_test_layer("polygons", 60_000, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    assert!(!l.overviews.is_empty());

    // Geometry into a field that doesn't exist yet: it is created.
    let (l, n) = e.calculate_geometry(&l.id, "AREA_ACRES", "area", "acres", "km", None).unwrap();
    assert_eq!(n, 60_000);
    let f = l.fields.iter().find(|f| f.name == "AREA_ACRES").expect("field created");
    assert_eq!(f.ty, "DOUBLE");
    assert!(first_value(&e, &l.id, "AREA_ACRES") > 0.0);

    // Add, edit, rename, calculate on selection, delete.
    let l = e.add_field(&l.id, "NOTE", "text").unwrap();
    let l = e.update_cell(&l.id, 1, "NOTE", Some("checked in field")).unwrap();
    assert!(e.update_cell(&l.id, 1, "value", Some("not a number")).is_err());
    let l = e.update_cell(&l.id, 1, "value", Some("42.5")).unwrap();
    let l = e.rename_field(&l.id, "NOTE", "COMMENT").unwrap();
    assert!(l.fields.iter().any(|f| f.name == "COMMENT"));
    let (l, n) = e.calculate_field(&l.id, "DOUBLED", "value * 2", None, Some(&[1, 2, 3])).unwrap();
    assert_eq!(n, 3);
    let row = e.feature(&l.id, 1).unwrap();
    let get = |k: &str| row.iter().find(|(n, _)| n == k).unwrap().1.clone();
    assert_eq!(get("COMMENT").as_deref(), Some("checked in field"));
    assert_eq!(get("DOUBLED").as_deref(), Some("85.0"));
    let l = e.delete_field(&l.id, "DOUBLED").unwrap();
    let (l, n) = e.delete_rows(&l.id, &[1, 2]).unwrap();
    assert_eq!(n, 2);
    assert_eq!(l.feature_count, 59_998);

    // Styled tiles read edited values from the overviews at low zoom.
    let t = e.tile(&l.id, 4, 3, 6, &["COMMENT".into()]).unwrap();
    assert!(!t.is_empty());

    // Bad expressions don't change anything.
    assert!(e.calculate_field(&l.id, "X", "no_such_column + 1", None, None).is_err());
    assert!(!e.layer(&l.id).unwrap().fields.iter().any(|f| f.name == "X"));
}

#[test]
fn new_geoprocessing_tools_run() {
    let dir = tempfile::tempdir().unwrap();
    let polys = dir.path().join("grid.gpkg");
    // 4 x 3 grid of 0.25° squares, plus one two-part feature.
    write_fixture(
        &polys,
        "SELECT x*3+y AS id, CASE WHEN x < 2 THEN 'west' ELSE 'east' END AS side, (x+1)*100 AS pop,
                ST_MakeEnvelope(-98 + x*0.25, 35 + y*0.25, -98 + (x+1)*0.25, 35 + (y+1)*0.25) AS geom
         FROM range(4) a(x), range(3) b(y)
         UNION ALL SELECT 99, 'multi', 1, ST_Union(ST_MakeEnvelope(-99, 35, -98.9, 35.1), ST_MakeEnvelope(-98.8, 35, -98.7, 35.1))",
        "GPKG",
        Some("EPSG:4326"),
    );
    let e = Engine::new().unwrap();
    let p = e.import(&polys).unwrap().remove(0);
    let pts = e.generate_test_layer("points", 500, [-98.0, 35.0, -97.0, 35.75]).unwrap();

    let run = |tool: &str, params: serde_json::Value, second: Option<&str>| {
        let mut nodes = vec![json!({"id": "a", "tool": "input_layer", "params": {"layer": p.id}}),
                             json!({"id": "b", "tool": "input_layer", "params": {"layer": pts.id}})];
        nodes.push(json!({"id": "t", "tool": tool, "params": params}));
        let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap();
        let mut edges = vec![json!({"source": "a", "target": "t", "targetHandle": spec.inputs[0].id})];
        if let (Some(port), Some(src)) = (spec.inputs.get(1), second) {
            edges.push(json!({"source": src, "target": "t", "targetHandle": port.id}));
        }
        let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
        let r = res.into_iter().find(|r| r.id == "t").unwrap();
        assert_eq!(r.status, Status::Done, "{tool}: {:?}", r.error);
        r.rows.unwrap()
    };

    assert_eq!(run("feature_to_point", json!({"method": "inside"}), None), 13);
    assert_eq!(run("bounding_geometry", json!({"shape": "hull", "group": "field", "field": "side"}), None), 3);
    assert_eq!(run("bounding_geometry", json!({"shape": "envelope", "group": "all"}), None), 1);
    assert_eq!(run("simplify", json!({"tolerance": 100, "units": "m", "topology": true}), None), 13);
    assert_eq!(run("explode", json!({}), None), 14); // the two-part feature becomes two
    assert_eq!(run("repair", json!({}), None), 13);
    let within = run("summarize_within", json!({"field": "value", "stat": "sum"}), Some("b"));
    assert_eq!(within, 13);
    assert_eq!(run("summary_statistics", json!({"field": "pop", "case": "side"}), None), 3);
    let cells = run("fishnet", json!({"size": 5, "units": "km", "clip": true}), None);
    assert!(cells > 20, "fishnet cells {cells}");
    assert_eq!(run("merge", json!({"source_field": true}), Some("b")), 513);
    let sel = run("select_by_location", json!({"relation": "intersects", "invert": false}), Some("b"));
    let not_sel = run("select_by_location", json!({"relation": "intersects", "invert": true}), Some("b"));
    assert_eq!(sel + not_sel, 13);
    assert!(run("select_by_location", json!({"relation": "distance", "distance": 50, "units": "km"}), Some("b")) >= sel);
    assert_eq!(run("near", json!({"radius": 50, "units": "km"}), Some("b")), 13);
    // Erase the grid by buffered points: fewer or equal features, smaller area.
    assert!(run("erase", json!({}), Some("b")) <= 13);
}

/// Needs internet access to tigerweb.geo.census.gov; run with --ignored.
#[test]
#[ignore]
fn tiger_sample_data_downloads() {
    let counties = engine::tiger::counties("40").unwrap(); // Oklahoma
    assert_eq!(counties.len(), 77);
    let cleveland = counties.iter().find(|(_, n)| n.starts_with("Cleveland")).unwrap().0.clone();
    let e = Engine::new().unwrap();
    let layers = e
        .tiger_import(&["tracts".into(), "primary_roads".into(), "blocks".into()], "40", Some(&cleveland), "Cleveland County, OK")
        .unwrap();
    assert_eq!(layers.len(), 3);
    for l in &layers {
        assert_eq!(l.crs.as_deref(), Some("EPSG:4326"), "{}", l.name);
        assert!(l.feature_count > 0, "{}", l.name);
        assert!(l.fields.iter().any(|f| f.name == "GEOID") || l.geom_type == "Line", "{} fields {:?}", l.name, l.fields);
    }
    let blocks = &layers[2];
    assert!(blocks.feature_count > 1000, "blocks {}", blocks.feature_count);
}
