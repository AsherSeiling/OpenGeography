//! Project save/open round trip: layers, rasters, edits, definition queries,
//! node trees' cached results, layouts, sources and the UI state string
//! (docs/TEST_PLAN.md §Project).

use engine::graph::{Graph, Status};
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{srs}', USE_TMP_FILE false)", path.display())).unwrap();
}

fn write_dem(path: &Path) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, 20, 20, 1).unwrap();
    ds.set_geo_transform(&[600_000.0, 30.0, 0.0, 3_900_600.0, 0.0, -30.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    let data: Vec<f32> = (0..400).map(|i| i as f32).collect();
    let mut buf = gdal::raster::Buffer::new((20, 20), data);
    ds.rasterband(1).unwrap().write((0, 0), (20, 20), &mut buf).unwrap();
}

fn all_rows(e: &Engine, id: &str) -> Vec<Vec<Option<String>>> {
    let n = e.layer(id).unwrap().feature_count.max(1) as usize;
    e.rows(id, &RowsQuery { limit: n, ..Default::default() }).unwrap().rows
}

fn layer_json(e: &Engine, id: &str) -> Value {
    serde_json::to_value(e.layer(id).unwrap()).unwrap()
}

fn tree(layer: &str) -> Graph {
    serde_json::from_value(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": layer}},
            {"id": "f", "tool": "filter", "params": {"where": "pop > 10"}},
            {"id": "b", "tool": "buffer", "params": {"distance": 5, "units": "m", "method": "planar"}},
            {"id": "o", "tool": "layer_output", "params": {"layer": layer}}
        ],
        "edges": [
            {"source": "in", "target": "f", "targetHandle": "in"},
            {"source": "f", "target": "b", "targetHandle": "in"},
            {"source": "b", "target": "o", "targetHandle": "in"}
        ]
    }))
    .unwrap()
}

fn layout() -> engine::layout::model::Layout {
    serde_json::from_value(json!({
        "id": "L1", "name": "Round trip ✓", "version": 1, "body": "earth",
        "page": { "width": 279.4, "height": 215.9, "units": "mm", "margin": 12.7, "background": "#ffffff" },
        "elements": [
            { "kind": "map_frame", "id": "f1", "x": 12.7, "y": 30.0, "w": 200.0, "h": 150.0, "crs": "EPSG:32614",
              "center": [600500.0, 3900500.0], "scale": 5000, "layers": "map", "border": { "width": 0.35, "color": "#000000" } },
            { "kind": "text", "id": "t1", "x": 12.7, "y": 10.0, "w": 250.0, "h": 14.0, "text": "Ünïcode {layout_name}", "font": "sans-bold", "size": 18, "align": "center", "color": "#000000" }
        ]
    }))
    .unwrap()
}

