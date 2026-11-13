//! Spatial regression tools end to end on a 12 x 12 grid of polygons: spatial lag, spatial error, GWR and MGWR, their output layers, and the
//! guarantee that the source layer is never changed.

use engine::graph::{Graph, NodeResult, Status};
use engine::stats::weights::Rng;
use engine::Engine;
use nalgebra::{DMatrix, DVector};
use serde_json::{json, Value};

const SIDE: usize = 12;

fn normal(rng: &mut Rng) -> f64 {
    let (u, v) = (rng.f64().max(1e-12), rng.f64());
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

/// Rook neighbour matrix, row-standardised, of the grid.
fn rook() -> DMatrix<f64> {
    let n = SIDE * SIDE;
    let mut w = DMatrix::zeros(n, n);
    for i in 0..n {
        let (x, y) = ((i % SIDE) as i64, (i / SIDE) as i64);
        let nb: Vec<usize> = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter_map(|(dx, dy)| {
            let (a, b) = (x + dx, y + dy);
            if a >= 0 && b >= 0 && a < SIDE as i64 && b < SIDE as i64 { Some((b as usize) * SIDE + a as usize) } else { None }
        }).collect();
        for j in &nb {
            w[(i, *j)] = 1.0 / nb.len() as f64;
        }
    }
    w
}

/// kind "lag": y = (I - rho W)^-1 (1 + 2 x + e); "error": y = 1 + 2 x + (I - lam W)^-1 e; "gwr": y = (1 + 0.3 col) x + e.
fn layer_text(kind: &str) -> String {
    let n = SIDE * SIDE;
    let mut rng = Rng::new(77);
    let x: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    let e = DVector::from_fn(n, |_, _| normal(&mut rng) * 0.4);
    let w = rook();
    let xb = DVector::from_fn(n, |i, _| 1.0 + 2.0 * x[i]);
    let a = DMatrix::identity(n, n) - &w * 0.6;
    let ainv = a.try_inverse().unwrap();
    let y: Vec<f64> = match kind {
        "lag" => (&ainv * (&xb + &e)).iter().copied().collect(),
        "error" => (&xb + &ainv * &e).iter().copied().collect(),
        _ => (0..n).map(|i| 1.0 + (1.0 + 0.3 * (i % SIDE) as f64) * x[i] + 0.2 * e[i]).collect(),
    };
    let mut f = vec![];
    for i in 0..n {
        let (cx, cy) = (500000.0 + (i % SIDE) as f64 * 100.0, 4000000.0 + (i / SIDE) as f64 * 100.0);
        f.push(json!({"type": "Feature", "properties": {"y": y[i], "x": x[i]}, "geometry": {"type": "Polygon", "coordinates": [[[cx, cy], [cx + 100.0, cy], [cx + 100.0, cy + 100.0], [cx, cy + 100.0], [cx, cy]]]}}));
    }
    json!({"type": "FeatureCollection", "crs": {"type": "name", "properties": {"name": "EPSG:32614"}}, "features": f}).to_string()
}

fn import(e: &Engine, dir: &std::path::Path, name: &str, text: &str) -> String {
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
    let t = st.tables.iter().find(|t| t.title == table).unwrap_or_else(|| panic!("no table {table}: {:?}", st.tables.iter().map(|t| &t.title).collect::<Vec<_>>()));
    let i = t.columns.iter().position(|c| c.name == col).unwrap_or_else(|| panic!("no column {col}"));
    t.rows[row][i].as_f64().unwrap_or(f64::NAN)
}

fn fields_of(e: &Engine, r: &NodeResult) -> Vec<String> {
    e.node_table_info(r.output.as_ref().unwrap()).unwrap().fields.iter().map(|f| f.name.clone()).collect()
}

#[test]
fn lag_and_error_tools_estimate_the_spatial_parameter_and_leave_the_source_alone() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import(&e, dir.path(), "lag.geojson", &layer_text("lag"));
    let before = e.layer(&l).unwrap();
    let r = ok(&e, "stat_spatial_lag", json!({"dependent": "y", "predictors": "x", "weights": "rook"}), &l);
    let rho = cell(&r, "Spatial parameter", 0, "estimate");
    assert!((rho - 0.6).abs() < 0.15, "rho {rho}");
    assert!(cell(&r, "Spatial parameter", 0, "p") < 0.001);
    assert!((cell(&r, "Coefficients", 1, "estimate") - 2.0).abs() < 0.3);
    assert!(cell(&r, "Model fit", 0, "lr_p") < 0.001);
    let st = r.stat.as_ref().unwrap();
    assert_eq!(st.n_used, Some(144));
    assert!(st.notes.iter().any(|n| n.contains("no R (spdep")), "verification statement");
    assert!(st.notes.iter().any(|n| n.contains("Features near each other")));
    let f = fields_of(&e, &r);
    for c in ["fitted", "residual", "innovation"] {
        assert!(f.contains(&c.to_string()), "{c} in {f:?}");
    }
    let after = e.layer(&l).unwrap();
    assert_eq!((before.version, before.fields.len()), (after.version, after.fields.len()), "source layer unchanged");

    let l2 = import(&e, dir.path(), "err.geojson", &layer_text("error"));
    let r = ok(&e, "stat_spatial_error", json!({"dependent": "y", "predictors": "x", "weights": "queen"}), &l2);
    // Queen weights differ from the rook process that made the data, so only the sign and significance are checked here.
    assert!(cell(&r, "Spatial parameter", 0, "estimate") > 0.3);
    assert!(cell(&r, "Model fit", 0, "lr_p") < 0.01);
    // A k = 1 nearest-neighbour structure on a grid can leave islands only if distances tie; a tiny distance band certainly does.
    let bad = run(&e, "stat_spatial_lag", json!({"dependent": "y", "predictors": "x", "weights": "band", "distance": 10}), &l);
    assert!(matches!(bad.status, Status::Error));
    assert!(bad.error.unwrap().contains("islands"));
}

