//! A Python filter must pick the same rows whether SQL or CPython answers it (UR-20). `ui/src/query/conformance.json` (written by
//! `ui/src/query/pySubset.test.ts`) lists the expressions the UI's safe subset sends to SQL (`accept`), the table, and expressions it
//! refuses (`reject`). Every accepted expression runs through both DuckDB and CPython and the rows must be identical, with no failed rows;
//! the refused ones marked `differs` are shown to differ (so they really had to leave the subset).
use engine::pyscript::{self, Mode, OutType};
use serde_json::Value;
use std::collections::BTreeSet;

fn fixture() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/query/conformance.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn table(c: &engine::duckdb::Connection, f: &Value) {
    let lit = |v: &Value| match v {
        Value::Null => "NULL".to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        other => other.to_string(),
    };
    let rows: Vec<String> = f["rows"].as_array().unwrap().iter().map(|r| format!("({})", r.as_array().unwrap().iter().map(lit).collect::<Vec<_>>().join(", "))).collect();
    let cols: Vec<String> = f["fields"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap().to_string()).collect();
    // Typed by the first row's casts, so NULLs in later rows keep the column types the UI was told about.
    let casts: Vec<String> = f["fields"].as_array().unwrap().iter().enumerate().map(|(i, x)| format!("CAST(c{i} AS {}) AS \"{}\"", x["type"].as_str().unwrap(), cols[i])).collect();
    let names: Vec<String> = (0..cols.len()).map(|i| format!("c{i}")).collect();
    c.execute_batch(&format!("CREATE TABLE t AS SELECT {} FROM (VALUES {}) v({})", casts.join(", "), rows.join(", "), names.join(", "))).unwrap();
}

fn sql_ids(c: &engine::duckdb::Connection, sql: &str) -> Result<BTreeSet<i64>, String> {
    let mut st = c.prepare(&format!("SELECT id FROM t WHERE ({sql})")).map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<BTreeSet<_>, _>>().map_err(|e| e.to_string())
}

fn py_ids(c: &engine::duckdb::Connection, script: &str, skip_failed: bool) -> Result<BTreeSet<i64>, String> {
    pyscript::approve(script).unwrap();
    let ev = pyscript::evaluate(c, "t", script, Mode::Filter, OutType::Boolean, skip_failed).map_err(|e| format!("{e:#}"))?;
    let mut st = c.prepare(&format!("SELECT s.id FROM \"{}\" s JOIN \"{}\" k USING (__rn)", ev.snapshot, ev.staging)).unwrap();
    let ids = st.query_map([], |r| r.get::<_, i64>(0)).unwrap().collect::<Result<BTreeSet<_>, _>>().unwrap();
    c.execute_batch(&format!("DROP TABLE \"{}\"; DROP TABLE \"{}\"", ev.snapshot, ev.staging)).unwrap();
    Ok(ids)
}

fn setup() -> Option<engine::duckdb::Connection> {
    let d = tempfile::tempdir().unwrap();
    std::env::set_var("GIS_STUDIO_DATA", d.path());
    std::mem::forget(d);
    if engine::resources::python_exe().is_none() {
        eprintln!("SKIPPED: no python3 on the PATH and GIS_STUDIO_PYTHON is not set");
        return None;
    }
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    table(&c, &fixture());
    Some(c)
}

#[test]
fn every_expression_the_subset_sends_to_sql_gives_the_same_rows_in_cpython() {
    let Some(c) = setup() else { return };
    let f = fixture();
    let cases = f["accept"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for case in cases {
        let (expr, sql) = (case["script"].as_str().unwrap(), case["sql"].as_str().unwrap_or_else(|| panic!("{case}")));
        let via_sql = sql_ids(&c, sql).unwrap_or_else(|e| panic!("{expr}: SQL failed: {e}"));
        let via_py = py_ids(&c, expr, false).unwrap_or_else(|e| panic!("{expr}: Python failed: {e}"));
        assert_eq!(via_sql, via_py, "{expr}\n  SQL: {sql}");
    }
}

#[test]
fn the_expressions_left_out_of_the_subset_really_do_differ_between_sql_and_python() {
    let Some(c) = setup() else { return };
    let f = fixture();
    let mut shown = 0;
    for r in f["reject"].as_array().unwrap() {
        if r["differs"] != true {
            continue;
        }
        let (expr, naive) = (r["script"].as_str().unwrap(), r["naive"].as_str());
        let Some(naive) = naive else { continue };
        let py = py_ids(&c, expr, false); // Err = some rows raised: nothing is produced
        let sql = sql_ids(&c, naive);
        let differs = match (&py, &sql) {
            (Ok(p), Ok(s)) => p != s,
            _ => true, // Python failed rows, or SQL could not even run it
        };
        assert!(differs, "{expr} gives the same rows both ways ({py:?}); it need not have left the subset. SQL: {naive}");
        shown += 1;
    }
    assert!(shown >= 8, "only {shown} differing cases ran");
}
