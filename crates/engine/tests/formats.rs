//! Import and export of every supported format, and what happens with bad
//! files (docs/TEST_PLAN.md §Import/Export).

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;
use std::path::Path;

fn duck() -> engine::duckdb::Connection {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c
}

fn write(path: &Path, select: &str, driver: &str, srs: Option<&str>) {
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    duck().execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

const SQUARES: &str = "SELECT 1 AS id, 'Ålesund' AS name, 2.5 AS val, ST_MakeEnvelope(-98, 35, -97.9, 35.1) AS geom
                       UNION ALL SELECT 2, 'b', NULL, ST_MakeEnvelope(-97.9, 35, -97.8, 35.1)";

fn export(e: &Engine, layer: &str, path: &Path, format: &str) -> engine::graph::NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "i", "tool": "input_layer", "params": {"layer": layer}},
            {"id": "x", "tool": "export", "params": {"path": path.to_str().unwrap(), "format": format}}
        ],
        "edges": [{"source": "i", "target": "x", "targetHandle": "in"}]
    }))
    .unwrap();
    e.run_graph(&g, None, &mut |_| {}).unwrap().into_iter().find(|r| r.id == "x").unwrap()
}

// ------------------------------------------------------------------ import

#[test]
fn each_vector_format_imports_with_crs_geometry_and_citation() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    for (file, driver) in [("a.geojson", "GeoJSON"), ("a.gpkg", "GPKG"), ("a.shp", "ESRI Shapefile"), ("a.fgb", "FlatGeobuf")] {
        let p = dir.path().join(file);
        write(&p, SQUARES, driver, Some("EPSG:4326"));
        let l = e.import(&p).unwrap_or_else(|err| panic!("{file}: {err}")).remove(0);
        assert_eq!((l.geom_type.as_str(), l.feature_count), ("Polygon", 2), "{file}");
        assert_eq!(l.crs.as_deref(), Some("EPSG:4326"), "{file}");
        assert!(l.warnings.is_empty(), "{file}: {:?}", l.warnings);
        assert!(l.fields.iter().any(|f| f.name == "name"), "{file}");
        let ext = l.extent.unwrap();
        assert!((ext[0] + 98.0).abs() < 1e-6 && (ext[3] - 35.1).abs() < 1e-6, "{file}: {ext:?}");
    }
    let srcs = e.sources().unwrap();
    assert_eq!(srcs.len(), 4, "one citation per imported file");
    assert!(srcs.iter().all(|s| s.citation.kind == "Local file" && !s.uses.is_empty()));
}

#[test]
fn non_ascii_attribute_values_and_names_survive_import_and_editing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("u.gpkg");
    write(&p, "SELECT 1 AS id, 'Ålesund – 東京 ✓' AS \"nombre año\", ST_Point(5, 60) AS geom", "GPKG", Some("EPSG:4326"));
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);
    assert!(l.fields.iter().any(|f| f.name == "nombre año"));
    let row = e.feature(&l.id, 1).unwrap();
    assert_eq!(row.iter().find(|(n, _)| n == "nombre año").unwrap().1.as_deref(), Some("Ålesund – 東京 ✓"));
    let l = e.update_cell(&l.id, 1, "nombre año", Some("Zürich")).unwrap();
    assert_eq!(e.feature(&l.id, 1).unwrap().iter().find(|(n, _)| n == "nombre año").unwrap().1.as_deref(), Some("Zürich"));
}

#[test]
fn multi_layer_geopackage_gives_one_layer_each_and_mixed_geometry_is_labelled() {
    use gdal::spatial_ref::SpatialRef;
    use gdal::vector::{Geometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("multi.gpkg");
    {
        let mut ds = gdal::DriverManager::get_driver_by_name("GPKG").unwrap().create_vector_only(&p).unwrap();
        let srs = SpatialRef::from_epsg(4326).unwrap();
        for (name, ty, wkt) in [("wells", OGRwkbGeometryType::wkbPoint, "POINT (1 1)"), ("roads", OGRwkbGeometryType::wkbLineString, "LINESTRING (0 0, 1 1)")] {
            let mut layer = ds.create_layer(LayerOptions { name, srs: Some(&srs), ty, ..Default::default() }).unwrap();
            layer.create_feature(Geometry::from_wkt(wkt).unwrap()).unwrap();
        }
    }
    let e = Engine::new().unwrap();
    let layers = e.import(&p).unwrap();
    let mut got: Vec<(String, String)> = layers.iter().map(|l| (l.name.clone(), l.geom_type.clone())).collect();
    got.sort();
    assert_eq!(got, [("roads".to_string(), "Line".to_string()), ("wells".to_string(), "Point".to_string())]);
    assert!(layers.iter().all(|l| l.crs.as_deref() == Some("EPSG:4326")));
    assert_eq!(e.sources().unwrap().len(), 1, "one file, one citation, used by both layers");
    assert_eq!(e.sources().unwrap()[0].uses.len(), 2);

    let mixed = dir.path().join("mixed.geojson");
    std::fs::write(
        &mixed,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"id":1},"geometry":{"type":"Point","coordinates":[1,1]}},
            {"type":"Feature","properties":{"id":2},"geometry":{"type":"LineString","coordinates":[[0,0],[1,1]]}}]}"#,
    )
    .unwrap();
    let m = e.import(&mixed).unwrap().remove(0);
    assert_eq!(m.geom_type, "Mixed");
    assert_eq!(m.feature_count, 2);
}

