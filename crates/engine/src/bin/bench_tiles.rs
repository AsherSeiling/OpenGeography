//! Map performance gate: times cold vector-tile builds at each zoom level.
//!
//! cargo run --release -p engine --bin bench_tiles -- [polygons] [points]
//!
//! Targets: zooms 0-8 are pre-built in the background after import
//! (reported as total time); zooms >= 9 are built on demand and must have
//! p95 <= 50 ms per tile.

use engine::{tiles::tiles_for_extent, Engine};
use std::time::Instant;

const TEXAS: [f64; 4] = [-106.0, 26.0, -94.0, 36.0];
const TARGET_MS: f64 = 50.0;
const PREBUILT_MAX_ZOOM: u32 = 8;

fn main() -> anyhow::Result<()> {
    let args: Vec<u64> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let polys = args.first().copied().unwrap_or(700_000);
    let points = args.get(1).copied().unwrap_or(5_000_000);
    let e = Engine::new()?;
    let mut all_ok = true;
    for (kind, n) in [("polygons", polys), ("points", points)] {
        let t = Instant::now();
        let l = e.generate_test_layer(kind, n, TEXAS)?;
        println!("\n{n} {kind}: generated + indexed + overviews in {:.1}s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let built = e.prewarm(&l.id, PREBUILT_MAX_ZOOM, &[])?;
        println!("background pre-build of zooms 0-{PREBUILT_MAX_ZOOM}: {built} tiles in {:.1}s", t.elapsed().as_secs_f64());
        println!("{:>4} {:>6} {:>8} {:>8} {:>8} {:>10}", "zoom", "tiles", "p50 ms", "p95 ms", "max ms", "max KB");
        for z in [4u32, 6, 8, 10, 12, 14] {
            let mut tiles = tiles_for_extent(TEXAS, z);
            // Sample evenly so high zooms don't take forever.
            let step = (tiles.len() / 40).max(1);
            tiles = tiles.into_iter().step_by(step).take(40).collect();
            let mut ms = Vec::new();
            let mut max_kb = 0.0f64;
            for (x, y) in &tiles {
                let t = Instant::now();
                let tile = e.tile_uncached(&l.id, z, *x, *y, &[])?;
                ms.push(t.elapsed().as_secs_f64() * 1000.0);
                max_kb = max_kb.max(tile.len() as f64 / 1024.0);
            }
            ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p = |q: f64| ms[((ms.len() as f64 - 1.0) * q).round() as usize];
            let ok = z <= PREBUILT_MAX_ZOOM || p(0.95) <= TARGET_MS;
            all_ok &= ok;
            println!(
                "{z:>4} {:>6} {:>8.1} {:>8.1} {:>8.1} {:>10.0} {}",
                tiles.len(),
                p(0.5),
                p(0.95),
                ms[ms.len() - 1],
                max_kb,
                if z <= PREBUILT_MAX_ZOOM { "(pre-built)" } else if ok { "ok" } else { "SLOW" }
            );
        }
    }
    println!("\n{}", if all_ok { "PASS: on-demand tiles within target" } else { "FAIL: some zooms exceed target" });
    std::process::exit(if all_ok { 0 } else { 1 });
}
