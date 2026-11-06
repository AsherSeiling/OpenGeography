//! UR-15: Export Features formats (KML, GPX, DXF, FlatGeobuf, GeoParquet, CSV with WKT) and options (output CRS, fields, clip).

use engine::graph::{NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn write(path: &Path, select: &str, srs: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{srs}', USE_TMP_FILE false)", path.display())).unwrap();
}

struct Fx {
    e: Engine,
    pts: String,
    lines: String,
    polys: String,
    dir: tempfile::TempDir,
}

const X: f64 = 500_000.0;
const Y: f64 = 4_649_776.0;

fn fx() -> Fx {
    // DXF needs GDAL's data folder (the app sets GDAL_DATA to its own); tests borrow Homebrew's or the one named by GDAL_DATA_SRC.
    if std::env::var("GDAL_DATA").is_err() {
        for d in [std::env::var("GDAL_DATA_SRC").unwrap_or_default(), "/opt/homebrew/share/gdal".into(), "/usr/share/gdal".into(), "/usr/local/share/gdal".into()] {
            if std::path::Path::new(&d).join("header.dxf").exists() {
                std::env::set_var("GDAL_DATA", d);
                break;
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("pts.gpkg");
    write(&p, &format!("SELECT x AS id, 'p' || x AS name, CAST(x AS DOUBLE) * 1.5 AS height, 'a fairly long text ' || x AS population_total, x AS population_density, ST_Point({X} + x * 1000, {Y} + x * 1000) AS geom FROM range(5) t(x)"), "EPSG:26915");
    let l = dir.path().join("lines.gpkg");
    write(&l, &format!("SELECT 1 AS id, 'road' AS name, ST_GeomFromText('LINESTRING({X} {Y}, {} {}, {} {})') AS geom", X + 2000.0, Y + 500.0, X + 4000.0, Y + 4000.0), "EPSG:26915");
    let g = dir.path().join("polys.gpkg");
    write(&g, &format!("SELECT 1 AS id, 'west' AS name, ST_MakeEnvelope({X}, {Y}, {}, {}) AS geom UNION ALL SELECT 2, 'east', ST_MakeEnvelope({}, {Y}, {}, {})", X + 2000.0, Y + 2000.0, X + 3000.0, X + 6000.0, Y + 2000.0), "EPSG:26915");
    let id = |p: &Path| e.import(p).unwrap().remove(0).id;
    let (pts, lines, polys) = (id(&p), id(&l), id(&g));
    Fx { e, pts, lines, polys, dir }
}

fn export(fx: &Fx, layer: &str, file: &str, format: &str, extra: Value) -> (NodeResult, std::path::PathBuf) {
    let path = fx.dir.path().join(file);
    let mut params = json!({"path": path.to_str().unwrap(), "format": format});
    params.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let g: engine::graph::Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "out", "tool": "export", "params": params}],
        "edges": [{"source": "in", "target": "out", "targetHandle": "in"}]
    }))
    .unwrap();
    let r = fx.e.run_graph(&g, Some("out"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "out").unwrap();
    (r, path)
}

fn done(r: &NodeResult) -> String {
    assert!(matches!(r.status, Status::Done), "{:?}", r.error);
    r.message.clone().unwrap_or_default()
}

#[test]
fn every_new_format_writes_and_reads_back() {
    let fx = fx();
    for (layer, name, kind) in [(&fx.pts, "points", "Point"), (&fx.lines, "lines", "Line")] {
        for (fmt, ext) in [("KML", "kml"), ("GPX", "gpx"), ("DXF", "dxf"), ("FlatGeobuf", "fgb"), ("PARQUET", "parquet")] {
            let (r, path) = export(&fx, layer, &format!("{name}.{ext}"), fmt, json!({}));
            done(&r);
            let back = fx.e.import(&path).unwrap_or_else(|er| panic!("{fmt} {name}: {er:#}"));
            let total: i64 = back.iter().map(|b| b.feature_count).sum();
            assert!(total >= if name == "points" { 5 } else { 1 }, "{fmt} {name}: {total} features back");
            assert!(back.iter().any(|b| b.geom_type == kind || b.geom_type == "Mixed"), "{fmt} {name}: {:?}", back.iter().map(|b| &b.geom_type).collect::<Vec<_>>());
        }
    }
    // Polygons go to everything but GPX.
    for (fmt, ext) in [("KML", "kml"), ("DXF", "dxf"), ("FlatGeobuf", "fgb"), ("PARQUET", "parquet")] {
        let (r, path) = export(&fx, &fx.polys, &format!("polys.{ext}"), fmt, json!({}));
        done(&r);
        let back = fx.e.import(&path).unwrap_or_else(|er| panic!("{fmt} polygons: {er:#}"));
        assert!(back.iter().map(|b| b.feature_count).sum::<i64>() >= 2, "{fmt} polygons");
    }
    // CSV carries the shape as WKT text.
    let (r, path) = export(&fx, &fx.polys, "polys.csv", "CSV", json!({}));
    done(&r);
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("POLYGON ((") && text.lines().count() == 3, "{text}");
}

