//! Statistics sheets (docs/STATISTICS_WORKSPACE_PLAN.md, ST1): saved in the project, reopened, old projects without the table, concurrent saves,
//! running a card, "selected features only", the data profile and the out-of-date data version.

use engine::stats::{CardInput, StatCard, StatResult, StatSheet};
use engine::Engine;
use serde_json::{json, Map};
use std::collections::BTreeMap;
use std::path::Path;

fn import(e: &Engine, dir: &Path) -> String {
    let p = dir.join("t.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT 1 AS id, 'a' AS name, 10.0 AS pop, ST_Point(0,0) AS geom
               UNION ALL SELECT 2, 'b', 20.0, ST_Point(1,1)
               UNION ALL SELECT 3, 'a', NULL, ST_Point(2,2)
               UNION ALL SELECT 4, NULL, 40.0, ST_Point(3,3)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)",
        p.display()
    ))
    .unwrap();
    e.import(&p).unwrap().remove(0).id
}

fn card(id: &str, layer: &str, fids: Option<Vec<i64>>) -> StatCard {
    let mut options = Map::new();
    options.insert("field".into(), json!("pop"));
    StatCard {
        id: id.into(),
        tool: "summary_statistics".into(),
        title: None,
        inputs: BTreeMap::from([("in".to_string(), CardInput { layer: layer.into(), fids })]),
        options,
        result: None,
    }
}

fn sheet(id: &str, name: &str, cards: Vec<StatCard>) -> StatSheet {
    StatSheet { id: id.into(), name: name.into(), version: 1, cards }
}

#[test]
fn sheets_save_list_rename_delete_and_survive_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let layer = import(&e, dir.path());
    let mut s = sheet("S1", "Population", vec![card("c1", &layer, None)]);
    e.save_stat_sheet(&s).unwrap();
    s.name = "Renamed".into();
    e.save_stat_sheet(&s).unwrap(); // same id: replaced
    assert_eq!(e.list_stat_sheets().unwrap(), vec![s.clone()]);
    let proj = dir.path().join("p.gproj");
    e.save_project(&proj, "{}").unwrap();
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    assert_eq!(e2.list_stat_sheets().unwrap(), vec![s]);
    e2.rename_stat_sheet("S1", "  Again ").unwrap();
    assert_eq!(e2.list_stat_sheets().unwrap()[0].name, "Again");
    assert_eq!(e2.list_stat_sheets().unwrap()[0].cards.len(), 1, "rename keeps the cards");
    assert!(e2.rename_stat_sheet("nope", "x").is_err());
    e2.delete_stat_sheet("S1").unwrap();
    assert!(e2.list_stat_sheets().unwrap().is_empty());
}

#[test]
fn a_project_made_before_sheets_existed_opens() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let proj = dir.path().join("old.gproj");
    e.save_project(&proj, "{}").unwrap();
    drop(e);
    // Take the table out of the saved file, as a project from before this feature has it.
    let db = std::fs::read_dir(&proj).unwrap().filter_map(|f| f.ok()).map(|f| f.path()).find(|p| p.extension().is_some_and(|x| x == "duckdb" || x == "db"));
    let db = db.expect("project database file");
    {
        let c = engine::duckdb::Connection::open(&db).unwrap();
        c.execute_batch("DROP TABLE __stat_sheets").unwrap();
    }
    let e2 = Engine::new().unwrap();
    e2.open_project(&proj).unwrap();
    assert!(e2.list_stat_sheets().unwrap().is_empty());
    e2.save_stat_sheet(&sheet("S1", "New", vec![])).unwrap();
    assert_eq!(e2.list_stat_sheets().unwrap().len(), 1);
}

