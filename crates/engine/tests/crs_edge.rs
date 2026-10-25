//! CRS edge cases: no CRS, geographic vs projected, US survey feet, CRSs with
//! no area of use, invalid definitions (docs/TEST_PLAN.md §CRS).

use engine::graph::{Graph, Status};
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::json;
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: Option<&str>) {
    let driver = if path.extension().is_some_and(|e| e == "shp") { "ESRI Shapefile" } else { "GPKG" };
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

fn import(e: &Engine, dir: &Path, name: &str, select: &str, srs: Option<&str>) -> engine::layers::LayerInfo {
    let p = dir.join(format!("{name}.{}", if srs.is_some() { "gpkg" } else { "shp" }));
    write_fixture(&p, select, srs);
    e.import(&p).unwrap().remove(0)
}

/// Value of `field` on the layer's first row, sorted by id.
fn first(e: &Engine, layer: &str, field: &str) -> f64 {
    let info = e.layer(layer).unwrap();
    let idx = info.fields.iter().position(|f| f.name == field).unwrap();
    let page = e.rows(layer, &RowsQuery { limit: 1, sort: Some("id".into()), ..Default::default() }).unwrap();
    page.rows[0][idx].as_ref().unwrap().parse().unwrap()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= b.abs() * rel, "{a} vs {b} (±{}%)", rel * 100.0);
}

#[test]
fn layer_with_no_crs_refuses_measurement_and_projection_instead_of_guessing() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = import(&e, dir.path(), "nocrs", "SELECT 1 AS id, ST_MakeEnvelope(0, 0, 1, 1) AS geom", None);
    assert!(l.crs.is_none() && !l.warnings.is_empty(), "import flags it");
    let err = e.calculate_geometry(&l.id, "AREA", "area", "sqkm", "km", None).expect_err("area on an unknown CRS must not be computed");
    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("coordinate system") || msg.contains("crs"), "{msg}");
    assert!(e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty(), "never drawn");
    assert!(e.copy_layer(&l.id, "copy", None).unwrap().crs.is_none(), "a copy stays undefined");
    // Defining the CRS is the only way forward, and it clears the warning.
    let fixed = e.define_projection(&l.id, "EPSG:32614", None).unwrap();
    assert_eq!(fixed.crs.as_deref(), Some("EPSG:32614"));
    assert!(fixed.warnings.is_empty());
}

#[test]
fn define_projection_rejects_bad_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = import(&e, dir.path(), "nocrs", "SELECT 1 AS id, ST_Point(1, 2) AS geom", None);
    for bad in ["", "EPSG:0", "EPSG:99999999", "not a crs", "+proj=bogus"] {
        assert!(e.define_projection(&l.id, bad, None).is_err(), "{bad:?} must be rejected");
        assert!(e.layer(&l.id).unwrap().crs.is_none(), "{bad:?} must leave the layer undefined");
    }
    assert!(e.validate_crs("EPSG:4326").is_ok());
    assert!(e.validate_crs("EPSG:2277").is_ok());
}

#[test]
fn suggestions_follow_the_coordinate_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let kinds = |name: &str, sel: &str| -> Vec<String> {
        let l = import(&e, dir.path(), name, sel, None);
        e.suggest_crs(&l.id).unwrap().into_iter().map(|s| s.crs).collect()
    };
    assert_eq!(kinds("deg", "SELECT 1 AS id, ST_Point(-97.5, 35.2) AS geom")[0], "EPSG:4326");
    assert_eq!(kinds("utm", "SELECT 1 AS id, ST_Point(636548, 3896253) AS geom")[0], "UTM");
    assert_eq!(kinds("sp", "SELECT 1 AS id, ST_Point(2300000, 10000000) AS geom")[0], "STATEPLANE");
    assert!(kinds("tiny", "SELECT 1 AS id, ST_Point(5000, 5000) AS geom").is_empty(), "no guess when nothing fits");
}

