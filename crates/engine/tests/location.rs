//! Go To Coordinates (RB4): every input format against known answers (the `mgrs` package / NGA GeoTrans values already used in
//! `usng.rs`), all four hemispheres, UTM north and south, a polar reference, refused input (swapped, out of range), a NAD 83 input
//! moved through an explicit transform, and positions the map's coordinate system cannot show.
use engine::location::{check_lat_lon, parse_angle, LocationRequest, LocationResult};
use engine::Engine;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use std::sync::OnceLock;

fn engine() -> (Engine, std::sync::MutexGuard<'static, ()>) {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static D: OnceLock<tempfile::TempDir> = OnceLock::new();
    let guard = ONE.lock().unwrap_or_else(|p| p.into_inner());
    std::env::set_var("GIS_STUDIO_DATA", D.get_or_init(|| tempfile::tempdir().unwrap()).path());
    (Engine::new().unwrap(), guard)
}

/// lon, lat, MGRS (5 digits): one place in each hemisphere pair, from the `mgrs` package (same as usng.rs).
const REFS: &[(f64, f64, &str)] = &[
    (10.75, 59.91, "32VNM9786842681"),       // north, east
    (-122.3321, 47.6062, "10TET5020072748"), // north, west
    (151.2093, -33.8688, "56HLH3436850948"), // south, east
    (-70.5, -33.4, "19HCD6050503362"),       // south, west
];
/// Polar (UPS) references from NGA GeoTrans (same as usng.rs).
const POLAR: &[(f64, f64, &str)] = &[(10.0, 89.5, "ZAG0963945331"), (-120.0, 89.5, "YZH5192427756"), (100.0, -89.5, "BAM5466890360"), (-100.0, -89.5, "AZM4533190360")];

fn latlon(lat: &str, lon: &str) -> LocationRequest {
    LocationRequest { kind: "latlon".into(), lat: lat.into(), lon: lon.into(), ..Default::default() }
}
fn fmt<'a>(r: &'a LocationResult, id: &str) -> &'a str {
    &r.formats.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("no format {id}")).text
}
fn metres(a: (f64, f64), b: (f64, f64)) -> f64 {
    Geodesic::wgs84().inverse(a.1, a.0, b.1, b.0)
}

#[test]
fn decimal_and_dms_inputs_read_the_same_in_all_four_hemispheres() {
    for (s, v) in [("34.052222", 34.052222), ("34°03′08″N", 34.052222), ("N 34 03 08", 34.052222), ("34 03 08 S", -34.052222), ("-34.052222", -34.052222)] {
        assert!((parse_angle(s, true).unwrap() - v).abs() < 1e-6, "{s}");
    }
    assert!((parse_angle("118 15 0 W", false).unwrap() + 118.25).abs() < 1e-12);
    assert!((parse_angle("118°15'00\"E", false).unwrap() - 118.25).abs() < 1e-12);
    assert!((parse_angle("W118.25", false).unwrap() + 118.25).abs() < 1e-12);
    // Refused: wrong axis letter, minus plus letter, minutes of 60, decimals before the last part, empty, comma.
    for (s, lat) in [("34E", true), ("118N", false), ("-34 S", true), ("34 61 00", true), ("34.5 10", true), ("", true), ("3,5", true), ("abc", true)] {
        assert!(parse_angle(s, lat).is_err(), "{s} should be refused");
    }
    assert!(check_lat_lon(91.0, 0.0).is_err() && check_lat_lon(0.0, 181.0).is_err() && check_lat_lon(90.0, 180.0).is_ok());
}

