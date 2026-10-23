use engine::graph::{Graph, Status};
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::json;
use std::path::Path;

/// Write a fixture file with DuckDB's GDAL writer.
fn write_fixture(path: &Path, select: &str, driver: &str, srs: Option<&str>) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
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
    drop(e); // release the project file (Windows locks it while open)
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
    let copy = e.copy_layer(&result.id, "Filtered points", None).unwrap();
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
    assert_eq!(run("bounding_geometry", json!({"shape": "concave", "ratio": 0.5, "group": "all"}), None), 1);
    assert!(run("delaunay", json!({}), None) > 13); // triangles over the grid's vertices
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

#[test]
fn union_symmetrical_difference_and_collect() {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.gpkg"), dir.path().join("b.gpkg"));
    // A = [0,2]², B = [1,3]²: one 1×1 overlap, 3 units of A only, 3 of B only.
    write_fixture(&pa, "SELECT 1 AS id, 'a' AS name, ST_MakeEnvelope(0, 0, 2, 2) AS geom", "GPKG", Some("EPSG:4326"));
    write_fixture(
        &pb,
        "SELECT 7 AS id, 'b' AS name, ST_MakeEnvelope(1, 1, 3, 3) AS geom
         UNION ALL SELECT 8, 'b', ST_MakeEnvelope(10, 10, 11, 11)",
        "GPKG",
        Some("EPSG:4326"),
    );
    let e = Engine::new().unwrap();
    let a = e.import(&pa).unwrap().remove(0);
    let b = e.import(&pb).unwrap().remove(0);
    let run = |tool: &str, params: serde_json::Value| {
        let nodes = vec![
            json!({"id": "a", "tool": "input_layer", "params": {"layer": a.id}}),
            json!({"id": "b", "tool": "input_layer", "params": {"layer": b.id}}),
            json!({"id": "t", "tool": tool, "params": params}),
        ];
        let mut edges = vec![json!({"source": "a", "target": "t", "targetHandle": "in"})];
        let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap();
        edges[0]["targetHandle"] = json!(spec.inputs[0].id);
        if let Some(port) = spec.inputs.get(1) {
            edges.push(json!({"source": "b", "target": "t", "targetHandle": port.id}));
        }
        let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
        let r = res.into_iter().find(|r| r.id == "t").unwrap();
        assert_eq!(r.status, Status::Done, "{tool}: {:?}", r.error);
        r.rows.unwrap()
    };
    // Union: overlap + A-only + B-only (feature 7) + the untouched B feature 8.
    assert_eq!(run("union", json!({})), 4);
    // Symmetrical difference: A-only + B-only (7) + B feature 8.
    assert_eq!(run("symmetrical_difference", json!({})), 3);
    // Collect: A is one feature; grouping the other layer would give 1 group per name.
    assert_eq!(run("collect", json!({})), 1);
}

#[test]
fn bad_settings_are_rejected_with_a_readable_message() {
    let dir = tempfile::tempdir().unwrap();
    let pa = dir.path().join("a.gpkg");
    write_fixture(&pa, "SELECT 1 AS id, ST_MakePoint(-97.0, 35.0) AS geom", "GPKG", Some("EPSG:4326"));
    let e = Engine::new().unwrap();
    let a = e.import(&pa).unwrap().remove(0);
    let run = |tool: &str, params: serde_json::Value| {
        let nodes = vec![
            json!({"id": "a", "tool": "input_layer", "params": {"layer": a.id}}),
            json!({"id": "t", "tool": tool, "params": params}),
        ];
        let edges = vec![json!({"source": "a", "target": "t", "targetHandle": "in"})];
        let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
        res.into_iter().find(|r| r.id == "t").unwrap()
    };
    let bad = run("buffer", json!({"distance": "abc", "units": "m"}));
    assert_eq!(bad.status, Status::Error);
    assert!(bad.error.unwrap().contains("must be a number"));
    let bad = run("bounding_geometry", json!({"shape": "blob", "group": "each"}));
    assert_eq!(bad.status, Status::Error);
    assert!(bad.error.unwrap().contains("must be one of"));
    assert_eq!(run("buffer", json!({"distance": "250", "units": "m"})).status, Status::Done);
}

/// Needs internet access to tigerweb.geo.census.gov; run with --ignored.
#[test]
#[ignore]
fn tiger_sample_data_downloads() {
    let counties = engine::tiger::counties("40", "Census2020").unwrap(); // Oklahoma
    assert_eq!(counties.len(), 77);
    let cleveland = counties.iter().find(|(_, n)| n.starts_with("Cleveland")).unwrap().0.clone();
    let e = Engine::new().unwrap();
    let layers = e
        .tiger_import(&["tracts".into(), "primary_roads".into(), "blocks".into()], "Census2020", "40", Some(&cleveland), "Cleveland County, OK")
        .unwrap();
    assert_eq!(layers.len(), 3);
    for l in &layers {
        assert_eq!(l.crs.as_deref(), Some("EPSG:4326"), "{}", l.name);
        assert!(l.feature_count > 0, "{}", l.name);
        assert!(l.fields.iter().any(|f| f.name == "GEOID") || l.geom_type == "Line", "{} fields {:?}", l.name, l.fields);
    }
    let blocks = &layers[2];
    assert!(blocks.feature_count > 1000, "blocks {}", blocks.feature_count);
    assert!(blocks.fields.iter().any(|f| f.name == "POP100"), "blocks should carry 2020 population");

    // Years: newest first, Current on top, decennial years flagged.
    let vs = engine::tiger::vintages().unwrap();
    assert_eq!(vs[0].id, "Current");
    assert!(vs.windows(2).skip(1).all(|w| w[0].year >= w[1].year), "{vs:?}");
    assert!(vs.iter().any(|v| v.id == "Census2010" && v.decennial));
    assert!(vs.iter().any(|v| v.id == "ACS2015" && !v.decennial));
    // ACS-year geography has no blocks; ZIP areas resolve under that year's layer name.
    let avail: std::collections::HashMap<String, bool> = engine::tiger::availability("ACS2015").unwrap().into_iter().collect();
    assert!(!avail["blocks"] && avail["tracts"] && avail["zcta"], "{avail:?}");
    assert!(engine::tiger::availability("../etc").is_err(), "vintage ids are validated");
    // An older year downloads and is labelled with it.
    let old = e.tiger_import(&["tracts".into()], "ACS2015", "40", Some(&cleveland), "Cleveland County, OK").unwrap();
    assert!(old[0].feature_count > 20 && old[0].name.contains("(2015)"), "{} {}", old[0].name, old[0].feature_count);
    let err = e.tiger_import(&["blocks".into()], "ACS2015", "40", Some(&cleveland), "x").unwrap_err().to_string();
    assert!(err.contains("isn't available for this year"), "{err}");
}