#[test]
fn gpx_refuses_polygons_and_says_why_and_the_coordinate_systems_are_right() {
    let fx = fx();
    let (r, path) = export(&fx, &fx.polys, "p.gpx", "GPX", json!({}));
    assert_eq!(r.status, Status::Error);
    let msg = r.error.unwrap();
    assert!(msg.contains("GPX holds points") && msg.contains("polygons") && msg.contains("KML"), "{msg}");
    assert!(!path.exists(), "nothing written");

    // KML and GPX are WGS 84 whatever the layer is in, and the message says so; positions are right.
    let (r, path) = export(&fx, &fx.pts, "p.gpx", "GPX", json!({}));
    assert!(done(&r).contains("always WGS 84"));
    let back = fx.e.import(&path).unwrap().into_iter().find(|b| b.geom_type == "Point").unwrap();
    assert!(back.crs.as_deref().is_some_and(|c| c.contains("4326")), "{:?}", back.crs);
    let ext = back.extent.unwrap();
    assert!(ext[0] < -92.0 && ext[0] > -94.0 && ext[1] > 41.0 && ext[1] < 43.0, "{ext:?}");
}

#[test]
fn an_output_coordinate_system_moves_the_shapes_and_blank_keeps_the_layers_own() {
    let fx = fx();
    let (r, path) = export(&fx, &fx.pts, "own.gpkg", "GPKG", json!({}));
    done(&r);
    assert!(fx.e.import(&path).unwrap()[0].crs.as_deref().is_some_and(|c| c.contains("26915")));
    let (r, path) = export(&fx, &fx.pts, "ll.gpkg", "GPKG", json!({"crs": "EPSG:4326"}));
    assert!(done(&r).contains("EPSG:4326"));
    let back = fx.e.import(&path).unwrap().remove(0);
    assert!(back.crs.as_deref().is_some_and(|c| c.contains("4326")));
    assert!(back.extent.unwrap()[0] < -92.0);
    // A bad CRS is refused in words.
    let (r, _) = export(&fx, &fx.pts, "bad.gpkg", "GPKG", json!({"crs": "EPSG:99999999"}));
    assert!(r.error.unwrap().contains("Can't use the coordinate system"));
    // GeoJSON stays WGS 84 unless asked otherwise, even when another system is chosen.
    let (r, _) = export(&fx, &fx.pts, "x.geojson", "GeoJSON", json!({"crs": "EPSG:3857"}));
    assert!(done(&r).contains("Reprojected to WGS 84"));
}

#[test]
fn chosen_fields_are_the_only_ones_written_and_an_unknown_field_is_refused() {
    let fx = fx();
    let (r, path) = export(&fx, &fx.pts, "f.gpkg", "GPKG", json!({"fields": ["name", "height"]}));
    assert!(done(&r).contains("2 of 6 fields written"), "the layer's fid column counts as a field");
    let back = fx.e.import(&path).unwrap().remove(0);
    let names: Vec<&str> = back.fields.iter().map(|f| f.name.as_str()).filter(|n| *n != "fid").collect();
    assert_eq!(names, vec!["name", "height"]);
    let (r, _) = export(&fx, &fx.pts, "f2.gpkg", "GPKG", json!({"fields": "name, nope"}));
    let msg = r.error.unwrap();
    assert!(msg.contains("Field 'nope' isn't in this layer") && msg.contains("height"), "{msg}");
}