#[test]
fn latlon_in_each_hemisphere_gives_the_known_mgrs_and_comes_back_through_usng_and_utm() {
    let (e, _g) = engine();
    for (lon, lat, mgrs) in REFS {
        let r = e.parse_location(Some("EPSG:4326"), &latlon(&lat.to_string(), &lon.to_string())).unwrap();
        assert_eq!(fmt(&r, "mgrs"), *mgrs, "{lon} {lat}");
        assert!((r.lon - lon).abs() < 1e-12 && (r.lat - lat).abs() < 1e-12);
        assert_eq!(r.map_x, Some(*lon));
        assert!(r.reading.contains("WGS 84"), "{}", r.reading);
        // The same place typed as a grid reference lands within the 1 m cell.
        let u = e.parse_location(None, &LocationRequest { kind: "usng".into(), text: mgrs.to_string(), ..Default::default() }).unwrap();
        assert!(metres((u.lon, u.lat), (*lon, *lat)) < 1.5, "{mgrs}: {}", metres((u.lon, u.lat), (*lon, *lat)));
        // And through UTM: the zone text of the format, read back as zone/hemisphere/easting/northing.
        let t = fmt(&r, "utm").to_string(); // "Zone 32 N, easting 123.4 m, northing 567.8 m"
        let words: Vec<&str> = t.split(|c: char| c == ',' || c == ' ').filter(|w| !w.is_empty()).collect();
        let (zone, hemi, east, north) = (words[1], words[2], words[4], words[7]);
        let back = e.parse_location(None, &LocationRequest { kind: "utm".into(), zone: zone.into(), hemisphere: hemi.into(), easting: east.into(), northing: north.into(), ..Default::default() }).unwrap();
        assert!(metres((back.lon, back.lat), (*lon, *lat)) < 0.2, "{t}");
    }
    let r = e.parse_location(None, &latlon("34.05", "-118.25")).unwrap();
    assert_eq!(fmt(&r, "dd"), "34.050000, -118.250000");
    assert_eq!(fmt(&r, "dms"), "34°03′00.00″N 118°15′00.00″W");
}

#[test]
fn known_utm_zone_15_and_the_washington_monument() {
    let (e, _g) = engine();
    // 15T VK 99976 82994 is E 499976, N 4982994 in zone 15 (north).
    let r = e.parse_location(None, &LocationRequest { kind: "utm".into(), zone: "15".into(), hemisphere: "N".into(), easting: "499976.5".into(), northing: "4982994.5".into(), ..Default::default() }).unwrap();
    assert!(metres((r.lon, r.lat), (-93.0003, 45.0004)) < 2.0);
    assert_eq!(fmt(&r, "mgrs"), "15TVK9997682994");
    let w = e.parse_location(None, &LocationRequest { kind: "usng".into(), text: "18S UJ 23407 06483".into(), ..Default::default() }).unwrap();
    assert!((w.lon + 77.0353).abs() < 0.001 && (w.lat - 38.8895).abs() < 0.001);
    // Zone, hemisphere, easting and northing are checked.
    let bad = |zone: &str, h: &str, east: &str, north: &str, datum: &str| {
        e.parse_location(None, &LocationRequest { kind: "utm".into(), zone: zone.into(), hemisphere: h.into(), easting: east.into(), northing: north.into(), datum: datum.into(), ..Default::default() }).unwrap_err().to_string()
    };
    assert!(bad("61", "N", "500000", "4000000", "").contains("1 to 60"));
    assert!(bad("15", "", "500000", "4000000", "").contains("hemisphere"));
    assert!(bad("15", "N", "50", "4000000", "").contains("easting"));
    assert!(bad("15", "S", "500000", "400000", "").contains("southern"));
    assert!(bad("30", "N", "500000", "4000000", "nad83").contains("NAD 83 UTM exists only"));
}

#[test]
fn polar_references_read_and_write_back() {
    let (e, _g) = engine();
    for (lon, lat, mgrs) in POLAR {
        let r = e.parse_location(None, &latlon(&lat.to_string(), &lon.to_string())).unwrap();
        assert_eq!(fmt(&r, "mgrs"), *mgrs, "{lon} {lat}");
        assert!(fmt(&r, "utm").starts_with("Not in UTM"));
        let u = e.parse_location(None, &LocationRequest { kind: "usng".into(), text: mgrs.to_string(), ..Default::default() }).unwrap();
        assert!(metres((u.lon, u.lat), (*lon, *lat)) < 2.0, "{mgrs}: {}", metres((u.lon, u.lat), (*lon, *lat)));
    }
}

#[test]
fn swapped_and_out_of_range_input_is_refused_and_never_swapped() {
    let (e, _g) = engine();
    let err = |lat: &str, lon: &str| e.parse_location(None, &latlon(lat, lon)).unwrap_err().to_string();
    let m = err("-118.25", "34.05");
    assert!(m.contains("swapped") && m.contains("Nothing was changed"), "{m}");
    let m = err("95", "200");
    assert!(m.contains("beyond 90") && !m.contains("swapped"), "{m}");
    assert!(err("45", "181").contains("beyond 180"));
    assert!(err("", "10").contains("latitude"));
    // A valid pair is read exactly as typed (latitude first).
    let r = e.parse_location(None, &latlon("34.05", "-118.25")).unwrap();
    assert!((r.lat - 34.05).abs() < 1e-12 && (r.lon + 118.25).abs() < 1e-12);
}