#[test]
fn definition_query_and_selected_only_export() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 1000, [-98.0, 35.0, -97.0, 36.0]).unwrap();

    // Bad queries are rejected and leave the layer alone.
    assert!(e.set_definition_query(&l.id, Some("no_such_field = 1")).is_err());
    assert!(e.layer(&l.id).unwrap().definition_query.is_none());

    let l = e.set_definition_query(&l.id, Some("class < 3")).unwrap();
    let n = l.query_count.unwrap();
    assert!(n > 0 && n < 1000);
    // Table, selection, tools and copies all see only matching features.
    assert_eq!(e.rows(&l.id, &RowsQuery { limit: 10, ..Default::default() }).unwrap().total, n);
    assert_eq!(e.select_box(&l.id, [-99.0, 34.0, -96.0, 37.0]).unwrap().len() as i64, n);
    assert!(e.select_where(&l.id, "class >= 3").unwrap().is_empty());
    let g = graph(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": l.id}}],
        "edges": []
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert_eq!(res[0].rows, Some(n));
    assert_eq!(e.copy_layer(&l.id, "Copy", None).unwrap().feature_count, n);

    // Export only selected features (respecting the query too).
    let sel = e.select_where(&l.id, "value < 300").unwrap();
    let out = dir.path().join("sel.gpkg");
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id, "fids": sel}},
            {"id": "x", "tool": "export", "params": {"path": out.to_str().unwrap(), "format": "GPKG"}}
        ],
        "edges": [{"source": "in", "target": "x", "targetHandle": "in"}]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert!(res.iter().all(|r| r.status == Status::Done), "{res:?}");
    let back = Engine::new().unwrap().import(&out).unwrap().remove(0);
    assert_eq!(back.feature_count as usize, sel.len());
    assert_eq!(e.copy_layer(&l.id, "Selection", Some(&sel)).unwrap().feature_count as usize, sel.len());

    // Clearing restores everything.
    let l = e.set_definition_query(&l.id, None).unwrap();
    assert!(l.query_count.is_none());
    assert_eq!(e.rows(&l.id, &RowsQuery { limit: 1, ..Default::default() }).unwrap().total, 1000);
}

/// Every coordinate system offered in the UI's picker must work in the
/// engine. Run with --ignored (checks ~7,000 systems).
#[test]
#[ignore]
fn every_catalog_crs_is_usable() {
    let list: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/crs/crs-list.json")).unwrap()).unwrap();
    let e = Engine::new().unwrap();
    let mut bad = Vec::new();
    for row in &list {
        let id = row[0].as_str().unwrap();
        if e.validate_crs(id).is_err() {
            bad.push(format!("{id} {}", row[1].as_str().unwrap()));
        }
    }
    std::fs::write(std::env::temp_dir().join("unusable_crs.txt"), bad.join("\n")).unwrap();
    assert!(bad.is_empty(), "{} of {} unusable, e.g. {:?}", bad.len(), list.len(), &bad[..bad.len().min(8)]);
}

