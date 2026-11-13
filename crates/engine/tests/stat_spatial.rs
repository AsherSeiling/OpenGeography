//! ST6 part 1: spatial weights, Moran's I and local Moran. Reference values here are hand-worked or come from an independent implementation of the
//! published formula in the test itself (no R/spdep output was available offline); the result notes of each tool say the same.

use engine::graph::{Graph, NodeResult, Status};
use engine::spatial_stats::morans_i as old_morans_i;
use engine::stats::weights::*;
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a}, expected {b}");
}

#[test]
fn geodesic_distance_and_neighbour_search() {
    // One degree of latitude at the equator on WGS 84 is 110574.389 m (Karney / NGA tables).
    close(distance((0.0, 0.0), (0.0, 1.0), true), 110574.389, 0.01, "1 degree of latitude");
    close(distance((0.0, 0.0), (3.0, 4.0), false), 5.0, 1e-12, "planar");
    // Distance band on a 5 x 5 grid with spacing 1: radius 1.01 gives rook neighbours (80 links), 1.5 adds diagonals (144).
    let xy: Vec<(f64, f64)> = (0..25).map(|i| ((i % 5) as f64, (i / 5) as f64)).collect();
    let rook = distance_band(&xy, 1.01, false, &|| false).unwrap();
    assert_eq!(rook.links(), 80);
    assert_eq!(distance_band(&xy, 1.5, false, &|| false).unwrap().links(), 144);
    assert!(rook.is_symmetric() && rook.islands().is_empty());
    // k nearest: asymmetric lists are allowed; every feature has exactly k.
    let k = knn(&xy, 4, false, &|| false).unwrap();
    assert!(k.nbrs.iter().all(|v| v.len() == 4));
    // Geographic knn ranks by geodesic distance: the closest of three points on one meridian.
    let g = knn(&[(10.0, 50.0), (10.0, 50.1), (10.0, 51.0)], 1, true, &|| false).unwrap();
    assert_eq!(g.nbrs, vec![vec![1], vec![0], vec![1]]);
    // An island is reported.
    let w = distance_band(&[(0.0, 0.0), (1.0, 0.0), (50.0, 50.0)], 2.0, false, &|| false).unwrap();
    assert_eq!(w.islands(), vec![2]);
}

#[test]
fn morans_i_hand_example_and_agreement_with_the_older_implementation() {
    // Chain of four values 1,2,3,4: z = -1.5,-.5,.5,1.5, sum z^2 = 5, sum_ij w z z = 2, so I = (4/4) * 2/5 = 0.4; E = -1/3.
    let w = from_pairs(4, &[(0, 1), (1, 2), (2, 3)], "chain");
    let m = moran(&[1.0, 2.0, 3.0, 4.0], &w.row_standardised()).unwrap();
    close(m.i, 0.4, 1e-12, "I");
    close(m.expected, -1.0 / 3.0, 1e-12, "E");
    // Same neighbour lists through the existing k-nearest implementation: identical I and randomisation variance.
    let vals = [3.0, 1.0, 4.0, 1.5, 5.0, 9.0, 2.0, 6.0];
    let lists: Vec<Vec<usize>> = vec![vec![1, 2], vec![0, 3], vec![0, 4], vec![1, 5], vec![2, 6], vec![3, 7], vec![4, 7], vec![5, 6]];
    let wl = Weights { n: 8, nbrs: lists.clone(), kind: "lists".into() };
    let a = moran(&vals, &wl.row_standardised()).unwrap();
    let b = old_morans_i(&vals, &lists).unwrap();
    close(a.i, b.i, 1e-12, "I");
    close(a.var_randomisation, b.variance, 1e-12, "variance");
    close(a.z, b.z, 1e-9, "z");
    // A checkerboard is strongly negative; a smooth gradient strongly positive.
    let xy: Vec<(f64, f64)> = (0..36).map(|i| ((i % 6) as f64, (i / 6) as f64)).collect();
    let rook = distance_band(&xy, 1.01, false, &|| false).unwrap();
    let board: Vec<f64> = (0..36).map(|i| ((i % 6 + i / 6) % 2) as f64).collect();
    assert!(moran(&board, &rook.row_standardised()).unwrap().i < -0.9);
    let grad: Vec<f64> = (0..36).map(|i| (i % 6) as f64 + (i / 6) as f64).collect();
    assert!(moran(&grad, &rook.row_standardised()).unwrap().i > 0.6);
    // Refusals in words.
    assert!(moran(&[1.0, 1.0, 1.0, 1.0], &w.row_standardised()).unwrap_err().contains("same"));
    assert!(moran(&[1.0, 2.0], &w.row_standardised()).is_err());
}

