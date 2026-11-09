//! A layer's Python definition query (UR-20): the script runs once, never at draw time; its result is a fid set kept apart from the layer; and
//! until the script is approved (or while it fails) the layer has NO features anywhere, never unfiltered ones.
use engine::table::RowsQuery;
use engine::Engine;
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

fn engine() -> Option<(Engine, std::sync::MutexGuard<'static, ()>)> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    if engine::resources::python_exe().is_none() {
        eprintln!("SKIPPED: no python3");
        return None;
    }
    Some((Engine::new().unwrap(), guard))
}

fn layer(e: &Engine) -> engine::layers::LayerInfo {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("t.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT * FROM (VALUES (1, 10.0, ST_Point(-97.5, 35.4)), (2, 4.0, ST_Point(-97.4, 35.5)), (3, 0.0, ST_Point(-97.3, 35.6)), (4, 8.0, ST_Point(-97.2, 35.7))) t(id, area, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)",
        p.display()
    ))
    .unwrap();
    e.import(&p).unwrap().remove(0)
}

fn ids(e: &Engine, id: &str) -> Vec<String> {
    let info = e.layer(id).unwrap();
    let idx = info.fields.iter().position(|f| f.name == "id").unwrap();
    e.rows(id, &RowsQuery { limit: 100, ..Default::default() }).unwrap().rows.into_iter().map(|r| r[idx].clone().unwrap()).collect()
}

const BIG: &str = "def calculate(row):\n    return row[\"area\"] > 5\n";

#[test]
fn an_approved_python_definition_query_filters_every_consumer_through_a_separate_fid_set() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    engine::pyscript::approve(BIG).unwrap();
    let info = e.set_definition_python(&l.id, Some(BIG)).unwrap();
    assert_eq!(info.definition_python.as_deref(), Some(BIG));
    assert_eq!(info.definition_query, None);
    assert_eq!(info.query_count, Some(2));
    assert_eq!(ids(&e, &l.id), ["1", "4"]);
    // The layer's own table is untouched: no column was added to it (output never modifies source data).
    assert_eq!(info.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["fid", "id", "area"]);
    let cols: Vec<String> = e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&format!("DESCRIBE SELECT * FROM \"{}\"", info.table))?;
        Ok(st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<_, _>>()?)
    })
    .unwrap();
    assert!(!cols.iter().any(|c| c.contains("defq") || c.contains("python")), "{cols:?}");
    // The set is a row set keyed by layer, script hash and data version.
    let (n, v): (i64, i64) = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*), max(version) FROM __defq_fids", [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!((n, v), (2, info.version as i64));
    // Copying the layer, a graph's Layer Input and an export all respect it.
    assert_eq!(e.copy_layer(&l.id, "copy", None).unwrap().feature_count, 2);
    let csv = data_dir().join("out.csv");
    e.export_table(&l.id, &engine::export_table::TableExportSpec { path: csv.display().to_string(), format: "csv".into(), crs: "EPSG:4326".into(), grid_ref: None, fids: None }).unwrap();
    assert_eq!(std::fs::read_to_string(&csv).unwrap().lines().count(), 3, "header + 2 features");
    assert!(!e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty());
}

#[test]
fn an_unapproved_script_shows_no_features_anywhere_until_approved_and_then_runs_once() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    let script = "def calculate(row):\n    return row[\"id\"] >= 3  # shared-project script\n";
    let info = e.set_definition_python(&l.id, Some(script)).unwrap();
    assert!(!engine::pyscript::is_approved(script));
    // Never the unfiltered four.
    assert_eq!(ids(&e, &l.id), Vec::<String>::new());
    assert_eq!(e.copy_layer(&l.id, "copy", None).unwrap().feature_count, 0);
    let csv = data_dir().join("unapproved.csv");
    e.export_table(&l.id, &engine::export_table::TableExportSpec { path: csv.display().to_string(), format: "csv".into(), crs: "EPSG:4326".into(), grid_ref: None, fids: None }).unwrap();
    assert_eq!(std::fs::read_to_string(&csv).unwrap().lines().count(), 1, "header only");
    assert!(e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty() || e.rows(&l.id, &RowsQuery { limit: 10, ..Default::default() }).unwrap().total == 0);
    assert_eq!(info.definition_error, None, "waiting is not an error: the UI asks for approval");
    // Approving it runs it (no re-setting needed) and the layer shows the kept features.
    let trust = e.approve_script(script).unwrap();
    assert!(trust.approved);
    assert_eq!(ids(&e, &l.id), ["3", "4"]);
    assert_eq!(e.layer(&l.id).unwrap().query_count, Some(2));
}

