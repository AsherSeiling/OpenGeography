//! CRS in tools (docs/CRS_PLAN.md R2/R3): mixing spellings of one CRS never reprojects; a datum change is reported.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;
use std::path::Path;

const ESRI_26915: &str = r#"PROJCS["NAD_1983_UTM_Zone_15N",GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",-93.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn square(path: &Path, srs_ddl: &str, cx: f64, cy: f64, half: f64, driver: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_MakeEnvelope({}, {}, {}, {}) AS geom) TO '{}' (FORMAT GDAL, DRIVER '{driver}', {srs_ddl}USE_TMP_FILE false)", cx - half, cy - half, cx + half, cy + half, path.display())).unwrap();
}

fn erase(e: &Engine, a: &str, b: &str) -> engine::graph::NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "a", "tool": "input_layer", "params": {"layer": a}}, {"id": "b", "tool": "input_layer", "params": {"layer": b}}, {"id": "t", "tool": "erase", "params": {}}],
        "edges": [{"source": "a", "target": "t", "targetHandle": "in"}, {"source": "b", "target": "t", "targetHandle": "erase"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let t = res.into_iter().find(|r| r.id == "t").unwrap();
    assert_eq!(t.status, Status::Done, "{t:?}");
    t
}

#[test]
fn r_b1_mixed_spellings_of_one_crs_are_not_reprojected() {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.gpkg"), dir.path().join("b.shp"));
    square(&pa, "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0, "GPKG");
    square(&pb, "", 500200.0, 4_400_200.0, 300.0, "ESRI Shapefile");
    std::fs::write(dir.path().join("b.prj"), ESRI_26915).unwrap();
    let e = Engine::new().unwrap();
    let (a, b) = (e.import(&pa).unwrap().remove(0), e.import(&pb).unwrap().remove(0));
    assert!(engine::crs::same(a.crs.as_deref().unwrap(), b.crs.as_deref().unwrap()), "the .prj is the same CRS as the GPKG's");
    let t = erase(&e, &a.id, &b.id);
    assert!(!t.reprojected, "the same CRS in two spellings must not be reprojected: {t:?}");
    assert!(!t.message.as_deref().unwrap_or("").contains("Datum change"), "{:?}", t.message);
}

#[test]
fn r_d5_a_datum_change_in_a_tool_is_reported_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.gpkg"), dir.path().join("b.gpkg"));
    square(&pa, "SRS 'EPSG:26915', ", 500000.0, 4_400_000.0, 1000.0, "GPKG");
    // The same area in NAD27 geographic coordinates.
    square(&pb, "SRS 'EPSG:4267', ", -93.0, 39.75, 0.002, "GPKG");
    let e = Engine::new().unwrap();
    let (a, b) = (e.import(&pa).unwrap().remove(0), e.import(&pb).unwrap().remove(0));
    let t = erase(&e, &a.id, &b.id);
    assert!(t.reprojected);
    let msg = t.message.unwrap_or_default();
    assert!(msg.contains("Datum change North American Datum 1927 → North American Datum 1983"), "{msg}");
    // A change of projection on one datum is not a datum change.
    assert!(engine::crs::datum_change_note("EPSG:4269", "EPSG:26915").is_none());
    assert!(engine::crs::datum_change_note("EPSG:4326", "EPSG:32615").is_none());
    assert!(engine::crs::datum_change_note("EPSG:4267", "EPSG:4326").is_some());
}

// ---------------------------------------------------------------- R-J1: poles and points a projection cannot show (bug C-2, RD-6)

fn project_to(e: &Engine, layer: &str, crs: &str) -> engine::graph::NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "a", "tool": "input_layer", "params": {"layer": layer}}, {"id": "p", "tool": "project", "params": {"crs": crs}}, {"id": "o", "tool": "layer_output", "params": {"layer": layer}}],
        "edges": [{"source": "a", "target": "p", "targetHandle": "in"}, {"source": "p", "target": "o", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let p = res.iter().find(|r| r.id == "p").unwrap().clone();
    assert_eq!(p.status, Status::Done, "{p:?}");
    let mut out = res.into_iter().find(|r| r.id == "o").unwrap();
    out.message = p.message;
    out
}

#[test]
fn r_j1_poles_are_counted_and_left_out_never_kept_as_infinite_coordinates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("poles.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT * FROM (VALUES (1, ST_Point(-97.5, 31.5)), (2, ST_Point(0.0, 90.0)), (3, ST_Point(0.0, -90.0)), (4, ST_Point(-97.0, 30.0))) t(id, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)",
        path.display()
    ))
    .unwrap();
    let e = Engine::new().unwrap();
    let layer = e.import(&path).unwrap().remove(0);
    let mut dropped_somewhere = false;
    for crs in ["EPSG:2277", "EPSG:32139", "EPSG:5070", "EPSG:3857", "EPSG:32614", "EPSG:3031", "EPSG:3413", "EPSG:4269", "EPSG:3035"] {
        let r = project_to(&e, &layer.id, crs);
        let l = r.layer.clone().unwrap();
        let n = l.feature_count;
        assert!((2..=4).contains(&n), "{crs}: {n} features kept");
        // Nothing infinite or NaN is left in the result.
        let [w, s, ee, no] = l.native_extent.expect("a native extent");
        assert!([w, s, ee, no].iter().all(|v| v.is_finite()), "{crs}: extent {:?}", l.native_extent);
        if n < 4 {
            dropped_somewhere = true;
            let m = r.message.unwrap_or_default();
            assert!(m.contains("could not be projected") && m.contains("left out"), "{crs}: {m:?}");
        }
    }
    assert!(dropped_somewhere, "a pole in a Lambert conic can't be shown, so at least one projection must have counted a drop");
}
