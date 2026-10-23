//! Attribute history: node rows and table diffs (docs/ATTRIBUTE_HISTORY_PLAN.md H8–H15).

use engine::graph::Graph;
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::json;
use std::sync::{Mutex, MutexGuard};

/// The task list is global and tests share a process: run these one at a time
/// so the cancel test can find its own "Comparing tables" task.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn sql(e: &Engine, s: &str) {
    e.with_conn_for_tests(|c| Ok(c.execute_batch(s)?)).unwrap();
}

fn num(e: &Engine, s: &str) -> i64 {
    e.with_conn_for_tests(|c| Ok(c.query_row(s, [], |r| r.get(0))?)).unwrap()
}

fn kinds(e: &Engine, id: &str, kind: &str) -> Vec<engine::history::DiffRow> {
    e.node_diff_rows(id, kind, 0, 1000).unwrap()
}

fn cell<'a>(r: &'a engine::history::DiffRow, field: &str) -> &'a engine::history::DiffValue {
    r.values.iter().find(|v| v.field == field).unwrap()
}

#[test]
fn h8_key_diff_counts_and_changed_cells_including_nulls() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT * FROM (VALUES (1, 'x', 10), (2, 'y', 20), (3, 'z', NULL), (4, 'w', 40)) t(GEOID, NAME, POP);
         CREATE TABLE __node_b AS SELECT * FROM (VALUES (1, 'x', 10), (2, 'y', 21), (3, 'z', 30), (5, 'v', 50)) t(GEOID, NAME, POP);",
    );
    let d = e.node_diff("__node_a", "__node_b", Some("GEOID")).unwrap();
    assert_eq!((d.added, d.removed, d.changed, d.unchanged), (1, 1, 2, 1));
    assert_eq!((d.rows_a, d.rows_b), (4, 4));
    assert_eq!((d.key_used.as_deref(), d.key_source.as_str()), (Some("GEOID"), "user"));
    assert!(d.note.is_none(), "{:?}", d.note);
    let changed = kinds(&e, &d.diff_id, "changed");
    assert_eq!(changed.len(), 2);
    let r2 = changed.iter().find(|r| r.key.as_deref() == Some("2")).unwrap();
    assert_eq!((cell(r2, "POP").a.as_deref(), cell(r2, "POP").b.as_deref(), cell(r2, "POP").changed), (Some("20"), Some("21"), true));
    assert!(!cell(r2, "NAME").changed);
    let r3 = changed.iter().find(|r| r.key.as_deref() == Some("3")).unwrap();
    assert_eq!((cell(r3, "POP").a.as_deref(), cell(r3, "POP").b.as_deref(), cell(r3, "POP").changed), (None, Some("30"), true), "NULL → value is a change");
    assert_eq!(kinds(&e, &d.diff_id, "added")[0].key.as_deref(), Some("5"));
    assert_eq!(kinds(&e, &d.diff_id, "removed")[0].key.as_deref(), Some("4"));
    assert_eq!(kinds(&e, &d.diff_id, "all").len(), 4);
    // Temporary tables aren't indexed as snapshots, and can be dropped.
    assert_eq!(num(&e, "SELECT count(*) FROM __node_cache WHERE table_name LIKE '\\_\\_diff%' ESCAPE '\\'"), 0);
    e.drop_node_diff(&d.diff_id).unwrap();
    assert_eq!(num(&e, "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'"), 0);
    assert!(e.node_diff_rows(&d.diff_id, "all", 0, 10).is_err());
}

#[test]
fn h9_schema_diff_added_removed_retyped_and_case_insensitive() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT * FROM (VALUES (1, 'a', 5, 1.0), (2, 'b', 6, 2.0)) t(id, name, old, score);
         CREATE TABLE __node_b AS SELECT * FROM (VALUES (1, 'a', 'five', 9), (2, 'b', 'six', 2)) t(ID, Name, score, extra);",
    );
    let d = e.node_diff("__node_a", "__node_b", None).unwrap();
    assert_eq!(d.fields_added, vec!["extra"]);
    assert_eq!(d.fields_removed, vec!["old"]);
    assert_eq!(d.fields_retyped.len(), 1);
    assert_eq!((d.fields_retyped[0].name.as_str(), d.fields_retyped[0].from.as_str(), d.fields_retyped[0].to.as_str()), ("score", "DECIMAL(2,1)", "VARCHAR"));
    assert_eq!(d.key_used.as_deref(), Some("id"), "ID and id are the same field");
    // The retyped column still compares its values (cast to text): '1.0' vs 'five' differ, 2.0 vs 'six' differ.
    assert_eq!(d.changed, 2);
    let r = &kinds(&e, &d.diff_id, "changed")[0];
    assert!(cell(r, "score").changed && !cell(r, "name").changed);
    // Same value in a retyped column is not a change: 1 (INT) vs '1' (VARCHAR).
    sql(
        &e,
        "CREATE TABLE __node_c AS SELECT * FROM (VALUES (1, 7)) t(id, v);
         CREATE TABLE __node_d AS SELECT * FROM (VALUES (1, '7')) t(id, v);",
    );
    let d = e.node_diff("__node_c", "__node_d", Some("id")).unwrap();
    assert_eq!((d.fields_retyped.len(), d.changed), (1, 0));
}