#[test]
fn editing_the_data_runs_the_script_again_and_a_changed_script_waits_again() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    engine::pyscript::approve(BIG).unwrap();
    e.set_definition_python(&l.id, Some(BIG)).unwrap();
    assert_eq!(ids(&e, &l.id), ["1", "4"]);
    // Row 2's area goes from 4 to 99: it now passes, with no action from the user.
    let fid2 = e.rows(&l.id, &RowsQuery { limit: 10, ..Default::default() }).unwrap();
    let _ = fid2;
    let all = e.rows(&l.id, &RowsQuery { limit: 10, filter: Some("TRUE".into()), ..Default::default() }).unwrap();
    let _ = all;
    let fid = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT __fid FROM \"{}\" WHERE id = 2", e.layer(&l.id).unwrap().table), [], |r| r.get::<_, i64>(0))?)).unwrap();
    e.update_cell(&l.id, fid, "area", Some("99")).unwrap();
    assert_eq!(ids(&e, &l.id), ["1", "2", "4"]);
    // Another script is another approval: until then the layer is empty, not showing the old result.
    let edited = format!("{BIG} ");
    e.set_definition_python(&l.id, Some(&edited)).unwrap();
    assert_eq!(ids(&e, &l.id), Vec::<String>::new());
    // A SQL definition query replaces it and the stored set goes.
    let info = e.set_definition_query(&l.id, Some("area > 5")).unwrap();
    assert_eq!(info.definition_python, None);
    assert_eq!(ids(&e, &l.id), ["1", "2", "4"]);
    let n: i64 = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM __defq_fids", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn a_failing_script_leaves_the_layer_empty_with_the_reason() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    let s = "def calculate(row):\n    return 10 / row[\"area\"] > 1\n";
    engine::pyscript::approve(s).unwrap();
    let info = e.set_definition_python(&l.id, Some(s)).unwrap();
    let err = info.definition_error.clone().unwrap_or_default();
    assert!(err.contains("1 of 4 rows failed") && err.contains("row 3: ZeroDivisionError"), "{err}");
    assert_eq!(ids(&e, &l.id), Vec::<String>::new(), "a failing definition query never lets features through");
}

#[test]
fn a_project_opened_on_another_computer_waits_for_approval_there() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    let script = "def calculate(row):\n    return row[\"area\"] > 5  # project-from-elsewhere\n";
    engine::pyscript::approve(script).unwrap();
    e.set_definition_python(&l.id, Some(script)).unwrap();
    assert_eq!(ids(&e, &l.id), ["1", "4"]);
    let project = tempfile::tempdir().unwrap();
    e.save_project(project.path(), "{}").unwrap();
    // "Another computer": the same project, an empty list of approved scripts.
    let elsewhere = tempfile::tempdir().unwrap();
    let mine = std::env::var_os("GIS_STUDIO_DATA").unwrap();
    std::env::set_var("GIS_STUDIO_DATA", elsewhere.path());
    let e2 = Engine::new().unwrap();
    e2.open_project(project.path()).unwrap();
    let id = e2.list_layers()[0].id.clone();
    assert!(e2.layer(&id).unwrap().definition_python.is_some());
    assert!(!engine::pyscript::is_approved(script), "the project cannot mark itself trusted");
    assert_eq!(ids(&e2, &id), Vec::<String>::new(), "waiting for approval: no features, not the unfiltered four");
    // Approving there runs it.
    e2.approve_script(script).unwrap();
    assert_eq!(ids(&e2, &id), ["1", "4"]);
    std::env::set_var("GIS_STUDIO_DATA", mine);
}

fn col(e: &Engine, id: &str, field: &str) -> Vec<Option<String>> {
    let info = e.layer(id).unwrap();
    let idx = info.fields.iter().position(|f| f.name == field).unwrap_or_else(|| panic!("no field {field}"));
    e.rows(id, &RowsQuery { limit: 100, ..Default::default() }).unwrap().rows.into_iter().map(|r| r[idx].clone()).collect()
}