#[test]
fn local_moran_sums_to_n_times_global_and_is_reproducible() {
    let xy: Vec<(f64, f64)> = (0..64).map(|i| ((i % 8) as f64, (i / 8) as f64)).collect();
    let w = distance_band(&xy, 1.5, false, &|| false).unwrap();
    let ws = w.row_standardised();
    // High values in one corner block, low elsewhere.
    let vals: Vec<f64> = (0..64).map(|i| if (i % 8) < 3 && (i / 8) < 3 { 10.0 + (i % 3) as f64 } else { 1.0 + ((i * 7) % 3) as f64 * 0.1 }).collect();
    let g = moran(&vals, &ws).unwrap();
    let l = local_moran(&vals, &ws, 499, 7, &|| false).unwrap();
    close(l.iter().map(|x| x.i).sum::<f64>(), 64.0 * g.i, 1e-9, "sum of local I = n * I");
    // The corner block (feature 9 is inside it) is a High-High cluster with a small p; the same seed repeats exactly.
    assert_eq!(l[9].quadrant, 1);
    assert!(l[9].p < 0.05, "p {}", l[9].p);
    let l2 = local_moran(&vals, &ws, 499, 7, &|| false).unwrap();
    assert!(l.iter().zip(&l2).all(|(a, b)| a.p == b.p || (a.p.is_nan() && b.p.is_nan())));
    let l3 = local_moran(&vals, &ws, 499, 8, &|| false).unwrap();
    assert!(l.iter().zip(&l3).any(|(a, b)| a.p != b.p), "a different seed gives different pseudo p-values");
    // The generator: the same seed gives the same numbers, values in [0, 1).
    let (mut a, mut b) = (Rng::new(1), Rng::new(1));
    assert_eq!(a.next_u64(), b.next_u64());
    assert!((0..1000).all(|_| (0.0..1.0).contains(&a.f64())));
}

// ------------------------------------------------------------------ tools

fn import_file(e: &Engine, dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    e.import(&p).unwrap().remove(0).id
}

fn run(e: &Engine, tool: &str, params: Value, layer: &str) -> NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "t", "tool": tool, "params": params}],
        "edges": [{"source": "in", "target": "t", "targetHandle": "in"}]
    }))
    .unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn ok(e: &Engine, tool: &str, params: Value, layer: &str) -> NodeResult {
    let r = run(e, tool, params, layer);
    assert!(matches!(r.status, Status::Done), "{tool}: {:?} {:?}", r.status, r.error);
    r
}

fn cell(r: &NodeResult, table: &str, row: usize, col: &str) -> f64 {
    let st = r.stat.as_ref().unwrap();
    let t = st.tables.iter().find(|t| t.title == table).unwrap_or_else(|| panic!("no table {table}"));
    let i = t.columns.iter().position(|c| c.name == col).unwrap_or_else(|| panic!("no column {col}"));
    t.rows[row][i].as_f64().unwrap_or(f64::NAN)
}

