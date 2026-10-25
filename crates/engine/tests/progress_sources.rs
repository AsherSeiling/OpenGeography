//! Progress/cancel plumbing and the sources log (docs/TEST_PLAN.md §Progress
//! and §Sources). The task list is process-global, so each test finds its
//! task by a unique label.

use engine::progress::{self, Task};
use engine::sources;
use engine::Engine;
use std::path::Path;

fn mine(label: &str) -> Option<progress::TaskInfo> {
    progress::snapshot().into_iter().find(|t| t.label == label)
}

#[test]
fn a_task_lives_exactly_as_long_as_its_handle() {
    let label = "ps-lifetime";
    {
        let t = Task::start(label);
        let info = mine(label).expect("listed while running");
        assert_eq!((info.fraction, info.cancellable, info.detail.as_str()), (Some(0.0), false, ""));
        assert_eq!(t.id(), info.id);
    }
    assert!(mine(label).is_none(), "dropped handle removes the task");
}

#[test]
fn fractions_clamp_never_go_backwards_and_split_into_parts() {
    let label = "ps-fraction";
    let t = Task::start(label);
    let f = || mine(label).unwrap().fraction.unwrap();
    t.set(0.4, Some("reading"));
    assert_eq!((f(), mine(label).unwrap().detail.as_str()), (0.4, "reading"));
    t.set(0.1, None::<&str>);
    assert_eq!(f(), 0.4, "progress never moves backwards");
    t.set(7.0, None::<&str>);
    assert_eq!(f(), 1.0, "values above 1 clamp");
    t.set(f64::NAN, None::<&str>);
    assert!(f().is_finite() && (0.0..=1.0).contains(&f()), "NaN doesn't poison the bar");

    let label2 = "ps-parts";
    let mut t2 = Task::start(label2);
    let g = || mine(label2).unwrap().fraction.unwrap();
    t2.part(0, 4);
    t2.set(1.0, None::<&str>);
    assert!((g() - 0.25).abs() < 1e-12);
    t2.part(2, 4);
    t2.set(0.5, None::<&str>);
    assert!((g() - 0.625).abs() < 1e-12, "half of the third quarter");
}

#[test]
fn a_part_index_past_the_end_still_reports_at_most_100_percent() {
    let label = "ps-part-overflow";
    let mut t = Task::start(label);
    t.part(3, 0); // zero parts is treated as one, so part 3 of 1
    t.set(1.0, None::<&str>);
    let f = mine(label).unwrap().fraction.unwrap();
    assert!(f <= 1.0, "{f}");
}

#[test]
fn cancel_is_seen_only_by_cancellable_work_that_checks_and_only_for_that_task() {
    let (a, b) = ("ps-cancel-a", "ps-cancel-b");
    let ta = Task::start(a).cancellable();
    let tb = Task::start(b).cancellable();
    assert!(ta.check().is_ok());
    let h = ta.handle();
    progress::cancel(ta.id());
    let err = ta.check().expect_err("cancel requested");
    assert!(progress::is_cancelled(&err));
    assert!(progress::is_cancelled(&err.context("while buffering")), "still recognised after context is added");
    assert!(h.check().is_err(), "other threads' handles see it too");
    assert!(tb.check().is_ok(), "a neighbouring task is untouched");
    assert_eq!(mine(a).unwrap().detail, "Cancelling…");
    assert!(!progress::is_cancelled(&anyhow::anyhow!("disk full")));
    // Cancelling an id that has ended, or never existed, is harmless.
    let id = tb.id();
    drop(tb);
    progress::cancel(id);
    progress::cancel(u64::MAX);
    drop(ta);
    assert!(h.check().is_ok(), "a handle to a finished task never reports cancelled");
}

#[test]
fn task_ids_are_unique_across_threads() {
    let ids: Vec<u64> = (0..8)
        .map(|i| std::thread::spawn(move || Task::start(format!("ps-thread-{i}")).id()))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert_eq!(ids.iter().collect::<std::collections::HashSet<_>>().len(), 8);
}

#[test]
fn raster_steps_stop_when_the_run_flag_is_set() {
    // The GDAL progress callback reads this thread's flag; outside a run there is none.
    use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
    let flag = Arc::new(AtomicBool::new(false));
    let seen = progress::with_cancel_flag(flag.clone(), || {
        flag.store(true, Ordering::Relaxed);
        flag.load(Ordering::Relaxed)
    });
    assert!(seen);
    // The flag is restored on exit, even when nested.
    let outer = Arc::new(AtomicBool::new(false));
    progress::with_cancel_flag(outer.clone(), || {
        progress::with_cancel_flag(Arc::new(AtomicBool::new(true)), || {});
    });
    assert!(!outer.load(Ordering::Relaxed));
}

// ------------------------------------------------------------------ sources

fn point_file(dir: &Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY (SELECT 1 AS id, ST_Point(-97.5, 35.2) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)", p.display())).unwrap();
    p
}

