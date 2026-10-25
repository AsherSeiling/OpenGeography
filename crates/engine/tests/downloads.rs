//! Downloads without a network: dataset tables, input validation that must
//! reject bad requests before any request is made, Esri JSON conversion, and
//! that a refused download leaves nothing behind (docs/TEST_PLAN.md §Downloads).
//! Tests that need the internet are in engine.rs and at the bottom here (ignored).

use engine::{tiger, tnm_vector, Engine};
use serde_json::json;

fn noop(_: f64, _: &str) -> anyhow::Result<()> {
    Ok(())
}

#[test]
fn dataset_tables_are_consistent() {
    let mut seen = std::collections::HashSet::new();
    for d in tiger::DATASETS {
        assert!(seen.insert(d.id), "duplicate TIGER id {}", d.id);
        assert!(!d.label.is_empty());
        assert_eq!(tiger::dataset(d.id).unwrap().label, d.label);
        // Everything too large for a whole state must say so.
        if d.id == "blocks" || d.id == "local_roads" {
            assert!(d.needs_county, "{}", d.id);
        }
    }
    assert!(tiger::DATASETS.iter().any(|d| d.by_year) && tiger::DATASETS.iter().any(|d| !d.by_year));
    assert!(tiger::dataset("nope").is_err());
    let mut seen = std::collections::HashSet::new();
    for d in tnm_vector::DATASETS {
        assert!(seen.insert(d.id), "duplicate TNM id {}", d.id);
        assert!(!d.label.is_empty() && !d.group.is_empty(), "{}", d.id);
        assert_eq!(tnm_vector::dataset(d.id).unwrap().id, d.id);
    }
    assert!(tnm_vector::dataset("nope").is_err());
    assert!(tnm_vector::dataset("").is_err());
}

#[test]
fn tiger_requests_are_validated_before_any_download() {
    let dir = tempfile::tempdir().unwrap();
    let go = |id: &str, vintage: &str, state: &str, county: Option<&str>| tiger::fetch(id, vintage, state, county, dir.path(), &noop).map(|_| ()).unwrap_err().to_string();
    assert!(go("nope", "Current", "40", None).contains("Unknown TIGER dataset"));
    for bad in ["", "4", "400", "ab", "4a", "../", "40;", " 40", "٤٠"] {
        assert!(go("tracts", "Current", bad, None).contains("Invalid FIPS"), "state {bad:?}");
    }
    for bad in ["", "1", "1234", "abc", "../x"] {
        assert!(go("tracts", "Current", "40", Some(bad)).contains("Invalid FIPS"), "county {bad:?}");
    }
    assert!(go("blocks", "Current", "40", None).contains("choose a county"), "blocks need a county");
    assert!(go("local_roads", "Current", "40", None).contains("choose a county"));
    for bad in ["", "current", "Census", "Census20", "Census20201", "ACS20x3", "../etc", "Current/../x", "Census2020?f=json"] {
        assert!(go("tracts", bad, "40", None).contains("Unknown TIGER year"), "vintage {bad:?}");
        assert!(tiger::availability(bad).is_err(), "availability {bad:?}");
    }
    assert!(tiger::counties("4", "Current").is_err());
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none(), "no cache file for refused requests");
}

#[test]
fn tnm_requests_reject_empty_or_garbage_areas() {
    let dir = tempfile::tempdir().unwrap();
    for bbox in [[1.0, 1.0, 1.0, 2.0], [2.0, 1.0, 1.0, 2.0], [0.0, 2.0, 1.0, 1.0], [f64::NAN, 0.0, 1.0, 1.0], [0.0, 0.0, f64::NAN, 1.0]] {
        let err = tnm_vector::fetch("nhd_flowlines", bbox, dir.path(), &noop).unwrap_err().to_string();
        assert!(err.contains("area is empty"), "{bbox:?}: {err}");
        assert!(engine::tnm::download_elevation(bbox, 10.0, &noop).unwrap_err().to_string().contains("area is empty"), "{bbox:?}");
    }
    assert!(tnm_vector::fetch("not_a_dataset", [0.0, 0.0, 1.0, 1.0], dir.path(), &noop).is_err());
}

#[test]
fn a_refused_download_adds_no_layers_citations_or_lingering_tasks() {
    let e = Engine::new().unwrap();
    let err = e.tiger_import(&["bogus".into(), "tracts".into()], "Current", "40", None, "Offline area").unwrap_err().to_string();
    assert!(err.contains("Unknown TIGER dataset"), "{err}");
    let err = e.tiger_import(&["blocks".into()], "Current", "40", None, "Offline area").unwrap_err().to_string();
    assert!(err.contains("choose a county"), "{err}");
    assert!(e.tnm_import(&["bogus".into()], [0.0, 0.0, 1.0, 1.0], None, true, "Offline area").is_err());
    assert!(e.download_elevation([1.0, 1.0, 1.0, 1.0], 10.0, "Offline area").is_err());
    assert!(e.list_layers().is_empty());
    assert!(e.sources().unwrap().is_empty());
    assert!(engine::progress::snapshot().iter().all(|t| !t.label.contains("Offline area")), "tasks ended with the failed requests");
}

