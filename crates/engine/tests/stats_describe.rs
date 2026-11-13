//! Statistics › Describe (package ST2). Descriptive statistics are checked against the certified values of the NIST Statistical Reference
//! Datasets (public domain; files in tests/data/nist), the Excel documentation examples for skewness and kurtosis, and hand calculations.
//! The "NumAcc" sets are built to break one-pass formulas: the mean and standard deviation must agree to at least 10 significant digits.

use engine::graph::{Graph, NodeResult, Status};
use engine::stats::{CardInput, StatCard};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn nist(name: &str) -> Vec<f64> {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/nist").join(format!("{name}.dat"))).unwrap();
    text.lines().skip(60).filter_map(|l| l.trim().parse::<f64>().ok()).collect()
}

/// Log relative error: the number of correct significant digits (NIST's measure).
fn lre(got: f64, certified: f64) -> f64 {
    if got == certified {
        return 16.0;
    }
    let rel = if certified == 0.0 { got.abs() } else { ((got - certified) / certified).abs() };
    -(rel.log10())
}

fn import_csv(e: &Engine, dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(format!("{name}.csv"));
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

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn cell(r: &NodeResult, row: usize, col: &str) -> Value {
    let t = &r.stat.as_ref().unwrap().tables[0];
    let i = t.columns.iter().position(|c| c.name == col).unwrap_or_else(|| panic!("no column {col}: {:?}", t.columns.iter().map(|c| &c.name).collect::<Vec<_>>()));
    t.rows[row][i].clone()
}

#[test]
fn st2_nist_certified_mean_and_standard_deviation() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // (dataset, certified mean, certified s (n - 1), observations, least digits of s). The certified values are for the decimal numbers; the
    // data are stored as doubles, whose spacing near 1e6 and 1e7 (1.2e-10 and 1.9e-9) is already 1e-9 and 1e-8 of the 0.1 spread, so for NumAcc3
    // and NumAcc4 about 9 and 8 digits are all any program can reach.
    let sets: [(&str, f64, f64, usize, f64); 8] = [
        ("Michelso", 299.852400000000, 0.0790105478190518, 100, 12.0),
        ("Mavro", 2.00185600000000, 0.000429123454003053, 50, 12.0),
        ("NumAcc1", 10000002.0, 1.0, 3, 14.0),
        ("NumAcc2", 1.2, 0.1, 1001, 12.0),
        ("NumAcc3", 1000000.2, 0.1, 1001, 9.0),
        ("NumAcc4", 10000000.2, 0.1, 1001, 7.5),
        ("Lottery", 518.958715596330, 291.699727470969, 218, 12.0),
        ("Lew", -177.435000000000, 277.332168044316, 200, 12.0),
    ];
    for (name, mean, sd, n, min_digits) in sets {
        let y = nist(name);
        assert_eq!(y.len(), n, "{name}");
        let csv = format!("y\n{}\n", y.iter().map(|v| format!("{v:e}")).collect::<Vec<_>>().join("\n"));
        let l = import_csv(&e, dir.path(), name, &csv);
        let r = ok(&e, "stat_describe", json!({"fields": "y"}), &l);
        let (m, s) = (f(&cell(&r, 0, "mean")), f(&cell(&r, 0, "sd")));
        assert!(lre(m, mean) >= 12.0, "{name}: mean {m} vs {mean} (LRE {})", lre(m, mean));
        eprintln!("{name}: mean LRE {:.1}, sd LRE {:.1}", lre(m, mean), lre(s, sd));
        assert!(lre(s, sd) >= min_digits, "{name}: sd {s} vs {sd} (LRE {})", lre(s, sd));
        assert_eq!(cell(&r, 0, "n"), json!(n));
        assert_eq!(r.stat.as_ref().unwrap().n_used, Some(n as u64));
    }
}

