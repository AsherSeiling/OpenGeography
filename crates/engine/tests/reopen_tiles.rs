//! BUGS.md RT-1: concurrent first tile queries on a reopened project (a race was suspected; it never reproduced, the real cause is the next test).
//! This test reopens a project with several layers (with overviews and display tables) many times and hits tiles from many threads at once.
use engine::Engine;
use std::sync::{Arc, Barrier};

const LAYERS: usize = 3;
const THREADS: usize = 16;

fn tile_xy(lon: f64, lat: f64, z: u32) -> (u32, u32) {
    let n = 2f64.powi(z as i32);
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let y = ((1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor();
    (x as u32, y as u32)
}

#[test]
fn reopened_project_survives_concurrent_first_tiles() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", data.path());
    let proj = data.path().join("stress.gproj");
    let key;
    let ids: Vec<String>;
    {
        let e = Engine::new().unwrap();
        let src = tempfile::tempdir().unwrap();
        let c = engine::duckdb::Connection::open_in_memory().unwrap();
        engine::db::load_spatial(&c).unwrap();
        let mut v = vec![];
        for k in 0..LAYERS {
            let p = src.path().join(format!("p{k}.gpkg"));
            // 60 000 points: above the overview threshold, so each layer has overview tables with their own R-trees.
            c.execute_batch(&format!(
                "COPY (SELECT i::INTEGER AS id, ST_Point(-100.0 + {k} * 0.1 + (i % 600) * 0.0005, 30.0 + (i // 600) * 0.0005) AS geom FROM range(1, 60001) t(i)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)",
                p.display()
            ))
            .unwrap();
            let l = e.import(&p).unwrap().remove(0);
            assert!(!l.overviews.is_empty(), "the fixture must have overviews");
            e.build_display_blocking(&l.id, "EPSG:26914").unwrap();
            v.push(l.id);
        }
        key = e.crs_display_info("EPSG:26914", None).unwrap().crs_key;
        ids = v;
        e.save_project(&proj, "{}").unwrap();
    }
    let cycles: usize = std::env::var("STRESS_CYCLES").ok().and_then(|s| s.parse().ok()).unwrap_or(25);
    let t0 = std::time::Instant::now();
    for cycle in 0..cycles {
        let e = Arc::new(Engine::new().unwrap());
        e.open_project(&proj).unwrap();
        assert_eq!(e.crs_display_info("EPSG:26914", None).unwrap().crs_key, key); // the map's CRS is registered, as the app does before asking for tiles
        assert!(e.display_status(&ids[0], "EPSG:26914").unwrap().ready, "the saved display tables are used, not rebuilt");
        let barrier = Arc::new(Barrier::new(THREADS));
        let hs: Vec<_> = (0..THREADS)
            .map(|t| {
                let (e, b, id, key) = (e.clone(), barrier.clone(), ids[if std::env::var_os("STRESS_SAME_LAYER").is_some() { 0 } else { t % LAYERS }].clone(), key.clone());
                std::thread::spawn(move || {
                    b.wait();
                    for i in 0..4 {
                        let z = 4 + ((t + i) % 8) as u32; // different tiles per thread: nothing comes from the tile cache
                        let (x, y) = tile_xy(-99.9 + (t % LAYERS) as f64 * 0.1, 30.1, z);
                        e.tile(&id, z, x, y, &[]).unwrap();
                        if let Err(er) = e.tile_in_crs(&id, &key, z, x, y, &[]) {
                            panic!("display tile {z}/{x}/{y}: {er:#}");
                        }
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap_or_else(|_| panic!("a tile thread panicked in cycle {cycle}"));
        }
    }
    println!("{cycles} open cycles × {THREADS} threads: {:?}", t0.elapsed());
}

/// RT-1, the real cause: opening the project file that is already open in the engine (the app does this after Save As) made a second DuckDB
/// instance on one file; asking for a tile afterwards killed the process (SIGBUS in the R-tree). A display table built after the save is the
/// case that mattered: its index is only in the write-ahead log. With and without display tables, and tiles asked for straight after.
#[test]
fn reopening_the_project_that_is_already_open_keeps_working() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", data.path());
    for with_display in [false, true] {
        for i in 0..5 {
            let e = Engine::new().unwrap();
            let l = e.generate_test_layer("points", 5, [-100.0, 30.0, -90.0, 40.0]).unwrap();
            let dir = data.path().join(format!("p{with_display}{i}.gproj"));
            e.save_project(&dir, "{}").unwrap();
            if with_display {
                e.build_display_blocking(&l.id, "EPSG:4326").unwrap(); // written into the open project file after the save
            }
            e.open_project(&dir).unwrap(); // the same file, still open in this engine
            let di = e.crs_display_info("EPSG:4326", None).unwrap();
            e.tile(&l.id, 3, 1, 2, &[]).unwrap();
            if with_display {
                assert!(e.display_status(&l.id, "EPSG:4326").unwrap().ready, "the display table written after the save is still there");
                e.tile_in_crs(&l.id, &di.crs_key, 3, 1, 2, &[]).unwrap();
            }
            assert_eq!(e.layer(&l.id).unwrap().feature_count, 5);
        }
    }
}

/// Projects saved by earlier builds open and draw. By hand, on throwaway copies:
/// `OLD_PROJECTS="/path/a.gproj:/path/b.gproj" cargo test -p engine --test reopen_tiles old_projects -- --ignored --nocapture`
#[test]
#[ignore]
fn old_projects_open_and_draw_from_copies() {
    let list = std::env::var("OLD_PROJECTS").expect("OLD_PROJECTS");
    for src in list.split(':').filter(|s| !s.is_empty()) {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("copy.gproj");
        std::fs::create_dir_all(&dst).unwrap();
        for f in std::fs::read_dir(src).unwrap().flatten() {
            if f.path().is_file() {
                std::fs::copy(f.path(), dst.join(f.file_name())).unwrap();
            }
        }
        std::env::set_var("GIS_STUDIO_DATA", tmp.path());
        let e = Engine::new().unwrap();
        if let Err(er) = e.open_project(&dst) {
            println!("{src}: does not open ({er:#})");
            continue;
        }
        let ls = e.list_layers();
        let mut tiles = 0;
        for l in &ls {
            if l.has_geometry() && !l.is_raster() {
                for z in [2u32, 6] {
                    e.tile(&l.id, z, 1, 1, &[]).unwrap();
                    tiles += 1;
                }
            }
        }
        e.open_project(&dst).unwrap(); // and again in place
        println!("{src}: {} layers, {tiles} tiles, reopened in place", ls.len());
    }
}
