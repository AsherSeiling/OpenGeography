//! ST5 tools end to end: OLS (Anscombe, categorical terms, missing rows), logistic regression (mtcars), residual outputs, and the spatial
//! autocorrelation test of the residuals on a layer of points.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

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
    let t = st.tables.iter().find(|t| t.title == table).unwrap_or_else(|| panic!("no table {table}: {:?}", st.tables.iter().map(|t| &t.title).collect::<Vec<_>>()));
    let i = t.columns.iter().position(|c| c.name == col).unwrap_or_else(|| panic!("no column {col}"));
    t.rows[row][i].as_f64().unwrap_or(f64::NAN)
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a}, expected {b}");
}

const X1: [f64; 11] = [10.0, 8.0, 13.0, 9.0, 11.0, 14.0, 6.0, 4.0, 12.0, 7.0, 5.0];
const Y1: [f64; 11] = [8.04, 6.95, 7.58, 8.81, 8.33, 9.96, 7.24, 4.26, 10.84, 4.82, 5.68];

#[test]
fn ols_tool_matches_r_and_counts_dropped_rows_and_codes_categories() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let mut s = String::from("x,y,grp\n");
    for i in 0..11 {
        s.push_str(&format!("{},{},{}\n", X1[i], Y1[i], if i % 2 == 0 { "a" } else { "b" }));
    }
    s.push_str(",5.0,a\n9.0,,b\n");
    let l = import_file(&e, dir.path(), "anscombe.csv", &s);
    let r = ok(&e, "stat_ols", json!({"dependent": "y", "predictors": "x"}), &l);
    close(cell(&r, "Coefficients", 1, "estimate"), 0.5001, 5e-4, "slope");
    close(cell(&r, "Coefficients", 1, "p"), 0.00217, 2e-5, "p");
    close(cell(&r, "Model fit", 0, "r_squared"), 0.6665, 1e-4, "r2");
    close(cell(&r, "Model fit", 0, "n"), 11.0, 0.0, "n");
    let st = r.stat.as_ref().unwrap();
    assert_eq!(st.n_used, Some(11));
    assert_eq!(st.dropped.len(), 2);
    assert!(st.charts.len() >= 2, "residual and Q-Q charts");
    assert!(st.citations.iter().any(|c| c.contains("Belsley")));
    // The output keeps every row and adds the fitted values and diagnostics; the two unused rows have empty values.
    assert_eq!(r.rows, Some(13), "every input row stays in the output");
    // A categorical term adds dummy columns with a named reference level.
    let r = ok(&e, "stat_ols", json!({"dependent": "y", "predictors": "x", "categorical": "grp"}), &l);
    let names: Vec<String> = r.stat.as_ref().unwrap().tables[0].rows.iter().map(|row| row[0].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["(Intercept)", "x", "grp[b]"]);
    assert!(r.stat.as_ref().unwrap().notes.iter().any(|n| n.contains("reference level is a")));
    // Collinear terms are refused in words.
    let mut dup = String::from("x,x2,y\n");
    for i in 0..11 {
        dup.push_str(&format!("{},{},{}\n", X1[i], 2.0 * X1[i], Y1[i]));
    }
    let d = import_file(&e, dir.path(), "dup.csv", &dup);
    let bad = run(&e, "stat_ols", json!({"dependent": "y", "predictors": "x,x2"}), &d);
    assert!(matches!(bad.status, Status::Error));
    assert!(bad.error.unwrap().contains("collinear"));
}

#[test]
fn logistic_tool_matches_r_glm() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let wt = [2.620, 2.875, 2.320, 3.215, 3.440, 3.460, 3.570, 3.190, 3.150, 3.440, 3.440, 4.070, 3.730, 3.780, 5.250, 5.424, 5.345, 2.200, 1.615, 1.835, 2.465, 3.520, 3.435, 3.840, 3.845, 1.935, 2.140, 1.513, 3.170, 2.770, 3.570, 2.780];
    let am = [1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1];
    let mut s = String::from("wt,am\n");
    for i in 0..32 {
        s.push_str(&format!("{},{}\n", wt[i], am[i]));
    }
    let l = import_file(&e, dir.path(), "mt.csv", &s);
    let r = ok(&e, "stat_logistic", json!({"outcome": "am", "success": "1", "predictors": "wt"}), &l);
    close(cell(&r, "Coefficients", 1, "estimate"), -4.024, 1e-3, "wt");
    close(cell(&r, "Coefficients", 1, "odds_ratio"), (-4.024f64).exp(), 1e-3, "or");
    close(cell(&r, "Model fit", 0, "deviance"), 19.176, 1e-3, "deviance");
    close(cell(&r, "Model fit", 0, "aic"), 23.176, 1e-3, "aic");
    assert_eq!(r.stat.as_ref().unwrap().n_used, Some(32));
}

/// A 12 x 12 grid of points with a smooth spatial trend in y: OLS on an unrelated predictor leaves residuals that are strongly clustered.
#[test]
fn residual_moran_flags_a_spatial_trend_and_the_output_layer_keeps_geometry() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let mut feats = vec![];
    for i in 0..12 {
        for j in 0..12 {
            let (x, yy) = (i as f64, j as f64);
            let noise = ((i * 7 + j * 13) % 5) as f64 * 0.05;
            let z = ((i * 31 + j * 17) % 11) as f64;
            feats.push(json!({"type": "Feature", "properties": {"y": x * 2.0 + yy + noise, "z": z}, "geometry": {"type": "Point", "coordinates": [-100.0 + x * 0.01, 40.0 + yy * 0.01]}}));
        }
    }
    let gj = json!({"type": "FeatureCollection", "features": feats}).to_string();
    let l = import_file(&e, dir.path(), "grid.geojson", &gj);
    let before = e.layer(&l).unwrap();
    let r = ok(&e, "stat_ols", json!({"dependent": "y", "predictors": "z", "weights": "knn", "k": 4}), &l);
    let st = r.stat.as_ref().unwrap();
    // The result is a NEW table: it has the added fields, and the source layer is exactly as it was.
    let info = e.node_table_info(r.output.as_ref().unwrap()).unwrap();
    let names: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
    for f in ["fitted", "residual", "std_residual", "leverage", "cooks_d"] {
        assert!(names.contains(&f), "{f} in {names:?}");
    }
    assert!(info.has_geom && info.rows == 144);
    let after = e.layer(&l).unwrap();
    assert_eq!(before.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), after.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), "source fields unchanged");
    assert_eq!(before.version, after.version, "source data version unchanged");
    assert_eq!(before.feature_count, after.feature_count);
    assert!(!after.fields.iter().any(|f| f.name == "fitted" || f.name == "residual"));
    assert_eq!(r.rows, Some(144));
    assert!(r.crs.as_deref().map_or(false, |c| c.contains("4326")), "geometry and CRS kept: {:?}", r.crs);
    assert!(st.tables.iter().any(|t| t.title == "Residual spatial autocorrelation"));
    assert!(cell(&r, "Residual spatial autocorrelation", 0, "morans_i") > 0.5);
    assert!(cell(&r, "Residual spatial autocorrelation", 0, "p") < 1e-6);
    assert!(st.notes.iter().any(|n| n.contains("spatially autocorrelated")));
    assert!(st.notes.iter().any(|n| n.contains("Features near each other")), "spatial independence note");
    // Turning the test off leaves it out.
    let r = ok(&e, "stat_ols", json!({"dependent": "y", "predictors": "z", "residual_moran": "no"}), &l);
    assert!(!r.stat.as_ref().unwrap().tables.iter().any(|t| t.title == "Residual spatial autocorrelation"));
}
