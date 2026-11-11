//! UX9: the personal style library file: save, replace by name, delete, tolerance, limits.

use engine::style_library::{delete_from, load_from, save_to, MAX_ENTRIES};
use serde_json::json;

fn style(fill: &str) -> serde_json::Value {
    json!({ "kind": "single", "fill": fill, "stroke": "#333333", "strokeWidth": 1, "opacity": 1, "radius": 5 })
}

#[test]
fn styles_round_trip_newest_first_and_a_same_name_replaces() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a/b/style-library.json");
    assert!(load_from(&file).is_empty(), "a missing file is an empty library");
    save_to(&file, "Roads", "Line", style("#111111")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let lib = save_to(&file, "Lakes", "Polygon", style("#2255ff")).unwrap();
    assert_eq!(lib.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Lakes", "Roads"]);
    let again = save_to(&file, "  roads ", "Line", style("#999999")).unwrap();
    assert_eq!(again.len(), 2, "same name, any case: replaced, not added");
    assert_eq!(again[0].name, "roads");
    assert_eq!(again[0].style["fill"], "#999999");
    assert_eq!(load_from(&file), again);
    assert!(!file.with_extension("json.tmp").exists());
}

#[test]
fn delete_removes_one_and_a_missing_id_is_harmless() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.json");
    let lib = save_to(&file, "A", "Point", style("#000000")).unwrap();
    assert_eq!(delete_from(&file, "nope").unwrap().len(), 1);
    assert!(delete_from(&file, &lib[0].id).unwrap().is_empty());
    assert!(load_from(&file).is_empty());
}

#[test]
fn bad_input_is_refused_with_a_sentence_and_a_damaged_file_is_an_empty_library() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.json");
    assert!(save_to(&file, "  ", "Point", style("#000000")).unwrap_err().to_string().contains("Give the style a name"));
    assert!(save_to(&file, &"x".repeat(101), "Point", style("#000000")).unwrap_err().to_string().contains("too long"));
    assert!(save_to(&file, "A", "Point", json!({ "fill": "#000" })).unwrap_err().to_string().contains("not a layer style"));
    let mut big = style("#000000");
    big["markerSvg"] = json!("x".repeat(1_100_000));
    assert!(save_to(&file, "Big", "Point", big).unwrap_err().to_string().contains("too large"));
    assert!(!file.exists(), "nothing was written");
    std::fs::write(&file, "not json").unwrap();
    assert!(load_from(&file).is_empty());
    // An entry with a missing field is skipped, the rest kept.
    std::fs::write(&file, r##"[{"id":"1","name":"ok","geomType":"Point","savedAt":5,"style":{"kind":"single"}},{"id":"2","name":"","geomType":"Point","savedAt":6,"style":{"kind":"single"}},{"id":"3","name":"nokind","geomType":"Point","savedAt":7,"style":{}}]"##).unwrap();
    assert_eq!(load_from(&file).len(), 1);
}

#[test]
fn a_full_library_refuses_more_instead_of_dropping_a_style() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lib.json");
    let entries: Vec<_> = (0..MAX_ENTRIES).map(|i| json!({ "id": format!("i{i}"), "name": format!("s{i}"), "geomType": "Point", "savedAt": i, "style": { "kind": "single" } })).collect();
    std::fs::write(&file, serde_json::to_string(&entries).unwrap()).unwrap();
    let err = save_to(&file, "one more", "Point", style("#000000")).unwrap_err().to_string();
    assert!(err.contains("library is full"), "{err}");
    assert_eq!(load_from(&file).len(), MAX_ENTRIES);
    // Replacing an existing name still works at the limit.
    assert_eq!(save_to(&file, "s5", "Point", style("#ffffff")).unwrap().len(), MAX_ENTRIES);
}
