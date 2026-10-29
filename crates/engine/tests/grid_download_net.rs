//! Real download from the PROJ-data server (network). `cargo test -p engine --test grid_download_net -- --ignored`

#[test]
#[ignore = "network: cdn.proj.org"]
fn the_smallest_catalog_grid_downloads_verifies_installs_and_is_removed() {
    let scratch = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", scratch.path());
    let all = engine::grids::list();
    let smallest = all.iter().filter(|g| !g.installed).min_by_key(|g| g.size).expect("a grid to download").clone();
    assert!(smallest.size < 200_000, "{} is {} bytes", smallest.name, smallest.size);

    let got = engine::grids::download(&smallest.name, false).unwrap();
    assert!(got.installed && got.location == "downloaded", "{got:?}");
    let file = engine::resources::grid_dir().unwrap().join(&smallest.name);
    assert_eq!(std::fs::metadata(&file).unwrap().len(), smallest.size);
    assert!(!file.with_extension("tif.part").exists());

    engine::grids::remove(&smallest.name).unwrap();
    assert!(!file.exists());
    assert!(!engine::grids::list().into_iter().find(|g| g.name == smallest.name).unwrap().installed);
}
