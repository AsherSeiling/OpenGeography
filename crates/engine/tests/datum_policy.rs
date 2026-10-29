//! CRS accuracy plan A3/A5 in an installation without the datum grids: the result says the shift is approximate (Warn), and Strict mode
//! refuses it. This file is its own process, so it can point PROJ at a folder that has `proj.db` but no grids before PROJ starts.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;
use std::path::PathBuf;

fn find_proj_db() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = vec![];
    for var in ["PROJ_DATA", "PROJ_LIB"] {
        if let Some(d) = std::env::var_os(var) {
            dirs.push(d.into());
        }
    }
    if let Some(p) = std::env::var_os("CONDA_PREFIX") {
        dirs.push(PathBuf::from(&p).join("share/proj"));
        dirs.push(PathBuf::from(&p).join("Library/share/proj"));
    }
    dirs.extend(["/opt/homebrew/share/proj", "/usr/local/share/proj", "/usr/share/proj"].map(PathBuf::from));
    dirs.into_iter().map(|d| d.join("proj.db")).find(|p| p.is_file())
}

#[test]
fn without_grids_the_shift_is_approximate_warns_and_strict_refuses() {
    let Some(db) = find_proj_db() else {
        eprintln!("SKIPPED: no proj.db found");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let proj = scratch.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::copy(&db, proj.join("proj.db")).unwrap();
    // Before anything starts PROJ: a data folder without grids, and the app-data folder in scratch space (the policy file lives there).
    std::env::set_var("PROJ_DATA", &proj);
    std::env::set_var("PROJ_LIB", &proj);
    std::env::set_var("GIS_STUDIO_DATA", scratch.path().join("data"));

    let e = Engine::new().unwrap();
    // A5: the coverage table is honest about the missing grids.
    let us = engine::transform_report::coverage().into_iter().find(|(n, _)| n.starts_with("United States: NAD27")).unwrap().1;
    if !us.grid_missing {
        // Some installs find datum grids next to the PROJ library whatever PROJ_DATA says (conda on Windows): nothing to test here.
        eprintln!("SKIPPED: this machine's PROJ finds grids outside PROJ_DATA ({us:?})");
        return;
    }
    assert!(us.warn && us.grid_missing, "{us:?}");
    assert!(us.text.contains("not installed"), "{}", us.text);

    // A layer in NAD27 near Oklahoma City.
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = scratch.path().join("n27.gpkg");
    c.execute_batch(&format!("COPY (SELECT ST_Point(-97.5 + i * 0.01, 35.4) AS geom FROM range(5) t(i)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'p', SRS 'EPSG:4267', USE_TMP_FILE false)", gpkg.display())).unwrap();
    let id = e.import(&gpkg).unwrap().remove(0).id;
    let run = |e: &Engine| {
        let g: Graph = serde_json::from_value(json!({
            "nodes": [{"id": "in0", "tool": "input_layer", "params": {"layer": id}}, {"id": "t", "tool": "project", "params": {"crs": "EPSG:4326"}}],
            "edges": [{"source": "in0", "target": "t", "targetHandle": "in"}]
        })).unwrap();
        e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
    };

    // Warn (the default): the step runs, and the result says the shift is approximate.
    assert!(!e.datum_policy_strict());
    let r = run(&e);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?}", r.error);
    assert!(r.message.as_deref().unwrap_or("").contains("Warning, datum change"), "{:?}", r.message);

    // Strict: refused, in words, and nothing is produced.
    e.set_datum_policy(true);
    assert!(e.datum_policy_strict());
    let r = run(&e);
    assert!(matches!(r.status, Status::Error), "{:?}", r.status);
    let err = r.error.unwrap_or_default();
    assert!(err.contains("Stopped") && err.contains("Strict mode"), "{err}");
    // The choice is remembered in the app-data folder.
    let saved = std::fs::read_to_string(scratch.path().join("data/datum_policy.txt")).unwrap();
    assert_eq!(saved.trim(), "strict");
    e.set_datum_policy(false);
    assert!(matches!(run(&e).status, Status::Done | Status::Cached));
}
