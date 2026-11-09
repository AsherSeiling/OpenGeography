//! UR-16: per-user settings file: round trip, tolerance, and nothing but the known fields.

use engine::settings::{load_from, save_to, DefaultCrs, Settings};
use serde_json::json;

#[test]
fn settings_round_trip_through_a_file_in_a_new_folder() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a/b/settings.json");
    assert_eq!(load_from(&file), Settings::default(), "a missing file is the defaults");
    let s = Settings {
        theme: Some("dark".into()),
        length_unit: Some("km".into()),
        area_unit: Some("ha".into()),
        default_crs: Some(DefaultCrs { id: "EPSG:5070".into(), name: "NAD83 / Conus Albers".into() }),
        startup_layout: Some(json!({ "version": 3, "layout": { "grid": {} } })),
        scripting_language: Some("python".into()),
        coordinate_format: Some("mgrs".into()),
    };
    let saved = save_to(&file, &s).unwrap();
    assert_eq!(saved, s);
    assert_eq!(load_from(&file), s);
    assert!(!file.with_extension("json.tmp").exists(), "no temp file left");
}

#[test]
fn a_broken_or_half_valid_file_gives_defaults_field_by_field_without_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    std::fs::write(&file, "{ this is not json").unwrap();
    assert_eq!(load_from(&file), Settings::default());
    std::fs::write(&file, "[1, 2]").unwrap();
    assert_eq!(load_from(&file), Settings::default());
    // One good field, the rest wrong types, wrong values or too long.
    std::fs::write(&file, serde_json::to_string(&json!({ "theme": "purple", "lengthUnit": "km", "areaUnit": 7, "defaultCrs": { "id": "", "name": "x" }, "startupLayout": "no", "scriptingLanguage": "x".repeat(200) })).unwrap()).unwrap();
    let s = load_from(&file);
    assert_eq!((s.theme, s.length_unit.as_deref(), s.area_unit, s.default_crs, s.startup_layout, s.scripting_language), (None, Some("km"), None, None, None, None));
}

#[test]
fn only_the_known_fields_are_ever_written_so_nothing_else_can_get_in() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    // A file with extra keys (an API key someone pasted in by hand) is read for its known fields and the rest is gone after a save.
    std::fs::write(&file, serde_json::to_string(&json!({ "theme": "light", "censusApiKey": "SECRET123", "token": "abc" })).unwrap()).unwrap();
    let s = load_from(&file);
    assert_eq!(s.theme.as_deref(), Some("light"));
    save_to(&file, &s).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("SECRET123") && !text.contains("token") && !text.contains("censusApiKey"), "{text}");
}

#[test]
fn coordinate_format_keeps_only_the_known_formats() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    std::fs::write(&file, r#"{ "coordinateFormat": "dms" }"#).unwrap();
    assert_eq!(load_from(&file).coordinate_format.as_deref(), Some("dms"));
    std::fs::write(&file, r#"{ "coordinateFormat": "utm-zone-soup" }"#).unwrap();
    assert_eq!(load_from(&file).coordinate_format, None);
}