/// A synthetic DEM: a round hill (0–100 m) on a 200×200 grid of 30 m cells in UTM 14N.
fn write_test_dem(path: &Path) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, 200, 200, 1).unwrap();
    ds.set_geo_transform(&[600_000.0, 30.0, 0.0, 3_900_000.0, 0.0, -30.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    let mut data = vec![0f32; 200 * 200];
    for r in 0..200 {
        for c in 0..200 {
            let d = (((r as f32 - 100.0).powi(2) + (c as f32 - 100.0).powi(2)).sqrt() / 100.0).min(1.0);
            data[r * 200 + c] = 100.0 * (1.0 - d);
        }
    }
    let mut buf = gdal::raster::Buffer::new((200, 200), data);
    ds.rasterband(1).unwrap().write((0, 0), (200, 200), &mut buf).unwrap();
}

#[test]
fn rasters_contours_hillshade_slope() {
    let dir = tempfile::tempdir().unwrap();
    let dem = dir.path().join("hill.tif");
    write_test_dem(&dem);
    let e = Engine::new().unwrap();
    let l = e.import(&dem).unwrap().remove(0);
    assert_eq!(l.geom_type, "Raster");
    assert_eq!(l.crs.as_deref(), Some("EPSG:32614"));
    let r = l.raster.clone().unwrap();
    assert!(r.max > 99.0 && r.min < 1.0, "stats {} {}", r.min, r.max);

    // A map tile over the raster is a PNG.
    let ext = l.extent.unwrap();
    let (lon, lat) = ((ext[0] + ext[2]) / 2.0, (ext[1] + ext[3]) / 2.0);
    let z = 12u32;
    let n = 2f64.powi(z as i32);
    let x = ((lon + 180.0) / 360.0 * n) as u32;
    let y = ((1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI) / 2.0 * n) as u32;
    let png = e.tile(&l.id, z, x, y, &[]).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // Identify gives the elevation at the summit.
    assert!(e.raster_value(&l.id, lon, lat).unwrap().unwrap() > 90.0);

    // Contours every 10 m: 9 rings (10..90), each with an ELEV value.
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "c", "tool": "contours", "params": {"interval": 10, "base": 0, "index_every": 5}},
            {"id": "h", "tool": "hillshade", "params": {"azimuth": 315, "altitude": 45, "z": 1}},
            {"id": "s", "tool": "slope", "params": {"units": "degrees"}},
            {"id": "out", "tool": "layer_output", "params": {"layer": l.id}}
        ],
        "edges": [
            {"source": "in", "target": "c", "targetHandle": "in"},
            {"source": "in", "target": "h", "targetHandle": "in"},
            {"source": "in", "target": "s", "targetHandle": "in"},
            {"source": "h", "target": "out", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    for r in &res {
        assert!(matches!(r.status, Status::Done | Status::Cached), "{}: {:?}", r.id, r.error);
    }
    let c = res.iter().find(|r| r.id == "c").unwrap();
    assert!(c.rows.unwrap() >= 9, "contours {:?}", c.rows);
    assert_eq!(c.crs.as_deref(), Some("EPSG:32614"));
    // The Output shows the hillshade raster in place of the DEM.
    let out = res.iter().find(|r| r.id == "out").unwrap().layer.clone().unwrap();
    assert_eq!(out.geom_type, "Raster");
    assert_eq!(out.result_of.as_deref(), Some(l.id.as_str()));
    // Contours feeding a vector tool works (Buffer the contour lines).
    let g2 = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "c", "tool": "contours", "params": {"interval": 25}},
            {"id": "b", "tool": "buffer", "params": {"distance": 10, "units": "m"}}
        ],
        "edges": [
            {"source": "in", "target": "c", "targetHandle": "in"},
            {"source": "c", "target": "b", "targetHandle": "in"}
        ]
    }));
    let res2 = e.run_graph(&g2, None, &mut |_| {}).unwrap();
    assert!(res2.iter().all(|r| matches!(r.status, Status::Done | Status::Cached)), "{res2:?}");
    // A raster into a vector port is a clear error.
    let g3 = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "b", "tool": "buffer", "params": {"distance": 10}}
        ],
        "edges": [{"source": "in", "target": "b", "targetHandle": "in"}]
    }));
    let r3 = e.run_graph(&g3, None, &mut |_| {}).unwrap();
    assert!(r3[1].error.as_ref().unwrap().contains("raster"));
}

/// Needs internet (USGS 3DEP); run with --ignored.
#[test]
#[ignore]
fn download_3dep_elevation() {
    let e = Engine::new().unwrap();
    let l = e.download_elevation([-97.47, 35.17, -97.43, 35.21], 10.0, "Norman, OK").unwrap();
    let r = l.raster.unwrap();
    assert!(r.min > 300.0 && r.max < 420.0, "elevation range {} {}", r.min, r.max);
    assert!(l.crs.unwrap().contains("32614"));
}

#[test]
fn qgis_equivalent_tools_run() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let polys = e.generate_test_layer("polygons", 40, [-97.6, 35.1, -97.3, 35.3]).unwrap();
    let pts = e.generate_test_layer("points", 200, [-97.6, 35.1, -97.3, 35.3]).unwrap();
    let dem_path = dir.path().join("hill.tif");
    write_test_dem(&dem_path);
    let dem = e.import(&dem_path).unwrap().remove(0);

    let run = |tool: &str, params: serde_json::Value, a: &str, b: Option<&str>| -> i64 {
        let spec = e.tool_specs().into_iter().find(|s| s.id == tool).unwrap();
        let mut nodes = vec![json!({"id": "a", "tool": "input_layer", "params": {"layer": a}})];
        let mut edges = vec![json!({"source": "a", "target": "t", "targetHandle": spec.inputs[0].id})];
        if let Some(b) = b {
            nodes.push(json!({"id": "b", "tool": "input_layer", "params": {"layer": b}}));
            edges.push(json!({"source": "b", "target": "t", "targetHandle": spec.inputs[1].id}));
        }
        nodes.push(json!({"id": "t", "tool": tool, "params": params}));
        let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
        let r = res.into_iter().find(|r| r.id == "t").unwrap();
        assert!(matches!(r.status, Status::Done | Status::Cached), "{tool}: {:?}", r.error);
        r.rows.unwrap_or(-1)
    };
    let (p, q, d) = (polys.id.as_str(), pts.id.as_str(), dem.id.as_str());
    assert_eq!(run("polygons_to_lines", json!({}), p, None), 40);
    let lines = run("polygons_to_lines", json!({}), p, None);
    assert!(lines > 0);
    assert!(run("extract_vertices", json!({}), p, None) >= 40 * 4);
    assert_eq!(run("delete_duplicates", json!({}), q, None), 200);
    assert_eq!(run("remove_null", json!({}), q, None), 200);
    assert_eq!(run("random_points", json!({"count": 5}), p, None), 200);
    let cells = run("voronoi", json!({"buffer": 10}), q, None);
    assert!(cells >= 190, "voronoi cells {cells}");
    assert_eq!(run("concave_hull", json!({"ratio": 0.3}), q, None), 1);
    assert_eq!(run("join_nearest", json!({"radius": 50, "units": "km"}), q, Some(p)), 200);
    assert_eq!(run("points_to_path", json!({"order": "id"}), q, None), 1);
    assert!(run("points_along_lines", json!({"distance": 100, "units": "m"}), p, None) > 0, "points along polygon outlines are skipped (lines only)");
    assert!(run("fishnet", json!({"shape": "hexagon", "size": 2, "units": "km", "clip": false}), p, None) > 50);
    assert_eq!(run("buffer", json!({"distance_field": "class", "units": "m"}), q, None), 200);
    // Raster tools.
    assert_eq!(run("aspect", json!({}), d, None), -1);
    assert_eq!(run("ruggedness", json!({"mode": "TPI"}), d, None), -1);
    assert_eq!(run("warp", json!({"crs": "EPSG:4326", "resampling": "bilinear"}), d, None), -1);
    assert!(run("polygonize", json!({}), d, None) > 0);
    // Sample and zonal stats on features placed over the synthetic DEM (UTM 14N).
    let over = dir.path().join("over.gpkg");
    write_fixture(&over, "SELECT 1 AS id, ST_Point(603000, 3897000) AS geom UNION ALL SELECT 2, ST_Point(600100, 3894100)", "GPKG", Some("EPSG:32614"));
    let ov = e.import(&over).unwrap().remove(0);
    let zones = dir.path().join("zones.gpkg");
    write_fixture(&zones, "SELECT 1 AS id, ST_Buffer(ST_Point(603000, 3897000), 300) AS geom", "GPKG", Some("EPSG:32614"));
    let zl = e.import(&zones).unwrap().remove(0);
    assert_eq!(run("sample_raster", json!({"field": "ELEV"}), &ov.id, Some(d)), 2);
    assert_eq!(run("zonal_statistics", json!({"prefix": "Z_"}), &zl.id, Some(d)), 1);
    // Check sampled values: summit ~100 m, near corner ~0 m.
    let g = graph(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": ov.id}},
            {"id": "b", "tool": "input_layer", "params": {"layer": d}},
            {"id": "t", "tool": "sample_raster", "params": {"field": "ELEV"}},
            {"id": "z", "tool": "layer_output", "params": {"layer": ov.id}}
        ],
        "edges": [
            {"source": "a", "target": "t", "targetHandle": "in"},
            {"source": "b", "target": "t", "targetHandle": "raster"},
            {"source": "t", "target": "z", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "z").unwrap().layer.clone().unwrap();
    let page = e.rows(&out.id, &RowsQuery { limit: 10, sort: Some("id".into()), ..Default::default() }).unwrap();
    let idx = out.fields.iter().position(|f| f.name == "ELEV").unwrap();
    let summit: f64 = page.rows[0][idx].as_ref().unwrap().parse().unwrap();
    assert!(summit > 95.0, "summit {summit}");
}

