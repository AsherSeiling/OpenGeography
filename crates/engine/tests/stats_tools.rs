//! Statistics › Tests tools end to end (package ST4): the tools load the right rows, count what they drop, and report the numbers `stats_tests.rs`
//! checks against R (sleep, PlantGrowth, mtcars).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

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

fn cell(r: &NodeResult, table: usize, row: usize, col: &str) -> f64 {
    let t = &r.stat.as_ref().unwrap().tables[table];
    let i = t.columns.iter().position(|c| c.name == col).unwrap_or_else(|| panic!("no column {col}: {:?}", t.columns.iter().map(|c| &c.name).collect::<Vec<_>>()));
    t.rows[row][i].as_f64().unwrap_or(f64::NAN)
}

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: got {a}, expected {b}");
}

const S1: [f64; 10] = [0.7, -1.6, -0.2, -1.2, -0.1, 3.4, 3.7, 0.8, 0.0, 2.0];
const S2: [f64; 10] = [1.9, 0.8, 1.1, 0.1, -0.1, 4.4, 5.5, 1.6, 4.6, 3.4];

fn sleep_csv() -> String {
    let mut s = String::from("extra,group,id\n");
    for i in 0..10 {
        s.push_str(&format!("{},1,{}\n", S1[i], i));
    }
    for i in 0..10 {
        s.push_str(&format!("{},2,{}\n", S2[i], i));
    }
    // One row with a missing value and one with a missing group, to be counted.
    s.push_str(",1,99\n9.9,,98\n");
    s
}

fn plant_csv() -> String {
    let c = [4.17, 5.58, 5.18, 6.11, 4.50, 4.61, 5.17, 4.53, 5.33, 5.14];
    let t1 = [4.81, 4.17, 4.41, 3.59, 5.87, 3.83, 6.03, 4.89, 4.32, 4.69];
    let t2 = [6.31, 5.12, 5.54, 5.50, 5.37, 5.29, 4.92, 6.15, 5.80, 5.26];
    let mut s = String::from("weight,group\n");
    for (g, v) in [("ctrl", c), ("trt1", t1), ("trt2", t2)] {
        for x in v {
            s.push_str(&format!("{x},{g}\n"));
        }
    }
    s
}

#[test]
fn ttest_tool_reports_welch_pooled_paired_and_counts_dropped_rows() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = import_csv(&e, dir.path(), "sleep", &sleep_csv());
    let r = ok(&e, "stat_ttest", json!({"mode": "two", "value": "extra", "group": "group"}), &l);
    close(cell(&r, 0, 0, "t"), -1.8608, 1e-4, "t");
    close(cell(&r, 0, 0, "df"), 17.776, 1e-3, "df");
    close(cell(&r, 0, 0, "p"), 0.07939, 1e-5, "p");
    close(cell(&r, 0, 0, "ci_low"), -3.3654832, 1e-6, "ci");
    let st = r.stat.as_ref().unwrap();
    assert!(st.method.starts_with("Welch"), "{}", st.method);
    assert_eq!(st.n_used, Some(20));
    let reasons: Vec<&str> = st.dropped.iter().map(|d| d.reason.as_str()).collect();
    assert!(reasons.iter().any(|x| x.contains("extra")) && reasons.iter().any(|x| x.contains("group")), "{reasons:?}");
    assert!(st.dropped.iter().all(|d| d.count == 1));
    assert!(!st.citations.is_empty() && st.notes.iter().any(|n| n.contains("Shapiro-Wilk")));
    // Pooled.
    let r = ok(&e, "stat_ttest", json!({"mode": "two", "value": "extra", "group": "group", "variance": "pooled"}), &l);
    close(cell(&r, 0, 0, "df"), 18.0, 0.0, "pooled df");
    // Paired needs the two fields side by side.
    let mut wide = String::from("a,b\n");
    for i in 0..10 {
        wide.push_str(&format!("{},{}\n", S1[i], S2[i]));
    }
    let w = import_csv(&e, dir.path(), "wide", &wide);
    let r = ok(&e, "stat_ttest", json!({"mode": "paired", "value": "a", "value2": "b"}), &w);
    close(cell(&r, 0, 0, "t"), -4.0621, 1e-4, "paired t");
    close(cell(&r, 0, 0, "p"), 0.002833, 1e-6, "paired p");
    // A group field with three values is refused in words.
    let p = import_csv(&e, dir.path(), "plant", &plant_csv());
    let bad = run(&e, "stat_ttest", json!({"mode": "two", "value": "weight", "group": "group"}), &p);
    assert!(matches!(bad.status, Status::Error), "{:?}", bad.status);
    assert!(bad.error.unwrap().contains("exactly 2"));
}

