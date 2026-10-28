//! CRS reference suite (docs/CRS_PLAN.md R1): coordinates against `cs2cs` output (`fixtures/crs/reference.json`, made by
//! `scripts/gen_crs_reference.py`), analytic anchors and edge cases. Cases are the R-A, R-C, R-D and R-J rows of
//! docs/TEST_PLAN.md §2A. A test that finds a real bug is `#[ignore]`d with its `C-n` id from docs/BUGS.md.
//! Expected values never come from the engine itself (RD-1).

#[path = "common/crs_tol.rs"]
mod crs_tol;

use engine::Engine;
use serde_json::Value;
use std::sync::OnceLock;

fn reference() -> &'static Value {
    static R: OnceLock<Value> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("fixtures/crs/reference.json")).expect("reference.json parses"))
}

/// Definition of a zoo CRS.
fn def(key: &str) -> String {
    reference()["crs"][key].as_str().unwrap_or_else(|| panic!("{key} is not in the zoo")).to_string()
}

fn zoo() -> Vec<String> {
    reference()["crs"].as_object().unwrap().keys().cloned().collect()
}

fn engine() -> &'static Engine {
    static E: OnceLock<Engine> = OnceLock::new();
    E.get_or_init(|| Engine::new().unwrap())
}

/// x east / y north through the engine's transform (DuckDB Spatial `ST_Transform`, always_xy).
fn tf(x: f64, y: f64, from: &str, to: &str) -> anyhow::Result<(f64, f64)> {
    engine().transform_point(x, y, from, to)
}

fn near(got: f64, want: f64, tol: f64, what: &str) {
    assert!((got - want).abs() <= tol, "{what}: got {got}, want {want} (tolerance {tol})");
}

fn point(v: &Value) -> Option<(f64, f64)> {
    v.as_array().map(|a| (a[0].as_f64().unwrap(), a[1].as_f64().unwrap()))
}

fn log_versions() {
    eprintln!("CRS reference made with PROJ {}", reference()["proj_version"].as_str().unwrap_or("?"));
}

/// Datum of each zoo CRS: a transform between two CRSs of the same datum is a pure projection change (exact).
fn datum(key: &str) -> &'static str {
    match key {
        "4269" | "26915" | "5070" | "2277" | "32139" | "ft_utm15" | "esri_26915" => "NAD83",
        "4267" => "NAD27",
        "3035" => "ETRS89",
        "2193" => "NZGD2000",
        "4807" => "NTF",
        _ => "WGS84", // 4326, 3857, 32615, 32756, 32660, 32601, 3031, 3413, aeqd
    }
}

/// A lon/lat box in which the zoo CRS is meant to be used (a crude area of use, enough to keep round trips well conditioned).
fn domain(key: &str) -> [f64; 4] {
    match key {
        "26915" | "32615" | "ft_utm15" | "esri_26915" => [-96.0, 0.0, -90.0, 80.0],
        "5070" => [-125.0, 24.0, -66.0, 50.0],
        "2277" | "32139" => [-104.0, 26.0, -93.0, 36.5],
        "4267" => [-125.0, 24.0, -66.0, 50.0],
        "3035" => [-10.0, 30.0, 40.0, 70.0],
        "2193" => [166.0, -48.0, 179.0, -34.0],
        "32660" => [174.0, 0.0, 180.0, 80.0],
        "32601" => [-180.0, 0.0, -174.0, 80.0],
        "32756" => [150.0, -80.0, 156.0, 0.0],
        "3031" => [-180.0, -90.0, 180.0, -60.0],
        "3413" => [-180.0, 55.0, 180.0, 90.0],
        "aeqd" => [-110.0, 30.0, -90.0, 50.0],
        "3857" => [-180.0, -80.0, 180.0, 80.0],
        _ => [-180.0, -89.0, 180.0, 89.0],
    }
}

fn inside(key: &str, lon: f64, lat: f64) -> bool {
    let d = domain(key);
    lon >= d[0] && lon <= d[2] && lat >= d[1] && lat <= d[3]
}

// ---------------------------------------------------------------- R-A: transforms against a reference

