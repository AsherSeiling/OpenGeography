//! CRS accuracy plan B2: a grid placed in the user's grid folder is found by PROJ (GDAL's copy and DuckDB's own), so a download from the
//! Datum Grids menu takes effect without restarting. Own process: PROJ is pointed at a folder with `proj.db` and no grids first.

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
fn a_grid_in_the_user_folder_is_found_by_both_proj_copies_without_a_restart() {
    let Some(db) = find_proj_db() else {
        eprintln!("SKIPPED: no proj.db found");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    let proj = scratch.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::copy(&db, proj.join("proj.db")).unwrap();
    std::env::set_var("PROJ_DATA", &proj);
    std::env::set_var("PROJ_LIB", &proj);
    std::env::set_var("GIS_STUDIO_DATA", scratch.path().join("data"));
    std::env::remove_var("PROJ_USER_WRITABLE_DIRECTORY");

    let e = engine::Engine::new().unwrap();
    let grid_dir = engine::resources::grid_dir().unwrap();
    assert!(grid_dir.starts_with(scratch.path()), "{}", grid_dir.display());
    assert_eq!(std::env::var_os("PROJ_USER_WRITABLE_DIRECTORY").map(PathBuf::from), Some(grid_dir.clone()));

    // WGS 84 → NAD27 at Oklahoma City: with the grids this is the reference answer (r_d2 in crs_reference.rs).
    let want = (-97.499683431, 35.199935272);
    let before = e.transform_point(-97.5, 35.2, "EPSG:4326", "EPSG:4267").unwrap();
    assert!((before.0 - want.0).abs() > 1e-6 || (before.1 - want.1).abs() > 1e-6, "already exact without grids? {before:?}");

    // The Datum Grids menu would put these in the folder; here they are copied in from the repository's bundled set.
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/proj-grids");
    for g in ["us_noaa_conus.tif", "us_noaa_okhpgn.tif"] {
        std::fs::copy(assets.join(g), grid_dir.join(g)).unwrap();
    }
    let listed: Vec<_> = e.list_datum_grids().into_iter().filter(|g| g.installed).map(|g| (g.name, g.location)).collect();
    assert!(listed.iter().any(|(n, l)| n == "us_noaa_conus.tif" && l == "downloaded"), "{listed:?}");

    let after = e.transform_point(-97.5, 35.2, "EPSG:4326", "EPSG:4267").unwrap();
    assert!((after.0 - want.0).abs() < 5e-8 && (after.1 - want.1).abs() < 5e-8, "DuckDB's PROJ ignored the user folder: {after:?}");
    let r1 = e.transform_report_extent("EPSG:4267", "EPSG:4326", Some([-98.0, 35.0, -97.0, 36.0])).unwrap();
    assert!(!r1.grid_missing && !r1.warn, "GDAL's PROJ ignored the user folder: {r1:?}");
}
