//! Split by attribute (B6, docs/DECISIONS.md): one layer or one file per value.

use engine::split::{safe_name, SplitTarget, NO_VALUE};
use engine::Engine;
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{srs}', USE_TMP_FILE false)", path.display())).unwrap();
}

/// Six points in UTM 15N: classes a ×3, b ×1, "A/B" ×1, NULL ×1.
fn counties(e: &Engine, dir: &Path) -> String {
    let p = dir.join("counties.gpkg");
    write_fixture(
        &p,
        "SELECT * FROM (VALUES (1, 'a', 10.0, ST_Point(500000, 4500000)), (2, 'b', 20.0, ST_Point(500100, 4500000)), (3, 'a', 30.0, ST_Point(500200, 4500000)),
                              (4, 'A/B', 40.0, ST_Point(500300, 4500000)), (5, NULL, 50.0, ST_Point(500400, 4500000)), (6, 'a', 60.0, ST_Point(500500, 4500000))) t(id, cls, pop, geom)",
        "EPSG:26915",
    );
    e.import(&p).unwrap()[0].id.clone()
}

fn rows(e: &Engine, id: &str) -> i64 {
    e.layer(id).unwrap().feature_count
}

#[test]
fn b6_groups_count_rows_and_name_null_as_no_value() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path());
    let g = e.split_groups(&id, "cls").unwrap();
    let got: Vec<(&str, i64)> = g.iter().map(|g| (g.value.as_str(), g.rows)).collect();
    assert_eq!(got, vec![("A/B", 1), ("a", 3), ("b", 1), (NO_VALUE, 1)]);
    assert!(e.split_groups(&id, "nope").unwrap_err().to_string().contains("no field called 'nope'"));
}

#[test]
fn b6_layers_one_per_value_keep_crs_sources_and_rows() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path());
    let r = e.split_by_attribute(&id, "cls", &SplitTarget::Layers, false).unwrap();
    assert_eq!(r.layers.len(), 4);
    assert_eq!(r.skipped_rows, 0);
    let by = |v: &str| r.outputs.iter().position(|o| o.value == v).unwrap();
    assert_eq!(rows(&e, &r.layers[by("a")].id), 3);
    assert_eq!(rows(&e, &r.layers[by(NO_VALUE)].id), 1);
    assert_eq!(r.layers.iter().map(|l| l.feature_count).sum::<i64>(), 6, "no row lost, none doubled");
    for l in &r.layers {
        assert_eq!(l.crs.as_deref(), Some("EPSG:26915"), "CRS kept");
    }
    assert!(r.layers[by("A/B")].name.contains("A/B"), "layer names keep the value as written");
    let cited = format!("{:?}", e.sources().unwrap());
    for l in &r.layers {
        assert!(cited.contains(&l.name), "citations carried to {}", l.name);
    }
    assert!(r.notes.iter().any(|n| n.contains("native:splitvectorlayer")));
    // The input is untouched.
    assert_eq!(rows(&e, &id), 6);
}

#[test]
fn b6_a_definition_query_limits_the_split() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path());
    e.set_definition_query(&id, Some("pop > 25")).unwrap();
    let g = e.split_groups(&id, "cls").unwrap();
    assert_eq!(g.iter().map(|g| g.rows).sum::<i64>(), 4, "rows hidden by the query are not split");
    let r = e.split_by_attribute(&id, "cls", &SplitTarget::Layers, false).unwrap();
    assert_eq!(r.layers.iter().map(|l| l.feature_count).sum::<i64>(), 4);
}

#[test]
fn b6_files_one_per_value_with_safe_unique_names_and_the_crs() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let id = counties(&e, dir.path());
    let out = dir.path().join("out");
    // A file with a clashing name already there is never overwritten.
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("a.gpkg"), b"keep me").unwrap();
    let r = e.split_by_attribute(&id, "cls", &SplitTarget::Files { dir: out.clone(), format: "GPKG".into() }, false).unwrap();
    assert_eq!(std::fs::read(out.join("a.gpkg")).unwrap(), b"keep me");
    let names: Vec<String> = std::fs::read_dir(&out).unwrap().map(|f| f.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(names.contains(&"a_2.gpkg".to_string()) && names.contains(&"A_B.gpkg".to_string()) && names.contains(&"b.gpkg".to_string()), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("_no value_")), "the NULL group gets a file too: {names:?}");
    // Each file reads back with its rows and the CRS.
    let a2 = e.import(&out.join("a_2.gpkg")).unwrap();
    assert_eq!((a2[0].feature_count, a2[0].crs.as_deref()), (3, Some("EPSG:26915")));
    assert_eq!(r.outputs.len(), 4);
    // The input file is untouched.
    assert_eq!(rows(&e, &id), 6);
}