#[test]
fn h10_auto_key_prefers_unique_id_and_duplicate_user_key_falls_back() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT * FROM (VALUES ('Smith', 1), ('Smith', 2), ('Jones', 3)) t(NAME, GEOID);
         CREATE TABLE __node_b AS SELECT * FROM (VALUES ('Smith', 1), ('Smith', 2), ('Brown', 3)) t(NAME, GEOID);",
    );
    let d = e.node_diff("__node_a", "__node_b", None).unwrap();
    assert_eq!((d.key_used.as_deref(), d.key_source.as_str()), (Some("GEOID"), "auto"));
    assert_eq!((d.changed, d.added, d.removed), (1, 0, 0));
    // A user key that isn't unique: content diff with a note.
    let d = e.node_diff("__node_a", "__node_b", Some("NAME")).unwrap();
    assert_eq!((d.key_used.as_deref(), d.key_source.as_str()), (None, "none"));
    let note = d.note.unwrap();
    assert!(note.contains("Key 'NAME' isn't unique in A (1 duplicate"), "{note}");
    assert!(note.contains("showing rows that exist in only one table"), "{note}");
    assert_eq!((d.added, d.removed, d.changed), (1, 1, 0));
    // A key missing from one table is an error, not a silent fallback.
    assert!(e.node_diff("__node_a", "__node_b", Some("nope")).is_err());
}

#[test]
fn h11_content_diff_is_a_multiset() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT * FROM (VALUES ('p', 1), ('p', 1), ('p', 1), ('q', NULL)) t(k, v);
         CREATE TABLE __node_b AS SELECT * FROM (VALUES ('p', 1), ('p', 1), ('q', NULL)) t(k, v);",
    );
    let d = e.node_diff("__node_a", "__node_b", None).unwrap();
    assert_eq!(d.key_source, "none");
    assert_eq!((d.added, d.removed, d.changed, d.unchanged), (0, 1, 0, 3), "3 copies → 2 copies = 1 removed; NULLs match");
    let r = &kinds(&e, &d.diff_id, "removed")[0];
    assert_eq!((cell(r, "k").a.as_deref(), cell(r, "k").b.as_deref()), (Some("p"), None));
}

#[test]
fn h12_geometry_vertex_move_is_a_change_and_crs_mismatch_skips_geometry() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT 1 AS id, ST_GeomFromText('POLYGON((0 0, 4 0, 4 4, 0 0))') AS geom UNION ALL SELECT 2, ST_Point(1, 1);
         CREATE TABLE __node_b AS SELECT 1 AS id, ST_GeomFromText('POLYGON((0 0, 4 0, 4 5, 0 0))') AS geom UNION ALL SELECT 2, ST_Point(1, 1);",
    );
    let d = e.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    assert!(d.geometry_compared);
    assert_eq!((d.changed, d.unchanged), (1, 1));
    assert_eq!(d.fields.last().map(String::as_str), Some("geom"));
    let r = &kinds(&e, &d.diff_id, "changed")[0];
    assert!(cell(r, "geom").changed && !cell(r, "id").changed);
    // Different CRSs: geometry isn't compared (the CRS comes from the cache index).
    for (t, crs) in [("a", "EPSG:4326"), ("b", "EPSG:3857")] {
        sql(&e, &format!("INSERT INTO __node_cache VALUES ('h{t}', '__node_{t}', 'table', '{crs}', true, 2, 0, 'test', now(), now())"));
    }
    let d = e.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    assert!(!d.geometry_compared);
    assert_eq!(d.changed, 0, "the moved vertex is not seen");
    assert!(d.note.unwrap().contains("Geometry not compared: coordinate systems differ (EPSG:4326 → EPSG:3857)"));
}

