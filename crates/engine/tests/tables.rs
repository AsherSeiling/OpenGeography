//! Attribute table, editing, selection, identify and measure — edge cases
//! beyond the happy paths in engine.rs (docs/TEST_PLAN.md §Table/editing and
//! §Selection/identify/measure).

use engine::table::RowsQuery;
use engine::Engine;
use std::path::Path;

fn write(path: &Path, select: &str, srs: Option<&str>) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

/// Three polygons, a line and a point layer in WGS 84, all near (-97.5, 35.2).
struct Fx {
    e: Engine,
    _dir: tempfile::TempDir,
    polys: String,
    line: String,
    pts: String,
}

fn fx() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let add = |name: &str, sel: &str| {
        let p = dir.path().join(format!("{name}.gpkg"));
        write(&p, sel, Some("EPSG:4326"));
        e.import(&p).unwrap().remove(0).id
    };
    let polys = add(
        "polys",
        "SELECT 1 AS id, 'a' AS name, 10 AS pop, 1.5 AS ratio, ST_MakeEnvelope(-97.51, 35.19, -97.50, 35.20) AS geom
         UNION ALL SELECT 2, 'b', 20, NULL, ST_MakeEnvelope(-97.50, 35.19, -97.49, 35.20)
         UNION ALL SELECT 3, NULL, 30, 3.5, ST_MakeEnvelope(-97.51, 35.20, -97.50, 35.21)",
    );
    let line = add("line", "SELECT 1 AS id, ST_GeomFromText('LINESTRING(-97.5 35.2, -97.4 35.2)') AS geom");
    let pts = add("pts", "SELECT 1 AS id, ST_Point(-97.505, 35.195) AS geom UNION ALL SELECT 2, ST_Point(-97.495, 35.195) UNION ALL SELECT 3, ST_Point(-97.0, 35.0)");
    Fx { e, _dir: dir, polys, line, pts }
}

fn page(e: &Engine, id: &str, q: RowsQuery) -> engine::table::RowsPage {
    e.rows(id, &q).unwrap()
}

fn cell(e: &Engine, id: &str, fid: i64, field: &str) -> Option<String> {
    e.feature(id, fid).unwrap().into_iter().find(|(n, _)| n == field).unwrap().1
}

// ------------------------------------------------------------------ paging, sorting, filters

#[test]
fn paging_sorting_and_filtering() {
    let f = fx();
    let q = |offset, limit| RowsQuery { offset, limit, sort: Some("pop".into()), desc: true, ..Default::default() };
    let p = page(&f.e, &f.polys, q(0, 2));
    assert_eq!((p.total, p.rows.len()), (3, 2));
    let pops: Vec<_> = p.rows.iter().map(|r| r[3].clone().unwrap()).collect();
    assert_eq!(pops, ["30", "20"]);
    assert_eq!(page(&f.e, &f.polys, q(2, 2)).rows.len(), 1, "last page is short");
    assert!(page(&f.e, &f.polys, q(99, 10)).rows.is_empty(), "offset past the end is empty, not an error");
    assert_eq!(page(&f.e, &f.polys, q(0, 0)).rows.len(), 1, "limit 0 is raised to 1");
    assert_eq!(page(&f.e, &f.polys, q(0, 1_000_000)).rows.len(), 3, "limit is capped, not an error");

    // NULLs sort last in either direction.
    for desc in [false, true] {
        let p = page(&f.e, &f.polys, RowsQuery { limit: 10, sort: Some("ratio".into()), desc, ..Default::default() });
        assert_eq!(p.rows.last().unwrap()[4], None, "desc={desc}");
    }
    assert!(f.e.rows(&f.polys, &RowsQuery { limit: 5, sort: Some("nope".into()), ..Default::default() }).is_err());
    assert!(f.e.rows(&f.polys, &RowsQuery { limit: 5, filter: Some("pop >>> 3".into()), ..Default::default() }).is_err());
    let none = page(&f.e, &f.polys, RowsQuery { limit: 5, filter: Some("pop > 1000".into()), ..Default::default() });
    assert_eq!((none.total, none.rows.len()), (0, 0));
    // Selected-only view: empty selection shows nothing, not everything.
    assert_eq!(page(&f.e, &f.polys, RowsQuery { limit: 5, fids: Some(vec![]), ..Default::default() }).total, 0);
    assert_eq!(page(&f.e, &f.polys, RowsQuery { limit: 5, fids: Some(vec![2, 999]), ..Default::default() }).fids, [2]);
}