#[test]
fn geodesic_buffer_near_and_measure() {
    let e = Engine::new().unwrap();
    // Two points ~4,000 km apart in one layer: Seattle-ish and Maine-ish.
    let pts = e
        .create_features(
            "far points",
            &[
                ("POINT(-122 47)".into(), serde_json::from_value(json!({"name": "west"})).unwrap()),
                ("POINT(-69 45)".into(), serde_json::from_value(json!({"name": "east"})).unwrap()),
            ],
        )
        .unwrap();
    assert_eq!(pts.feature_count, 2);
    let area_of = |method: &str| -> Vec<f64> {
        let g = graph(json!({
            "nodes": [
                {"id": "in", "tool": "input_layer", "params": {"layer": pts.id}},
                {"id": "b", "tool": "buffer", "params": {"distance": 100, "units": "km", "method": method}},
                {"id": "a", "tool": "calculate_geometry", "params": {"field": "A", "property": "area", "area_units": "sqkm"}},
                {"id": "o", "tool": "layer_output", "params": {"layer": pts.id}}
            ],
            "edges": [
                {"source": "in", "target": "b", "targetHandle": "in"},
                {"source": "b", "target": "a", "targetHandle": "in"},
                {"source": "a", "target": "o", "targetHandle": "in"}
            ]
        }));
        let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
        let out = res.iter().find(|r| r.id == "o").unwrap().layer.clone().unwrap();
        let page = e.rows(&out.id, &RowsQuery { limit: 10, ..Default::default() }).unwrap();
        let idx = out.fields.iter().position(|f| f.name == "A").unwrap();
        page.rows.iter().map(|r| r[idx].as_ref().unwrap().parse().unwrap()).collect()
    };
    let truth = std::f64::consts::PI * 100.0 * 100.0;
    for a in area_of("geodesic") {
        assert!((a - truth).abs() / truth < 0.005, "geodesic buffer area {a} vs {truth}");
    }
    let planar = area_of("planar");
    assert!(planar.iter().any(|a| (a - truth).abs() / truth > 0.05), "planar should distort far from the zone: {planar:?}");

    // Geodesic Near: one degree of longitude at 60°N ≈ 55.80 km on WGS 84.
    let a = e.create_features("a", &[("POINT(10 60)".into(), Default::default())]).unwrap();
    let b = e.create_features("b", &[("POINT(11 60)".into(), Default::default())]).unwrap();
    let g = graph(json!({
        "nodes": [
            {"id": "a", "tool": "input_layer", "params": {"layer": a.id}},
            {"id": "b", "tool": "input_layer", "params": {"layer": b.id}},
            {"id": "n", "tool": "near", "params": {"radius": 100, "units": "km"}},
            {"id": "o", "tool": "layer_output", "params": {"layer": a.id}}
        ],
        "edges": [
            {"source": "a", "target": "n", "targetHandle": "in"},
            {"source": "b", "target": "n", "targetHandle": "near"},
            {"source": "n", "target": "o", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "o").unwrap().layer.clone().unwrap();
    let row = e.feature(&out.id, 1).unwrap();
    let d: f64 = row.iter().find(|(k, _)| k == "NEAR_DIST").unwrap().1.as_ref().unwrap().parse().unwrap();
    assert!((d - 55.80).abs() < 0.05, "near distance {d} km");

    // Identify measurements: a 0.1° square at the equator ≈ 123.09 km² (cf. 12,308.78 km² for 1°).
    let sq = e.create_features("sq", &[("POLYGON((0 0, 0.1 0, 0.1 0.1, 0 0.1, 0 0))".into(), Default::default())]).unwrap();
    let m = e.measure_feature(&sq.id, 1).unwrap();
    assert!((m.area.unwrap() / 1e6 - 123.09).abs() < 0.05, "area {:?}", m.area);
    assert!(m.length.is_none());
}

/// Network: The National Map vector services (slow; run with --ignored).
#[test]
#[ignore]
fn national_map_vector_downloads() {
    let e = Engine::new().unwrap();
    let bbox = [-97.56, 35.44, -97.46, 35.50]; // downtown Oklahoma City
    let ids: Vec<String> = ["st_emergency", "gn_places", "roads_major", "wbd_huc12"].iter().map(|s| s.to_string()).collect();
    let r = e.tnm_import(&ids, bbox, None, true, "OKC").unwrap();
    assert!(r.failed.is_empty(), "{:?}", r.failed);
    let layers = r.layers;
    assert_eq!(layers.len(), 4);
    for l in &layers {
        assert!(l.feature_count > 0, "{} is empty", l.name);
        let [w, s, ea, n] = l.extent.unwrap();
        assert!(w >= bbox[0] - 1e-6 && s >= bbox[1] - 1e-6 && ea <= bbox[2] + 1e-6 && n <= bbox[3] + 1e-6, "{} not clipped: {:?}", l.name, l.extent);
    }
    let emergency = &layers[0];
    assert_eq!(emergency.geom_type, "Point");
    assert!(emergency.fields.iter().any(|f| f.name == "CATEGORY"));
    assert_eq!(layers[2].geom_type, "Line", "clipped highways stay lines");
    assert_eq!(layers[3].geom_type, "Polygon");
}

#[test]
fn data_sources_are_logged_with_citations() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("wells.gpkg");
    write_fixture(&p, "SELECT 1 AS id, ST_Point(-97.5, 35.4) AS geom", "GPKG", Some("EPSG:4326"));
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);

    let list = e.sources().unwrap();
    assert_eq!(list.len(), 1);
    let src = &list[0];
    assert_eq!(src.citation.kind, "Local file");
    assert!(!src.citation.complete, "a local file starts as an incomplete citation");
    assert_eq!(src.uses[0].layer_id, l.id);

    // Copies inherit the source; renames follow.
    let copy = e.copy_layer(&l.id, "Wells copy", None).unwrap();
    e.rename_layer(&l.id, "Water wells").unwrap();
    let names: Vec<String> = e.sources().unwrap()[0].uses.iter().map(|u| u.layer_name.clone()).collect();
    assert!(names.contains(&"Water wells".to_string()) && names.contains(&copy.name), "{names:?}");

    // The user's completed citation survives re-importing the same file.
    let mut c = src.citation.clone();
    c.author = "Oklahoma Water Resources Board".into();
    c.year = "2025".into();
    e.update_source(&c).unwrap();
    e.import(&p).unwrap();
    let after = &e.sources().unwrap()[0];
    assert_eq!(after.citation.author, "Oklahoma Water Resources Board");
    assert!(after.citation.complete);

    // Basemaps are logged once.
    let osm = engine::sources::for_basemap("osm").unwrap();
    e.record_source(&osm, "Basemap").unwrap();
    e.record_source(&osm, "Basemap").unwrap();
    let all = e.sources().unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.iter().find(|s| s.citation.key == "osm").unwrap().uses.len(), 1);
    assert!(engine::sources::for_tnm("nhd_flowlines").title.contains("National Hydrography Dataset"));
    assert_eq!(engine::sources::for_tiger("tracts", "Tracts", "Census2020").len(), 2, "2020 Census tracts cite TIGER and the 2020 Census counts");
    let acs = engine::sources::for_tiger("tracts", "Tracts", "ACS2023");
    assert_eq!(acs.len(), 1, "ACS-year geography has no decennial counts");
    assert_eq!((acs[0].year.as_str(), acs[0].key.as_str()), ("2023", "census-tigerweb-acs2023"));
    assert_eq!(engine::sources::for_tiger("blocks", "Blocks", "Census2010")[1].year, "2011");
}

#[test]
fn model_run_can_be_cancelled_mid_query() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    // DuckDB notices an interrupt between work units (up to ~100k rows per
    // thread), so use a big but cheap-per-row query: a planar buffer.
    let pts = e.generate_test_layer("points", 3_000_000, [-100.0, 30.0, -94.0, 36.0]).unwrap();
    let owner = pts.id.clone();
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": owner}},
            {"id": "b", "tool": "buffer", "params": {"distance": 500, "units": "m", "method": "planar"}},
            {"id": "o", "tool": "layer_output", "params": {"layer": owner}}
        ],
        "edges": [
            {"source": "in", "target": "b", "targetHandle": "in"},
            {"source": "b", "target": "o", "targetHandle": "in"}
        ]
    }));

    // Press Cancel shortly after the buffer query starts.
    let canceller = std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(400));
        // Tests run in parallel and the task list is global: pick this run by its layer name.
        let t = engine::progress::snapshot().into_iter().find(|t| t.label == "Running model — Test points (3000000)").expect("model task is running");
        assert!(t.cancellable);
        engine::progress::cancel(t.id);
        std::time::Instant::now()
    });
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let stopped = std::time::Instant::now();
    let pressed = canceller.join().unwrap();
    assert!(stopped.duration_since(pressed) < std::time::Duration::from_secs(2), "stopped {:?} after Cancel", stopped.duration_since(pressed));

    let status = |id: &str| res.iter().find(|r| r.id == id).unwrap().status.clone();
    assert_eq!(status("b"), Status::Cancelled, "the running buffer query was interrupted");
    assert_eq!(status("o"), Status::Cancelled);
    assert!(e.layer(&engine::result_id(&owner)).is_err(), "no half-made result layer");
    assert!(engine::progress::snapshot().iter().all(|t| t.label != "Running model — Test points (3000000)"), "task ended");

    // The engine (and its pooled connection) still works afterwards.
    let quick = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": owner}},
            {"id": "o", "tool": "layer_output", "params": {"layer": owner}}
        ],
        "edges": [{"source": "in", "target": "o", "targetHandle": "in"}]
    }));
    let res = e.run_graph(&quick, None, &mut |_| {}).unwrap();
    assert!(res.iter().all(|r| r.status == Status::Done || r.status == Status::Cached), "{res:?}");
    let before = res.iter().find(|r| r.id == "o").unwrap().layer.clone().unwrap();
    assert_eq!(before.feature_count, 3_000_000);

    // Cancel while the Output step is writing the layer's result: the write is
    // all-or-nothing, so the previous result survives intact.
    let canceller = std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let t = engine::progress::snapshot().into_iter().find(|t| t.label == "Running model — Test points (3000000)").expect("model task is running");
        engine::progress::cancel(t.id);
    });
    let res = e.run_graph(&quick, None, &mut |_| {}).unwrap();
    canceller.join().unwrap();
    assert_eq!(res.iter().find(|r| r.id == "o").unwrap().status, Status::Cancelled, "the result write was interrupted");
    let after = e.layer(&before.id).unwrap();
    assert_eq!((after.feature_count, after.version, after.fields.len()), (before.feature_count, before.version, before.fields.len()));
    let page = e.rows(&after.id, &RowsQuery { limit: 3, ..Default::default() }).unwrap();
    assert_eq!(page.rows.len(), 3, "the previous result is still readable");
    assert!(!e.tile_uncached(&after.id, 5, 7, 12, &[]).unwrap().is_empty(), "and still draws");
}