#[test]
fn the_table_field_calculator_runs_python_strictly_and_changes_nothing_when_a_row_fails() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    // Needs approval first; an unapproved script changes nothing.
    let s = "def calculate(row):\n    return row[\"area\"] * 2  # calc-1\n";
    let err = e.calculate_field_python(&l.id, "TWICE", s, None, None, false).unwrap_err().to_string();
    assert!(err.starts_with(engine::pyscript::WAITING), "{err}");
    assert!(e.layer(&l.id).unwrap().fields.iter().all(|f| f.name != "TWICE"));
    engine::pyscript::approve(s).unwrap();
    let (info, n, note) = e.calculate_field_python(&l.id, "TWICE", s, None, None, false).unwrap();
    assert_eq!((n, note), (4, None));
    assert_eq!(info.fields.iter().find(|f| f.name == "TWICE").unwrap().ty, "DOUBLE");
    assert_eq!(col(&e, &l.id, "TWICE"), ["20.0", "8.0", "0.0", "16.0"].map(|x| Some(x.to_string())));

    // Only the chosen rows (the selection) change; the others keep their value.
    let fids: Vec<i64> = e.select_where(&l.id, "id <= 2").unwrap();
    let s2 = "def calculate(row):\n    return 1.5  # calc-2\n";
    engine::pyscript::approve(s2).unwrap();
    let (_, n, _) = e.calculate_field_python(&l.id, "TWICE", s2, None, Some(&fids), false).unwrap();
    assert_eq!(n, 2);
    assert_eq!(col(&e, &l.id, "TWICE"), ["1.5", "1.5", "0.0", "16.0"].map(|x| Some(x.to_string())));

    // A row that fails stops it and the table is unchanged; "skip" keeps the old value for that row and says so.
    let bad = "def calculate(row):\n    return 100 / row[\"area\"]  # calc-3\n";
    engine::pyscript::approve(bad).unwrap();
    let err = e.calculate_field_python(&l.id, "TWICE", bad, None, None, false).unwrap_err().to_string();
    assert!(err.contains("1 of 4 rows failed (row 3: ZeroDivisionError") && err.contains("Nothing was changed"), "{err}");
    assert_eq!(col(&e, &l.id, "TWICE"), ["1.5", "1.5", "0.0", "16.0"].map(|x| Some(x.to_string())));
    let (_, n, note) = e.calculate_field_python(&l.id, "TWICE", bad, None, None, true).unwrap();
    assert_eq!(n, 3);
    assert!(note.unwrap().contains("row 3") );
    assert_eq!(col(&e, &l.id, "TWICE"), ["10.0", "25.0", "0.0", "12.5"].map(|x| Some(x.to_string())), "row 3 kept 0.0, not NULL");

    // The field's type is enforced: text into the number field fails every row; None is a real NULL.
    let text = "def calculate(row):\n    return 'x'  # calc-4\n";
    engine::pyscript::approve(text).unwrap();
    assert!(e.calculate_field_python(&l.id, "TWICE", text, None, None, false).unwrap_err().to_string().contains("returned text, but the field is decimal"));
    let nul = "def calculate(row):\n    return None if row['id'] == 2 else 7  # calc-5\n";
    engine::pyscript::approve(nul).unwrap();
    e.calculate_field_python(&l.id, "INTS", nul, Some("integer"), None, false).unwrap();
    assert_eq!(col(&e, &l.id, "INTS"), [Some("7".into()), None, Some("7".into()), Some("7".into())]);
}

#[test]
fn select_by_python_respects_the_definition_query_and_never_returns_partial_results() {
    let Some((e, _one)) = engine() else { return };
    let l = layer(&e);
    let s = "def calculate(row):\n    return row[\"area\"] >= 4  # sel-1\n";
    assert!(e.select_python(&l.id, s).unwrap_err().to_string().starts_with(engine::pyscript::WAITING));
    engine::pyscript::approve(s).unwrap();
    assert_eq!(e.select_python(&l.id, s).unwrap().len(), 3);
    e.set_definition_query(&l.id, Some("id > 1")).unwrap();
    assert_eq!(e.select_python(&l.id, s).unwrap().len(), 2, "features hidden by the definition query are not selected");
    let bad = "def calculate(row):\n    return 1  # sel-2\n";
    engine::pyscript::approve(bad).unwrap();
    assert!(e.select_python(&l.id, bad).unwrap_err().to_string().contains("rows failed"));
}