#[test]
fn r_a1_utm_central_meridian() {
    let (x, y) = tf(-93.0, 0.0, "EPSG:4326", "EPSG:32615").unwrap();
    near(x, 500000.0, crs_tol::PROJECTED_M, "easting");
    near(y, 0.0, crs_tol::PROJECTED_M, "northing");
}

#[test]
fn r_a2_web_mercator_half_world_and_latitude_limit() {
    let (x, y) = tf(180.0, 0.0, "EPSG:4326", "EPSG:3857").unwrap();
    near(x, 20037508.343, 1e-3, "half world");
    near(y, 0.0, 1e-3, "equator");
    let (_, y) = tf(180.0, 85.0511287798066, "EPSG:4326", "EPSG:3857").unwrap();
    near(y, 20037508.343, 1e-3, "y at the latitude limit");
    let (_, y) = tf(0.0, -85.0511287798066, "EPSG:4326", "EPSG:3857").unwrap();
    near(y, -20037508.343, 1e-3, "y at the southern limit");
}

#[test]
fn r_a3_polar_stereographic() {
    let (x, y) = tf(0.0, -90.0, "EPSG:4326", "EPSG:3031").unwrap();
    near(x, 0.0, 1e-3, "south pole x");
    near(y, 0.0, 1e-3, "south pole y");
    let (x, y) = tf(0.0, -71.0, "EPSG:4326", "EPSG:3031").unwrap();
    near(x, 0.0, 1e-3, "-71 x");
    near(y, 2082760.109, 1e-3, "-71 y");
    let (x, y) = tf(0.0, 90.0, "EPSG:4326", "EPSG:3413").unwrap();
    near(x, 0.0, 1e-3, "north pole x");
    near(y, 0.0, 1e-3, "north pole y");
}

#[test]
fn r_a4_albers_origin() {
    let (x, y) = tf(-96.0, 23.0, "EPSG:4326", "EPSG:5070").unwrap();
    near(x, 0.0, 1e-3, "x");
    near(y, 0.0, 1e-3, "y");
}

/// Zoo CRSs whose transform from the reference points all exist (nothing out of domain).
fn round_trip_keys() -> Vec<String> {
    // 4807 is in grads and checked in R-C6; the antimeridian and polar-only CRSs have their own cases.
    zoo().into_iter().filter(|k| k != "4807").collect() // 4807 is in grads: R-C6
}

#[test]
fn r_a5_round_trip_over_the_zoo() {
    // Only inside the area both CRSs are meant for: far outside it a projection is ill-conditioned and 1 mm is not promised.
    let pts = [(-93.0, 0.0), (-93.0, 40.0), (-95.0, 30.0), (-100.0, 40.0), (-98.0, 31.0), (10.0, 52.0), (175.0, -41.0), (177.0, 45.0), (-177.0, 30.0), (152.0, -33.0), (0.0, -80.0), (-45.0, 75.0), (-96.0, 25.0), (-110.0, 45.0)];
    let mut checked = 0;
    for a in round_trip_keys() {
        for b in round_trip_keys() {
            if a == b {
                continue;
            }
            for (lon, lat) in pts {
                if !(inside(&a, lon, lat) && inside(&b, lon, lat)) {
                    continue;
                }
                let (ax, ay) = tf(lon, lat, "EPSG:4326", &def(&a)).unwrap();
                let (bx, by) = tf(ax, ay, &def(&a), &def(&b)).unwrap_or_else(|e| panic!("{a} → {b} at ({lon}, {lat}): {e}"));
                assert!(bx.is_finite() && by.is_finite(), "{a} → {b} at ({lon}, {lat}) gave ({bx}, {by})");
                let (rx, ry) = tf(bx, by, &def(&b), &def(&a)).unwrap();
                // A NAD27 shift comes from a grid, whose inverse is iterative and exact only to about 1 mm: pairs with NAD27
                // get the geographic tolerance of RD-4 (1e-8°) or 3 mm. Everything else keeps the 1 mm / 1e-9° of RD-4.
                let grid = a == "4267" || b == "4267";
                let tol = match (crs_tol::is_geographic(&a), grid) {
                    (true, false) => crs_tol::ROUND_TRIP_DEG,
                    (true, true) => crs_tol::GEOGRAPHIC_DEG,
                    (false, false) => crs_tol::ROUND_TRIP_M,
                    (false, true) => 3.0 * crs_tol::ROUND_TRIP_M,
                };
                assert!((rx - ax).abs() <= tol && (ry - ay).abs() <= tol, "{a} → {b} → {a} at ({lon}, {lat}): ({ax}, {ay}) came back as ({rx}, {ry})");
                checked += 1;
            }
        }
    }
    assert!(checked > 300, "only {checked} round trips ran");
}