#[test]
fn anova_tool_gives_anova_welch_and_tukey_tables() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let p = import_csv(&e, dir.path(), "plant", &plant_csv());
    let r = ok(&e, "stat_anova", json!({"value": "weight", "group": "group"}), &p);
    close(cell(&r, 0, 0, "F"), 4.846, 1e-3, "F");
    close(cell(&r, 0, 0, "p"), 0.01591, 1e-5, "p");
    let st = r.stat.as_ref().unwrap();
    let names: Vec<&str> = st.tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(names, ["ANOVA", "Group summary", "Welch ANOVA", "Tukey HSD"]);
    close(cell(&r, 3, 2, "p_adjusted"), 0.0120064, 1e-4, "trt2-trt1");
    close(cell(&r, 2, 0, "F"), 5.181, 2e-3, "welch F");
    let k = ok(&e, "stat_rank_test", json!({"mode": "kruskal", "value": "weight", "group": "group"}), &p);
    close(cell(&k, 0, 0, "H"), 7.9882, 1e-4, "H");
    let lv = ok(&e, "stat_levene", json!({"value": "weight", "group": "group"}), &p);
    close(cell(&lv, 0, 0, "p"), 0.3412, 1e-4, "levene p");
    let sw = ok(&e, "stat_shapiro", json!({"value": "weight", "group": "group"}), &p);
    assert_eq!(sw.stat.as_ref().unwrap().tables[0].rows.len(), 3);
}

#[test]
fn chi_square_proportion_and_correlation_tools() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Counts 762/327/468 and 484/239/477 as one row per observation is long; use the weights through repeated rows of a compact table instead.
    let mut s = String::from("sex,grade\n");
    for (sex, counts) in [("f", [762, 327, 468]), ("m", [484, 239, 477])] {
        for (g, n) in ["a", "b", "c"].iter().zip(counts) {
            for _ in 0..n {
                s.push_str(&format!("{sex},{g}\n"));
            }
        }
    }
    let c = import_csv(&e, dir.path(), "tab", &s);
    let r = ok(&e, "stat_chisq", json!({"mode": "independence", "field_a": "sex", "field_b": "grade"}), &c);
    close(cell(&r, 0, 0, "chi_square"), 30.07, 5e-3, "X2");
    close(cell(&r, 0, 0, "p"), 2.954e-7, 5e-10, "p");
    assert_eq!(r.stat.as_ref().unwrap().n_used, Some(2757));
    // Goodness of fit with rescaled proportions.
    let mut g = String::from("c\n");
    for (k, n) in ["a", "b", "c", "d", "e"].iter().zip([89, 37, 30, 28, 2]) {
        for _ in 0..n {
            g.push_str(&format!("{k}\n"));
        }
    }
    let gl = import_csv(&e, dir.path(), "gof", &g);
    let r = ok(&e, "stat_chisq", json!({"mode": "gof", "field_a": "c", "expected": "40,20,20,15,5"}), &gl);
    close(cell(&r, 0, 0, "chi_square"), 9.9901, 1e-3, "gof");
    // Smallest expected count is 186 * 0.05 = 9.3, so no low-count warning.
    assert!(!r.stat.as_ref().unwrap().notes.iter().any(|n| n.contains("below 5")));
    close(cell(&r, 0, 0, "min_expected"), 9.3, 1e-9, "min expected");
    // Proportions: smokers example, groups 1..4.
    let mut pr = String::from("smoker,grp\n");
    for (grp, (x, n)) in [(83, 86), (90, 93), (129, 136), (70, 82)].iter().enumerate() {
        for i in 0..*n {
            pr.push_str(&format!("{},{}\n", if i < *x { "smk" } else { "non" }, grp + 1));
        }
    }
    let pl = import_csv(&e, dir.path(), "smk", &pr);
    let r = ok(&e, "stat_proportion", json!({"outcome": "smoker", "success": "smk", "group": "grp"}), &pl);
    close(cell(&r, 0, 0, "chi_square"), 12.6, 5e-2, "prop");
    close(cell(&r, 0, 0, "p"), 0.005585, 2e-5, "prop p");
    // Correlation with Holm across three pairs.
    let cars = [(21.0, 2.62, 110.0), (21.0, 2.875, 110.0), (22.8, 2.32, 93.0), (21.4, 3.215, 110.0), (18.7, 3.44, 175.0), (18.1, 3.46, 105.0), (14.3, 3.57, 245.0), (24.4, 3.19, 62.0)];
    let mut cs = String::from("mpg,wt,hp\n");
    for (a, b, h) in cars {
        cs.push_str(&format!("{a},{b},{h}\n"));
    }
    cs.push_str("30.4,,52\n");
    let cl = import_csv(&e, dir.path(), "cars", &cs);
    let r = ok(&e, "stat_correlation", json!({"fields": "mpg, wt, hp", "method": "spearman", "adjust": "holm"}), &cl);
    let st = r.stat.as_ref().unwrap();
    assert_eq!(st.tables[0].rows.len(), 3);
    assert_eq!(st.n_used, Some(8));
    assert!(st.dropped.iter().any(|d| d.reason.contains("wt") && d.count == 1));
    for row in 0..3 {
        assert!(cell(&r, 0, row, "p_adjusted") >= cell(&r, 0, row, "p") - 1e-12);
    }
    // Wilcoxon / Mann-Whitney run through the tool.
    let l = import_csv(&e, dir.path(), "sleep", &sleep_csv());
    let r = ok(&e, "stat_rank_test", json!({"mode": "mw", "value": "extra", "group": "group"}), &l);
    close(cell(&r, 0, 0, "W"), 25.5, 1e-9, "W");
    close(cell(&r, 0, 0, "p"), 0.06933, 1e-5, "p");
}
