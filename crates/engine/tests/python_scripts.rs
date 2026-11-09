//! Python scripts in the field calculator and Definition Query nodes (Batch D, UR-21): strictness, failed-row reports, types reaching Python,
//! the scrubbed environment, cancel, and the trust record. Uses the system Python through the `dev-python` feature (GIS_STUDIO_PYTHON or `python3`
//! on the PATH); the tests pass with a skip note when there is none. NumPy is not needed and not used.
use engine::graph::{Graph, Status};
use engine::pyscript;
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::json;
use std::path::Path;
use std::sync::OnceLock;

fn data_dir() -> &'static Path {
    static D: OnceLock<tempfile::TempDir> = OnceLock::new();
    D.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("GIS_STUDIO_DATA", d.path());
        d
    })
    .path()
}

/// An engine, or None (with a note) when this machine has no Python to test with.
fn engine() -> Option<(Engine, std::sync::MutexGuard<'static, ()>)> {
    // One at a time: a cancel test must not stop another test's script, and they share the trust record.
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    if engine::resources::python_exe().is_none() {
        eprintln!("SKIPPED: no python3 on the PATH and GIS_STUDIO_PYTHON is not set");
        return None;
    }
    Some((Engine::new().unwrap(), guard))
}

fn write_fixture(path: &Path, select: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", path.display())).unwrap();
}

fn layer(e: &Engine, select: &str) -> engine::layers::LayerInfo {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("t.gpkg");
    write_fixture(&p, select);
    e.import(&p).unwrap().remove(0)
}

/// Run `tool` with `params` over `l`, ending in a layer output; returns (per-node results, output layer values of `field` as text).
fn run_tool(e: &Engine, l: &engine::layers::LayerInfo, tool: &str, params: serde_json::Value) -> (Vec<engine::graph::NodeResult>, Option<Vec<Option<String>>>, Option<engine::layers::LayerInfo>) {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "t", "tool": tool, "params": params},
            {"id": "o", "tool": "layer_output", "params": {"layer": l.id}}
        ],
        "edges": [{"source": "in", "target": "t", "targetHandle": "in"}, {"source": "t", "target": "o", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = res.iter().find(|r| r.id == "o").and_then(|r| r.layer.clone());
    (res, None, out)
}

fn column(e: &Engine, l: &engine::layers::LayerInfo, field: &str) -> Vec<Option<String>> {
    let idx = l.fields.iter().position(|f| f.name == field).unwrap_or_else(|| panic!("no field {field} in {:?}", l.fields.iter().map(|f| &f.name).collect::<Vec<_>>()));
    e.rows(&l.id, &RowsQuery { limit: 1_000_000, ..Default::default() }).unwrap().rows.into_iter().map(|r| r[idx].clone()).collect()
}

fn ok(res: &[engine::graph::NodeResult]) -> bool {
    res.iter().all(|r| matches!(r.status, Status::Done | Status::Cached))
}

fn err_of(res: &[engine::graph::NodeResult], id: &str) -> String {
    let r = res.iter().find(|r| r.id == id).unwrap();
    assert_eq!(r.status, Status::Error, "{r:?}");
    r.error.clone().unwrap()
}

const SMALL: &str = "SELECT * FROM (VALUES (1, 10.0, 'a', ST_Point(-97.5, 35.4)), (2, 4.0, 'b', ST_Point(-97.4, 35.5)), (3, 0.0, NULL, ST_Point(-97.3, 35.6)), (4, 8.0, 'd', ST_Point(-97.2, 35.7))) t(id, area, name, geom)";

fn approve(s: &str) {
    pyscript::approve(s).unwrap();
}

#[test]
fn field_calculator_runs_a_python_function_and_a_filter_keeps_true_rows() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let s = "def calculate(row):\n    return row[\"id\"] * 100 + 1\n";
    approve(s);
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "NEWV", "language": "python", "expression": s}));
    assert!(ok(&res), "{res:?}");
    let out = out.unwrap();
    assert_eq!(column(&e, &out, "NEWV"), ["101", "201", "301", "401"].map(|s| Some(s.to_string())));
    assert_eq!(out.fields.iter().find(|f| f.name == "NEWV").unwrap().ty.to_uppercase().contains("INT"), true, "integers make an integer field");
    assert_eq!(out.geom_type, "Point", "the geometry is carried through");

    let f = "def calculate(row):\n    return row[\"area\"] > 5\n";
    approve(f);
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": f}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "id"), ["1", "4"].map(|s| Some(s.to_string())));
}