#[test]
fn select_by_attribute_respects_filters_and_reports_bad_ones() {
    let f = fx();
    assert_eq!(f.e.select_where(&f.polys, "pop >= 20").unwrap(), [2, 3]);
    assert_eq!(f.e.select_where(&f.polys, "name IS NULL").unwrap(), [3]);
    assert_eq!(f.e.select_where(&f.polys, "name = 'a' OR pop = 30").unwrap(), [1, 3]);
    assert!(f.e.select_where(&f.polys, "pop > 1000").unwrap().is_empty());
    assert!(f.e.select_where(&f.polys, "nonexistent = 1").is_err());
    assert!(f.e.select_where(&f.polys, "pop >").is_err());
    assert!(f.e.select_where("no-such-layer", "1=1").is_err());
}

#[test]
fn definition_queries_reject_bad_input_and_apply_everywhere() {
    let f = fx();
    for bad in ["nope = 1", "pop >", "1 +"] {
        assert!(f.e.set_definition_query(&f.polys, Some(bad)).is_err(), "{bad}");
        assert!(f.e.layer(&f.polys).unwrap().definition_query.is_none());
    }
    // Blank means no query.
    assert!(f.e.set_definition_query(&f.polys, Some("   ")).unwrap().definition_query.is_none());
    let l = f.e.set_definition_query(&f.polys, Some("pop < 30")).unwrap();
    assert_eq!(l.query_count, Some(2));
    assert_eq!(f.e.select_where(&f.polys, "pop >= 0").unwrap(), [1, 2]);
    assert_eq!(f.e.unique_values(&f.polys, "pop", 10).unwrap().len(), 2);
    assert_eq!(f.e.select_rect(&f.polys, [-98.0, 35.0, -97.0, 36.0], "EPSG:4326").unwrap(), [1, 2]);
    // A query that matches nothing is allowed.
    assert_eq!(f.e.set_definition_query(&f.polys, Some("pop < 0")).unwrap().query_count, Some(0));
    assert_eq!(page(&f.e, &f.polys, RowsQuery { limit: 5, ..Default::default() }).total, 0);
}

// ------------------------------------------------------------------ editing

#[test]
fn field_edits_reject_bad_names_and_types_and_leave_the_layer_alone() {
    let f = fx();
    let names = |e: &Engine| e.layer(&f.polys).unwrap().fields.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
    let before = names(&f.e);
    for bad in ["", "   ", "geom", "GEOM", "__fid", "name", "NAME"] {
        assert!(f.e.add_field(&f.polys, bad, "text").is_err(), "add {bad:?}");
    }
    assert!(f.e.add_field(&f.polys, "ok", "complex128").is_err(), "unknown type");
    assert!(f.e.rename_field(&f.polys, "name", "pop").is_err(), "rename onto an existing field");
    assert!(f.e.rename_field(&f.polys, "ghost", "x").is_err());
    assert!(f.e.rename_field(&f.polys, "name", "geom").is_err());
    assert!(f.e.delete_field(&f.polys, "ghost").is_err());
    assert!(f.e.delete_field(&f.polys, "geom").is_err());
    assert_eq!(names(&f.e), before);
    // Odd but legal names work and survive a round trip through the table.
    let l = f.e.add_field(&f.polys, "my field \"q\"", "integer").unwrap();
    assert!(l.fields.iter().any(|x| x.name == "my field \"q\""));
    f.e.update_cell(&f.polys, 1, "my field \"q\"", Some("7")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 1, "my field \"q\"").as_deref(), Some("7"));
    // All five types can be added.
    for ty in ["text", "integer", "double", "date", "boolean"] {
        f.e.add_field(&f.polys, &format!("t_{ty}"), ty).unwrap();
    }
}