#[test]
fn model_results_cite_every_input_layer() {
    let dir = tempfile::tempdir().unwrap();
    let wells_p = dir.path().join("wells.gpkg");
    let county_p = dir.path().join("county.gpkg");
    write_fixture(&wells_p, "SELECT * FROM (VALUES (1, ST_Point(-97.5, 35.4)), (2, ST_Point(-90.0, 30.0))) t(id, geom)", "GPKG", Some("EPSG:4326"));
    write_fixture(&county_p, "SELECT 1 AS id, ST_GeomFromText('POLYGON((-98 35,-97 35,-97 36,-98 36,-98 35))') AS geom", "GPKG", Some("EPSG:4326"));
    let e = Engine::new().unwrap();
    let wells = e.import(&wells_p).unwrap().remove(0);
    let county = e.import(&county_p).unwrap().remove(0);
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": wells.id}},
            {"id": "c", "tool": "input_layer", "params": {"layer": county.id}},
            {"id": "clip", "tool": "clip", "params": {}},
            {"id": "o", "tool": "layer_output", "params": {"layer": wells.id}}
        ],
        "edges": [
            {"source": "in", "target": "clip", "targetHandle": "in"},
            {"source": "c", "target": "clip", "targetHandle": "clip"},
            {"source": "clip", "target": "o", "targetHandle": "in"}
        ]
    }));
    let uses_of = |e: &Engine, layer: &str| -> Vec<String> {
        let mut keys: Vec<String> = e.sources().unwrap().into_iter().filter(|s| s.uses.iter().any(|u| u.layer_id == layer)).map(|s| s.citation.title).collect();
        keys.sort();
        keys
    };
    for _ in 0..2 {
        let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
        let out = res.iter().find(|r| r.id == "o").unwrap().layer.clone().unwrap();
        assert_eq!(out.feature_count, 1, "one well inside the county");
        // Both inputs are credited for the result — once each, even after re-running.
        assert_eq!(uses_of(&e, &out.id), vec!["county.gpkg".to_string(), "wells.gpkg".to_string()]);
        let rows: usize = e.sources().unwrap().iter().map(|s| s.uses.iter().filter(|u| u.layer_id == out.id).count()).sum();
        assert_eq!(rows, 2);
    }
    // Saving the result as a new layer keeps both citations.
    let saved = e.copy_layer(&engine::result_id(&wells.id), "Wells in county", None).unwrap();
    assert_eq!(uses_of(&e, &saved.id), vec!["county.gpkg".to_string(), "wells.gpkg".to_string()]);
    let county_src = e.sources().unwrap().into_iter().find(|s| s.citation.title == "county.gpkg").unwrap();
    assert!(county_src.uses.iter().any(|u| u.layer_name.ends_with("(model result)")), "{:?}", county_src.uses);
}