#[test]
fn esri_json_conversion_edge_cases() {
    use tnm_vector::esri_to_geojson as conv;
    assert!(conv(&json!(null)).is_none());
    assert!(conv(&json!({"rings": []})).is_none());
    assert!(conv(&json!({"rings": [[[0,0],[1,1],[0,0]]]})).is_none(), "a ring of fewer than 4 points is dropped");
    assert!(conv(&json!({"paths": []})).is_none());
    assert!(conv(&json!({"points": []})).is_none());
    assert_eq!(conv(&json!({"points": [[0,0],[1,1]]})).unwrap()["type"], "MultiPoint");
    // A lone counter-clockwise ring (unexpected orientation) still becomes a polygon rather than vanishing.
    let ccw = conv(&json!({"rings": [[[0,0],[10,0],[10,10],[0,10],[0,0]]]})).unwrap();
    assert!(ccw["type"] == "Polygon" || ccw["type"] == "MultiPolygon", "{ccw}");
    // An orphan hole (not inside any outer ring) is kept as its own polygon, not discarded.
    let orphan = conv(&json!({"rings": [[[0,0],[0,10],[10,10],[10,0],[0,0]], [[50,50],[52,50],[52,52],[50,52],[50,50]]]})).unwrap();
    assert_eq!(orphan["type"], "MultiPolygon");
    assert_eq!(orphan["coordinates"].as_array().unwrap().len(), 2);
    // x = 0, y = 0 is a point, not "missing".
    assert_eq!(conv(&json!({"x": 0.0, "y": 0.0})).unwrap()["coordinates"], json!([0.0, 0.0]));
    // Null coordinates (NaN markers in Esri JSON) don't panic.
    let _ = conv(&json!({"rings": [[[null,null],[0,10],[10,10],[10,0],[null,null]]]}));
}

// ------------------------------------------------------------------ need the internet (ignored)

/// National Map download that is cancelled part-way: no layer, no half-written file reused later.
#[test]
#[ignore = "network: needs hydro.nationalmap.gov"]
fn cancelling_a_national_map_download_leaves_nothing_behind() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let label = "Downloading from The National Map for Cancel test";
    let canceller = std::thread::spawn(move || {
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == label) {
                engine::progress::cancel(t.id);
                return true;
            }
        }
        false
    });
    let ids: Vec<String> = ["nhd_flowlines", "roads_local"].iter().map(|s| s.to_string()).collect();
    let r = e.tnm_import(&ids, [-97.8, 35.2, -97.2, 35.7], None, true, "Cancel test");
    assert!(canceller.join().unwrap(), "the download task appeared");
    assert!(r.is_err() || r.as_ref().is_ok_and(|x| x.layers.is_empty()), "a cancelled download imports nothing");
    assert!(e.list_layers().is_empty());
}

/// TIGER years: newest first; blocks only for decennial years and Current.
#[test]
#[ignore = "network: needs tigerweb.geo.census.gov"]
fn tiger_year_list_and_availability() {
    let vs = tiger::vintages().unwrap();
    assert_eq!(vs[0].id, "Current");
    assert!(vs.iter().filter(|v| v.decennial).all(|v| v.id.starts_with("Census")));
    assert!(vs.iter().map(|v| &v.id).collect::<std::collections::HashSet<_>>().len() == vs.len(), "no duplicate years");
    let a: std::collections::HashMap<_, _> = tiger::availability("Census2010").unwrap().into_iter().collect();
    assert!(a["blocks"] && a["counties"]);
    let c = tiger::counties("11", "Current").unwrap(); // District of Columbia: a single county-equivalent
    assert_eq!(c.len(), 1);
    assert!(tiger::counties("99", "Current").map_or(true, |v| v.is_empty()), "no such state: empty or an error, never invented counties");
}

/// 3DEP: a small area downloads at the requested resolution (a resolution below 1 m is raised to 1 m).
#[test]
#[ignore = "network: needs elevation.nationalmap.gov"]
fn elevation_download_small_area() {
    let e = Engine::new().unwrap();
    let l = e.download_elevation([-97.450, 35.200, -97.445, 35.204], 0.1, "Small area test").unwrap();
    let r = l.raster.unwrap();
    assert!((r.pixel_size - 1.0).abs() < 0.2, "pixel size {}", r.pixel_size);
    assert!(r.min > 250.0 && r.max < 450.0, "Norman, OK elevation range {} {}", r.min, r.max);
    assert!(l.name.contains("3DEP") && l.name.contains("Small area test"), "{}", l.name);
    assert!(l.crs.as_deref().is_some_and(|c| c.contains("32614")), "{:?}", l.crs);
    assert!(e.sources().unwrap().iter().any(|s| s.citation.title.contains("3DEP") || s.citation.title.to_lowercase().contains("elevation")));
}
