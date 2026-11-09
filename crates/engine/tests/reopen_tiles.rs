//! BUGS.md: after a project is reopened its R-tree indexes are bound lazily by the first query that could use them; tile threads
//! doing that at once on one table crashed the process. `Engine::init_schema` now binds them all before anything can draw.
//! This test reopens a project with several layers (with overviews and display tables) many times and hits tiles from many threads at once.
//! `GIS_STUDIO_SKIP_RTREE_BIND=1 cargo test -p engine --test reopen_tiles -- --nocapture` runs it without the fix.
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

/// Open time and first-tile time of a reopened project with a million points (overviews, display table). Run by hand, with and without
/// `GIS_STUDIO_SKIP_RTREE_BIND=1`: the binding cost moves from the first tile into the open.
/// `cargo test -p engine --test reopen_tiles bind_timing -- --ignored --nocapture`
#[test]
#[ignore]
fn bind_timing_on_a_million_points() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", data.path());
    let proj = data.path().join("big.gproj");
    let skip = std::env::var_os("GIS_STUDIO_SKIP_RTREE_BIND");
    std::env::remove_var("GIS_STUDIO_SKIP_RTREE_BIND");
    let (id, key);
    {
        let e = Engine::new().unwrap();
        let p = data.path().join("big.gpkg");
        let c = engine::duckdb::Connection::open_in_memory().unwrap();
        engine::db::load_spatial(&c).unwrap();
        c.execute_batch(&format!("COPY (SELECT i::INTEGER AS id, ST_Point(-100.0 + (i % 1000) * 0.001, 30.0 + (i // 1000) * 0.001) AS geom FROM range(1, 1000001) t(i)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", p.display())).unwrap();
        let l = e.import(&p).unwrap().remove(0);
        e.build_display_blocking(&l.id, "EPSG:26914").unwrap();
        key = e.crs_display_info("EPSG:26914", None).unwrap().crs_key;
        id = l.id;
        e.save_project(&proj, "{}").unwrap();
    }
    if let Some(v) = skip {
        std::env::set_var("GIS_STUDIO_SKIP_RTREE_BIND", v);
    }
    let e = Engine::new().unwrap();
    let t = std::time::Instant::now();
    e.open_project(&proj).unwrap();
    println!("open_project: {:?} (bind skipped: {})", t.elapsed(), std::env::var_os("GIS_STUDIO_SKIP_RTREE_BIND").is_some());
    e.crs_display_info("EPSG:26914", None).unwrap();
    let t0 = e.layer(&id).unwrap().table;
    let plan: String = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("EXPLAIN SELECT count(*) FROM \"{t0}\" WHERE ST_Intersects(__g, ST_MakeEnvelope(1e12, 1e12, 1e12 + 1, 1e12 + 1))"), [], |r| r.get::<_, String>(1))?)).unwrap();
    println!("probe plan uses the R-tree: {}", plan.contains("RTREE_INDEX_SCAN"));
    let t = std::time::Instant::now();
    e.tile(&id, 8, 57, 105, &[]).unwrap();
    println!("first tile: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    e.tile_in_crs(&id, &key, 8, 57, 105, &[]).unwrap();
    println!("first display tile: {:?}", t.elapsed());
}
