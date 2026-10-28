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