#[test]
fn geodesic_buffer_is_true_distance_with_shared_projections() {
    let e = Engine::new().unwrap();
    // Points ~17 km from their snapped projection centers, mid and high latitude.
    let pts = e
        .create_features(
            "probe",
            &[
                ("POINT(-97.12 35.12)".into(), serde_json::from_value(json!({"k": "mid"})).unwrap()),
                ("POINT(25.13 69.88)".into(), serde_json::from_value(json!({"k": "north"})).unwrap()),
            ],
        )
        .unwrap();
    // Buffer → vertices → true ellipsoidal distance from each point to each vertex.
    let d = "ST_Distance_Spheroid(CASE k WHEN 'mid' THEN ST_Point(-97.12, 35.12) ELSE ST_Point(25.13, 69.88) END, geom)";
    let g = graph(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": pts.id}},
            {"id": "b", "tool": "buffer", "params": {"distance": 50, "units": "km", "method": "geodesic"}},
            {"id": "v", "tool": "extract_vertices", "params": {}},
            {"id": "d", "tool": "calculate_field", "params": {"field": "D", "expression": d}},
            {"id": "o", "tool": "layer_output", "params": {"layer": pts.id}}
        ],
        "edges": [
            {"source": "in", "target": "b", "targetHandle": "in"},
            {"source": "b", "target": "v", "targetHandle": "in"},
            {"source": "v", "target": "d", "targetHandle": "in"},
            {"source": "d", "target": "o", "targetHandle": "in"}
        ]
    }));
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "o").unwrap().layer.clone().unwrap_or_else(|| panic!("{res:?}"));
    let idx = out.fields.iter().position(|f| f.name == "D").unwrap();
    let page = e.rows(&out.id, &RowsQuery { limit: 10_000, ..Default::default() }).unwrap();
    let n = page.rows.len();
    let worst = page.rows.iter().map(|r| (r[idx].as_ref().unwrap().parse::<f64>().unwrap() - 50_000.0).abs()).fold(0.0, f64::max);
    assert!(n > 60, "{n} vertices");
    assert!(worst < 5.0, "worst vertex is {worst:.2} m off 50 km (< 0.01%)");
}