#[test]
fn h13_node_rows_page_stably_for_tables_and_layer_input_views() {
    let _g = serial();
    let e = Engine::new().unwrap();
    // A layer big enough that DuckDB scans it in parallel.
    let l = e.generate_test_layer("points", 300_000, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": l.id}},
            {"id": "f", "tool": "filter", "params": {"where": "TRUE"}}
        ],
        "edges": [{"source": "in", "target": "f", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    let out = |id: &str| res.iter().find(|r| r.id == id).unwrap().output.clone().unwrap();
    let (view, table) = (out("in"), out("f"));
    assert_eq!(e.node_table_info(&view).unwrap().kind, "view");
    assert_eq!(e.node_table_info(&table).unwrap().kind, "table");
    let info = e.node_table_info(&table).unwrap();
    assert!(info.has_geom && info.rows == 300_000 && !info.fields.is_empty(), "{info:?}");
    assert!(info.fields.iter().all(|f| f.name != "geom"));

    for t in [&view, &table] {
        for sort in [None, Some(info.fields[0].name.clone())] {
            let q = |offset, limit| RowsQuery { offset, limit, sort: sort.clone(), ..Default::default() };
            let whole = e.node_rows(t, &q(0, 5000)).unwrap();
            assert_eq!(whole.total, 300_000);
            // Repeated calls give the same page, and pages tile the whole.
            assert_eq!(e.node_rows(t, &q(0, 5000)).unwrap().rows, whole.rows, "{t} sort {sort:?}");
            let mut stitched = e.node_rows(t, &q(0, 2000)).unwrap();
            let rest = e.node_rows(t, &q(2000, 3000)).unwrap();
            stitched.rows.extend(rest.rows);
            stitched.fids.extend(rest.fids);
            assert_eq!(stitched.rows, whole.rows, "{t} sort {sort:?}");
            assert_eq!(stitched.fids, whole.fids);
            let mut sorted = whole.fids.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), 5000, "positions are distinct");
            if sort.is_none() {
                assert!(whole.fids.windows(2).all(|w| w[0] < w[1]), "unsorted positions increase ({t})");
                assert_eq!(whole.fids[0], 0);
            }
            // The view and the table (a copy of it) agree on content.
        }
    }
    let q = RowsQuery { limit: 3000, ..Default::default() };
    assert_eq!(e.node_rows(&view, &q).unwrap().rows, e.node_rows(&table, &q).unwrap().rows);
    // Filter and sort errors are readable.
    assert!(e.node_rows(&table, &RowsQuery { limit: 5, sort: Some("nope".into()), ..Default::default() }).is_err());
    let f = e.node_rows(&table, &RowsQuery { limit: 5, filter: Some(format!("{} IS NULL", info.fields[0].name)), ..Default::default() }).unwrap();
    assert_eq!(f.total, 0);
    // Only model snapshots can be read.
    assert!(e.node_rows("__layers", &RowsQuery { limit: 5, ..Default::default() }).is_err());
    assert!(e.node_table_info("__node_cache").is_err());
}

#[test]
fn diff_tables_are_dropped_on_open_and_new_project() {
    let _g = serial();
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("d.gproj");
    let e = Engine::new().unwrap();
    sql(&e, "CREATE TABLE __node_a AS SELECT 1 AS id; CREATE TABLE __node_b AS SELECT 2 AS id;");
    let d = e.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    e.save_project(&p, "{}").unwrap(); // Save As reopens the copy: diffs are gone, orphan snapshots too
    assert!(e.node_diff_rows(&d.diff_id, "all", 0, 10).is_err());
    sql(&e, "CREATE TABLE __node_a AS SELECT 1 AS id; CREATE TABLE __node_b AS SELECT 2 AS id;");
    let d2 = e.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    e.save_project(&p, "{}").unwrap(); // same folder: just a checkpoint; the diff table is saved with the file
    assert_eq!(num(&e, "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'"), 1);
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(&p).unwrap();
    assert_eq!(num(&e2, "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'"), 0, "swept on open");
    assert!(e2.node_diff_rows(&d2.diff_id, "all", 0, 10).is_err());
    sql(&e2, "CREATE TABLE __node_a AS SELECT 1 AS id; CREATE TABLE __node_b AS SELECT 2 AS id;");
    e2.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    e2.new_project().unwrap();
    assert_eq!(num(&e2, "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'"), 0);
}

