//! CRS identity (docs/CRS_PLAN.md R2, RD-2): a CRS is the same by meaning, not by spelling.
//! Cases R-B1, R-B2, R-B3, R-B5, R-B7, R-B8 and the guard against CRS string comparisons.

use engine::crs;
use engine::Engine;
use gdal::spatial_ref::SpatialRef;

const ESRI_26915: &str = r#"PROJCS["NAD_1983_UTM_Zone_15N",GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",-93.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn spellings_of(epsg: u32) -> (String, String) {
    let s = SpatialRef::from_epsg(epsg).unwrap();
    (s.to_wkt().unwrap(), s.to_projjson().unwrap())
}

#[test]
fn r_b1_equivalent_spellings_are_one_crs_for_wgs84_and_utm() {
    let (wkt, json) = spellings_of(4326);
    let wgs84 = ["EPSG:4326", " epsg:4326 ", "urn:ogc:def:crs:EPSG::4326", "+proj=longlat +datum=WGS84 +no_defs", "OGC:CRS84", wkt.as_str(), json.as_str()];
    let k0 = crs::key(wgs84[0]).unwrap();
    for s in wgs84 {
        assert_eq!(crs::key(s).unwrap(), k0, "{s:.60}");
        assert!(crs::same(wgs84[0], s), "{s:.60}");
    }
    let (wkt, json) = spellings_of(26915);
    let utm = ["EPSG:26915", "epsg:26915", "urn:ogc:def:crs:EPSG::26915", "+proj=utm +zone=15 +datum=NAD83 +units=m +no_defs", "+proj=tmerc +lat_0=0 +lon_0=-93 +k=0.9996 +x_0=500000 +y_0=0 +datum=NAD83 +units=m +no_defs", wkt.as_str(), json.as_str(), ESRI_26915];
    let k1 = crs::key(utm[0]).unwrap();
    for s in utm {
        assert!(crs::same(utm[0], s), "{s:.80}");
        assert_eq!(crs::key(s).unwrap(), k1, "{s:.80}");
    }
}

#[test]
fn r_b2_different_crss_are_never_equal() {
    let pairs = [
        ("EPSG:4326", "EPSG:4269"),
        ("EPSG:4267", "EPSG:4269"),
        ("EPSG:26915", "EPSG:32615"),
        ("EPSG:2277", "EPSG:32139"),
        ("EPSG:2277", "+proj=lcc +lat_1=31.88333333333333 +lat_2=30.11666666666667 +lat_0=29.66666666666667 +lon_0=-100.3333333333333 +x_0=600000 +y_0=4000000 +datum=NAD83 +units=ft +no_defs"), // international foot, not US survey
        ("EPSG:3857", "EPSG:3395"),
        ("EPSG:32615", "EPSG:32616"),
        ("EPSG:4326", "EPSG:4979"), // 2D and 3D
        ("+proj=utm +zone=15 +datum=NAD83 +units=m", "+proj=utm +zone=15 +datum=NAD83 +units=us-ft"),
    ];
    for (a, b) in pairs {
        assert!(!crs::same(a, b), "{a:.50} and {b:.50} are different CRSs");
        assert_ne!(crs::key(a).unwrap(), crs::key(b).unwrap());
    }
}

#[test]
fn r_b3_esri_prj_is_recognised_as_its_epsg_twin() {
    assert!(crs::same(ESRI_26915, "EPSG:26915"));
    assert!(!crs::same(ESRI_26915, "EPSG:32615"), "the same projection on another datum is not the same CRS");
}

#[test]
fn r_b5_bad_definitions_are_errors_with_a_sentence_and_no_sql_runs() {
    let long = "x".repeat(100_000);
    for bad in ["", "   ", "EPSG:0", "EPSG:99999999", "+proj=bogus", "PROJCS[\"truncated\",GEOGCS[", long.as_str()] {
        let err = crs::key(bad).expect_err(&format!("{bad:.30} must be refused"));
        assert!(err.to_string().len() < 2000 && !err.to_string().is_empty(), "{err}");
        assert!(!crs::same(bad, "EPSG:4326"), "a bad definition never matches a good one");
    }
    // The same text twice is "the same" only as text; never matches anything else.
    assert!(crs::same("+proj=bogus", "+proj=bogus"));
    // An injection attempt is refused and executes nothing.
    let e = Engine::new().unwrap();
    e.with_conn_for_tests(|c| {
        c.execute_batch("CREATE TABLE crs_guard_x (a INTEGER); INSERT INTO crs_guard_x VALUES (1)")?;
        Ok(())
    })
    .unwrap();
    for evil in ["EPSG:4326'); DROP TABLE crs_guard_x;--", "'; DROP TABLE crs_guard_x; --"] {
        assert!(e.validate_crs(evil).is_err(), "{evil}");
        assert!(e.transform_point(1.0, 1.0, evil, "EPSG:4326").is_err());
    }
    let n: i64 = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM crs_guard_x", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 1, "the table is still there");
}