#[test]
fn everything_survives_save_close_open() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("demo.gproj");
    let polys = dir.path().join("polys.gpkg");
    write_fixture(
        &polys,
        "SELECT 1 AS id, 'a' AS name, 10 AS pop, ST_MakeEnvelope(600000, 3900000, 600100, 3900100) AS geom
         UNION ALL SELECT 2, 'b', 20, ST_MakeEnvelope(600100, 3900000, 600200, 3900100)
         UNION ALL SELECT 3, 'a', 30, ST_MakeEnvelope(600000, 3900100, 600100, 3900200)",
        "EPSG:32614",
    );
    let dem = dir.path().join("dem.tif");
    write_dem(&dem);
    let csv = dir.path().join("lookup.csv");
    std::fs::write(&csv, "name,label\na,Alpha\nb,Beta\n").unwrap();

    let e = Engine::new().unwrap();
    let p = e.import(&polys).unwrap().remove(0);
    let r = e.import(&dem).unwrap().remove(0);
    let t = e.import(&csv).unwrap().remove(0);

    // Edits that must persist: a new field with a value, a rename, a definition query, a deleted row.
    e.add_field(&p.id, "NOTE", "text").unwrap();
    e.update_cell(&p.id, 2, "NOTE", Some("checked ✓")).unwrap();
    e.rename_layer(&p.id, "Parcels").unwrap();
    e.set_definition_query(&p.id, Some("pop >= 10")).unwrap();
    let copy = e.copy_layer(&p.id, "Parcels copy", Some(&[1, 2])).unwrap();

    // Sources: the user completes a citation.
    let mut c = e.sources().unwrap().into_iter().find(|s| s.uses.iter().any(|u| u.layer_id == p.id)).unwrap().citation;
    c.author = "County Assessor".into();
    c.year = "2025".into();
    e.update_source(&c).unwrap();

    // A node tree runs (snapshots persist), a layout is saved.
    let g = tree(&p.id);
    let first = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert!(first.iter().all(|x| x.status == Status::Done), "{first:?}");
    e.save_layout(&layout()).unwrap();

    let ui = r#"{"trees":{"x":{"nodes":[{"id":"n1","data":{"params":{"text":"naïve ✓ \"quoted\" \\ back"}}}]}},"mapCrs":{"id":"EPSG:32614"},"empty":[],"big":1e21}"#;
    let before: Vec<Value> = [&p.id, &r.id, &t.id, &copy.id].iter().map(|id| layer_json(&e, id)).collect();
    let rows_before: Vec<_> = [&p.id, &copy.id, &t.id].iter().map(|id| all_rows(&e, id)).collect();
    let sources_before = serde_json::to_value(e.sources().unwrap()).unwrap();
    let raster_before = e.raster_value(&r.id, e.transform_point(600015.0, 3900585.0, "EPSG:32614", "EPSG:4326").unwrap().0, e.transform_point(600015.0, 3900585.0, "EPSG:32614", "EPSG:4326").unwrap().1).unwrap();

    e.save_project(&proj, ui).unwrap();
    drop(e);

    let e2 = Engine::new().unwrap();
    let ui_back = e2.open_project(&proj).unwrap();
    assert_eq!(ui_back, ui, "UI state comes back byte for byte");
    assert_eq!(e2.project_dir().as_deref(), Some(proj.as_path()));

    // Layers: same ids, names, CRS, fields, counts, definition query.
    for (id, was) in [&p.id, &r.id, &t.id, &copy.id].iter().zip(&before) {
        let now = layer_json(&e2, id);
        for key in ["id", "name", "geomType", "crs", "featureCount", "fields", "definitionQuery", "queryCount"] {
            assert_eq!(now[key], was[key], "{id}.{key}");
        }
    }
    assert_eq!(e2.layer(&p.id).unwrap().name, "Parcels");
    assert_eq!(e2.layer(&p.id).unwrap().definition_query.as_deref(), Some("pop >= 10"));
    for ((id, was), name) in [&p.id, &copy.id, &t.id].iter().zip(&rows_before).zip(["polys", "copy", "csv"]) {
        assert_eq!(&all_rows(&e2, id), was, "{name} rows");
    }
    // The raster moved into the project and reads the same.
    let rinfo = e2.layer(&r.id).unwrap();
    assert!(Path::new(&rinfo.raster.as_ref().unwrap().path).starts_with(&proj), "raster copied inside the project");
    let (lon, lat) = e2.transform_point(600015.0, 3900585.0, "EPSG:32614", "EPSG:4326").unwrap();
    assert_eq!(e2.raster_value(&r.id, lon, lat).unwrap(), raster_before);
    // Tiles still draw.
    assert!(!e2.tile(&p.id, 0, 0, 0, &[]).unwrap().is_empty());
    assert!(!e2.tile(&copy.id, 0, 0, 0, &[]).unwrap().is_empty());
    // Sources, including the completed citation and the layer links.
    assert_eq!(serde_json::to_value(e2.sources().unwrap()).unwrap(), sources_before);
    assert_eq!(e2.sources().unwrap().iter().find(|s| s.citation.author == "County Assessor").map(|s| s.citation.complete), Some(true));
    // Layouts.
    assert_eq!(e2.list_layouts().unwrap(), vec![layout()]);
    // The node cache: every step is a hit and gives the same numbers.
    let again = e2.run_graph(&g, None, &mut |_| {}).unwrap();
    for (a, b) in first.iter().zip(&again) {
        // The Output step rewrites its layer's result every run; the steps before it are cache hits.
        let want = if a.id == "o" { Status::Done } else { Status::Cached };
        assert_eq!(b.status, want, "{}: {again:?}", a.id);
        assert_eq!((a.id.as_str(), a.rows), (b.id.as_str(), b.rows));
        assert_eq!(a.output, b.output, "same snapshot table names");
    }
    assert_eq!(again.iter().find(|x| x.id == "o").unwrap().layer.as_ref().unwrap().feature_count, 2);

    // Edits after reopening save in place (same folder) and survive another reopen.
    e2.update_cell(&p.id, 1, "NOTE", Some("second session")).unwrap();
    e2.save_project(&proj, "{}").unwrap();
    drop(e2);
    let e3 = Engine::new().unwrap();
    assert_eq!(e3.open_project(&proj).unwrap(), "{}");
    let note = e3.feature(&p.id, 1).unwrap().into_iter().find(|(n, _)| n == "NOTE").unwrap().1;
    assert_eq!(note.as_deref(), Some("second session"));
    assert_eq!(e3.feature(&p.id, 2).unwrap().into_iter().find(|(n, _)| n == "NOTE").unwrap().1.as_deref(), Some("checked ✓"));
}