#[test]
fn csv_with_coordinates_becomes_points_and_says_which_datum_it_assumed() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let ll = dir.path().join("wells.csv");
    std::fs::write(&ll, "id,Latitude,Longitude,depth\n1,35.2,-97.5,100\n2,35.3,-97.4,200\n").unwrap();
    let l = e.import(&ll).unwrap().remove(0);
    assert_eq!((l.geom_type.as_str(), l.crs.as_deref(), l.feature_count), ("Point", Some("EPSG:4326"), 2));
    assert!(l.warnings.iter().any(|w| w.contains("assumed")), "the WGS 84 assumption is stated: {:?}", l.warnings);

    let xy = dir.path().join("grid.csv");
    std::fs::write(&xy, "id,x,y\n1,636548.8,3896253.1\n").unwrap();
    let l = e.import(&xy).unwrap().remove(0);
    assert_eq!((l.geom_type.as_str(), l.crs.as_deref()), ("Point", None), "x/y without a CRS stay undefined");
    assert!(!l.warnings.is_empty());

    let plain = dir.path().join("plain.csv");
    std::fs::write(&plain, "name,count\nfoo,1\nbar,2\n").unwrap();
    let t = e.import(&plain).unwrap().remove(0);
    assert_eq!((t.geom_type.as_str(), t.feature_count), ("Table", 2));
    assert!(t.crs.is_none() && t.extent.is_none());

    let tsv = dir.path().join("tabs.tsv");
    std::fs::write(&tsv, "name\tlat\tlon\nx\t35\t-97\n").unwrap();
    assert_eq!(e.import(&tsv).unwrap().remove(0).geom_type, "Point");
}

#[test]
fn geoparquet_round_trip_keeps_geometry_and_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let src = dir.path().join("src.gpkg");
    write(&src, SQUARES, "GPKG", Some("EPSG:26914"));
    let l = e.import(&src).unwrap().remove(0);
    let out = dir.path().join("out.parquet");
    let r = export(&e, &l.id, &out, "PARQUET");
    assert_eq!(r.status, Status::Done, "{:?}", r.error);
    let back = e.import(&out).unwrap().remove(0);
    assert_eq!((back.geom_type.as_str(), back.feature_count), ("Polygon", 2));
    assert_eq!(back.crs.as_deref().map(|c| c.contains("26914")), Some(true), "parquet carries the CRS: {:?}", back.crs);
}

#[test]
fn bad_files_are_reported_not_crashed_on() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    assert!(e.import(&dir.path().join("missing.shp")).is_err());
    for (name, body) in [("empty.geojson", ""), ("junk.geojson", "not json at all"), ("junk.shp", "garbage"), ("junk.gpkg", "garbage"), ("truncated.geojson", r#"{"type":"FeatureCollection","features":[{"type""#)] {
        let p = dir.path().join(name);
        std::fs::write(&p, body).unwrap();
        assert!(e.import(&p).is_err(), "{name} must be rejected");
    }
    let unknown = dir.path().join("notes.docx");
    std::fs::write(&unknown, "x").unwrap();
    assert!(e.import(&unknown).is_err());
    assert!(e.list_layers().is_empty(), "failed imports leave no layers behind");
    assert!(e.sources().unwrap().is_empty(), "and no citations");
    // The engine still works.
    let ok = dir.path().join("ok.geojson");
    write(&ok, SQUARES, "GeoJSON", Some("EPSG:4326"));
    assert_eq!(e.import(&ok).unwrap().len(), 1);
}

#[test]
fn zero_feature_files_import_as_empty_layers() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("none.geojson");
    std::fs::write(&p, r#"{"type":"FeatureCollection","features":[]}"#).unwrap();
    let e = Engine::new().unwrap();
    match e.import(&p) {
        Ok(mut l) => assert_eq!(l.remove(0).feature_count, 0),
        Err(err) => assert!(!err.to_string().is_empty(), "an empty file may be refused, but with a message"),
    }
}