/// Compare the engine with the cs2cs reference. `datum_change` picks the comparisons across datums (or not).
fn compare_reference(datum_change: bool) -> (usize, Vec<String>) {
    log_versions();
    let r = reference();
    let pts = r["points"].as_array().unwrap();
    let mut n = 0;
    let mut bad: Vec<String> = Vec::new();
    for key in zoo() {
        if key == "4807" || (datum(&key) != "WGS84") != datum_change {
            continue; // 4807 is in grads: R-C6
        }
        for (i, p) in pts.iter().enumerate() {
            let (lon, lat) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
            let want = point(&r["geo"][&key][i]);
            let got = tf(lon, lat, "EPSG:4326", &def(&key));
            match (want, got) {
                (Some((wx, wy)), Ok((gx, gy))) => {
                    let tol = crs_tol::coordinate(&key);
                    if (gx - wx).abs() > tol || (gy - wy).abs() > tol {
                        bad.push(format!("4326 → {key} at ({lon}, {lat}): got ({gx}, {gy}), cs2cs ({wx}, {wy})"));
                    }
                    n += 1;
                }
                (None, Ok((gx, gy))) if gx.is_finite() && gy.is_finite() && !(key == "3857" || key == "aeqd") => {
                    bad.push(format!("4326 → {key} at ({lon}, {lat}): cs2cs says it can't be projected, the engine returned ({gx}, {gy})"));
                }
                _ => {}
            }
        }
    }
    for pair in r["pairs"].as_array().unwrap() {
        let (a, b) = (pair["from"].as_str().unwrap(), pair["to"].as_str().unwrap());
        if a == "4807" || b == "4807" || (datum(a) != datum(b)) != datum_change {
            continue;
        }
        for (inp, out) in pair["in"].as_array().unwrap().iter().zip(pair["out"].as_array().unwrap()) {
            let (Some((x, y)), Some((wx, wy))) = (point(inp), point(out)) else { continue };
            match tf(x, y, &def(a), &def(b)) {
                Ok((gx, gy)) => {
                    let tol = crs_tol::coordinate(b);
                    if (gx - wx).abs() > tol || (gy - wy).abs() > tol {
                        bad.push(format!("{a} → {b} at ({x}, {y}): got ({gx}, {gy}), cs2cs ({wx}, {wy})"));
                    }
                    n += 1;
                }
                Err(e) => bad.push(format!("{a} → {b} at ({x}, {y}): {e}")),
            }
        }
    }
    (n, bad)
}