#[test]
fn concurrent_saves_of_one_sheet_do_not_collide() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let e = e.clone();
            std::thread::spawn(move || {
                for j in 0..10 {
                    e.save_stat_sheet(&sheet("S1", &format!("v{i}-{j}"), vec![])).unwrap();
                    e.save_stat_sheet(&sheet(&format!("T{i}"), "other", vec![])).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let all = e.list_stat_sheets().unwrap();
    assert_eq!(all.len(), 9);
}

#[test]
fn a_sheet_is_validated() {
    let e = Engine::new().unwrap();
    assert!(e.save_stat_sheet(&sheet("", "x", vec![])).is_err());
    assert!(e.save_stat_sheet(&sheet("a b", "x", vec![])).is_err());
    assert!(e.save_stat_sheet(&sheet("S1", "  ", vec![])).is_err());
    let dup = sheet("S1", "x", vec![card("c", "L1", None), card("c", "L1", None)]);
    assert!(e.save_stat_sheet(&dup).unwrap_err().to_string().contains("own id"));
    let mut newer = sheet("S1", "x", vec![]);
    newer.version = 99;
    assert!(e.save_stat_sheet(&newer).unwrap_err().to_string().contains("newer version"));
    assert!(e.list_stat_sheets().unwrap().is_empty());
}

#[test]
fn a_card_runs_the_tool_and_records_the_data_version() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let layer = import(&e, dir.path());
    assert!(e.is_stat_tool("summary_statistics"));
    assert!(!e.is_stat_tool("filter"));
    let r: StatResult = e.run_stat_card(&card("c1", &layer, None)).unwrap();
    assert_eq!(r.tables.len(), 1);
    let t = &r.tables[0];
    let col = |n: &str| t.columns.iter().position(|c| c.name == n).unwrap();
    assert_eq!(t.rows[0][col("COUNT")], json!(4));
    assert_eq!(t.rows[0][col("SUM")], json!(70.0));
    assert!(t.columns.iter().all(|c| !c.name.starts_with("__")), "hidden columns stay out");
    assert_eq!(r.n_used, None, "the adapter does not guess n");
    assert!(r.notes.iter().any(|n| n.contains("does not report")));
    assert_eq!(r.data_version.get(&layer), Some(&e.layer(&layer).unwrap().version));
    // JSON round trip keeps everything.
    let back: StatResult = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
}

#[test]
fn selected_features_only_runs_on_the_stored_ids() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let layer = import(&e, dir.path());
    let page = e.rows(&layer, &engine::table::RowsQuery { limit: 10, ..Default::default() }).unwrap();
    let two = vec![page.fids[0], page.fids[1]];
    let r = e.run_stat_card(&card("c", &layer, Some(two.clone()))).unwrap();
    let t = &r.tables[0];
    let i = t.columns.iter().position(|c| c.name == "COUNT").unwrap();
    assert_eq!(t.rows[0][i], json!(2));
    let p = e.stat_data_profile(&layer, Some(&two)).unwrap();
    assert_eq!(p.rows, 2);
    let none = e.run_stat_card(&card("c", &layer, Some(vec![]))).unwrap();
    assert_eq!(none.tables[0].rows[0][i], json!(0), "an empty selection is no rows, not all rows");
}

#[test]
fn the_data_version_changes_when_the_layer_is_edited() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let layer = import(&e, dir.path());
    let r = e.run_stat_card(&card("c1", &layer, None)).unwrap();
    let before = r.data_version[&layer];
    let page = e.rows(&layer, &engine::table::RowsQuery { limit: 10, ..Default::default() }).unwrap();
    e.update_cell(&layer, page.fids[0], "pop", Some("99")).unwrap();
    assert_ne!(e.layer(&layer).unwrap().version, before, "an edit changes the version the card compares with");
    let again = e.run_stat_card(&card("c1", &layer, None)).unwrap();
    assert_ne!(again.data_version[&layer], before);
}

#[test]
fn the_data_profile_counts_missing_values_and_a_card_refuses_what_it_cannot_run() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let layer = import(&e, dir.path());
    let p = e.stat_data_profile(&layer, None).unwrap();
    assert_eq!(p.rows, 4);
    let miss = |n: &str| p.fields.iter().find(|f| f.name == n).unwrap().missing;
    assert_eq!((miss("id"), miss("name"), miss("pop")), (0, 1, 1));
    // not a statistics tool, missing input, deleted layer
    let mut c = card("c", &layer, None);
    c.tool = "filter".into();
    assert!(e.run_stat_card(&c).unwrap_err().to_string().contains("not a statistics tool"));
    let mut c = card("c", &layer, None);
    c.inputs.clear();
    assert!(e.run_stat_card(&c).unwrap_err().to_string().contains("Choose a layer"));
    let c = card("c", "L999", None);
    assert!(e.run_stat_card(&c).unwrap_err().to_string().contains("no longer exists"));
    // a bad option is a sentence, not an internal table name
    let mut c = card("c", &layer, None);
    c.options.insert("field".into(), json!("nope"));
    let err = e.run_stat_card(&c).unwrap_err().to_string();
    assert!(!err.contains("__node_"), "{err}");
}