#[test]
fn an_empty_project_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("empty.gproj");
    let e = Engine::new().unwrap();
    e.save_project(&proj, "{}").unwrap();
    drop(e);
    let e2 = Engine::new().unwrap();
    assert_eq!(e2.open_project(&proj).unwrap(), "{}");
    assert!(e2.list_layers().is_empty() && e2.sources().unwrap().is_empty() && e2.list_layouts().unwrap().is_empty());
}

#[test]
fn opening_a_bad_folder_fails_cleanly_and_keeps_the_current_project() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 10, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    for bad in [dir.path().join("does-not-exist.gproj"), dir.path().to_path_buf()] {
        let err = e.open_project(&bad).expect_err("not a project");
        assert!(err.to_string().contains("not a project folder"), "{err}");
    }
    assert!(e.project_dir().is_none(), "a failed open must not set the project folder");
    assert_eq!(e.layer(&l.id).unwrap().feature_count, 10, "the open session is untouched");

    // A corrupt database is rejected without taking the session down either.
    let broken = dir.path().join("broken.gproj");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("project.duckdb"), b"this is not a duckdb file").unwrap();
    assert!(e.open_project(&broken).is_err());
    assert!(e.project_dir().is_none());
    assert_eq!(e.layer(&l.id).unwrap().feature_count, 10);
}

#[test]
fn save_overwrites_a_previous_project_in_the_folder_and_new_project_starts_clean() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("p.gproj");
    let e = Engine::new().unwrap();
    e.generate_test_layer("points", 5, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e.save_project(&proj, "{\"v\":1}").unwrap();
    drop(e);
    // A different session saves over the folder (Save As onto an existing project).
    let e2 = Engine::new().unwrap();
    let l = e2.generate_test_layer("points", 7, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e2.save_project(&proj, "{\"v\":2}").unwrap();
    drop(e2);
    let e3 = Engine::new().unwrap();
    assert_eq!(e3.open_project(&proj).unwrap(), "{\"v\":2}");
    assert_eq!(e3.list_layers().len(), 1, "the earlier project's layer is gone");
    assert_eq!(e3.list_layers()[0].feature_count, 7);
    let _ = l;
    e3.new_project().unwrap();
    assert!(e3.list_layers().is_empty() && e3.project_dir().is_none());
}

#[test]
fn layer_ids_keep_increasing_after_reopening_so_they_never_collide() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("ids.gproj");
    let e = Engine::new().unwrap();
    let a = e.generate_test_layer("points", 3, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let b = e.generate_test_layer("points", 3, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e.save_project(&proj, "{}").unwrap();
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    let c = e2.generate_test_layer("points", 3, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let ids = [a.id, b.id, c.id];
    assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 3, "{ids:?}");
}