#[test]
fn r_a6_reference_set_from_cs2cs_same_datum() {
    let (n, bad) = compare_reference(false);
    assert!(n >= 250, "only {n} reference values compared");
    assert!(bad.is_empty(), "{} of {n} differ from cs2cs:\n{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}

/// Bug C-1 (docs/BUGS.md): DuckDB Spatial's ST_Transform treats NAD83↔WGS 84 (and ETRS89↔WGS 84) as no change and picks another
/// NAD27 operation than PROJ's best, so cross-datum results are off by up to ~2 m from cs2cs. Fixed by R3 (operation + accuracy).
#[test]
#[ignore = "bug C-1: cross-datum transforms differ from PROJ's best operation by up to ~2 m"]
fn r_a6_reference_set_from_cs2cs_across_datums() {
    let (n, bad) = compare_reference(true);
    assert!(n >= 100, "only {n} reference values compared");
    assert!(bad.is_empty(), "{} of {n} differ from cs2cs:\n{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn r_a7_geodesic_anchors() {
    use geographiclib_rs::{Geodesic, InverseGeodesic};
    let g = Geodesic::wgs84();
    let quadrant: f64 = g.inverse(0.0, 0.0, 90.0, 0.0);
    near(quadrant, 10_001_965.729, 0.01, "meridian from the equator to the pole");
    let degree: f64 = g.inverse(0.0, 0.0, 0.0, 1.0);
    near(degree, 111_319.491, 0.001, "1° of longitude on the equator");
    near(g.area() / 1e6, 510_065_621.7, 510_065_621.7 * 1e-5, "WGS 84 surface, km²");
}

// ---------------------------------------------------------------- R-C: units, prime meridians, wrapped longitudes

#[test]
fn r_c4_linear_units_are_told_apart() {
    let unit = |key: &str| engine().crs_display_info(&def(key), Some([-100.0, 30.0, -90.0, 40.0])).unwrap().unit_m;
    near(unit("32139"), 1.0, 1e-12, "metre");
    near(unit("2277"), 1200.0 / 3937.0, 1e-12, "US survey foot");
    near(unit("ft_utm15"), 0.3048, 1e-12, "international foot");
    // 1 000 000 ft of each differ by 2 ft: the two feet are not interchangeable.
    near(1_000_000.0 * unit("2277") - 1_000_000.0 * unit("ft_utm15"), 1_000_000.0 * (1200.0 / 3937.0 - 0.3048), 1e-9, "metres between a million ftUS and a million ft (0.6096012 m, 2 ft)");
    // The same two points are 1000 apart in each CRS's own unit, so their ground distance follows the unit.
    let (x1, y1) = tf(-96.0, 31.0, "EPSG:4326", &def("2277")).unwrap();
    let (x2, y2) = tf(-96.0, 31.01, "EPSG:4326", &def("2277")).unwrap();
    let (a1, b1) = tf(-96.0, 31.0, "EPSG:4326", &def("32139")).unwrap();
    let (a2, b2) = tf(-96.0, 31.01, "EPSG:4326", &def("32139")).unwrap();
    let ftus = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt() * 1200.0 / 3937.0;
    let metres = ((a2 - a1).powi(2) + (b2 - b1).powi(2)).sqrt();
    near(ftus, metres, 1e-3, "one distance in ftUS and in metres (Texas Central, same projection)");
}

#[test]
fn r_c6_prime_meridian_and_grads() {
    // Longitude 0 grad of NTF (Paris) is the Paris meridian, 2.337229167° E of Greenwich, plus the NTF → WGS 84 datum shift
    // (about 75 m); latitude 50 grad is 45°. Pinned from cs2cs (PROJ 9.8.1): 44.999974683522, 2.336555728255.
    let (lon, lat) = tf(0.0, 50.0, "EPSG:4807", "EPSG:4326").unwrap();
    near(lon, 2.336555728255, 1e-8, "Paris meridian in degrees");
    near(lat, 44.999974683522, 1e-8, "50 grads of latitude");
    let (lon, _) = tf(0.0, 50.0, "EPSG:4807", "EPSG:4807").unwrap();
    near(lon, 0.0, 1e-12, "the Paris meridian is 0 in NTF itself");
}

#[test]
fn r_c7_longitudes_outside_180_are_wrapped() {
    let (x_190, y_190) = tf(190.0, 10.0, "EPSG:4326", "EPSG:32601").unwrap();
    let (x_m170, y_m170) = tf(-170.0, 10.0, "EPSG:4326", "EPSG:32601").unwrap();
    near(x_190, x_m170, 1e-3, "190° is −170° (UTM zone 1 easting)");
    near(y_190, y_m170, 1e-3, "190° is −170° (northing)");
    let (x_540, _) = tf(540.0, 10.0, "EPSG:4326", "EPSG:32601").unwrap_or((f64::NAN, 0.0));
    let (x_180, _) = tf(180.0, 10.0, "EPSG:4326", "EPSG:32601").unwrap();
    assert!(x_540.is_nan() || (x_540 - x_180).abs() < 1e-3, "540° is 180°, or refused");
}

// ---------------------------------------------------------------- R-D: datum shifts use the grid

#[test]
#[ignore = "bug C-1: DuckDB's ST_Transform picks another datum operation than PROJ's best"]
fn r_d1_nad27_to_nad83_uses_the_grid() {
    let (lon, lat) = tf(-97.5, 35.2, "EPSG:4267", "EPSG:4269").unwrap();
    near(lat, 35.200066504, 5e-9, "NAD83 latitude");
    near(lon, -97.500312182, 5e-9, "NAD83 longitude");
}

#[test]
#[ignore = "bug C-1: DuckDB's ST_Transform picks another datum operation than PROJ's best"]
fn r_d2_wgs84_to_nad27() {
    let (lon, lat) = tf(-97.5, 35.2, "EPSG:4326", "EPSG:4267").unwrap();
    near(lat, 35.199935272, 5e-9, "NAD27 latitude");
    near(lon, -97.499683431, 5e-9, "NAD27 longitude");
}

#[test]
#[ignore = "bug C-1: DuckDB's ST_Transform picks another datum operation than PROJ's best"]
fn r_d3_wgs84_to_nad83_operation() {
    let (lon, lat) = tf(-97.5, 35.2, "EPSG:4326", "EPSG:4269").unwrap();
    near(lat, 35.200001669, 5e-8, "NAD83 latitude (the 2.0 m Oklahoma operation, 5 mm)");
    near(lon, -97.499995543, 5e-8, "NAD83 longitude");
}

// ---------------------------------------------------------------- R-J: edges

/// Bug C-2: Lambert Conformal Conic (Texas Central) at the south pole returns (inf, inf) with no error.
#[test]
#[ignore = "bug C-2: ST_Transform returns inf without an error for points a projection cannot show"]
fn r_j1_poles_give_finite_numbers_or_an_error_never_nan() {
    let mut bad = Vec::new();
    for key in zoo() {
        for lat in [90.0, -90.0] {
            if let Ok((x, y)) = tf(0.0, lat, "EPSG:4326", &def(&key)) {
                if !(x.is_finite() && y.is_finite()) {
                    bad.push(format!("{key} at lat {lat}: ({x}, {y})"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "non-finite coordinates returned without an error: {bad:?}");
}

#[test]
fn r_j3_out_of_domain_is_finite_or_an_error() {
    for (lon, lat, key) in [(0.0, 89.9, "3857"), (-93.0, 85.0, "32615"), (170.0, 10.0, "5070"), (100.0, -80.0, "32615")] {
        if let Ok((x, y)) = tf(lon, lat, "EPSG:4326", &def(key)) {
            assert!(x.is_finite() && y.is_finite(), "{key} at ({lon}, {lat}) returned ({x}, {y})");
        }
    }
}

#[test]
fn r_j4_weird_numbers_do_not_panic() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300, -1e300, -0.0, 5e-324, f64::MAX] {
        for key in ["32615", "3857", "3031", "4269"] {
            let _ = tf(v, 0.0, "EPSG:4326", &def(key));
            let _ = tf(0.0, v, "EPSG:4326", &def(key));
            let _ = tf(v, v, &def(key), "EPSG:4326");
        }
    }
    // -0.0 is zero.
    let (x, y) = tf(-0.0, -0.0, "EPSG:4326", "EPSG:32631").unwrap();
    assert!(x.is_finite() && y.abs() < 1e-3, "-0.0 is the equator: ({x}, {y})");
    let (x0, _) = tf(0.0, 0.0, "EPSG:4326", "EPSG:32631").unwrap();
    assert_eq!(x, x0, "-0.0 and 0.0 give the same easting");
}

#[test]
fn r_j6_empty_and_null_geometries_stay_empty_and_null() {
    let rows: (Option<String>, Option<String>, Option<String>) = engine()
        .with_conn_for_tests(|c| {
            Ok(c.query_row(
                "SELECT ST_AsText(ST_Transform(ST_GeomFromText('POINT EMPTY'), 'EPSG:4326', 'EPSG:32615', true)),
                        ST_AsText(ST_Transform(ST_GeomFromText('POLYGON EMPTY'), 'EPSG:4326', 'EPSG:3857', true)),
                        ST_AsText(ST_Transform(NULL::GEOMETRY, 'EPSG:4326', 'EPSG:32615', true))",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();
    assert!(rows.0.as_deref().is_some_and(|t| t.contains("EMPTY")), "{rows:?}");
    assert!(rows.1.as_deref().is_some_and(|t| t.contains("EMPTY")), "{rows:?}");
    assert!(rows.2.is_none(), "{rows:?}");
}

#[test]
fn r_j7_precision_is_kept_to_a_nanodegree_and_a_millimetre() {
    for k in 0..20 {
        let lon = -93.0 + k as f64 * 1e-9;
        let (x, y) = tf(lon, 40.0, "EPSG:4326", "EPSG:32615").unwrap();
        let (lon2, lat2) = tf(x, y, "EPSG:32615", "EPSG:4326").unwrap();
        near(lon2, lon, 1e-9, "longitude after a round trip");
        near(lat2, 40.0, 1e-9, "latitude after a round trip");
    }
    // 1 mm steps in UTM stay 1 mm apart.
    let (x0, y0) = (500000.0, 4_428_000.0);
    let (lon0, lat0) = tf(x0, y0, "EPSG:32615", "EPSG:4326").unwrap();
    let (lon1, lat1) = tf(x0 + 0.001, y0, "EPSG:32615", "EPSG:4326").unwrap();
    assert!(lon1 > lon0 && (lat1 - lat0).abs() < 1e-9, "a 1 mm step east is visible and goes east");
    let (xb, yb) = tf(lon1, lat1, "EPSG:4326", "EPSG:32615").unwrap();
    near(xb, x0 + 0.001, 1e-4, "x after the round trip");
    near(yb, y0, 1e-4, "y after the round trip");
}

/// An ESRI-flavoured WKT (a shapefile `.prj`) behaves as the same CRS as its EPSG twin: it transforms to and from every zoo CRS.
#[test]
fn r_b_esri_flavour_transforms_build_for_every_zoo_crs() {
    let mut failed = Vec::new();
    for k in round_trip_keys() {
        if k == "esri_26915" || !inside(&k, -93.0, 40.0) {
            continue;
        }
        let Ok((x, y)) = tf(-93.0, 40.0, "EPSG:4326", &def(&k)) else { continue };
        let (ex, ey) = tf(-93.0, 40.0, "EPSG:4326", &def("esri_26915")).unwrap();
        if let Err(e) = tf(x, y, &def(&k), &def("esri_26915")) {
            failed.push(format!("{k} → esri: {e}"));
        }
        if let Err(e) = tf(ex, ey, &def("esri_26915"), &def(&k)) {
            failed.push(format!("esri → {k}: {e}"));
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
    // The ESRI WKT and EPSG:26915 give the same coordinates.
    let (ex, ey) = tf(-93.0, 40.0, "EPSG:4326", &def("esri_26915")).unwrap();
    let (px, py) = tf(-93.0, 40.0, "EPSG:4326", "EPSG:26915").unwrap();
    near(ex, px, 1e-3, "easting");
    near(ey, py, 1e-3, "northing");
}

// ---------------------------------------------------------------- R-J2, R-J8: measures across the antimeridian and over the world

fn one_feature(name: &str, wkt: &str) -> (String, i64) {
    let mut props = serde_json::Map::new();
    props.insert("k".into(), serde_json::json!(1));
    let info = engine().create_features(name, &[(wkt.to_string(), props)]).unwrap();
    (info.id.clone(), 1)
}

#[test]
fn r_j2_a_line_across_the_antimeridian_is_two_degrees_long() {
    use geographiclib_rs::{Geodesic, InverseGeodesic};
    let want: f64 = Geodesic::wgs84().inverse(0.0, 179.0, 0.0, -179.0);
    near(want, 222_638.98, 0.01, "2° of the equator (GeographicLib)");
    let (id, fid) = one_feature("anti line", "LINESTRING(179 0, -179 0)");
    let m = engine().measure_feature(&id, fid).unwrap();
    let got = m.length.expect("a length");
    assert!((got - want).abs() / want < crs_tol::LENGTH_REL, "geodesic length across 180°: got {got} m, want {want} m");
}

/// Bug C-3: the geodesic area of a polygon covering the whole world (edges on ±180° and the poles) comes out as 0.
#[test]
#[ignore = "bug C-3: measured area of a whole-world polygon is 0"]
fn r_j8_the_whole_world_polygon_has_the_ellipsoid_area() {
    let (id, fid) = one_feature("world", "POLYGON((-180 -90, 180 -90, 180 90, -180 90, -180 -90))");
    let area = engine().measure_feature(&id, fid).unwrap().area.expect("an area");
    let want = 510_065_621.7e6;
    assert!((area - want).abs() / want < 1e-5, "world area: got {} km², want 510 065 621.7 km²", area / 1e6);
}

// ---------------------------------------------------------------- R-C1: x is east in every format and axis order

/// One point, written with GDAL in `crs`, read back through the importer: the layer's lon/lat extent is where the point is.
fn import_one_point(dir: &std::path::Path, driver: &str, ext: &str, crs_key: &str, xy: (f64, f64)) -> engine::layers::LayerInfo {
    let path = dir.join(format!("pt_{crs_key}.{ext}"));
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, ST_Point({}, {}) AS geom) TO '{}' (FORMAT GDAL, DRIVER '{driver}', SRS '{}', USE_TMP_FILE false)",
        xy.0,
        xy.1,
        path.display(),
        def(crs_key)
    ))
    .unwrap();
    engine().import(&path).unwrap_or_else(|e| panic!("{driver} {crs_key}: {e}")).remove(0)
}

#[test]
fn r_c1_x_is_east_in_every_format_and_axis_order() {
    let dir = tempfile::tempdir().unwrap();
    // (zoo key, WGS 84 lon/lat the point stands for, index in reference points)
    let cases = [("4326", 0), ("4269", 1), ("3035", 4), ("2193", 6)];
    let mut bad = Vec::new();
    for (key, i) in cases {
        let r = reference();
        let (lon, lat) = (r["points"][i][0].as_f64().unwrap(), r["points"][i][1].as_f64().unwrap());
        let (x, y) = point(&r["geo"][key][i]).unwrap_or_else(|| panic!("{key} has no reference at point {i}"));
        for (driver, ext) in [("GPKG", "gpkg"), ("ESRI Shapefile", "shp"), ("GeoJSON", "geojson"), ("FlatGeobuf", "fgb")] {
            let sub = dir.path().join(format!("{driver}-{key}").replace(' ', "_"));
            std::fs::create_dir_all(&sub).unwrap();
            let info = import_one_point(&sub, driver, ext, key, (x, y));
            let e = info.extent.expect("an extent");
            // A datum shift of a metre or two is C-1's business; a swapped axis is thousands of kilometres.
            if (e[0] - lon).abs() > 1e-3 || (e[1] - lat).abs() > 1e-3 {
                bad.push(format!("{driver} in {key}: the point at lon {lon}, lat {lat} is at ({}, {}) after import", e[0], e[1]));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---------------------------------------------------------------- R-A8: other bodies at the PROJ level

#[test]
fn r_a8_mars_and_moon_spheres_at_the_proj_level() {
    // 1° of longitude on the equator of a sphere: R·π/180 (Mars 3 396 190 m, Moon 1 737 400 m). Spheres are given as PROJ
    // strings: PROJ moves coordinates only within one body, so the body's own CRS code can't be projected onto a bare sphere.
    for (name, radius, degree) in [("Mars", 3_396_190.0, 59_274.70), ("Moon", 1_737_400.0, 30_323.35)] {
        let geo = format!("+proj=longlat +R={radius} +no_defs");
        let eqc = format!("+proj=eqc +lat_ts=0 +R={radius} +units=m +no_defs");
        let (x, y) = tf(1.0, 0.0, &geo, &eqc).unwrap_or_else(|e| panic!("{name}: {e}"));
        near(x, degree, 0.01, &format!("{name}: 1° of longitude, metres"));
        near(y, 0.0, 1e-6, &format!("{name}: equator"));
    }
    // PROJ refuses to move coordinates between bodies (the guard stays when phase F1 lands).
    assert!(tf(10.0, 10.0, "EPSG:4326", "IAU_2015:49900").is_err(), "Earth → Mars must be an error");
}