#[test]
fn b6_shapefile_output_and_no_clash_between_case_variants() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("t.gpkg");
    write_fixture(&p, "SELECT * FROM (VALUES (1, 'x', ST_Point(0, 0)), (2, 'X', ST_Point(1, 1)), (3, 'x?', ST_Point(2, 2)), (4, 'x:', ST_Point(3, 3))) t(id, v, geom)", "EPSG:4326");
    let id = e.import(&p).unwrap()[0].id.clone();
    let out = dir.path().join("shp");
    let r = e.split_by_attribute(&id, "v", &SplitTarget::Files { dir: out.clone(), format: "ESRI Shapefile".into() }, false).unwrap();
    assert_eq!(r.outputs.len(), 4);
    let mut shp: Vec<String> = std::fs::read_dir(&out).unwrap().filter_map(|f| f.ok()).map(|f| f.file_name().to_string_lossy().to_lowercase()).filter(|n| n.ends_with(".shp")).collect();
    shp.sort();
    shp.dedup();
    assert_eq!(shp.len(), 4, "four distinct files even on a case-insensitive file system: {shp:?}");
}

#[test]
fn b6_safe_names() {
    assert_eq!(safe_name("Harris County"), "Harris County");
    assert_eq!(safe_name("A/B\\C:D*E?"), "A_B_C_D_E_");
    assert_eq!(safe_name("  .. "), "empty");
    assert_eq!(safe_name(""), "empty");
    assert_eq!(safe_name("CON"), "_CON");
    assert_eq!(safe_name("nul.txt"), "_nul.txt");
    assert!(safe_name(&"x".repeat(300)).len() <= 100);
}

#[test]
fn b6_limits_confirm_and_cap() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("many.gpkg");
    write_fixture(&p, "SELECT x AS id, 'v' || x AS v, ST_Point(x, 0) AS geom FROM range(1001) t(x)", "EPSG:4326");
    let id = e.import(&p).unwrap()[0].id.clone();
    let err = e.split_by_attribute(&id, "v", &SplitTarget::Layers, true).unwrap_err().to_string();
    assert!(err.contains("1001 outputs") && err.contains("limit is 1000"), "{err}");
    assert!(e.list_layers().len() == 1, "nothing was made");
    let q = dir.path().join("two_hundred.gpkg");
    write_fixture(&q, "SELECT x AS id, 'v' || x AS v, ST_Point(x, 0) AS geom FROM range(201) t(x)", "EPSG:4326");
    let id2 = e.import(&q).unwrap()[0].id.clone();
    let out = dir.path().join("o");
    let target = SplitTarget::Files { dir: out.clone(), format: "GPKG".into() };
    let err = e.split_by_attribute(&id2, "v", &target, false).unwrap_err().to_string();
    assert!(err.contains("201 outputs") && err.contains("Confirm"), "{err}");
    assert!(!out.exists() || std::fs::read_dir(&out).unwrap().next().is_none(), "nothing written before the user confirms");
    assert_eq!(e.split_by_attribute(&id2, "v", &target, true).unwrap().outputs.len(), 201);
}

#[test]
fn b6_cancel_leaves_nothing_behind_and_the_engine_keeps_working() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("cancel_me_b6.gpkg");
    write_fixture(&p, "SELECT x AS id, 'v' || x AS v, ST_Point(x, 0) AS geom FROM range(1000) t(x)", "EPSG:4326");
    let id = e.import(&p).unwrap()[0].id.clone();
    let before = e.list_layers().len();
    let canceller = std::thread::spawn(|| {
        for _ in 0..4000 {
            std::thread::sleep(std::time::Duration::from_millis(2));
            if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label.contains("cancel_me_b6")) {
                std::thread::sleep(std::time::Duration::from_millis(100));
                assert!(t.cancellable);
                engine::progress::cancel(t.id);
                return true;
            }
        }
        false
    });
    let err = e.split_by_attribute(&id, "v", &SplitTarget::Layers, true).unwrap_err().to_string();
    assert!(canceller.join().unwrap(), "the task was seen running");
    assert!(err.to_lowercase().contains("cancel"), "{err}");
    assert_eq!(e.list_layers().len(), before, "layers made before the cancel are removed");
    assert!(engine::progress::snapshot().iter().all(|t| !t.label.contains("cancel_me_b6")), "task removed");
    assert_eq!(e.split_groups(&id, "v").unwrap().len(), 1000, "still usable");
}