#[test]
fn cell_edits_convert_types_and_explain_failures() {
    let f = fx();
    f.e.add_field(&f.polys, "when", "date").unwrap();
    f.e.add_field(&f.polys, "flag", "boolean").unwrap();
    f.e.update_cell(&f.polys, 1, "when", Some("2026-10-23")).unwrap();
    f.e.update_cell(&f.polys, 1, "flag", Some("true")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 1, "when").as_deref(), Some("2026-10-23"));
    for (field, bad) in [("when", "yesterday"), ("flag", "maybe"), ("pop", "1.5.2"), ("pop", "abc"), ("ratio", "1,5")] {
        let err = f.e.update_cell(&f.polys, 1, field, Some(bad)).expect_err(bad);
        assert!(err.to_string().contains("not a valid"), "{field}={bad}: {err}");
    }
    assert_eq!(cell(&f.e, &f.polys, 1, "pop").as_deref(), Some("10"), "failed edits change nothing");
    // Empty text clears numeric cells to NULL, but is a real empty string in text cells.
    f.e.update_cell(&f.polys, 1, "pop", Some("")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 1, "pop"), None);
    f.e.update_cell(&f.polys, 1, "name", Some("")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 1, "name").as_deref(), Some(""));
    f.e.update_cell(&f.polys, 1, "name", None).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 1, "name"), None);
    // Whitespace around numbers is tolerated; quotes in text are data.
    f.e.update_cell(&f.polys, 2, "pop", Some("  42  ")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 2, "pop").as_deref(), Some("42"));
    f.e.update_cell(&f.polys, 2, "name", Some("O'Brien \"x\"; DROP TABLE y")).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 2, "name").as_deref(), Some("O'Brien \"x\"; DROP TABLE y"));
    // Fields that do not exist and the geometry column are not editable cells.
    assert!(f.e.update_cell(&f.polys, 1, "ghost", Some("1")).is_err());
    assert!(f.e.update_cell(&f.polys, 1, "geom", Some("POINT(0 0)")).is_err());
    // A feature id that does not exist changes nothing and does not error out of the engine.
    let n_before = f.e.layer(&f.polys).unwrap().feature_count;
    let _ = f.e.update_cell(&f.polys, 9999, "pop", Some("1"));
    assert_eq!(f.e.layer(&f.polys).unwrap().feature_count, n_before);
}

#[test]
fn calculate_field_handles_selection_nulls_and_division() {
    let f = fx();
    let (l, n) = f.e.calculate_field(&f.polys, "half", "pop / 2", None, Some(&[1, 3])).unwrap();
    assert_eq!(n, 2);
    assert_eq!(cell(&f.e, &l.id, 1, "half").as_deref(), Some("5.0"));
    assert_eq!(cell(&f.e, &l.id, 2, "half"), None, "unselected rows stay NULL");
    assert_eq!(f.e.calculate_field(&f.polys, "half", "1", None, Some(&[])).unwrap().1, 0, "empty selection updates nothing");
    // NULL propagates; division by zero is an error or NULL, never a panic or a silent 0.
    f.e.calculate_field(&f.polys, "r2", "ratio * 2", None, None).unwrap();
    assert_eq!(cell(&f.e, &f.polys, 2, "r2"), None);
    match f.e.calculate_field(&f.polys, "inv", "1 / (pop - 10)", None, None) {
        Ok(_) => assert!(matches!(cell(&f.e, &f.polys, 1, "inv").as_deref(), None | Some("inf") | Some("NaN")), "{:?}", cell(&f.e, &f.polys, 1, "inv")),
        Err(e) => assert!(!e.to_string().is_empty()),
    }
    // Expressions with syntax errors, unknown columns and reserved targets are refused before any change.
    let fields = f.e.layer(&f.polys).unwrap().fields.len();
    for (target, expr) in [("x", "pop +"), ("x", "nope * 2"), ("geom", "1"), ("", "1"), ("__fid", "1")] {
        assert!(f.e.calculate_field(&f.polys, target, expr, None, None).is_err(), "{target:?} = {expr:?}");
    }
    assert_eq!(f.e.layer(&f.polys).unwrap().fields.len(), fields);
    // Text results get a text field; an explicit type wins.
    let (l, _) = f.e.calculate_field(&f.polys, "label", "upper(name) || '-' || pop", None, None).unwrap();
    assert_eq!(l.fields.iter().find(|x| x.name == "label").unwrap().ty, "VARCHAR");
    let (l, _) = f.e.calculate_field(&f.polys, "as_text", "pop", Some("text"), None).unwrap();
    assert_eq!(l.fields.iter().find(|x| x.name == "as_text").unwrap().ty, "VARCHAR");
}

#[test]
fn deleting_rows_is_exact_and_idempotent() {
    let f = fx();
    assert_eq!(f.e.delete_rows(&f.polys, &[]).unwrap().1, 0);
    assert_eq!(f.e.delete_rows(&f.polys, &[999]).unwrap().1, 0);
    let (l, n) = f.e.delete_rows(&f.polys, &[1, 1, 2]).unwrap();
    assert_eq!((n, l.feature_count), (2, 1));
    assert_eq!(f.e.delete_rows(&f.polys, &[1, 2]).unwrap().1, 0, "already gone");
    assert_eq!(f.e.select_rect(&f.polys, [-98.0, 35.0, -97.0, 36.0], "EPSG:4326").unwrap(), [3]);
    // Delete everything: an empty layer is still a layer.
    let (l, _) = f.e.delete_rows(&f.polys, &[3]).unwrap();
    assert_eq!(l.feature_count, 0);
    assert!(f.e.extent(&f.polys, None).unwrap().is_none());
}