#[test]
fn gwr_and_mgwr_tools_find_the_varying_slope() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import(&e, dir.path(), "gwr.geojson", &layer_text("gwr"));
    let r = ok(&e, "stat_gwr", json!({"dependent": "y", "predictors": "x", "kernel": "bisquare", "bandwidth_type": "adaptive", "weights": "queen"}), &l);
    let st = r.stat.as_ref().unwrap();
    assert!(st.tables.iter().any(|t| t.title == "Global OLS and GWR"));
    // The local slope of x runs from 1 to 4.3 across the grid: the summary shows a wide range and AICc prefers GWR.
    let row = st.tables.iter().find(|t| t.title == "Local coefficients").unwrap().rows.iter().find(|r| r[0] == "x").unwrap().clone();
    let (min, max) = (row[1].as_f64().unwrap(), row[5].as_f64().unwrap());
    assert!(max - min > 1.5, "range of the local slope {min}..{max}");
    assert!(cell(&r, "Global OLS and GWR", 1, "aicc") < cell(&r, "Global OLS and GWR", 0, "aicc") - 3.0);
    let f = fields_of(&e, &r);
    for c in ["b_x", "se_x", "t_x", "b_Intercept", "local_r2", "gwr_fitted", "gwr_residual"] {
        assert!(f.contains(&c.to_string()), "{c} in {f:?}");
    }
    assert_eq!(r.rows, Some(144));
    // A given bandwidth is validated in words.
    let bad = run(&e, "stat_gwr", json!({"dependent": "y", "predictors": "x", "bandwidth": 1.5}), &l);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("whole number"));
    // MGWR gives each term a bandwidth.
    let m = ok(&e, "stat_mgwr", json!({"dependent": "y", "predictors": "x", "weights": "queen"}), &l);
    let bw = m.stat.as_ref().unwrap().tables.iter().find(|t| t.title == "Bandwidths").unwrap();
    assert_eq!(bw.rows.len(), 2);
}