fn big_pair(e: &Engine, rows: i64) {
    let cols = (1..=18).map(|i| format!("(i * {i}) % 1000 AS f{i}")).collect::<Vec<_>>().join(", ");
    sql(
        e,
        &format!(
            "CREATE TABLE __node_a AS SELECT i AS id, 'n' || i AS name, {cols}, ST_Point(i % 360 - 180, (i % 170) - 85) AS geom FROM range({rows}) t(i);
             CREATE TABLE __node_b AS SELECT id, name, {cols2}, geom FROM __node_a WHERE id % 10 <> 0;",
            cols2 = (1..=18).map(|i| if i == 3 { "CASE WHEN id % 7 = 0 THEN f3 + 1 ELSE f3 END AS f3".to_string() } else { format!("f{i}") }).collect::<Vec<_>>().join(", ")
        ),
    );
}

#[test]
fn h15_diff_runs_as_a_cancellable_task_and_cancel_stops_it() {
    let _g = serial();
    let e = std::sync::Arc::new(Engine::new().unwrap());
    big_pair(&e, 4_000_000);
    let canceller = std::thread::spawn(|| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if let Some(t) = engine::progress::snapshot().into_iter().find(|t| t.label == "Comparing tables") {
                assert!(t.cancellable);
                std::thread::sleep(std::time::Duration::from_millis(300)); // let the join start
                engine::progress::cancel(t.id);
                return std::time::Instant::now();
            }
            assert!(std::time::Instant::now() < deadline, "no Comparing tables task appeared");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let started = std::time::Instant::now();
    let r = e.node_diff("__node_a", "__node_b", Some("id"));
    let stopped = std::time::Instant::now();
    let pressed = canceller.join().unwrap();
    assert!(r.is_err(), "cancelled diff returns an error");
    assert!(stopped.duration_since(pressed) < std::time::Duration::from_secs(3), "stopped {:?} after Cancel (ran {:?})", stopped.duration_since(pressed), stopped - started);
    assert_eq!(num(&e, "SELECT count(*) FROM information_schema.tables WHERE table_name LIKE '\\_\\_diff\\_%' ESCAPE '\\'"), 0, "no half-built diff table");
    assert!(engine::progress::snapshot().iter().all(|t| t.label != "Comparing tables"), "task ended");
    // The pooled connection still works afterwards.
    sql(&e, "CREATE TABLE __node_c AS SELECT 1 AS id; CREATE TABLE __node_d AS SELECT 2 AS id");
    assert_eq!(e.node_diff("__node_c", "__node_d", Some("id")).unwrap().added, 1);
}

/// H14: 1M rows × 20 fields (+ geometry), key diff, release build. Run with
/// `cargo test --release -p engine --test history -- --ignored --nocapture h14`.
#[test]
#[ignore]
fn h14_million_row_key_diff_performance() {
    let _g = serial();
    let e = Engine::new().unwrap();
    big_pair(&e, 1_000_000);
    let t = std::time::Instant::now();
    let d = e.node_diff("__node_a", "__node_b", Some("id")).unwrap();
    let took = t.elapsed();
    println!("H14: key diff of 1M rows x 20 fields (+geometry) took {took:?}: {} removed, {} changed, {} unchanged", d.removed, d.changed, d.unchanged);
    assert_eq!(d.removed, 100_000);
    assert!(d.changed > 0 && d.added == 0);
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
}

#[test]
fn diff_ids_are_unique_per_call_and_drops_are_independent() {
    let _g = serial();
    let e = Engine::new().unwrap();
    sql(
        &e,
        "CREATE TABLE __node_a AS SELECT * FROM (VALUES (1, 'x'), (2, 'y')) t(GEOID, NAME);
         CREATE TABLE __node_b AS SELECT * FROM (VALUES (1, 'x'), (3, 'z')) t(GEOID, NAME);",
    );
    let d1 = e.node_diff("__node_a", "__node_b", Some("GEOID")).unwrap();
    let d2 = e.node_diff("__node_a", "__node_b", Some("GEOID")).unwrap();
    assert_ne!(d1.diff_id, d2.diff_id);
    e.drop_node_diff(&d1.diff_id).unwrap();
    assert_eq!(kinds(&e, &d2.diff_id, "all").len(), 2, "the second comparison survives the first's drop");
    assert!(e.node_diff_rows(&d1.diff_id, "all", 0, 10).is_err());
    e.drop_node_diff(&d2.diff_id).unwrap();
}