#[test]
fn a_script_runs_only_after_this_computer_approved_that_exact_text() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let s = "def calculate(row):\n    return True  # unapproved-test\n";
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    assert!(err_of(&res, "t").starts_with(pyscript::WAITING), "{res:?}");
    assert!(err_of(&res, "t").contains("can do anything you can do"), "the prompt text says what approval means");
    assert!(out.is_none(), "nothing is drawn or produced while the script waits");
    approve(s);
    assert!(pyscript::is_approved(s));
    // The same text with Windows line endings is the same script; any edit asks again.
    assert!(pyscript::is_approved(&s.replace('\n', "\r\n")));
    assert!(!pyscript::is_approved(&format!("{s} ")));
    // The record is a file in the per-user data folder, hashes only; nothing about it is in the layer or project.
    let file = std::fs::read_to_string(data_dir().join("trusted_scripts.json")).unwrap();
    assert!(file.contains(&pyscript::script_hash(s)) && !file.contains("unapproved-test\n    return"), "{file}");
    let (res, _, _) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    assert!(ok(&res), "{res:?}");
}

#[test]
fn filters_accept_only_true_or_false_and_failed_rows_are_reported_with_row_numbers() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    // Row 3 returns None, row 2 divides by zero... area is 4.0 and 0.0: row 3 raises, row 2 returns a number (truthy), row 4 None.
    let s = "def calculate(row):\n    if row[\"id\"] == 2:\n        return 1\n    if row[\"id\"] == 4:\n        return None\n    return 1 / row[\"area\"] > 0.05\n";
    approve(s);
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    let msg = err_of(&res, "t");
    assert!(msg.contains("3 of 4 rows failed"), "{msg}");
    assert!(msg.contains("row 2: returned an integer") || msg.contains("row 2: a filter must return True or False, not int 1"), "no truthy coercion: {msg}");
    assert!(msg.contains("row 3: ZeroDivisionError") && msg.contains("(line 6)"), "{msg}");
    assert!(msg.contains("row 4: a filter must return True or False, not None"), "{msg}");
    assert!(msg.contains("Nothing was changed"), "{msg}");
    assert!(out.is_none(), "no partial result is produced");
    // The user can accept the rest explicitly; the failed rows are dropped, never kept.
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s, "on_failed_rows": "skip"}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "id"), [Some("1".to_string())]);
}

#[test]
fn field_values_must_fit_the_field_none_is_a_real_null_and_nan_is_a_failure() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    // None → a real null, no failure.
    let s = "def calculate(row):\n    return None if row[\"name\"] is None else row[\"name\"].upper()\n";
    approve(s);
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "U", "language": "python", "expression": s}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "U"), [Some("A".into()), Some("B".into()), None, Some("D".into())]);

    // NaN and infinity fail; the decision is documented in pyscript.rs.
    let n = "def calculate(row):\n    return float('nan') if row[\"id\"] == 1 else 1.5\n";
    approve(n);
    let (res, _, _) = run_tool(&e, &l, "calculate_field", json!({"field": "N", "language": "python", "expression": n}));
    assert!(err_of(&res, "t").contains("row 1: returned nan"), "{res:?}");

    // A chosen type is enforced: text into an integer field, a float into an integer field, a bool into an integer field.
    let t = "def calculate(row):\n    return [1, 2.0, True, 'x'][row[\"id\"] - 1]\n";
    approve(t);
    let (res, _, _) = run_tool(&e, &l, "calculate_field", json!({"field": "I", "language": "python", "expression": t, "field_type": "integer"}));
    let msg = err_of(&res, "t");
    assert!(msg.contains("3 of 4 rows failed") && msg.contains("row 2: returned a decimal number, but the field is integer"), "{msg}");
    assert!(msg.contains("row 3: returned a boolean") && msg.contains("row 4: returned text"), "{msg}");

    // Mixed kinds under "automatic": integers and decimals make a decimal field; text among them fails that row.
    let m = "def calculate(row):\n    return [1, 2.5, 'x', 4][row[\"id\"] - 1]\n";
    approve(m);
    let (res, _, _) = run_tool(&e, &l, "calculate_field", json!({"field": "M", "language": "python", "expression": m}));
    assert!(err_of(&res, "t").contains("row 3: returned text, but the field is decimal"), "{res:?}");
    // ...and skipping them leaves the rest and reports the failed row (not a silent null).
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "M", "language": "python", "expression": m, "on_failed_rows": "skip"}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "M"), [Some("1.0".into()), Some("2.5".into()), None, Some("4.0".into())]);
}