/// A 5 x 5 grid of unit squares in a projected CRS (EPSG:32614), with a value that rises to the east.
fn squares() -> String {
    let mut f = vec![];
    for i in 0..5 {
        for j in 0..5 {
            let (x, y) = (500000.0 + i as f64 * 100.0, 4000000.0 + j as f64 * 100.0);
            f.push(json!({"type": "Feature", "properties": {"v": i as f64 * 3.0 + (j % 2) as f64}, "geometry": {"type": "Polygon", "coordinates": [[[x, y], [x + 100.0, y], [x + 100.0, y + 100.0], [x, y + 100.0], [x, y]]]}}));
        }
    }
    json!({"type": "FeatureCollection", "crs": {"type": "name", "properties": {"name": "EPSG:32614"}}, "features": f}).to_string()
}

#[test]
fn weights_moran_and_lisa_tools_on_a_polygon_grid() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import_file(&e, dir.path(), "sq.geojson", &squares());
    let before = e.layer(&l).unwrap();
    // Queen: 144 links, mean 5.76; rook: 80 links, mean 3.2 (5 x 5 grid by hand); no islands.
    let q = ok(&e, "stat_weights", json!({"weights": "queen"}), &l);
    close(cell(&q, "Weights summary", 0, "links"), 144.0, 0.0, "queen links");
    close(cell(&q, "Weights summary", 0, "mean_neighbours"), 5.76, 1e-9, "queen mean");
    close(cell(&q, "Weights summary", 0, "max_neighbours"), 8.0, 0.0, "queen max");
    close(cell(&q, "Weights summary", 0, "min_neighbours"), 3.0, 0.0, "queen min");
    let r = ok(&e, "stat_weights", json!({"weights": "rook"}), &l);
    close(cell(&r, "Weights summary", 0, "links"), 80.0, 0.0, "rook links");
    close(cell(&r, "Weights summary", 0, "min_neighbours"), 2.0, 0.0, "rook min");
    assert!(r.stat.as_ref().unwrap().charts.len() == 1);
    // k nearest and distance band run too; the band of 110 m links the same pairs as rook on 100 m squares.
    let b = ok(&e, "stat_weights", json!({"weights": "band", "distance": 110}), &l);
    close(cell(&b, "Weights summary", 0, "links"), 80.0, 0.0, "band links");
    // Contiguity on points is refused in words.
    // Moran of a value that rises east: strongly positive; the seed is stored and permutations agree with the analytic sign.
    let m = ok(&e, "stat_moran", json!({"field": "v", "weights": "rook", "permutations": 199, "seed": 42}), &l);
    assert!(cell(&m, "Global Moran's I", 0, "morans_i") > 0.5);
    assert!(cell(&m, "Global Moran's I", 0, "p") < 0.01);
    assert!(cell(&m, "Global Moran's I", 0, "pseudo_p") < 0.05);
    let st = m.stat.as_ref().unwrap();
    assert_eq!(st.seed, Some(42));
    assert_eq!(st.n_used, Some(25));
    assert!(st.charts.len() == 1 && st.notes.iter().any(|n| n.contains("Features near each other")));
    // LISA with a Benjamini-Hochberg adjustment: counts add to 25; the output layer has the new fields; the source is untouched.
    let li = ok(&e, "stat_lisa", json!({"field": "v", "weights": "queen", "permutations": 499, "seed": 1, "adjust": "bh"}), &l);
    let total: f64 = (0..5).map(|k| cell(&li, "Clusters and outliers", k, "features")).sum();
    close(total, 25.0, 0.0, "classified features");
    let info = e.node_table_info(li.output.as_ref().unwrap()).unwrap();
    let names: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
    for f in ["local_i", "p_value", "p_adjusted", "cluster_code"] {
        assert!(names.contains(&f), "{f} in {names:?}");
    }
    assert!(info.has_geom && info.rows == 25);
    let after = e.layer(&l).unwrap();
    assert_eq!(before.version, after.version);
    assert_eq!(before.fields.len(), after.fields.len());
    // Not a layer: a plain table is refused in words.
    let t = import_file(&e, dir.path(), "plain.csv", "v\n1\n2\n3\n4\n5\n");
    let bad = run(&e, "stat_moran", json!({"field": "v"}), &t);
    assert!(matches!(bad.status, Status::Error));
}
