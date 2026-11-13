//! ST6-10: Anselin's Columbus, Ohio data (49 neighbourhoods; tests/data/columbus) against the values published in the GeoDa and spdep
//! documentation: queen contiguity, row-standardised weights, CRIME ~ INC + HOVAL.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

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
    t.rows[row][i].as_f64().unwrap()
}

fn rel(got: f64, want: f64, what: &str) {
    let tol = 1e-3 * want.abs().max(1e-3);
    assert!((got - want).abs() <= tol, "{what}: got {got}, expected {want}");
}

/// Agreement with the independent numpy computation (see the comment in the test): 5 decimals.
fn exact(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() <= 2e-5 * want.abs().max(1.0), "{what}: got {got}, expected {want}");
}

fn columbus(e: &Engine) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/columbus/columbus.geojson");
    e.import(&p).unwrap().remove(0).id
}

#[test]
fn columbus_weights_moran_ols_lag_and_error_match_the_published_values() {
    let e = Engine::new().unwrap();
    let l = columbus(&e);
    // Queen contiguity of the 49 neighbourhoods: 236 links (average 4.82), no islands (spdep poly2nb on columbus).
    let w = ok(&e, "stat_weights", json!({"weights": "queen"}), &l);
    eprintln!("queen links {}", cell(&w, "Weights summary", 0, "links"));
    assert_eq!(cell(&w, "Weights summary", 0, "links"), 236.0);
    assert_eq!(cell(&w, "Weights summary", 0, "islands"), 0.0);
    // Global Moran's I of CRIME.
    let m = ok(&e, "stat_moran", json!({"field": "CRIME", "weights": "queen", "permutations": 0}), &l);
    rel(cell(&m, "Global Moran's I", 0, "morans_i"), 0.5, "Moran's I of CRIME (GeoDa shows 0.500)");
    exact(cell(&m, "Global Moran's I", 0, "morans_i"), 0.50019, "Moran's I of CRIME, exact");
    // OLS.
    let o = ok(&e, "stat_ols", json!({"dependent": "CRIME", "predictors": "INC,HOVAL", "residual_moran": "no"}), &l);
    rel(cell(&o, "Coefficients", 0, "estimate"), 68.619, "OLS intercept");
    rel(cell(&o, "Coefficients", 1, "estimate"), -1.5973, "OLS INC");
    rel(cell(&o, "Coefficients", 2, "estimate"), -0.27393, "OLS HOVAL");
    // Spatial lag and error with queen weights. The figures usually quoted for this data set (spdep: lag rho 0.43102, error lambda 0.56179;
    // Moran's I 0.5111) belong to the neighbour file Anselin drew for the book (spdep's col.gal.nb), which is not the same as the polygons'
    // queen contiguity (GeoDa's Moran's I for queen weights is 0.500, which this test does match). For the polygon queen weights the expected
    // values were computed independently with numpy: contiguity from shared polygon vertices, the full log-likelihood with numpy's
    // slogdet, maximised by grid and golden-section search, coefficients by least squares (script: tests/data/columbus/check_ml.py, needs numpy).
    let s = ok(&e, "stat_spatial_lag", json!({"dependent": "CRIME", "predictors": "INC,HOVAL", "weights": "queen"}), &l);
    exact(cell(&s, "Spatial parameter", 0, "estimate"), 0.42333, "lag rho");
    exact(cell(&s, "Coefficients", 0, "estimate"), 45.60325, "lag intercept");
    exact(cell(&s, "Coefficients", 1, "estimate"), -1.04873, "lag INC");
    exact(cell(&s, "Coefficients", 2, "estimate"), -0.26633, "lag HOVAL");
    let r = ok(&e, "stat_spatial_error", json!({"dependent": "CRIME", "predictors": "INC,HOVAL", "weights": "queen"}), &l);
    exact(cell(&r, "Spatial parameter", 0, "estimate"), 0.54675, "error lambda");
    exact(cell(&r, "Coefficients", 0, "estimate"), 60.27947, "error intercept");
    exact(cell(&r, "Coefficients", 1, "estimate"), -0.95731, "error INC");
    exact(cell(&r, "Coefficients", 2, "estimate"), -0.30456, "error HOVAL");
    // Rook weights (200 links) likewise.
    let s = ok(&e, "stat_spatial_lag", json!({"dependent": "CRIME", "predictors": "INC,HOVAL", "weights": "rook"}), &l);
    exact(cell(&s, "Spatial parameter", 0, "estimate"), 0.42281, "lag rho (rook)");
    exact(cell(&s, "Coefficients", 0, "estimate"), 45.26498, "lag intercept (rook)");
    let m = ok(&e, "stat_moran", json!({"field": "CRIME", "weights": "rook", "permutations": 0}), &l);
    exact(cell(&m, "Global Moran's I", 0, "morans_i"), 0.52367, "Moran's I (rook)");
}