#[test]
fn st2_skewness_and_kurtosis_match_the_excel_documentation_examples() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import_csv(&e, dir.path(), "sk", "x\n3\n4\n5\n2\n3\n4\n5\n6\n4\n7\n");
    let r = ok(&e, "stat_describe", json!({"fields": "x"}), &l);
    assert!((f(&cell(&r, 0, "skewness")) - 0.359543071).abs() < 1e-8);
    assert!((f(&cell(&r, 0, "kurtosis")) + 0.151799637).abs() < 1e-8);
    // Quartiles are R type 7: quantile(c(2,3,3,4,4,4,5,5,6,7), c(.25,.5,.75)) = 3.25, 4, 5.
    assert_eq!(f(&cell(&r, 0, "q1")), 3.25);
    assert_eq!(f(&cell(&r, 0, "median")), 4.0);
    assert_eq!(f(&cell(&r, 0, "q3")), 5.0);
    assert_eq!(f(&cell(&r, 0, "iqr")), 1.75);
    assert_eq!(f(&cell(&r, 0, "mode")), 4.0);
    // Mean 4.3, sample variance 2.2333...: standard error sqrt(2.2333/10), 95 % interval from t(9) = 2.262157.
    let (m, se) = (f(&cell(&r, 0, "mean")), f(&cell(&r, 0, "se_mean")));
    assert!((m - 4.3).abs() < 1e-12 && (se - (20.1f64 / 9.0 / 10.0).sqrt()).abs() < 1e-12, "{se}");
    assert!((f(&cell(&r, 0, "ci95_low")) - (m - 2.262157 * se)).abs() < 1e-4);
}

#[test]
fn st2_every_dropped_value_is_counted_by_reason_and_groups_are_separate() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // 10 rows: group a has values 1, 2, 3, (empty), "n/a"; group b has 10, 20, 30; one row has no group.
    let csv = "g,x,w\na,1,1\na,2,1\na,3,2\na,,1\na,n/a,1\nb,10,1\nb,20,1\nb,30,1\n,5,1\nb,40,-1\n";
    let l = import_csv(&e, dir.path(), "drop", csv);
    let r = ok(&e, "stat_describe", json!({"fields": "x", "group": "g"}), &l);
    let st = r.stat.as_ref().unwrap();
    assert_eq!(st.tables[0].rows.len(), 2);
    assert_eq!(cell(&r, 0, "g"), json!("a"));
    assert_eq!(cell(&r, 0, "n"), json!(3));
    assert_eq!(cell(&r, 0, "missing"), json!(2));
    assert_eq!(f(&cell(&r, 0, "mean")), 2.0);
    assert_eq!(cell(&r, 1, "g"), json!("b"));
    assert_eq!(cell(&r, 1, "n"), json!(4));
    assert_eq!(f(&cell(&r, 1, "mean")), 25.0);
    // 10 rows, one without a group: 9 used. x: one empty, one text that is not a number.
    assert_eq!(st.n_used, Some(9));
    let reasons: Vec<(String, u64)> = st.dropped.iter().map(|d| (d.reason.clone(), d.count)).collect();
    assert!(reasons.contains(&("g: no group value".to_string(), 1)), "{reasons:?}");
    assert!(reasons.contains(&("x: empty or missing".to_string(), 1)), "{reasons:?}");
    assert!(reasons.contains(&("x: not a finite number".to_string(), 1)), "{reasons:?}");
    // The method and its references are stated.
    assert!(st.method.contains("n - 1") && st.method.contains("type 7"));
    assert!(st.citations.iter().any(|c| c.contains("Hyndman")) && st.citations.iter().any(|c| c.contains("NIST")));
}

#[test]
fn st2_weighted_statistics_use_reliability_weights() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // x = 1, 2, 4 with weights 1, 2, 1 (by hand: mean 2.25, variance 1.9); a negative weight is left out of the weighted columns and counted.
    let l = import_csv(&e, dir.path(), "wt", "x,w\n1,1\n2,2\n4,1\n9,-3\n");
    let r = ok(&e, "stat_describe", json!({"fields": "x", "weight": "w"}), &l);
    assert!((f(&cell(&r, 0, "weighted_mean")) - 2.25).abs() < 1e-12);
    assert!((f(&cell(&r, 0, "weighted_sd")) - 1.9f64.sqrt()).abs() < 1e-12);
    assert_eq!(f(&cell(&r, 0, "sum_weights")), 4.0);
    assert!((f(&cell(&r, 0, "effective_n")) - 16.0 / 6.0).abs() < 1e-12);
    // The unweighted columns still use all four values.
    assert_eq!(cell(&r, 0, "n"), json!(4));
    assert!(r.stat.as_ref().unwrap().dropped.iter().any(|d| d.reason.contains("negative") && d.count == 1));
}

