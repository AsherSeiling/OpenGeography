//! CRS through importers (docs/CRS_PLAN.md R2): format × CRS round trips, shapefile `.prj` flavours, GeoPackage SRS variants,
//! GeoJSON legacy `crs` members, CSV columns. Cases R-E1, R-E2, R-E4, R-C2, R-C3. Expected values come from the files' own
//! definitions and `cs2cs` (fixtures/crs/reference.json), never from the engine.

use engine::crs;
use engine::Engine;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

fn reference() -> &'static Value {
    static R: OnceLock<Value> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("fixtures/crs/reference.json")).unwrap())
}

fn def(key: &str) -> String {
    reference()["crs"][key].as_str().unwrap().to_string()
}

fn write_point(path: &Path, driver: &str, srs: Option<&str>, x: f64, y: f64) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{}'", s.replace('\'', "''"))).unwrap_or_default();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_Point({x}, {y}) AS geom) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

#[test]
fn r_e1_format_by_crs_round_trip_keeps_the_crs_and_the_coordinates() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    // A point per CRS at a place that CRS is meant for.
    let cases = [("4326", -97.5, 35.2), ("26915", 400000.0, 4_000_000.0), ("5070", 100000.0, 200000.0), ("2277", 3_000_000.0, 10_000_000.0), ("3031", 100000.0, -2_000_000.0), ("aeqd", 10000.0, -20000.0)];
    let mut bad = Vec::new();
    for (driver, ext) in [("GPKG", "gpkg"), ("ESRI Shapefile", "shp"), ("FlatGeobuf", "fgb")] {
        for (key, x, y) in cases {
            let sub = dir.path().join(format!("{ext}_{key}"));
            std::fs::create_dir_all(&sub).unwrap();
            let p = sub.join(format!("p.{ext}"));
            write_point(&p, driver, Some(&def(key)), x, y);
            let info = e.import(&p).unwrap_or_else(|err| panic!("{driver} {key}: {err}")).remove(0);
            match info.crs.as_deref() {
                Some(c) if crs::same(c, &def(key)) => {}
                other => bad.push(format!("{driver} × {key}: the CRS came back as {other:?}")),
            }
            let n = info.native_extent.expect("a native extent");
            let tol = if key == "4326" { 1e-9 } else { 1e-3 };
            if (n[0] - x).abs() > tol || (n[1] - y).abs() > tol {
                bad.push(format!("{driver} × {key}: ({x}, {y}) came back as ({}, {})", n[0], n[1]));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

const ESRI_26915: &str = r#"PROJCS["NAD_1983_UTM_Zone_15N",GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",-93.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn shapefile_with_prj(dir: &Path, name: &str, prj: Option<&str>) -> std::path::PathBuf {
    let p = dir.join(format!("{name}.shp"));
    write_point(&p, "ESRI Shapefile", None, 500000.0, 4_400_000.0);
    let _ = std::fs::remove_file(dir.join(format!("{name}.prj")));
    if let Some(t) = prj {
        std::fs::write(dir.join(format!("{name}.prj")), t).unwrap();
    }
    p
}

#[test]
fn r_e2_shapefile_prj_flavours() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ogc = gdal::spatial_ref::SpatialRef::from_epsg(26915).unwrap().to_wkt().unwrap();
    // ESRI and OGC WKT are recognised as EPSG:26915.
    for (name, prj) in [("esri", ESRI_26915.to_string()), ("ogc", ogc)] {
        let info = e.import(&shapefile_with_prj(dir.path(), name, Some(&prj))).unwrap().remove(0);
        let c = info.crs.unwrap_or_else(|| panic!("{name}: no CRS"));
        assert!(crs::same(&c, "EPSG:26915"), "{name} .prj is EPSG:26915, got {c:.80}");
    }
    // No .prj: undefined, with the warning (existing behaviour, 1.3).
    let none = e.import(&shapefile_with_prj(dir.path(), "none", None)).unwrap().remove(0);
    assert!(none.crs.is_none());
    assert!(none.warnings.iter().any(|w| w.contains("no coordinate system")), "{:?}", none.warnings);
    // A garbled .prj is undefined with a warning, never a guess (I1).
    let bad = e.import(&shapefile_with_prj(dir.path(), "garbled", Some("PROJCS[\"broken\",GEOGCS[\"x\",DATUM[\"y\""))).unwrap().remove(0);
    assert!(bad.crs.is_none(), "{:?}", bad.crs);
    assert!(bad.warnings.iter().any(|w| w.contains("coordinate system")), "{:?}", bad.warnings);
}

#[test]
fn r_e4_geopackage_srs_variants() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    // A custom definition without an authority code is kept (as itself).
    let custom = "+proj=aeqd +lat_0=40 +lon_0=-100 +datum=WGS84 +units=m +no_defs";
    let p1 = dir.path().join("custom.gpkg");
    write_point(&p1, "GPKG", Some(custom), 1000.0, 2000.0);
    let a = e.import(&p1).unwrap().remove(0);
    assert!(crs::same(a.crs.as_deref().expect("kept"), custom), "{:?}", a.crs);
    // No SRS: undefined, with a warning, never EPSG:4326.
    let p2 = dir.path().join("undefined.gpkg");
    write_point(&p2, "GPKG", None, 1.0, 2.0);
    let b = e.import(&p2).unwrap().remove(0);
    assert!(b.crs.is_none(), "an undefined SRS must stay undefined, got {:?}", b.crs);
    assert!(b.warnings.iter().any(|w| w.contains("no coordinate system")));
    // Two layers (files) in different CRSs each keep their own.
    let p3 = dir.path().join("a.gpkg");
    let p4 = dir.path().join("b.gpkg");
    write_point(&p3, "GPKG", Some("EPSG:4269"), -97.0, 35.0);
    write_point(&p4, "GPKG", Some("EPSG:26915"), 400000.0, 4_000_000.0);
    let (c, d) = (e.import(&p3).unwrap().remove(0), e.import(&p4).unwrap().remove(0));
    assert!(crs::same(c.crs.as_deref().unwrap(), "EPSG:4269") && crs::same(d.crs.as_deref().unwrap(), "EPSG:26915"));
}