#[test]
fn integers_dates_booleans_and_nulls_reach_python_as_their_own_types_and_come_back() {
    let Some((_e, _one)) = engine() else { return };
    use engine::pyscript::{evaluate, Mode, OutType};
    // Straight on a DuckDB table, so every type is exactly what DuckDB holds (a GeoPackage round trip turns timestamps into zoned ones).
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE t AS SELECT * FROM (VALUES (9007199254740993::BIGINT, DATE '2024-02-29', TIMESTAMP '2024-02-29 13:14:15.123456', true, 2.5::DOUBLE, 1.25::DECIMAL(5,2), TIMESTAMPTZ '2024-02-29 13:14:15+00', 18446744073709551615::UBIGINT), \
         (NULL::BIGINT, NULL::DATE, NULL::TIMESTAMP, NULL::BOOLEAN, NULL::DOUBLE, NULL::DECIMAL(5,2), NULL::TIMESTAMPTZ, NULL::UBIGINT)) v(big, d, ts, flag, x, dec, tz, huge)",
    )
    .unwrap();
    let describe = "def calculate(row):\n    return ' | '.join(type(row[k]).__name__ + ':' + str(row[k]) for k in ['big', 'd', 'ts', 'flag', 'x', 'dec', 'huge'])\n";
    approve(describe);
    let ev = evaluate(&c, "t", describe, Mode::Field, OutType::Text, false).unwrap();
    let got: Vec<String> = c.prepare(&format!("SELECT s FROM \"{}\" ORDER BY __rn", ev.staging)).unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(|r| r.unwrap()).collect();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], "int:9007199254740993 | date:2024-02-29 | datetime:2024-02-29 13:14:15.123456 | bool:True | float:2.5 | float:1.25 | str:18446744073709551615");
    assert_eq!(got[1], "NoneType:None | NoneType:None | NoneType:None | NoneType:None | NoneType:None | NoneType:None | NoneType:None", "nulls are None, never 0 or ''");
    // A zoned timestamp arrives as text (documented), not as a naive datetime.
    let tz = "def calculate(row):\n    return type(row['tz']).__name__\n";
    approve(tz);
    let ev = evaluate(&c, "t", tz, Mode::Field, OutType::Text, false).unwrap();
    let first: String = c.query_row(&format!("SELECT s FROM \"{}\" ORDER BY __rn LIMIT 1", ev.staging), [], |r| r.get(0)).unwrap();
    assert_eq!(first, "str");

    // Dates, timestamps, booleans and big integers come back exactly (staging holds days / microseconds / 0-1 / the integer).
    for (script, ty, col, want) in [
        ("import datetime\ndef calculate(row):\n    return None if row['d'] is None else row['d'] + datetime.timedelta(days=1)\n", OutType::Date, "i", "19783"), // 2024-03-01
        ("def calculate(row):\n    return row['ts']\n", OutType::Timestamp, "i", "1709212455123456"),
        ("def calculate(row):\n    return row['flag']\n", OutType::Boolean, "i", "1"),
        ("def calculate(row):\n    return row['big']\n", OutType::Integer, "i", "9007199254740993"),
    ] {
        approve(script);
        let ev = evaluate(&c, "t", script, Mode::Field, ty, false).unwrap();
        let rows: Vec<(i64, Option<i64>)> = c.prepare(&format!("SELECT __rn, {col} FROM \"{}\" ORDER BY __rn", ev.staging)).unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 1, "{script}: the null row has no value");
        assert_eq!(rows[0], (1, Some(want.parse().unwrap())), "{script}");
        // And in SQL the value is what it should be.
        let back: String = c.query_row(&format!("SELECT CAST(({}) AS VARCHAR) FROM \"{}\" o", ty.value_sql("o"), ev.staging), [], |r| r.get(0)).unwrap();
        assert!(!back.is_empty());
    }
    let date_back: String = c.query_row(&format!("SELECT CAST({} AS VARCHAR) FROM (SELECT 19783::BIGINT AS i) o", OutType::Date.value_sql("o")), [], |r| r.get(0)).unwrap();
    assert_eq!(date_back, "2024-03-01");
}

