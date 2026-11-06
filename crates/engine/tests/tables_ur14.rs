//! UR-14: tables with no spatial file: make one, add / edit / delete rows (with undo), import and export many formats.

use engine::graph::{Graph, Status};
use engine::Engine;
use serde_json::json;

fn export(e: &Engine, layer: &str, path: &std::path::Path, format: &str) -> engine::graph::NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "out", "tool": "export", "params": {"path": path.to_str().unwrap(), "format": format}}],
        "edges": [{"source": "in", "target": "out", "targetHandle": "in"}]
    }))
    .unwrap();
    e.run_graph(&g, Some("out"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "out").unwrap()
}

fn people(e: &Engine) -> String {
    let t = e.create_table("People", &[("name".into(), "text".into()), ("age".into(), "integer".into()), ("score".into(), "double".into()), ("born".into(), "date".into()), ("member".into(), "boolean".into())]).unwrap();
    for (n, a, s, b, m) in [("Ann", "31", "88.5", "1994-03-02", "true"), ("Bo", "27", "70", "1998-11-30", "false"), ("Cy, Jr.", "45", "91.25", "1980-01-15", "true")] {
        let (_, fid) = e.add_row(&t.id).unwrap();
        for (f, v) in [("name", n), ("age", a), ("score", s), ("born", b), ("member", m)] {
            e.update_cell(&t.id, fid, f, Some(v)).unwrap();
        }
    }
    t.id
}

#[test]
fn a_new_table_has_typed_fields_and_rows_can_be_added_edited_deleted_and_put_back() {
    let e = Engine::new().unwrap();
    let id = people(&e);
    let info = e.layer(&id).unwrap();
    assert_eq!((info.geom_type.as_str(), info.feature_count), ("Table", 3));
    let ty = |n: &str| info.fields.iter().find(|f| f.name == n).unwrap().ty.clone();
    assert_eq!((ty("name").as_str(), ty("age").as_str(), ty("score").as_str(), ty("born").as_str(), ty("member").as_str()), ("VARCHAR", "BIGINT", "DOUBLE", "DATE", "BOOLEAN"));

    // A value that doesn't fit the type is refused in words and changes nothing.
    let err = e.update_cell(&id, 1, "age", Some("old")).unwrap_err().to_string();
    assert!(err.contains("not a valid whole number value for age"), "{err}");

    // Delete a row, keep what was in it, put it back with its old id.
    let saved = e.rows_by_fid(&id, &[2]).unwrap();
    assert_eq!(saved[0].values, vec![Some("Bo".into()), Some("27".into()), Some("70.0".into()), Some("1998-11-30".into()), Some("false".into())]);
    let (after, n) = e.delete_rows(&id, &[2]).unwrap();
    assert_eq!((n, after.feature_count), (1, 2));
    let (back, k) = e.restore_rows(&id, &saved).unwrap();
    assert_eq!((k, back.feature_count), (1, 3));
    assert_eq!(e.rows_by_fid(&id, &[2]).unwrap(), saved, "the same row, the same id");
    // A row id that is taken can't be restored over.
    assert!(e.restore_rows(&id, &saved).unwrap_err().to_string().contains("already in use"));
    // Adding after restoring does not reuse an id.
    let (_, fid) = e.add_row(&id).unwrap();
    assert_eq!(fid, 4);
}

#[test]
fn a_new_table_needs_a_name_and_distinct_valid_fields_and_rows_are_for_tables_only() {
    let e = Engine::new().unwrap();
    let f = |n: &str, t: &str| (n.to_string(), t.to_string());
    assert!(e.create_table(" ", &[f("a", "text")]).unwrap_err().to_string().contains("name"));
    assert!(e.create_table("T", &[]).unwrap_err().to_string().contains("at least one field"));
    assert!(e.create_table("T", &[f("a", "text"), f("A", "integer")]).unwrap_err().to_string().contains("must differ"));
    assert!(e.create_table("T", &[f("geom", "text")]).unwrap_err().to_string().contains("reserved"));
    assert!(e.create_table("T", &[f("a", "blob")]).unwrap_err().to_string().contains("Unknown field type"));
    let pts = e.generate_test_layer("points", 3, [-100.0, 30.0, -95.0, 35.0]).unwrap();
    assert!(e.add_row(&pts.id).unwrap_err().to_string().contains("works on tables"));
}

#[test]
fn a_table_exports_and_imports_back_in_every_format() {
    let e = Engine::new().unwrap();
    let id = people(&e);
    let dir = tempfile::tempdir().unwrap();
    for (format, file) in [("CSV", "p.csv"), ("TSV", "p.tsv"), ("XLSX", "p.xlsx"), ("ODS", "p.ods"), ("PARQUET", "p.parquet"), ("JSON", "p.json"), ("DBF", "p.dbf")] {
        let path = dir.path().join(file);
        let r = export(&e, &id, &path, format);
        assert!(matches!(r.status, Status::Done), "{format}: {:?}", r.error);
        assert!(path.exists(), "{format} wrote {file}");
        let back = e.import(&path).unwrap_or_else(|er| panic!("{format}: import failed: {er:#}")).remove(0);
        assert_eq!(back.geom_type, "Table", "{format}");
        assert_eq!(back.feature_count, 3, "{format}");
        let names: Vec<&str> = back.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names.len(), 5, "{format}: {names:?}");
        // The value with a comma in it survives (quoting) and so do the numbers.
        let rows = e.rows(&back.id, &engine::table::RowsQuery { offset: 0, limit: 100, sort: None, desc: false, filter: None, fids: None }).unwrap().rows;
        assert!(rows.iter().any(|r| r.iter().any(|v| v.as_deref() == Some("Cy, Jr."))), "{format}: {rows:?}");
        assert!(rows.iter().any(|r| r.iter().any(|v| v.as_deref().is_some_and(|v| v.starts_with("91.25")))), "{format}: {rows:?}");
    }
}
