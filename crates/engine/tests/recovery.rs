//! Autosave and crash recovery (UX1; docs/TEST_PLAN.md §Recovery): the copy is separate, never over the project, whole or refused,
//! restorable, removable, and listed only when its run has ended.

use engine::recovery;
use engine::Engine;
use std::path::Path;

fn tree_bytes(dir: &Path) -> Vec<(String, u64)> {
    let mut v = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        v.push((e.file_name().to_string_lossy().into_owned(), e.metadata().unwrap().len()));
    }
    v.sort();
    v
}

/// The copies of other (finished) runs: this test process's own session is never listed, so move the copy to a fake earlier session.
fn as_crashed(root: &Path) -> String {
    let own = root.join(recovery::session_id());
    let fake = "s1-deadbeef".to_string();
    // A pid that is certainly not running: the manifest names this process, so rewrite it to one that is not.
    let manifest = own.join("recovery.json");
    let text = std::fs::read_to_string(&manifest).unwrap().replace(&format!("\"pid\": {}", std::process::id()), "\"pid\": 4194303");
    std::fs::write(&manifest, text).unwrap();
    std::fs::rename(&own, root.join(&fake)).unwrap();
    fake
}

#[test]
fn the_copy_is_separate_and_the_saved_project_is_untouched() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("recovery");
    let proj = work.path().join("demo.gproj");
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 50, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e.save_project(&proj, r#"{"version":2,"saved":true}"#).unwrap();
    let before = tree_bytes(&proj);
    let json_before = std::fs::read_to_string(proj.join("project.json")).unwrap();

    // More work, then an autosave with a different UI state.
    e.copy_layer(&l.id, "Second", None).unwrap();
    e.write_recovery(&root, r#"{"version":2,"saved":false}"#, Some(&proj), "demo").unwrap();

    // The project's own files are exactly as saved; the copy is elsewhere and the engine still points at the project.
    assert_eq!(std::fs::read_to_string(proj.join("project.json")).unwrap(), json_before);
    assert_eq!(e.project_dir().as_deref(), Some(proj.as_path()));
    // (The engine's own edit went to the project's write-ahead log, as editing always does; the autosave added or changed nothing.)
    let after: Vec<_> = tree_bytes(&proj).into_iter().filter(|f| !f.0.ends_with(".wal")).collect();
    assert_eq!(before, after, "the autosave added no file to the project and changed none of its saved files");
    assert!(root.join(recovery::session_id()).join("recovery.json").is_file());
    assert!(!root.starts_with(&proj));
    // The engine kept working on the same two layers.
    assert_eq!(e.list_layers().len(), 2);
}

#[test]
fn a_crashed_run_is_offered_restores_and_is_then_removed() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("recovery");
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("polygons", 25, [-98.0, 35.0, -97.0, 36.0]).unwrap();
    e.rename_layer(&l.id, "Counties").unwrap();
    // A never-saved project: no origin.
    e.write_recovery(&root, r#"{"version":2,"marker":"abc"}"#, None, "Untitled").unwrap();
    // Our own run's copy is not offered to ourselves.
    assert!(recovery::list(&root).is_empty());
    let id = as_crashed(&root);
    let found = recovery::list(&root);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);
    assert_eq!(found[0].name, "Untitled");
    assert!(found[0].problem.is_none() && found[0].origin.is_none());

    // Restore into a fresh engine: layers, data and UI state come back; the work is not tied to a folder.
    let fresh = Engine::new().unwrap();
    let r = fresh.restore_recovery(&root, &id).unwrap();
    assert!(r.ui_state.contains("\"marker\":\"abc\""));
    let layers = fresh.list_layers();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "Counties");
    assert_eq!(layers[0].feature_count, 25);
    assert!(fresh.project_dir().is_none());

    // Discarding removes it, and a second discard of the same id is harmless.
    recovery::discard(&root, &id).unwrap();
    assert!(recovery::list(&root).is_empty());
    recovery::discard(&root, &id).unwrap();
}

#[test]
fn a_clean_save_removes_this_runs_copy_and_later_writes_replace_the_earlier_generation() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("recovery");
    let e = Engine::new().unwrap();
    e.generate_test_layer("points", 5, [0.0, 0.0, 1.0, 1.0]).unwrap();
    e.write_recovery(&root, "{}", None, "a").unwrap();
    e.write_recovery(&root, "{}", None, "a").unwrap();
    let dir = root.join(recovery::session_id());
    let dbs = std::fs::read_dir(&dir).unwrap().flatten().filter(|f| f.file_name().to_string_lossy().ends_with(".duckdb")).count();
    assert_eq!(dbs, 1, "only the newest generation is kept");
    recovery::discard_own(&root).unwrap();
    assert!(!dir.exists());
    recovery::discard_own(&root).unwrap();
}

#[test]
fn a_partial_or_damaged_copy_is_refused_with_a_sentence_and_the_open_work_is_kept() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("recovery");
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 7, [0.0, 0.0, 1.0, 1.0]).unwrap();
    e.write_recovery(&root, "{}", None, "a").unwrap();
    let id = as_crashed(&root);
    let dir = root.join(&id);

    // Truncated database file.
    let db = std::fs::read_dir(&dir).unwrap().flatten().find(|f| f.file_name().to_string_lossy().ends_with(".duckdb")).unwrap().path();
    let full = std::fs::read(&db).unwrap();
    std::fs::write(&db, &full[..full.len() / 2]).unwrap();
    let err = e.restore_recovery(&root, &id).err().expect("refused").to_string();
    assert!(err.starts_with("This recovery copy can't be used"), "{err}");
    assert_eq!(e.layer(&l.id).unwrap().feature_count, 7, "the open work is untouched");
    let listed = recovery::list(&root);
    assert!(listed[0].problem.as_deref().unwrap().contains("incomplete or damaged"));

    // Missing manifest (the program stopped before finishing): also refused, and listed once a minute has passed is not needed to refuse.
    std::fs::write(&db, &full).unwrap();
    std::fs::remove_file(dir.join("recovery.json")).unwrap();
    let err = e.restore_recovery(&root, &id).err().expect("refused").to_string();
    assert!(err.contains("never finished"), "{err}");

    // A path-like id is not a recovery copy.
    assert!(e.restore_recovery(&root, "../x").is_err());
    assert!(recovery::discard(&root, "..").is_err());
    assert_eq!(e.layer(&l.id).unwrap().feature_count, 7);
}

#[test]
fn a_run_that_is_still_going_is_not_offered() {
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("recovery");
    let e = Engine::new().unwrap();
    e.generate_test_layer("points", 3, [0.0, 0.0, 1.0, 1.0]).unwrap();
    e.write_recovery(&root, "{}", None, "a").unwrap();
    // Move it to another session folder but leave the pid as this (running) process.
    std::fs::rename(root.join(recovery::session_id()), root.join("s2-other")).unwrap();
    assert!(recovery::list(&root).is_empty());
}