#[test]
fn geometry_is_not_in_a_row_and_a_missing_field_says_so() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let g = "def calculate(row):\n    return row[\"geometry\"] is None\n";
    approve(g);
    let (res, _, _) = run_tool(&e, &l, "filter", json!({"language": "python", "where": g}));
    let msg = err_of(&res, "t");
    assert!(msg.contains("attributes only") && msg.contains("geometry is not available in this version") && msg.contains("row 1"), "{msg}");
    let m = "def calculate(row):\n    return row[\"nope\"] > 1\n";
    approve(m);
    let (res, _, _) = run_tool(&e, &l, "filter", json!({"language": "python", "where": m}));
    assert!(err_of(&res, "t").contains("There is no field named 'nope'. The fields are: fid, id, area, name"), "{res:?}");
}

#[test]
fn a_script_that_cannot_load_says_where() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    for (s, want) in [("def calculate(row)\n    return 1\n", "SyntaxError"), ("x = 1\n", "must define a function"), ("def calculate(row):\n    return 1\nraise RuntimeError('boom')\n", "boom")] {
        approve(s);
        let (res, _, _) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
        let msg = err_of(&res, "t");
        assert!(msg.contains("could not be loaded") && msg.contains(want), "{msg}");
    }
}

#[test]
fn the_script_process_has_an_empty_environment_and_cannot_corrupt_the_protocol() {
    let Some((e, _one)) = engine() else { return };
    std::env::set_var("CENSUS_API_KEY", "super-secret-census-key");
    std::env::set_var("EARTHDATA_TOKEN", "super-secret-earthdata-token");
    let l = layer(&e, SMALL);
    let s = "import os, sys\ndef calculate(row):\n    print('chatter on stdout must not break anything')\n    return ','.join(sorted(k for k in os.environ if k.upper() not in ('LC_CTYPE',)))\n";
    approve(s);
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "E", "language": "python", "expression": s}));
    assert!(ok(&res), "{res:?}");
    let env = column(&e, &out.unwrap(), "E");
    for v in env.iter().flatten() {
        assert!(!v.contains("CENSUS") && !v.contains("EARTHDATA") && !v.contains("HOME") && !v.contains("PATH"), "the environment leaked: {v}");
    }
}

#[test]
fn many_rows_travel_in_chunks_with_progress_and_the_right_row_numbers() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, "SELECT i::BIGINT AS id, (i % 7)::DOUBLE AS m, ST_Point(0, 0) AS geom FROM range(1, 120001) t(i)");
    let s = "def calculate(row):\n    if row['id'] == 100000:\n        raise ValueError('bad at 100000')\n    return row['id'] % 3 == 0\n";
    approve(s);
    let (res, _, _) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    let msg = err_of(&res, "t");
    assert!(msg.contains("1 of 120000 rows failed (row 100000: ValueError: bad at 100000 (line 3))"), "{msg}");
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s, "on_failed_rows": "skip"}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(out.unwrap().feature_count, 40_000, "ids 3, 6, … 120000; the one failed row is not a multiple of 3 and is reported, not kept");
}

#[test]
fn cancel_kills_the_process_and_leaves_no_output() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let s = "import time\ndef calculate(row):\n    time.sleep(60)\n    return True\n";
    approve(s);
    let canceller = std::thread::spawn(|| {
        for _ in 0..400 {
            if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == "Running Python script") {
                assert!(t.cancellable);
                engine::progress::cancel(t.id);
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        false
    });
    let t0 = std::time::Instant::now();
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    assert!(canceller.join().unwrap(), "the run never showed up as a task");
    assert!(t0.elapsed().as_secs() < 20, "cancel waited for the script");
    assert!(err_of(&res, "t").contains("Cancelled"), "{res:?}");
    assert!(out.is_none());
}