#[test]
fn every_import_cites_its_file_and_a_reimport_adds_a_use_not_a_source() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let (a, b) = (point_file(dir.path(), "a.gpkg"), point_file(dir.path(), "b.gpkg"));
    let la = e.import(&a).unwrap().remove(0);
    let lb = e.import(&b).unwrap().remove(0);
    let again = e.import(&a).unwrap().remove(0);
    let list = e.sources().unwrap();
    assert_eq!(list.len(), 2, "two files → two sources");
    let sa = list.iter().find(|s| s.citation.key.contains("a.gpkg")).unwrap();
    let ids: Vec<_> = sa.uses.iter().map(|u| u.layer_id.clone()).collect();
    assert!(ids.contains(&la.id) && ids.contains(&again.id) && !ids.contains(&lb.id));
    assert_eq!(sa.citation.title, "a.gpkg");
    assert!(!sa.citation.complete && sa.citation.author.is_empty(), "local files start incomplete");
    assert!(sa.citation.annotation.contains("GPKG"), "{}", sa.citation.annotation);
    // first_used is an ISO date.
    assert_eq!(sa.first_used.len(), 10);
    assert!(sa.first_used.chars().nth(4) == Some('-'));
}

#[test]
fn editing_and_removing_citations() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = e.import(&point_file(dir.path(), "a.gpkg")).unwrap().remove(0);
    let mut c = e.sources().unwrap().remove(0).citation;
    c.author = "Müller, A. & O'Neil, B.".into();
    c.title = "Wells — “final” ✓".into();
    e.update_source(&c).unwrap();
    let got = e.sources().unwrap().remove(0).citation;
    assert_eq!((got.author.as_str(), got.title.as_str()), (c.author.as_str(), c.title.as_str()), "unicode and quotes round trip");
    assert!(got.complete, "saving an edit marks the citation complete");
    // The key is the identity: editing a citation whose key doesn't exist changes nothing.
    let mut ghost = c.clone();
    ghost.key = "no-such-key".into();
    let _ = e.update_source(&ghost);
    assert_eq!(e.sources().unwrap().len(), 1);
    // Removing a source removes its uses; the layer itself remains.
    e.remove_source(&c.key).unwrap();
    assert!(e.sources().unwrap().is_empty());
    assert!(e.layer(&l.id).is_ok());
    e.remove_source("never-existed").unwrap();
}

#[test]
fn derived_layers_cite_every_input_once() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let a = e.import(&point_file(dir.path(), "a.gpkg")).unwrap().remove(0);
    let b = e.import(&point_file(dir.path(), "b.gpkg")).unwrap().remove(0);
    let copy = e.copy_layer(&a.id, "A copy", None).unwrap();
    let uses_of = |layer: &str| -> Vec<String> {
        let mut v: Vec<_> = e.sources().unwrap().into_iter().filter(|s| s.uses.iter().any(|u| u.layer_id == layer)).map(|s| s.citation.key).collect();
        v.sort();
        v
    };
    assert_eq!(uses_of(&copy.id), uses_of(&a.id), "a copy cites what its source cites");
    // Replace-derived: a model result that used both inputs, twice over, still has one row per source.
    e.with_conn_for_tests(|c| {
        sources::replace_derived(c, &[a.id.as_str(), b.id.as_str(), a.id.as_str()], "RESULT", "Result")?;
        sources::replace_derived(c, &[a.id.as_str(), b.id.as_str()], "RESULT", "Result")
    })
    .unwrap();
    assert_eq!(uses_of("RESULT").len(), 2);
    let total: usize = e.sources().unwrap().iter().map(|s| s.uses.iter().filter(|u| u.layer_id == "RESULT").count()).sum();
    assert_eq!(total, 2);
    // Re-deriving from a single input drops the other source from the result.
    e.with_conn_for_tests(|c| sources::replace_derived(c, &[b.id.as_str()], "RESULT", "Result")).unwrap();
    assert_eq!(uses_of("RESULT").len(), 1);
    // Inheriting from a layer with no sources (e.g. a generated test layer) adds nothing.
    let t = e.generate_test_layer("points", 3, [0.0, 0.0, 1.0, 1.0]).unwrap();
    e.with_conn_for_tests(|c| sources::inherit(c, &[t.id.as_str()], "X", "X")).unwrap();
    assert!(uses_of("X").is_empty());
}

#[test]
fn builtin_citations_are_complete_and_well_formed() {
    for id in ["usgsTopo", "usgsImagery", "usgsRelief", "osm"] {
        let c = sources::for_basemap(id).unwrap_or_else(|| panic!("no citation for basemap {id}"));
        assert_eq!(c.kind, "Basemap", "{id}");
        assert!(!c.title.is_empty() && !c.author.is_empty() && !c.url.is_empty() && !c.license.is_empty() || id == "osm", "{id}: {c:?}");
        assert!(c.url.starts_with("https://"), "{id}");
    }
    assert!(sources::for_basemap("none").is_none());
    assert!(sources::for_basemap("").is_none());
    assert!(sources::for_basemap("nonsense").is_none());
    let t = sources::for_3dep();
    assert!(t.title.contains("3DEP") || t.title.to_lowercase().contains("elevation"), "{}", t.title);
    // Keys are stable identities: the same dataset gives the same key every time.
    assert_eq!(sources::for_tnm("nhd_flowlines").key, sources::for_tnm("nhd_flowlines").key);
    assert_ne!(sources::for_tnm("nhd_flowlines").key, sources::for_tnm("wbd_hu8").key);
    // File citations: key is the path, kind Local file, CRS in the annotation when known.
    let c = sources::for_file(Path::new("/data/roads.shp"), "SHP", 12, Some("NAD83 / UTM 14N"));
    assert_eq!((c.kind.as_str(), c.title.as_str()), ("Local file", "roads.shp"));
    assert!(c.key.contains("roads.shp") && c.annotation.contains("12") && c.annotation.contains("UTM"), "{c:?}");
    let none = sources::for_file(Path::new("noext"), "DATA", 0, None);
    assert_eq!(none.title, "noext");
}