#[test]
fn features_with_null_geometry_import_and_are_counted() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("nullgeom.geojson");
    std::fs::write(
        &p,
        r#"{"type":"FeatureCollection","features":[
            {"type":"Feature","properties":{"id":1},"geometry":{"type":"Point","coordinates":[1,1]}},
            {"type":"Feature","properties":{"id":2},"geometry":null}]}"#,
    )
    .unwrap();
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);
    assert_eq!(l.feature_count, 2, "the attribute-only feature is kept");
    assert_eq!(l.geom_type, "Point");
    let ext = l.extent.unwrap();
    assert!((ext[0] - 1.0).abs() < 1e-9, "extent ignores the null geometry: {ext:?}");
}

// ------------------------------------------------------------------ export

#[test]
fn every_export_format_writes_a_file_that_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let src = dir.path().join("src.gpkg");
    write(&src, "SELECT 1 AS id, 'Ålesund' AS name, ST_MakeEnvelope(600000, 3900000, 600100, 3900100) AS geom UNION ALL SELECT 2, 'b', ST_MakeEnvelope(600100, 3900000, 600200, 3900100)", "GPKG", Some("EPSG:26914"));
    let l = e.import(&src).unwrap().remove(0);
    for (format, file) in [("GPKG", "o.gpkg"), ("ESRI Shapefile", "o.shp"), ("GeoJSON", "o.geojson"), ("FlatGeobuf", "o.fgb"), ("PARQUET", "o.parquet")] {
        let out = dir.path().join(file);
        let r = export(&e, &l.id, &out, format);
        assert_eq!(r.status, Status::Done, "{format}: {:?}", r.error);
        let e2 = Engine::new().unwrap();
        let back = e2.import(&out).unwrap_or_else(|err| panic!("{format}: {err}")).remove(0);
        assert_eq!(back.feature_count, 2, "{format}");
        // GeoJSON is always written in WGS 84 by GDAL's driver; every other format keeps the CRS.
        if format != "GeoJSON" {
            assert!(back.crs.as_deref().is_some_and(|c| c.contains("26914")), "{format} CRS {:?}", back.crs);
        } else {
            assert!(back.crs.is_some());
        }
        let ext = back.extent.unwrap();
        assert!(ext[0] > -98.0 && ext[2] < -97.8 && ext[1] > 35.2 && ext[3] < 35.3, "{format} extent {ext:?}: coordinates must not have moved");
    }
    // CSV: attributes plus WKT.
    let csv = dir.path().join("o.csv");
    assert_eq!(export(&e, &l.id, &csv, "CSV").status, Status::Done);
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.lines().next().unwrap().contains("geom") && text.contains("POLYGON"), "{text}");
    assert_eq!(text.lines().count(), 3);
}

#[test]
fn exporting_a_table_without_geometry_works_and_missing_folders_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let csv = dir.path().join("t.csv");
    std::fs::write(&csv, "a,b\n1,x\n2,y\n").unwrap();
    let t = e.import(&csv).unwrap().remove(0);
    let out = dir.path().join("t_out.csv");
    assert_eq!(export(&e, &t.id, &out, "CSV").status, Status::Done);
    assert_eq!(std::fs::read_to_string(&out).unwrap().lines().count(), 3);
    let r = export(&e, &t.id, &dir.path().join("no/such/folder/x.csv"), "CSV");
    assert_eq!(r.status, Status::Error);
    assert!(r.error.is_some_and(|m| !m.is_empty()));
}

#[test]
fn exporting_never_overwrites_the_layers_own_source_file() {
    // CLAUDE.md: "never overwrite source data". Exporting a layer onto the file it was imported from must not damage that file.
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("src.gpkg");
    write(&p, SQUARES, "GPKG", Some("EPSG:4326"));
    let before = std::fs::read(&p).unwrap();
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);
    e.set_definition_query(&l.id, Some("id = 1")).unwrap();
    let r = export(&e, &l.id, &p, "GPKG");
    let after = std::fs::read(&p).unwrap();
    assert_eq!(r.status, Status::Error, "export onto the source file must be refused");
    assert!(r.error.is_some_and(|m| m.contains("source file")));
    assert_eq!(after, before, "the source file is untouched");
    // A relative spelling of the same path is refused too.
    let alt = dir.path().join(".").join("src.gpkg");
    assert_eq!(export(&e, &l.id, &alt, "GPKG").status, Status::Error);
}