#[test]
fn node_results_and_unknown_layers_cannot_be_edited() {
    let f = fx();
    assert!(f.e.update_cell("ghost", 1, "pop", Some("1")).is_err());
    assert!(f.e.add_field("ghost", "x", "text").is_err());
    assert!(f.e.delete_rows("ghost", &[1]).is_err());
    assert!(f.e.set_definition_query("ghost", None).is_err());
    assert!(f.e.rename_layer("ghost", "x").is_err());
    assert!(f.e.copy_layer("ghost", "x", None).is_err());
    assert!(f.e.remove_layer("ghost").is_err());
    assert!(f.e.rows("ghost", &RowsQuery::default()).is_err());
}

#[test]
#[ignore = "BUG: rename_layer accepts a blank name (docs/BUGS.md B-T2)"]
fn renaming_a_layer_to_blank_is_refused() {
    let f = fx();
    let renamed = f.e.rename_layer(&f.polys, "   ");
    assert!(renamed.is_err() || !renamed.unwrap().name.trim().is_empty(), "a layer must keep a visible name");
}

#[test]
fn layer_removal() {
    let f = fx();
    // Remove drops the table, its citation uses and any later access.
    let sources_before = f.e.sources().unwrap();
    f.e.remove_layer(&f.pts).unwrap();
    assert!(f.e.layer(&f.pts).is_err());
    assert!(f.e.list_layers().iter().all(|l| l.id != f.pts));
    assert!(f.e.tile(&f.pts, 0, 0, 0, &[]).is_err());
    assert_eq!(f.e.list_layers().len(), 2);
    let uses = |s: &[engine::sources::SourceEntry]| s.iter().map(|x| x.uses.len()).sum::<usize>();
    assert!(uses(&f.e.sources().unwrap()) <= uses(&sources_before), "removing a layer never adds citation uses");
}

// ------------------------------------------------------------------ identify, box select, measure

#[test]
fn identify_hits_the_nearest_feature_within_a_few_pixels() {
    let f = fx();
    // The point at (-97.505, 35.195): a click exactly on it, and one a whole degree away.
    assert_eq!(f.e.identify(&f.pts, -97.505, 35.195, "EPSG:4326", 0.0004).unwrap(), [1]);
    assert!(f.e.identify(&f.pts, -96.0, 34.0, "EPSG:4326", 0.0004).unwrap().is_empty());
    // Inside a polygon counts as a hit; just outside at high zoom does not.
    assert_eq!(f.e.identify(&f.polys, -97.505, 35.195, "EPSG:4326", 0.0004).unwrap(), [1]);
    assert!(f.e.identify(&f.polys, -97.505, 35.1, "EPSG:4326", 0.00002).unwrap().is_empty());
    // Tolerance grows as you zoom out: a near miss at z4 hits, at z18 it does not.
    assert!(f.e.identify(&f.pts, -97.5, 35.195, "EPSG:4326", 0.00002).unwrap().is_empty());
    assert!(!f.e.identify(&f.pts, -97.5, 35.195, "EPSG:4326", 0.03).unwrap().is_empty());
    // Overlapping features: nearest first, ties by newest id.
    let both = f.e.identify(&f.polys, -97.5, 35.195, "EPSG:4326", 0.0004).unwrap();
    assert!(both.contains(&1) && both.contains(&2), "{both:?}");
    // Out-of-range clicks don't error.
    for (lon, lat) in [(200.0, 95.0), (-181.0, -91.0), (0.0, 90.0), (f64::NAN, 0.0)] {
        let _ = f.e.identify(&f.pts, lon, lat, "EPSG:4326", 0.001);
    }
}

#[test]
fn identify_and_selection_ignore_layers_without_a_crs_or_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let csv = dir.path().join("t.csv");
    std::fs::write(&csv, "a,b\n1,2\n").unwrap();
    let t = e.import(&csv).unwrap().remove(0);
    assert!(e.identify(&t.id, 0.0, 0.0, "EPSG:4326", 0.001).unwrap().is_empty());
    assert!(e.select_rect(&t.id, [-180.0, -90.0, 180.0, 90.0], "EPSG:4326").unwrap().is_empty());
    assert!(e.extent(&t.id, None).unwrap().is_none());
    let m = e.measure_feature(&t.id, 1).unwrap();
    assert!(m.area.is_none() && m.length.is_none() && m.centroid.is_none());
}