#[test]
fn catalog_inspects_datasets() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("wells.gpkg");
    write_fixture(&p, "SELECT * FROM (VALUES (1, ST_Point(-97.5, 35.4)), (2, ST_Point(-97.4, 35.5))) t(id, geom)", "GPKG", Some("EPSG:4326"));
    let d = engine::catalog::inspect(p.to_str().unwrap()).unwrap();
    assert_eq!(d.kind, "vector");
    assert_eq!(d.layers.len(), 1);
    let l = &d.layers[0];
    assert_eq!((l.features, l.geometry.as_str()), (2, "Point"));
    assert!(l.crs.as_deref().unwrap_or("").contains("EPSG:4326"), "{:?}", l.crs);
    assert!(engine::catalog::inspect(dir.path().to_str().unwrap()).is_err(), "a folder isn't a dataset");
}

#[test]
fn projects_carry_their_rasters_and_move_between_folders() {
    let dir = tempfile::tempdir().unwrap();
    let dem = dir.path().join("hill.tif");
    write_test_dem(&dem);
    let e = Engine::new().unwrap();
    let l = e.import(&dem).unwrap().remove(0);
    let proj = dir.path().join("a").join("Hill.gproj");
    e.save_project(&proj, "{}").unwrap();
    let saved = e.layer(&l.id).unwrap().raster.unwrap().path;
    assert!(Path::new(&saved).starts_with(proj.join("rasters")), "raster copied into the project: {saved}");
    assert!(Path::new(&saved).is_file());
    // Saving again doesn't copy it twice.
    e.save_project(&proj, "{}").unwrap();
    assert_eq!(std::fs::read_dir(proj.join("rasters")).unwrap().filter(|f| f.as_ref().unwrap().path().extension().is_some_and(|x| x == "tif")).count(), 1);
    drop(e);

    // Move the whole project folder (another machine, another user) and reopen it.
    let moved = dir.path().join("b").join("Hill moved.gproj");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::rename(&proj, &moved).unwrap();
    let e2 = Engine::new().unwrap();
    e2.open_project(&moved).unwrap();
    let r = e2.layer(&l.id).unwrap().raster.unwrap();
    assert!(Path::new(&r.path).starts_with(&moved), "resolved inside the moved project: {}", r.path);
    // It still reads and draws.
    assert!(e2.tile(&l.id, 8, 59, 104, &[]).is_ok());
    assert!(e2.raster_value(&l.id, -97.88, 35.22).is_ok());
}

#[test]
fn cancelling_stops_gdal_raster_steps_and_cleans_up() {
    use std::sync::{atomic::AtomicBool, Arc};
    let dir = tempfile::tempdir().unwrap();
    let dem = dir.path().join("hill.tif");
    write_test_dem(&dem);
    let src = gdal::Dataset::open(&dem).unwrap();
    let out = dir.path().join("copy.tif");
    // A cancelled run: GDAL stops at its first progress report and the partial output is removed.
    let cancelled = || Arc::new(AtomicBool::new(true));
    let err = engine::progress::with_cancel_flag(cancelled(), || engine::raster::translate(&src, &out, &[])).unwrap_err();
    assert!(err.to_string().contains("User terminated"), "{err}");
    assert!(!out.exists(), "no half-written output left behind");
    let err = engine::progress::with_cancel_flag(cancelled(), || engine::raster::warp(&src, out.to_str().unwrap(), &["-t_srs".into(), "EPSG:4326".into()]).map(|_| ())).unwrap_err();
    assert!(err.to_string().contains("User terminated"), "{err}");
    assert!(engine::progress::with_cancel_flag(cancelled(), || engine::raster::contours(&dem, 10.0, 0.0)).is_err());
    assert!(engine::progress::with_cancel_flag(cancelled(), || engine::raster::polygonize(&dem)).is_err());
    // Not cancelled, or outside any model run: they complete.
    engine::progress::with_cancel_flag(Arc::new(AtomicBool::new(false)), || engine::raster::translate(&src, &out, &[])).unwrap();
    assert!(out.exists());
    engine::raster::contours(&dem, 10.0, 0.0).unwrap();
}