#[test]
fn r_b7_key_is_stable_across_parameter_order_and_runs() {
    let a = "+proj=lcc +lat_1=33 +lat_2=45 +lat_0=39 +lon_0=-96 +x_0=0 +y_0=0 +datum=NAD83 +units=m +no_defs";
    let b = "+units=m +no_defs +datum=NAD83 +y_0=0 +x_0=0 +lon_0=-96 +lat_0=39 +lat_2=45 +lat_1=33 +proj=lcc";
    assert_eq!(crs::key(a).unwrap(), crs::key(b).unwrap());
    assert_eq!(crs::key(a).unwrap(), crs::key(a).unwrap());
    // Pinned: a change here means every cached display table is rebuilt (and a different key on another OS is a bug).
    assert_eq!(crs::key("EPSG:4326").unwrap().len(), 12);
    assert!(crs::key("EPSG:4326").unwrap().chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn r_b8_wgs84_realisations_and_ensembles_are_epsg_4326() {
    assert!(crs::same("EPSG:4326", "EPSG:9057"), "WGS 84 (G1762) is WGS 84");
    // A realisation is not another datum such as NAD83 or ETRS89.
    assert!(!crs::same("EPSG:9057", "EPSG:4269"));
    assert!(!crs::same("EPSG:4326", "EPSG:4258"));
}

#[test]
fn r_b_axis_order_is_not_part_of_the_identity() {
    // EPSG:3035 and EPSG:2193 are north/east on paper; their east/north spelling is the same CRS.
    let s = SpatialRef::from_epsg(3035).unwrap();
    assert!(crs::same("EPSG:3035", &s.to_wkt().unwrap()));
    assert!(crs::same("EPSG:4326", "OGC:CRS84"), "lat/lon and lon/lat spellings of WGS 84");
}

/// Guard (R-B, G1): the engine never compares two CRS strings itself; it asks `crs::same`.
#[test]
fn no_crs_string_comparison_is_left_in_the_engine() {
    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    rust_files(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut hits = Vec::new();
    for f in files {
        if f.ends_with("crs.rs") {
            continue;
        }
        for (n, line) in std::fs::read_to_string(&f).unwrap().lines().enumerate() {
            let l = line.trim();
            if l.starts_with("//") {
                continue;
            }
            let crs_compare = (l.contains("crs") || l.contains("srs")) && (l.contains("eq_ignore_ascii_case(") || l.contains("crs == ") || l.contains("crs != ") || l.contains("crs) == ") || l.contains("resolve(&") && l.contains(") == "));
            if crs_compare && !l.contains("crs::same") {
                hits.push(format!("{}:{}: {l}", f.display(), n + 1));
            }
        }
    }
    assert!(hits.is_empty(), "compare CRSs with crs::same:\n{}", hits.join("\n"));
}

/// A PROJ string that names an ellipsoid and a `+towgs84` shift instead of a datum is a custom datum to PROJ. Calling it "the
/// same" as WGS 84 or NAD83 could hide a real shift, so identity says "different" (the safe direction: a tool then reprojects,
/// which for a null shift changes nothing).
#[test]
fn r_b_a_custom_datum_string_is_not_assumed_to_be_a_named_datum() {
    assert!(!crs::same("EPSG:4326", "+proj=longlat +ellps=WGS84 +towgs84=0,0,0 +no_defs"));
    assert!(!crs::same("EPSG:26915", "+proj=utm +zone=15 +ellps=GRS80 +towgs84=0,0,0 +units=m +no_defs"));
    // Equal customs are equal to each other.
    assert!(crs::same("+proj=longlat +ellps=WGS84 +towgs84=0,0,0 +no_defs", "+towgs84=0,0,0 +no_defs +ellps=WGS84 +proj=longlat"));
}

#[test]
fn crs_info_describes_codes_and_custom_definitions() {
    let i = engine::crs::info("EPSG:26914").unwrap();
    assert_eq!(i.code.as_deref(), Some("EPSG:26914"));
    assert!(!i.geographic);
    assert_eq!(i.unit.to_lowercase(), "metre");
    assert!(i.name.contains("UTM zone 14N"), "{}", i.name);
    assert_eq!(i.datum.as_deref(), Some("North American Datum 1983"));
    let a = i.area_of_use.expect("area of use");
    assert!(a[0] < -100.0 && a[2] >= -96.0 && a[2] < -90.0 && a[1] < a[3]);

    let g = engine::crs::info("EPSG:4326").unwrap();
    assert!(g.geographic);
    assert_eq!(g.unit.to_lowercase(), "degree");

    // A custom PROJ string has no code and still reads.
    let c = engine::crs::info("+proj=utm +zone=14 +datum=NAD83 +units=us-ft").unwrap();
    assert!(c.code.is_none() && !c.geographic);
    assert!(c.unit.to_lowercase().contains("foot"), "{}", c.unit);

    let e = engine::crs::info("not a crs").unwrap_err().to_string();
    assert!(e.contains("not a crs"), "{e}");
}

#[test]
fn map_factors_match_utm_theory() {
    let e = engine::Engine::new().unwrap();
    // On the central meridian of UTM 14N (99°W) k is 0.9996 and the convergence is 0.
    let (x, y) = e.transform_point(-99.0, 35.0, "EPSG:4326", "EPSG:26914").unwrap();
    let (k, g) = e.map_factors("EPSG:26914", x, y).unwrap();
    assert!((k - 0.9996).abs() < 1e-5, "{k}");
    assert!(g.abs() < 1e-4, "{g}");
    // 2° east at 35°N: convergence ≈ Δλ·sin φ = 1.147°.
    let (x, y) = e.transform_point(-97.0, 35.0, "EPSG:4326", "EPSG:26914").unwrap();
    let (_, g) = e.map_factors("EPSG:26914", x, y).unwrap();
    assert!((g - 1.147).abs() < 0.01, "{g}");
}