#[test]
fn nad83_input_goes_through_an_explicit_transform() {
    let (e, _g) = engine();
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let want = engine::crs::transform_point(&c, -118.25, 34.05, "EPSG:4269", engine::crs::WGS84).unwrap();
    let r = e.parse_location(Some("EPSG:4269"), &LocationRequest { datum: "nad83".into(), ..latlon("34.05", "-118.25") }).unwrap();
    assert!((r.lon - want.0).abs() < 1e-12 && (r.lat - want.1).abs() < 1e-12);
    assert!(r.reading.contains("NAD 83") && r.reading.contains("explicit"), "{}", r.reading);
    // The map is NAD 83 too: the typed numbers are kept as they are.
    assert_eq!((r.map_x, r.map_y), (Some(-118.25), Some(34.05)));
    assert!(fmt(&r, "nad83").starts_with("34.050000, -118.250000"));
    assert!(e.parse_location(None, &LocationRequest { datum: "ed50".into(), ..latlon("1", "1") }).is_err());
}

#[test]
fn the_position_is_moved_into_the_map_crs_and_x_y_kinds_read_back() {
    let (e, _g) = engine();
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    // Web Mercator.
    let r = e.parse_location(Some("EPSG:3857"), &latlon("34.05", "-118.25")).unwrap();
    let (mx, my) = engine::crs::lonlat_to_merc(-118.25, 34.05);
    assert!((r.map_x.unwrap() - mx).abs() < 1e-3 && (r.map_y.unwrap() - my).abs() < 1e-3, "{r:?}");
    assert!(r.formats.iter().any(|f| f.id == "map"));
    // UTM zone 11 map.
    let r = e.parse_location(Some("EPSG:32611"), &latlon("34.05", "-118.25")).unwrap();
    let back = engine::crs::transform_point(&c, r.map_x.unwrap(), r.map_y.unwrap(), "EPSG:32611", engine::crs::WGS84).unwrap();
    assert!((back.0 + 118.25).abs() < 1e-8 && (back.1 - 34.05).abs() < 1e-8);
    // Map x/y: the central meridian of zone 14 at the equator.
    let xy = e.parse_location(Some("EPSG:32614"), &LocationRequest { kind: "xy".into(), x: "500000".into(), y: "0".into(), ..Default::default() }).unwrap();
    assert!((xy.lon + 99.0).abs() < 1e-9 && xy.lat.abs() < 1e-9);
    assert_eq!((xy.map_x, xy.map_y), (Some(500000.0), Some(0.0)));
    // Any other CRS: UTM 14 x/y on a Web Mercator map.
    let other = e.parse_location(Some("EPSG:3857"), &LocationRequest { kind: "crs".into(), crs: "EPSG:32614".into(), x: "500000".into(), y: "0".into(), ..Default::default() }).unwrap();
    assert!((other.lon + 99.0).abs() < 1e-9 && other.map_x.is_some());
    assert!(e.parse_location(None, &LocationRequest { kind: "crs".into(), x: "1".into(), y: "2".into(), ..Default::default() }).unwrap_err().to_string().contains("Choose the coordinate system"));
    assert!(e.parse_location(None, &LocationRequest { kind: "xy".into(), x: "1".into(), y: "2".into(), ..Default::default() }).unwrap_err().to_string().contains("no coordinate system"));
    assert!(e.parse_location(Some("EPSG:3857"), &LocationRequest { kind: "xy".into(), x: "abc".into(), y: "2".into(), ..Default::default() }).is_err());
}

#[test]
fn a_place_the_map_cannot_show_gets_a_sentence_and_no_map_position() {
    let (e, _g) = engine();
    // UTM zone 15 map, a place on the other side of the world.
    let r = e.parse_location(Some("EPSG:32615"), &latlon("10", "100")).unwrap();
    assert!(r.map_x.is_none() && r.map_message.as_deref().unwrap().contains("outside"), "{r:?}");
    assert_eq!(fmt(&r, "dd"), "10.000000, 100.000000", "the other formats are still given");
    // Web Mercator at the pole.
    let r = e.parse_location(Some("EPSG:3857"), &latlon("89", "10")).unwrap();
    assert!(r.map_x.is_none() && r.map_message.is_some(), "{r:?}");
    // A map with no coordinate system yet.
    let r = e.parse_location(None, &latlon("34", "-118")).unwrap();
    assert!(r.map_x.is_none() && r.map_message.as_deref().unwrap().contains("no coordinate system"), "{r:?}");
    let r = e.parse_location(Some("  "), &latlon("34", "-118")).unwrap();
    assert!(r.map_message.is_some());
}
