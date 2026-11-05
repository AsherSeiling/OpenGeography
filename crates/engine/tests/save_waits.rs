//! SAVE-1: Save while another connection has a write open. DuckDB refuses to CHECKPOINT then; Save has to wait for that work, never abort it,
//! and when it cannot finish it must say so in words and leave the earlier saved copy exactly as it was.
use engine::Engine;
use std::sync::mpsc;
use std::time::Duration;

/// Hold a write transaction open on a pooled connection for `hold`, signalling once it has written.
fn writer(e: &std::sync::Arc<Engine>, table: &'static str, hold: Duration) -> std::thread::JoinHandle<()> {
    let (tx, rx) = mpsc::channel();
    let e = e.clone();
    let h = std::thread::spawn(move || {
        e.with_conn_for_tests(|c| {
            c.execute_batch(&format!("BEGIN; CREATE TABLE {table}(a INTEGER); INSERT INTO {table} VALUES (1);"))?;
            tx.send(()).unwrap();
            std::thread::sleep(hold);
            c.execute_batch("COMMIT")?;
            Ok(())
        })
        .unwrap();
    });
    rx.recv().unwrap();
    h
}

fn count(e: &Engine, table: &str) -> i64 {
    e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?)).unwrap()
}

#[test]
fn save_waits_for_a_write_in_flight_and_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("p.gproj");
    let e = std::sync::Arc::new(Engine::new().unwrap());
    e.save_project(&proj, "{}").unwrap(); // the project now lives in a file, so the next Save is a CHECKPOINT
    let w = writer(&e, "inflight", Duration::from_millis(700));
    // The precondition: with that write open, a bare CHECKPOINT is refused. This is the failure users saw.
    let err = e.with_conn_for_tests(|c| Ok(c.execute_batch("CHECKPOINT").map_err(|x| x.to_string()))).unwrap().unwrap_err();
    assert!(err.contains("other write transactions"), "{err}");
    // Save now waits for the writer, then succeeds.
    e.save_project(&proj, "{\"ui\":2}").unwrap();
    w.join().unwrap();
    assert_eq!(count(&e, "inflight"), 1, "the write that was in flight was not aborted");
    assert_eq!(std::fs::read_to_string(proj.join("project.json")).unwrap(), "{\"ui\":2}");
    drop(e);
    let e2 = Engine::new().unwrap();
    assert_eq!(e2.open_project(&proj).unwrap(), "{\"ui\":2}");
    assert_eq!(count(&e2, "inflight"), 1, "and it is in the saved project");
}

#[test]
fn a_first_save_and_a_save_as_wait_too() {
    let dir = tempfile::tempdir().unwrap();
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let w = writer(&e, "inflight", Duration::from_millis(500));
    e.save_project(&dir.path().join("a.gproj"), "{}").unwrap(); // not yet a project: the copy path
    w.join().unwrap();
    let w = writer(&e, "inflight2", Duration::from_millis(500));
    e.save_project(&dir.path().join("b.gproj"), "{}").unwrap(); // Save As to another folder
    w.join().unwrap();
    assert_eq!(count(&e, "inflight2"), 1);
}

#[test]
fn a_save_that_cannot_finish_says_so_and_leaves_the_earlier_copy_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path().join("p.gproj");
    let other = dir.path().join("other.gproj");
    let e = std::sync::Arc::new(Engine::new().unwrap());
    e.save_project(&proj, "{\"v\":1}").unwrap();
    let w = writer(&e, "inflight", Duration::from_millis(1500));
    // Same folder: CHECKPOINT path.
    let err = e.save_project_within(&proj, "{\"v\":2}", Duration::from_millis(200)).unwrap_err().to_string();
    assert!(err.starts_with("The project couldn't be saved yet") && err.contains("earlier saved copy is untouched"), "{err}");
    assert_eq!(std::fs::read_to_string(proj.join("project.json")).unwrap(), "{\"v\":1}", "project.json still the earlier one");
    assert!(!proj.join("project.json.saving").exists());
    w.join().unwrap();
    // Save As onto an existing, different project: the target is not touched.
    let first = Engine::new().unwrap();
    first.save_project(&other, "{\"other\":1}").unwrap();
    drop(first);
    let target_db = std::fs::read(other.join("project.duckdb")).unwrap();
    let w = writer(&e, "inflight3", Duration::from_millis(1500));
    let err = e.save_project_within(&other, "{\"other\":2}", Duration::from_millis(200)).unwrap_err().to_string();
    assert!(err.starts_with("The project couldn't be saved yet"), "{err}");
    assert_eq!(std::fs::read(other.join("project.duckdb")).unwrap(), target_db, "the project Save As would have replaced is byte for byte as it was");
    assert_eq!(std::fs::read_to_string(other.join("project.json")).unwrap(), "{\"other\":1}");
    assert!(!other.join("project.duckdb.saving").exists());
    w.join().unwrap();
    // The earlier copy of the first project still opens, and a later Save works.
    e.save_project(&proj, "{\"v\":3}").unwrap();
    let e2 = Engine::new().unwrap();
    assert_eq!(e2.open_project(&proj).unwrap(), "{\"v\":3}");
}