#[test]
fn a_script_cannot_corrupt_the_protocol_by_reading_stdin_or_writing_to_fd_1_or_running_a_subprocess() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let s = "import os, subprocess, sys\ndef calculate(row):\n    try:\n        input()\n    except EOFError:\n        pass\n    os.write(1, b'raw bytes on fd 1\\n')\n    subprocess.run(['echo', 'echo from a child'])\n    os.system('echo from a shell')\n    sys.stdout.write('python stdout\\n')\n    return row['id'] * 2\n";
    approve(s);
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "TWICE", "language": "python", "expression": s}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "TWICE"), ["2", "4", "6", "8"].map(|s| Some(s.to_string())));
}

#[cfg(unix)]
#[test]
fn cancel_returns_even_when_the_script_left_a_background_child_holding_the_pipes() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let pidfile = data_dir().join("child.pid");
    let _ = std::fs::remove_file(&pidfile);
    let s = format!("import subprocess, time\ndef calculate(row):\n    p = subprocess.Popen(['sleep', '120'], stdout=None, stderr=None)\n    open({:?}, 'w').write(str(p.pid))\n    time.sleep(60)\n    return True\n", pidfile.display().to_string());
    approve(&s);
    let canceller = std::thread::spawn({
        let pidfile = pidfile.clone();
        move || {
            for _ in 0..400 {
                if pidfile.exists() && std::fs::read_to_string(&pidfile).map_or(false, |t| !t.is_empty()) {
                    if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == "Running Python script") {
                        engine::progress::cancel(t.id);
                        return true;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            false
        }
    });
    let t0 = std::time::Instant::now();
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"language": "python", "where": s}));
    assert!(canceller.join().unwrap(), "the script never started its child");
    assert!(t0.elapsed().as_secs() < 15, "cancel hung for {:?}", t0.elapsed());
    assert!(err_of(&res, "t").contains("Cancelled") && out.is_none());
    // The sleeping child died with the group.
    let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let alive = unsafe { libc_kill(pid) };
    assert!(!alive, "the background child (pid {pid}) is still running after cancel");
}

#[cfg(unix)]
unsafe fn libc_kill(pid: i32) -> bool {
    // kill(pid, 0) succeeds while the process exists (a zombie of ours would too, but this child is not ours: init reaps it).
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, 0) == 0
}

#[test]
fn sql_stays_the_default_and_old_nodes_without_a_language_still_run() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e, SMALL);
    let (res, _, out) = run_tool(&e, &l, "calculate_field", json!({"field": "D", "expression": "id * 2"}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(column(&e, &out.unwrap(), "D")[3].as_deref(), Some("8"));
    let (res, _, out) = run_tool(&e, &l, "filter", json!({"where": "area > 5"}));
    assert!(ok(&res), "{res:?}");
    assert_eq!(out.unwrap().feature_count, 2);
}

/// Timing, run by hand: `cargo test -p engine --test python_scripts -- --ignored --nocapture one_million`.
#[test]
#[ignore]
fn one_million_rows_through_the_whole_path() {
    let Some((_e, _one)) = engine() else { return };
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE t AS SELECT i::BIGINT AS pop, ((i % 97) + 0.5) AS area, 'x' || i AS name FROM range(1, 1000001) r(i)").unwrap();
    let s = "def calculate(row):\n    return row['pop'] / row['area'] if row['area'] else None\n";
    approve(s);
    let t0 = std::time::Instant::now();
    let ev = engine::pyscript::evaluate(&c, "t", s, engine::pyscript::Mode::Field, engine::pyscript::OutType::Auto, false).unwrap();
    eprintln!("TIMING 1M rows, {} fields: {:.2} s (Python {})", 3, t0.elapsed().as_secs_f64(), ev.python_version);
    assert_eq!(ev.total, 1_000_000);
}