/// Each tool's declared output fields (`Tool::output_fields`, which the UI
/// uses for field pickers before a run) must match what it really produces.
#[test]
fn declared_output_fields_match_real_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let polys = dir.path().join("grid.gpkg");
    write_fixture(
        &polys,
        "SELECT x*3+y AS id, CASE WHEN x < 2 THEN 'west' ELSE 'east' END AS side, ST_MakeEnvelope(-98 + x*0.25, 35 + y*0.25, -98 + (x+1)*0.25, 35 + (y+1)*0.25) AS geom FROM range(4) a(x), range(3) b(y)",
        "GPKG",
        Some("EPSG:4326"),
    );
    let e = Engine::new().unwrap();
    let p = e.import(&polys).unwrap().remove(0);
    let pts = e.generate_test_layer("points", 50, [-98.0, 35.0, -97.0, 35.75]).unwrap();
    let names = |l: &engine::layers::LayerInfo| -> Vec<String> { l.fields.iter().map(|f| f.name.clone()).collect() };
    let specs = e.tool_specs();

    let check = |tool: &str, params: serde_json::Value, inputs: &[(&str, &engine::layers::LayerInfo)]| {
        let spec = specs.iter().find(|s| s.id == tool).unwrap();
        let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
        let mut edges = vec![];
        for (i, (port, l)) in inputs.iter().enumerate() {
            nodes.push(json!({"id": format!("in{i}"), "tool": "input_layer", "params": {"layer": l.id}}));
            edges.push(json!({"source": format!("in{i}"), "target": "t", "targetHandle": port}));
        }
        let res = e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
        let r = res.into_iter().find(|r| r.id == "t").unwrap();
        assert_eq!(r.status, Status::Done, "{tool}: {:?}", r.error);
        let out = e.preview_output(r.output.as_ref().unwrap(), r.crs.clone(), "check").unwrap();
        let actual = names(&out);

        // Evaluate the declared rule the way the UI does.
        let f = &spec.fields;
        let param = |k: &str| params.get(k).cloned().unwrap_or(serde_json::Value::Null);
        let mut expected: Vec<String> = Vec::new();
        if !f.none_if.is_some_and(|k| param(k).as_bool() == Some(true)) {
            let ports: Vec<&str> = f.from.clone().unwrap_or_else(|| vec![spec.inputs[0].id]);
            for port in ports {
                let l = inputs.iter().find(|(p, _)| *p == port).map(|(_, l)| l).unwrap();
                expected.extend(names(l).into_iter().filter(|n| f.keep_param.map_or(true, |k| param(k).as_str() == Some(n))));
            }
            for nf in &f.add {
                if nf.when_positive.is_some_and(|k| param(k).as_f64().unwrap_or(0.0) <= 0.0) {
                    continue;
                }
                let base = nf.name_param.and_then(|k| param(k).as_str().map(String::from)).filter(|s| !s.is_empty()).unwrap_or_else(|| nf.name.to_string());
                let name = match nf.prefix_param {
                    Some((k, d)) => format!("{}{base}", param(k).as_str().unwrap_or(d)),
                    None => base,
                };
                if !name.is_empty() {
                    expected.push(name);
                }
            }
        }
        for name in &expected {
            assert!(actual.iter().any(|a| a.eq_ignore_ascii_case(name)), "{tool}: declares field {name}, output has {actual:?}");
        }
        if f.none_if.is_some_and(|k| param(k).as_bool() == Some(true)) {
            assert!(actual.is_empty(), "{tool}: declares no fields, output has {actual:?}");
        }
    };

    check("near", json!({"distance": 100, "units": "km"}), &[("in", &pts), ("near", &p)]);
    check("join_nearest", json!({}), &[("in", &pts), ("near", &p)]);
    check("summarize_within", json!({"field": "value", "stat": "sum"}), &[("polys", &p), ("items", &pts)]);
    check("extract_vertices", json!({}), &[("in", &p)]);
    check("fishnet", json!({"size": 20, "units": "km", "clip": true}), &[("in", &p)]);
    check("dissolve", json!({"field": "side"}), &[("in", &p)]);
    check("calculate_field", json!({"field": "TWICE", "expression": "id * 2"}), &[("in", &p)]);
    check("calculate_geometry", json!({"field": "AREA_KM2", "property": "area", "area_units": "sqkm"}), &[("in", &p)]);
    check("spatial_join", json!({}), &[("target", &pts), ("join", &p)]);
    check("intersect", json!({}), &[("a", &p), ("b", &p)]);
    check("buffer", json!({"distance": 1, "units": "km"}), &[("in", &pts)]);
    check("buffer", json!({"distance": 1, "units": "km", "dissolve": true}), &[("in", &pts)]);
}

#[test]
fn select_by_location_on_the_map() {
    let dir = tempfile::tempdir().unwrap();
    let grid = dir.path().join("grid.gpkg");
    // Two side-by-side squares, west and east, in Web Mercator (a different CRS from the points).
    write_fixture(
        &grid,
        "SELECT * FROM (VALUES ('west', ST_Transform(ST_MakeEnvelope(-98, 35, -97.5, 35.5), 'EPSG:4326', 'EPSG:3857', true)),
                               ('east', ST_Transform(ST_MakeEnvelope(-97.5, 35, -97, 35.5), 'EPSG:4326', 'EPSG:3857', true))) t(side, geom)",
        "GPKG",
        Some("EPSG:3857"),
    );
    let e = Engine::new().unwrap();
    let polys = e.import(&grid).unwrap().remove(0);
    let pts = e
        .create_features(
            "pts",
            &[
                ("POINT(-97.9 35.2)".into(), serde_json::from_value(json!({"k": "in-west"})).unwrap()),
                ("POINT(-97.2 35.2)".into(), serde_json::from_value(json!({"k": "in-east"})).unwrap()),
                ("POINT(-97.9 35.52)".into(), serde_json::from_value(json!({"k": "2km-north-of-west"})).unwrap()),
                ("POINT(-99.5 35.2)".into(), serde_json::from_value(json!({"k": "far"})).unwrap()),
            ],
        )
        .unwrap();
    let kidx = pts.fields.iter().position(|f| f.name == "k").unwrap();
    let keys = |fids: Vec<i64>| -> Vec<String> {
        let rows = e.rows(&pts.id, &RowsQuery { limit: 100, ..Default::default() }).unwrap();
        let mut out: Vec<String> = rows.rows.iter().zip(&rows.fids).filter(|(_, f)| fids.contains(f)).map(|(r, _)| r[kidx].clone().unwrap()).collect();
        out.sort();
        out
    };
    let sel = |by_fids: Option<&[i64]>, rel: &str, dist: f64, invert: bool| keys(e.select_by_location(&pts.id, &polys.id, by_fids, rel, dist, "km", invert).unwrap());

    assert_eq!(sel(None, "intersects", 0.0, false), vec!["in-east", "in-west"]);
    // Only the selected polygon (west).
    let west = e.select_where(&polys.id, "side = 'west'").unwrap();
    assert_eq!(sel(Some(&west), "within", 0.0, false), vec!["in-west"]);
    // Geodesic distance: the point ~2 km north of the west square is within 5 km, not 1 km.
    assert_eq!(sel(Some(&west), "distance", 5.0, false), vec!["2km-north-of-west", "in-west"]);
    assert_eq!(sel(Some(&west), "distance", 1.0, false), vec!["in-west"]);
    // Invert.
    assert_eq!(sel(None, "intersects", 0.0, true), vec!["2km-north-of-west", "far"]);
    // The target layer's definition query applies.
    e.set_definition_query(&pts.id, Some("k <> 'in-east'")).unwrap();
    assert_eq!(sel(None, "intersects", 0.0, false), vec!["in-west"]);
    // Selecting with an empty selection is a clear error.
    assert!(e.select_by_location(&pts.id, &polys.id, Some(&[]), "intersects", 0.0, "m", false).unwrap_err().to_string().contains("no selected features"));
}