// ---------------------------------------------------------------- GeoJSON and CSV (R-C2, R-C3)

fn geojson(dir: &Path, name: &str, crs_member: Option<&str>, x: f64, y: f64) -> std::path::PathBuf {
    let member = crs_member.map(|c| format!(r#","crs":{{"type":"name","properties":{{"name":"{c}"}}}}"#)).unwrap_or_default();
    let p = dir.join(format!("{name}.geojson"));
    std::fs::write(&p, format!(r#"{{"type":"FeatureCollection"{member},"features":[{{"type":"Feature","properties":{{"id":1}},"geometry":{{"type":"Point","coordinates":[{x},{y}]}}}}]}}"#)).unwrap();
    p
}

#[test]
fn r_c2_geojson_is_lon_lat_and_legacy_members_are_honoured_not_flipped() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ext = |p: &Path| e.import(p).unwrap().remove(0);
    // RFC 7946: lon, lat, WGS 84.
    let a = ext(&geojson(dir.path(), "rfc", None, -97.5, 35.2));
    let ex = a.extent.unwrap();
    assert!((ex[0] + 97.5).abs() < 1e-9 && (ex[1] - 35.2).abs() < 1e-9, "{ex:?}");
    // Legacy member for EPSG:4326 with lon,lat values: not flipped.
    let b = ext(&geojson(dir.path(), "legacy4326", Some("urn:ogc:def:crs:EPSG::4326"), -97.5, 35.2));
    let ex = b.extent.unwrap();
    assert!((ex[0] + 97.5).abs() < 1e-9 && (ex[1] - 35.2).abs() < 1e-9, "legacy 4326 member flipped the axes: {ex:?}");
    // Legacy member for a projected CRS is honoured: EPSG:3035 values of lon 10, lat 52 (x = east 4321000, y = north 3210000).
    let c = ext(&geojson(dir.path(), "legacy3035", Some("urn:ogc:def:crs:EPSG::3035"), 4_321_000.0, 3_210_000.0));
    assert!(crs::same(c.crs.as_deref().expect("a CRS"), "EPSG:3035"), "{:?}", c.crs);
    let ex = c.extent.unwrap();
    assert!((ex[0] - 10.0).abs() < 1e-3 && (ex[1] - 52.0).abs() < 1e-3, "EPSG:3035 member: {ex:?}");
}

#[test]
fn r_c3_csv_columns_and_swapped_values() {
    let e = Engine::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let p = dir.path().join(name);
        std::fs::write(&p, text).unwrap();
        p
    };
    for (name, text) in [("a.csv", "id,Longitude,Latitude\n1,-97.5,35.2\n"), ("b.csv", "id,Latitude,Longitude\n1,35.2,-97.5\n"), ("c.csv", "id,lat,lon\n1,35.2,-97.5\n")] {
        let info = e.import(&write(name, text)).unwrap().remove(0);
        let ex = info.extent.unwrap_or_else(|| panic!("{name}: no extent"));
        assert!((ex[0] + 97.5).abs() < 1e-9 && (ex[1] - 35.2).abs() < 1e-9, "{name}: {ex:?}");
    }
    // Values that can't be a latitude (> 90) in the "latitude" column: warn that they look swapped.
    let swapped = e.import(&write("d.csv", "id,Longitude,Latitude\n1,35.2,-97.5\n2,36.0,-98.0\n3,40.0,-120.0\n")).unwrap().remove(0);
    assert!(swapped.warnings.iter().any(|w| w.to_lowercase().contains("swapped")), "{:?}", swapped.warnings);
}