#[test]
fn geographic_and_projected_copies_of_a_shape_measure_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // 1 km × 1 km in UTM 14N, and the same ground square written in degrees.
    let proj = import(&e, dir.path(), "utm", "SELECT 1 AS id, ST_MakeEnvelope(600000, 3900000, 601000, 3901000) AS geom", Some("EPSG:32614"));
    let g = e.transform_point(600000.0, 3900000.0, "EPSG:32614", "EPSG:4326").unwrap();
    let g2 = e.transform_point(601000.0, 3901000.0, "EPSG:32614", "EPSG:4326").unwrap();
    let corners = [(600000.0, 3900000.0), (601000.0, 3900000.0), (601000.0, 3901000.0), (600000.0, 3901000.0)].map(|(x, y)| e.transform_point(x, y, "EPSG:32614", "EPSG:4326").unwrap());
    let wkt = corners.iter().chain(std::iter::once(&corners[0])).map(|(x, y)| format!("{x} {y}")).collect::<Vec<_>>().join(", ");
    assert!(g.0 < g2.0 && g.1 < g2.1);
    let geo = import(&e, dir.path(), "deg", &format!("SELECT 1 AS id, ST_GeomFromText('POLYGON(({wkt}))') AS geom"), Some("EPSG:4326"));
    let area = |id: &str| {
        let l = e.calculate_geometry(id, "A_M2", "area", "sqm", "m", None).unwrap().0;
        first(&e, &l.id, "A_M2")
    };
    let (a, b) = (area(&proj.id), area(&geo.id));
    close(a, b, 0.002);
    close(a, 1.0e6 / 0.9996f64.powi(2), 0.002); // UTM grid metres are 0.9996 of ground at the central meridian's ±180 km
    // Planar buffers on geographic data run in a local metric CRS and agree with the projected buffer.
    let buf = |id: &str| -> f64 {
        let g: Graph = serde_json::from_value(json!({
            "nodes": [
                {"id": "i", "tool": "input_layer", "params": {"layer": id}},
                {"id": "b", "tool": "buffer", "params": {"distance": 50, "units": "m"}},
                {"id": "a", "tool": "calculate_geometry", "params": {"field": "A", "property": "area", "area_units": "sqm"}},
                {"id": "o", "tool": "layer_output", "params": {"layer": id}}
            ],
            "edges": [
                {"source": "i", "target": "b", "targetHandle": "in"},
                {"source": "b", "target": "a", "targetHandle": "in"},
                {"source": "a", "target": "o", "targetHandle": "in"}
            ]
        }))
        .unwrap();
        let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
        assert!(res.iter().all(|r| matches!(r.status, Status::Done | Status::Cached)), "{res:?}");
        first(&e, &res.iter().find(|r| r.id == "o").unwrap().layer.as_ref().unwrap().id, "A")
    };
    close(buf(&proj.id), buf(&geo.id), 0.01);
}

#[test]
fn us_survey_feet_are_not_treated_as_metres() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // NAD83 / Texas Central in metres (32139) and in US survey feet (2277): same grid, different unit.
    let (xm, ym) = (700_000.0, 3_300_000.0);
    let (xf, yf) = e.transform_point(xm, ym, "EPSG:32139", "EPSG:2277").unwrap();
    close(xf, xm / 0.3048006096012192, 1e-9);
    close(yf, ym / 0.3048006096012192, 1e-9);
    let back = e.transform_point(xf, yf, "EPSG:2277", "EPSG:32139").unwrap();
    assert!((back.0 - xm).abs() < 1e-6 && (back.1 - ym).abs() < 1e-6, "{back:?}");

    // A 100 ft × 100 ft square drawn in feet is 10,000 sq ft (not 10,000 m²).
    let sq = import(&e, dir.path(), "ft", &format!("SELECT 1 AS id, ST_MakeEnvelope({xf}, {yf}, {}, {}) AS geom", xf + 100.0, yf + 100.0), Some("EPSG:2277"));
    assert_eq!(sq.crs.as_deref(), Some("EPSG:2277"));
    let ft = e.calculate_geometry(&sq.id, "A_FT", "area", "sqft", "m", None).unwrap().0;
    close(first(&e, &ft.id, "A_FT"), 10_000.0, 0.003);
    let m = e.calculate_geometry(&sq.id, "A_M", "area", "sqm", "m", None).unwrap().0;
    close(first(&e, &m.id, "A_M"), 929.0341, 0.003);

    // Buffer by "100 usft": a circle of radius 30.48 m → area ≈ 3.1214·r² (8 segments per quarter).
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "i", "tool": "input_layer", "params": {"layer": sq.id}},
            {"id": "c", "tool": "feature_to_point", "params": {"method": "centroid"}},
            {"id": "b", "tool": "buffer", "params": {"distance": 100, "units": "usft", "method": "planar"}},
            {"id": "a", "tool": "calculate_geometry", "params": {"field": "A", "property": "area", "area_units": "sqft"}},
            {"id": "o", "tool": "layer_output", "params": {"layer": sq.id}}
        ],
        "edges": [
            {"source": "i", "target": "c", "targetHandle": "in"},
            {"source": "c", "target": "b", "targetHandle": "in"},
            {"source": "b", "target": "a", "targetHandle": "in"},
            {"source": "a", "target": "o", "targetHandle": "in"}
        ]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "o").unwrap();
    assert_eq!(out.status, Status::Done, "{:?}", out.error);
    assert_eq!(out.layer.as_ref().unwrap().crs.as_deref(), Some("EPSG:2277"), "buffer output stays in the input CRS");
    close(first(&e, &out.layer.as_ref().unwrap().id, "A"), 3.1214 * 100.0 * 100.0, 0.03);
}