#[test]
fn st2_frequency_table_and_cross_tabulation() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let csv = "sex,party,age\nF,D,30\nF,D,41\nF,R,52\nM,R,23\nM,R,64\nM,D,35\nM,,28\nF,I,\n";
    let l = import_csv(&e, dir.path(), "xt", csv);
    let fr = ok(&e, "stat_frequency", json!({"field": "party"}), &l);
    let t = &fr.stat.as_ref().unwrap().tables[0];
    // D 3, I 1, R 3, and one missing: percent of all 8 rows and of the 7 with a value.
    let get = |v: &str| t.rows.iter().find(|r| r[0] == json!(v)).unwrap().clone();
    assert_eq!(get("D")[1], json!(3));
    assert!((f(&get("D")[2]) - 37.5).abs() < 1e-12 && (f(&get("D")[3]) - 300.0 / 7.0).abs() < 1e-12);
    assert_eq!(get("(missing)")[1], json!(1));
    assert!((f(&get("R")[4]) - 100.0).abs() < 1e-12 || f(&get("R")[4]) > 99.0, "cumulative ends at 100");
    assert_eq!(fr.stat.as_ref().unwrap().n_used, Some(7));
    // Cross-tabulation sex by party: F: D2 I1 R1; M: D1 R2 (the row with no party is dropped).
    let ct = ok(&e, "stat_crosstab", json!({"row_field": "sex", "col_field": "party"}), &l);
    let t = &ct.stat.as_ref().unwrap().tables[0];
    let names: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["sex", "D", "I", "R", "Total"]);
    assert_eq!(t.rows[0], vec![json!("F"), json!(2), json!(1), json!(1), json!(4)]);
    assert_eq!(t.rows[1], vec![json!("M"), json!(1), json!(0), json!(2), json!(3)]);
    assert_eq!(t.rows[2], vec![json!("Total"), json!(3), json!(1), json!(3), json!(7)]);
    assert_eq!(ct.stat.as_ref().unwrap().n_used, Some(7));
    // Row percentages.
    let pr = ok(&e, "stat_crosstab", json!({"row_field": "sex", "col_field": "party", "show": "row"}), &l);
    let t = &pr.stat.as_ref().unwrap().tables[0];
    assert!((f(&t.rows[0][1]) - 50.0).abs() < 1e-12 && (f(&t.rows[1][3]) - 200.0 / 3.0).abs() < 1e-12);
}

#[test]
fn st2_a_sheet_card_keeps_the_tools_own_result_with_data_versions() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import_csv(&e, dir.path(), "card", "x\n1\n2\n3\n4\n");
    let mut inputs = std::collections::BTreeMap::new();
    inputs.insert("in".to_string(), CardInput { layer: l.clone(), fids: None });
    let mut options = serde_json::Map::new();
    options.insert("fields".into(), json!("x"));
    let card = StatCard { id: "c1".into(), tool: "stat_describe".into(), title: None, inputs, options, result: None };
    let a = e.run_stat_card(&card).unwrap();
    assert_eq!(a.n_used, Some(4));
    assert!(a.data_version.contains_key(&l));
    assert!(!a.notes.iter().any(|n| n.contains("does not report")), "the adapter's note must not appear for a tool that reports its own result");
    assert!(a.method.contains("Descriptive"));
    // Running again gives the same answer (a fresh run, not a table without its result).
    let b = e.run_stat_card(&card).unwrap();
    assert_eq!(a.tables, b.tables);
    assert!(e.is_stat_tool("stat_describe") && e.is_stat_tool("stat_frequency") && e.is_stat_tool("stat_crosstab"));
}

#[test]
fn st2_refusals_are_sentences() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import_csv(&e, dir.path(), "bad", "x\n1\n2\n");
    let r = run(&e, "stat_describe", json!({"fields": "nope"}), &l);
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.unwrap().contains("no field named 'nope'"));
    let r = run(&e, "stat_crosstab", json!({"row_field": "x", "col_field": "x"}), &l);
    assert!(r.error.unwrap().contains("two different fields"));
}