#[test]
fn box_select_covers_touching_and_whole_world_and_degenerate_boxes() {
    let f = fx();
    assert_eq!(f.e.select_rect(&f.pts, [-97.51, 35.19, -97.49, 35.20], "EPSG:4326").unwrap(), [1, 2]);
    assert_eq!(f.e.select_rect(&f.pts, [-180.0, -85.0, 180.0, 85.0], "EPSG:4326").unwrap(), [1, 2, 3]);
    assert!(f.e.select_rect(&f.pts, [10.0, 10.0, 11.0, 11.0], "EPSG:4326").unwrap().is_empty());
    // A box that only touches a polygon's edge still intersects it.
    assert!(f.e.select_rect(&f.polys, [-97.49, 35.19, -97.40, 35.20], "EPSG:4326").unwrap().contains(&2));
    // Selecting lines.
    assert_eq!(f.e.select_rect(&f.line, [-97.45, 35.19, -97.44, 35.21], "EPSG:4326").unwrap(), [1]);
}

#[test]
fn zero_size_box_selects_the_point_under_it() {
    let f = fx();
    assert_eq!(f.e.select_rect(&f.pts, [-97.505, 35.195, -97.505, 35.195], "EPSG:4326").unwrap(), [1]);
}

#[test]
fn measure_feature_is_geodesic_and_type_aware() {
    let f = fx();
    let poly = f.e.measure_feature(&f.polys, 1).unwrap();
    // 0.01° × 0.01° at 35.2°N: 0.01·111.32·cos(35.195°) km wide by 0.01·110.99 km high.
    let area = poly.area.unwrap();
    assert!((area - 1.0113e6).abs() / 1.0113e6 < 0.01, "area {area}");
    assert!(poly.perimeter.unwrap() > 3_900.0 && poly.perimeter.unwrap() < 4_100.0);
    assert!(poly.length.is_none());
    let c = poly.centroid.unwrap();
    assert!((c[0] + 97.505).abs() < 1e-6 && (c[1] - 35.195).abs() < 1e-6);
    let line = f.e.measure_feature(&f.line, 1).unwrap();
    assert!((line.length.unwrap() - 9_068.0).abs() < 60.0, "0.1° of longitude at 35.2°N is ~9.07 km: {:?}", line.length);
    assert!(line.area.is_none() && line.perimeter.is_none());
    let pt = f.e.measure_feature(&f.pts, 1).unwrap();
    assert!(pt.area.is_none() && pt.length.is_none() && pt.centroid.is_some());
    assert!(f.e.measure_feature(&f.polys, 12345).is_err(), "no such feature");
}

#[test]
fn unique_values_and_breaks_edge_cases() {
    let f = fx();
    let u = f.e.unique_values(&f.polys, "name", 10).unwrap();
    assert_eq!(u.iter().map(|(_, n)| n).sum::<i64>(), 3, "NULLs are counted too");
    assert_eq!(f.e.unique_values(&f.polys, "name", 1).unwrap().len(), 1, "limit applies");
    assert!(f.e.unique_values(&f.polys, "ghost", 10).is_err());
    assert!(f.e.unique_values(&f.polys, "name", 0).unwrap().is_empty());
    // Quantile breaks: n classes → n-1 interior breaks, NULLs ignored, non-numeric text gives no breaks.
    let b = f.e.quantile_breaks(&f.polys, "pop", 3).unwrap();
    assert_eq!(b.len(), 2);
    assert!(b[0] <= b[1] && b[0] >= 10.0 && b[1] <= 30.0);
    assert_eq!(f.e.quantile_breaks(&f.polys, "ratio", 2).unwrap(), [2.5], "median of the two non-null values");
    assert!(f.e.quantile_breaks(&f.polys, "name", 3).unwrap().is_empty(), "text with no numbers has no breaks");
    assert!(f.e.quantile_breaks(&f.polys, "ghost", 3).is_err());
    for classes in [0usize, 1] {
        let r = f.e.quantile_breaks(&f.polys, "pop", classes);
        assert!(r.as_ref().map_or(true, |v| v.is_empty()), "{classes} classes → no breaks, got {r:?}");
    }
}

#[test]
fn extents_for_selections() {
    let f = fx();
    let all = f.e.extent(&f.polys, None).unwrap().unwrap();
    assert!((all[0] + 97.51).abs() < 1e-9 && (all[3] - 35.21).abs() < 1e-9);
    let one = f.e.extent(&f.polys, Some(vec![2])).unwrap().unwrap();
    assert!((one[0] + 97.50).abs() < 1e-9 && (one[2] + 97.49).abs() < 1e-9);
    assert!(f.e.extent(&f.polys, Some(vec![])).unwrap().is_none(), "an empty selection has no extent");
    assert!(f.e.extent(&f.polys, Some(vec![999])).unwrap().is_none());
}