#[test]
fn crs_without_an_area_of_use_needs_a_fallback_extent_and_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // A bare PROJ string carries no area of use.
    let aeqd = "+proj=aeqd +lat_0=35.2 +lon_0=-97.5 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
    let err = e.crs_display_info(aeqd, None).expect_err("no area of use and no fallback");
    assert!(err.to_string().contains("area of use"), "{err}");
    let info = e.crs_display_info(aeqd, Some([-98.0, 35.0, -97.0, 35.5])).unwrap();
    assert!(!info.area_of_use_known && !info.geographic);
    assert!(info.area_of_use[0] < -98.0 && info.area_of_use[2] > -97.0, "fallback is padded: {:?}", info.area_of_use);
    // Different fallbacks make different grids, so cached tiles aren't shared between them.
    let other = e.crs_display_info(aeqd, Some([10.0, 10.0, 11.0, 11.0])).unwrap();
    assert_ne!(info.crs_key, other.crs_key);

    // The engine itself can still hold data in it, measure and project.
    let l = import(&e, dir.path(), "aeqd", "SELECT 1 AS id, ST_MakeEnvelope(-500, -500, 500, 500) AS geom", None);
    let l = e.define_projection(&l.id, aeqd, Some("Local AEQD".into())).unwrap();
    assert_eq!(l.crs.as_deref(), Some(aeqd));
    let (lon, lat) = e.transform_point(0.0, 0.0, aeqd, "EPSG:4326").unwrap();
    assert!((lon + 97.5).abs() < 1e-9 && (lat - 35.2).abs() < 1e-9, "{lon},{lat}");
    let area = e.calculate_geometry(&l.id, "A", "area", "sqm", "m", None).unwrap().0;
    close(first(&e, &area.id, "A"), 1.0e6, 0.001);
    let k = e.map_scale_factor(aeqd, 0.0, 0.0).unwrap();
    assert!((k - 1.0).abs() < 1e-6, "k at the AEQD origin {k}");
}

#[test]
fn geographic_crs_grid_and_unit_edge_cases() {
    let e = Engine::new().unwrap();
    // A geographic CRS has an implicit whole-world area of use and k = 1.
    let nad27 = e.crs_display_info("EPSG:4267", None).unwrap();
    assert!(nad27.geographic);
    assert_eq!(e.map_scale_factor("EPSG:4326", -97.5, 35.2).unwrap(), 1.0);
    // The antimeridian: a point either side of 180° transforms to nearby UTM zone-1/zone-60 coordinates without NaN.
    let a = e.transform_point(179.999, 10.0, "EPSG:4326", "EPSG:32660").unwrap();
    let b = e.transform_point(-179.999, 10.0, "EPSG:4326", "EPSG:32601").unwrap();
    assert!(a.0.is_finite() && b.0.is_finite() && (a.0 - 500_000.0).abs() < 400_000.0 && (b.0 - 500_000.0).abs() < 400_000.0, "{a:?} {b:?}");
    // The poles are outside any UTM zone's useful range; the answer is an error or a finite number, never a panic.
    let _ = e.transform_point(0.0, 90.0, "EPSG:4326", "EPSG:32631");
    let _ = e.transform_point(f64::NAN, f64::NAN, "EPSG:4326", "EPSG:32631");
    // Same CRS both sides: identity.
    let same = e.transform_point(-97.5, 35.2, "EPSG:4326", "EPSG:4326").unwrap();
    assert!((same.0 + 97.5).abs() < 1e-12 && (same.1 - 35.2).abs() < 1e-12);
}