#[test]
fn clip_to_an_area_or_to_a_layers_shapes_cuts_the_features() {
    let fx = fx();
    // Points at x = 0..4 km: the area keeps those inside it.
    let (west, south, east, north) = {
        let c = engine::duckdb::Connection::open_in_memory().unwrap();
        engine::db::load_spatial(&c).unwrap();
        let q = |x: f64, y: f64| -> (f64, f64) { c.query_row("SELECT ST_X(g), ST_Y(g) FROM (SELECT ST_Transform(ST_Point(?, ?), 'EPSG:26915', 'EPSG:4326', true) AS g)", [x, y], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
        let (w, s) = q(X + 500.0, Y + 500.0);
        let (e, n) = q(X + 2500.0, Y + 2500.0);
        (w, s, e, n)
    };
    let (r, path) = export(&fx, &fx.pts, "c.gpkg", "GPKG", json!({"clip_extent": format!("{west},{south},{east},{north}")}));
    done(&r);
    assert_eq!(fx.e.import(&path).unwrap().remove(0).feature_count, 2, "points 1 and 2 are inside"); // x = 1000 and 2000
    let (r, _) = export(&fx, &fx.pts, "c2.gpkg", "GPKG", json!({"clip_extent": "1,2,3"}));
    assert!(r.error.unwrap().contains("west,south,east,north"));

    // Clip lines to the west polygon (0..2000): the part of the road inside it only.
    let (r, path) = export(&fx, &fx.lines, "cl.gpkg", "GPKG", json!({"clip_layer": fx.polys}));
    done(&r);
    let back = fx.e.import(&path).unwrap().remove(0);
    assert_eq!(back.geom_type, "Line");
    let e = back.native_extent.unwrap();
    assert!(e[2] - e[0] <= 4000.0 && e[3] <= Y + 2000.5, "{e:?}");
    // Points clipped to the polygons: those inside either polygon.
    let (r, path) = export(&fx, &fx.pts, "cp.gpkg", "GPKG", json!({"clip_layer": fx.polys}));
    done(&r);
    assert_eq!(fx.e.import(&path).unwrap().remove(0).feature_count, 3); // x = 0, 1000 and 2000 km-steps: the west polygon covers 0..2000 m, and a point on its edge is kept; 3000+ is outside
    // What the clip removed is reported (5 points, 3 inside: 2 lay outside), and nothing is said when nothing was removed.
    let msg = fx.e.run_graph(&serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": fx.pts}}, {"id": "out", "tool": "export", "params": {"path": fx.dir.path().join("rep.gpkg").to_str().unwrap(), "format": "GPKG", "clip_layer": fx.polys}}],
        "edges": [{"source": "in", "target": "out", "targetHandle": "in"}]})).unwrap(), Some("out"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "out").unwrap().message.unwrap();
    assert!(msg.contains("2 features lay outside the clip area and were not written"), "{msg}");
    let (r, _) = export(&fx, &fx.lines, "cl2.gpkg", "GPKG", json!({"clip_layer": fx.polys}));
    assert!(!done(&r).contains("outside the clip area"), "the road touches the polygon, so nothing was dropped");
    // A road that only touches the polygon at one point: what is inside is a point, not a line, so it is dropped and reported.
    let touch = fx.dir.path().join("touch.gpkg");
    write(&touch, &format!("SELECT 1 AS id, ST_MakeEnvelope({}, {}, {X}, {Y}) AS geom", X - 1000.0, Y - 1000.0), "EPSG:26915");
    let tl = fx.e.import(&touch).unwrap().remove(0).id;
    let (r, _) = export(&fx, &fx.lines, "cw.gpkg", "GPKG", json!({"clip_layer": tl}));
    let msg = done(&r);
    assert!(msg.contains("1 feature had only a different kind of shape") && msg.contains("was not written"), "{msg}");
    // Clipping to a layer that isn't polygons is refused.
    let (r, _) = export(&fx, &fx.pts, "cx.gpkg", "GPKG", json!({"clip_layer": fx.lines}));
    assert!(r.error.unwrap().contains("needs polygons"));
}

#[test]
fn a_shapefile_reports_what_it_could_not_keep() {
    let fx = fx();
    let (r, path) = export(&fx, &fx.pts, "s.shp", "ESRI Shapefile", json!({}));
    let msg = done(&r);
    assert!(msg.contains("limited to 10 characters") && msg.contains("'population_total' became 'population'"), "{msg}");
    assert!(path.exists());
    // Nothing to report when names fit.
    let (r, _) = export(&fx, &fx.pts, "s2.shp", "ESRI Shapefile", json!({"fields": ["name", "height"]}));
    let msg = done(&r);
    assert!(!msg.contains("became"), "{msg}");
}
