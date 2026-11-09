//! Shape editing, package E1 (docs/SHAPE_EDITING_PLAN.md): the edit session, save and discard, exact coordinates, stable feature ids,
//! atomic save, crash recovery, and what the rest of the program sees while edits are pending.
use engine::edit::{bytes_to_hex, parse_wkb, write_wkb, EditOp};
use engine::graph::{Graph, Status};
use engine::table::RowsQuery;
use engine::Engine;
use serde_json::{json, Map, Value};
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

fn engine() -> (Engine, std::sync::MutexGuard<'static, ()>) {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = ONE.lock().unwrap_or_else(|p| p.into_inner());
    data_dir();
    (Engine::new().unwrap(), guard)
}

fn q<T: duckdb_row::FromRow>(e: &Engine, sql: &str) -> T {
    e.with_conn_for_tests(|c| Ok(T::get(c, sql))).unwrap()
}

mod duckdb_row {
    use engine::duckdb::Connection;
    pub trait FromRow {
        fn get(c: &Connection, sql: &str) -> Self;
    }
    impl FromRow for i64 {
        fn get(c: &Connection, sql: &str) -> Self {
            c.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
        }
    }
    impl FromRow for f64 {
        fn get(c: &Connection, sql: &str) -> Self {
            c.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
        }
    }
    impl FromRow for String {
        fn get(c: &Connection, sql: &str) -> Self {
            c.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
        }
    }
    impl FromRow for Vec<u8> {
        fn get(c: &Connection, sql: &str) -> Self {
            c.query_row(sql, [], |r| r.get(0)).unwrap_or_else(|e| panic!("{sql}: {e}"))
        }
    }
}

/// A point layer (WGS 84) of `n` points with an `id` and a `kind` field, imported from a GeoPackage.
fn points(e: &Engine, n: usize) -> engine::layers::LayerInfo {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("p.gpkg");
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT i::INTEGER AS id, 'k' || (i % 3) AS kind, i::DOUBLE AS area, ST_Point(-100.0 + (i % 1000) * 0.001, 30.0 + (i // 1000) * 0.001) AS geom FROM range(1, {} ) t(i)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:4326', USE_TMP_FILE false)",
        n + 1,
        p.display()
    ))
    .unwrap();
    e.import(&p).unwrap().remove(0)
}

fn rows_of(e: &Engine, id: &str) -> Vec<Vec<Option<String>>> {
    e.rows(id, &RowsQuery { limit: 1000, ..Default::default() }).unwrap().rows
}

fn pt(x: f64, y: f64) -> String {
    bytes_to_hex(&write_wkb(&parse_wkb(&hex_wkb(&format!("POINT({x} {y})"))).unwrap()))
}

/// WKB bytes of a WKT string (through DuckDB).
fn hex_wkb(wkt: &str) -> Vec<u8> {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.query_row(&format!("SELECT ST_AsWKB(ST_GeomFromText('{wkt}'))"), [], |r| r.get::<_, Vec<u8>>(0)).unwrap()
}

fn op(kind: &str, fid: Option<i64>, wkt: Option<&str>, attrs: Option<Value>) -> EditOp {
    EditOp {
        op: kind.into(),
        fid,
        geometry: wkt.map(|w| bytes_to_hex(&hex_wkb(w))),
        attributes: attrs.map(|a| a.as_object().cloned().unwrap_or_else(Map::new)),
        fix: false,
    }
}

fn fids(e: &Engine, id: &str) -> Vec<i64> {
    let t = e.layer(id).unwrap().table;
    e.with_conn_for_tests(|c| {
        let mut s = c.prepare(&format!("SELECT __fid FROM \"{t}\" ORDER BY __fid"))?;
        Ok(s.query_map([], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?)
    })
    .unwrap()
}

// ---------------------------------------------------------------- WKB

#[test]
fn wkb_round_trips_z_and_m_and_every_multi_type() {
    for wkt in [
        "POINT Z (1 2 3)",
        "POINT M (1 2 3)",
        "LINESTRING ZM (1 2 3 4, 5 6 7 8)",
        "POLYGON ((0 0, 4 0, 4 4, 0 0), (1 1, 2 1, 2 2, 1 1))",
        "MULTIPOINT (1 2, 3 4)",
        "MULTILINESTRING ((0 0, 1 1), (2 2, 3 3))",
        "MULTIPOLYGON Z (((0 0 1, 4 0 1, 4 4 1, 0 0 1)), ((10 10 2, 14 10 2, 14 14 2, 10 10 2)))",
    ] {
        let b = hex_wkb(wkt);
        let g = parse_wkb(&b).unwrap();
        assert_eq!(write_wkb(&g), b, "{wkt}");
    }
    assert!(parse_wkb(&hex_wkb("GEOMETRYCOLLECTION (POINT (1 2))")).is_err());
    assert!(parse_wkb(&[1, 1, 0]).is_err());
}

// ---------------------------------------------------------------- sessions

#[test]
fn save_applies_edits_in_one_step_and_discard_drops_them() {
    let (e, _g) = engine();
    let l = points(&e, 5);
    let before = rows_of(&e, &l.id);
    let v0 = l.version;
    let st = e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    assert!(st.editing && e.layer(&l.id).unwrap().editing);
    let r = e
        .edit_apply(
            &l.id,
            &[
                op("update", Some(2), Some("POINT(-99.5 30.5)"), Some(json!({"kind": "moved"}))),
                op("insert", None, Some("POINT(-99 31)"), Some(json!({"id": 99, "kind": "new"}))),
                op("delete", Some(4), None, None),
            ],
        )
        .unwrap();
    assert_eq!((r.status.added, r.status.changed, r.status.deleted, r.status.steps), (1, 1, 1, 1));
    // Nothing is saved yet: the table, its count and its version are as they were.
    assert_eq!(rows_of(&e, &l.id), before);
    assert_eq!(e.layer(&l.id).unwrap().version, v0);
    // The session sees its own edits.
    let f = e.edit_features_in(&l.id, [-101.0, 29.0, -98.0, 32.0], 100).unwrap();
    let got: Vec<(i64, Option<String>)> = f.features.iter().map(|f| (f.fid, f.pending.clone())).collect();
    assert!(got.contains(&(2, Some("changed".into()))) && got.contains(&(r.fids[0], Some("inserted".into()))));
    assert!(!f.features.iter().any(|x| x.fid == 4), "a deleted feature is not shown");
    assert_eq!(f.features.iter().find(|x| x.fid == 2).unwrap().attributes["kind"], "moved");

    // Discard: back to exactly the saved layer.
    let d = e.edit_discard(&l.id).unwrap();
    assert!(!d.editing);
    assert_eq!(rows_of(&e, &l.id), before);
    assert_eq!(e.edit_pending(&l.id).unwrap().editing, false);

    // Save: one transaction, version bumped, counts right.
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let r = e
        .edit_apply(
            &l.id,
            &[op("update", Some(2), Some("POINT(-99.5 30.5)"), Some(json!({"kind": "moved"}))), op("insert", None, Some("POINT(-99 31)"), Some(json!({"id": 99, "kind": "new"}))), op("delete", Some(4), None, None)],
        )
        .unwrap();
    let new_fid = r.fids[0];
    let saved = e.edit_save(&l.id).unwrap();
    assert_eq!((saved.added, saved.changed, saved.deleted), (1, 1, 1));
    assert_eq!(saved.layer.version, v0 + 1);
    assert_eq!(saved.layer.feature_count, 5);
    assert!(!e.layer(&l.id).unwrap().editing);
    let t = e.layer(&l.id).unwrap().table;
    assert_eq!(q::<String>(&e, &format!("SELECT kind FROM \"{t}\" WHERE __fid = 2")), "moved");
    assert_eq!(q::<f64>(&e, &format!("SELECT ST_X(geom) FROM \"{t}\" WHERE __fid = 2")), -99.5);
    assert_eq!(q::<i64>(&e, &format!("SELECT count(*) FROM \"{t}\" WHERE __fid = 4")), 0);
    assert_eq!(q::<i64>(&e, &format!("SELECT id FROM \"{t}\" WHERE __fid = {new_fid}")), 99);
    // The display columns agree with the new shapes (the tile source).
    assert!((q::<f64>(&e, &format!("SELECT __x0 FROM \"{t}\" WHERE __fid = 2")) - (-99.5 * 111319.490793)).abs() < 1.0);
    // History row, and a citation note keeping the sources.
    let h = e.edit_history(&l.id).unwrap();
    assert_eq!((h[0].added, h[0].changed, h[0].deleted), (1, 1, 1));
    let cites = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT count(*) FROM __sources WHERE key = ?", [format!("user-edit:{}", l.id)], |r| r.get::<_, i64>(0))?)).unwrap();
    assert_eq!(cites, 1);
    let file_cites: i64 = q(&e, "SELECT count(*) FROM __sources WHERE key LIKE 'file:%'");
    assert_eq!(file_cites, 1, "the layer keeps its own source");
}

#[test]
fn undo_and_redo_go_step_by_step_and_a_new_edit_drops_the_undone_steps() {
    let (e, _g) = engine();
    let l = points(&e, 3);
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), Some(json!({"id": 10})))]).unwrap();
    e.edit_apply(&l.id, &[op("delete", Some(1), None, None)]).unwrap();
    let s = e.edit_apply(&l.id, &[op("update", Some(2), Some("POINT(-98 30)"), None)]).unwrap().status;
    assert_eq!((s.added, s.changed, s.deleted, s.steps), (1, 1, 1, 3));
    let s = e.edit_undo(&l.id).unwrap();
    assert_eq!((s.added, s.changed, s.deleted, s.can_redo), (1, 0, 1, true));
    let s = e.edit_undo(&l.id).unwrap();
    assert_eq!((s.added, s.changed, s.deleted), (1, 0, 0));
    let s = e.edit_redo(&l.id).unwrap();
    assert_eq!((s.added, s.deleted), (1, 1));
    // A new edit makes the undone step unreachable.
    let s = e.edit_apply(&l.id, &[op("update", Some(3), Some("POINT(-98 31)"), None)]).unwrap().status;
    assert!(!s.can_redo);
    assert_eq!(e.edit_redo(&l.id).unwrap_err().to_string(), "Nothing to redo.");
    e.edit_undo(&l.id).unwrap();
    e.edit_undo(&l.id).unwrap();
    e.edit_undo(&l.id).unwrap();
    assert_eq!(e.edit_undo(&l.id).unwrap_err().to_string(), "Nothing to undo.");
}

#[test]
fn feature_ids_of_untouched_rows_never_change_and_new_ids_are_never_reused() {
    let (e, _g) = engine();
    let l = points(&e, 6);
    let before = fids(&e, &l.id);
    assert_eq!(before, [1, 2, 3, 4, 5, 6]);
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let r = e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), None), op("delete", Some(2), None, None)]).unwrap();
    assert_eq!(r.fids, [7]);
    e.edit_save(&l.id).unwrap();
    assert_eq!(fids(&e, &l.id), [1, 3, 4, 5, 6, 7], "untouched rows keep their ids; nothing was renumbered");
    // Delete the newest and save; a later session must not hand out 7 again.
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("delete", Some(7), None, None)]).unwrap();
    e.edit_save(&l.id).unwrap();
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let r = e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), None)]).unwrap();
    assert_eq!(r.fids, [8]);
    // Ids handed out in a discarded session are not reused either.
    e.edit_discard(&l.id).unwrap();
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    assert_eq!(e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), None)]).unwrap().fids, [9]);
    e.edit_discard(&l.id).unwrap();
}

// ---------------------------------------------------------------- coordinates

/// A UTM layer with one polygon with Z and M, put in directly (the shapes an import of such a file would give).
fn zm_layer(e: &Engine, crs: &str, wkt: &str) -> engine::layers::LayerInfo {
    let l = e.create_layer("zm", "polygon", crs, &[]).unwrap();
    let t = l.table.clone();
    e.with_conn_for_tests(|c| {
        c.execute_batch(&format!(
            "INSERT INTO \"{t}\" (__fid, geom, __g, __x0, __y0, __sz) SELECT 1, g, ST_Transform(g, '{crs}', 'EPSG:3857', true), 0, 0, 1 FROM (SELECT ST_GeomFromText('{wkt}') AS g)"
        ))?;
        Ok(())
    })
    .unwrap();
    l
}

fn stored(e: &Engine, l: &engine::layers::LayerInfo, fid: i64) -> engine::edit::Geom {
    parse_wkb(&q::<Vec<u8>>(e, &format!("SELECT ST_AsWKB(geom) FROM \"{}\" WHERE __fid = {fid}", l.table))).unwrap()
}

#[test]
fn moving_one_vertex_leaves_every_other_vertex_identical_including_z_and_m() {
    // Web Mercator over UTM, and a datum-shifting pair (British National Grid, OSGB36, seen on a WGS 84 map).
    for (layer_crs, map_crs, wkt) in [
        ("EPSG:32614", "EPSG:3857", "POLYGON ZM ((500123.123456789 4000000.987654321 11.5 101, 500923.5 4000000.25 12.5 102, 500923.5 4000800.75 13.5 103, 500123.123456789 4000800.5 14.5 104, 500123.123456789 4000000.987654321 11.5 101))"),
        ("EPSG:27700", "EPSG:4326", "POLYGON ZM ((530000.123456789 180000.987654321 21.5 201, 530900.5 180000.25 22.5 202, 530900.5 180900.75 23.5 203, 530000.123456789 180900.5 24.5 204, 530000.123456789 180000.987654321 21.5 201))"),
    ] {
        let (e, _g) = engine();
        let l = zm_layer(&e, layer_crs, wkt);
        let orig = stored(&e, &l, 1);
        assert!(orig.has_z && orig.has_m);
        e.edit_begin(&l.id, map_crs, Some(5.0)).unwrap();
        // The map gets the shape in its CRS and sends it back with one vertex moved.
        let f = e.edit_features_in(&l.id, [-180.0, -80.0, 180.0, 80.0], 10).unwrap();
        let mut shown = parse_wkb(&hex_to_vec(f.features[0].geometry.as_ref().unwrap())).unwrap();
        let (dx, dy) = if map_crs == "EPSG:4326" { (0.0001, 0.0001) } else { (50.0, 50.0) };
        shown.parts[0][0][2].x += dx;
        shown.parts[0][0][2].y += dy;
        let moved_hex = bytes_to_hex(&write_wkb(&shown));
        e.edit_apply(&l.id, &[EditOp { op: "update".into(), fid: Some(1), geometry: Some(moved_hex), ..Default::default() }]).unwrap();
        e.edit_save(&l.id).unwrap();
        let after = stored(&e, &l, 1);
        let (a, b): (Vec<_>, Vec<_>) = (orig.parts[0][0].clone(), after.parts[0][0].clone());
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            if i == 2 {
                assert!((a[i].x - b[i].x).abs() > 1.0 || (a[i].y - b[i].y).abs() > 1.0, "the moved vertex moved ({layer_crs})");
                assert_eq!((b[i].z, b[i].m), (a[i].z, a[i].m), "a moved vertex keeps its Z and M");
            } else {
                // Bit for bit, not nearly: a round trip through the map CRS would drift, a datum shift more so.
                assert_eq!(
                    (a[i].x.to_bits(), a[i].y.to_bits(), a[i].z.to_bits(), a[i].m.to_bits()),
                    (b[i].x.to_bits(), b[i].y.to_bits(), b[i].z.to_bits(), b[i].m.to_bits()),
                    "vertex {i} of {layer_crs} changed"
                );
            }
        }
        // Byte-identical for the whole stored geometry when nothing was moved.
        e.edit_begin(&l.id, map_crs, None).unwrap();
        let f = e.edit_features_in(&l.id, [-180.0, -80.0, 180.0, 80.0], 10).unwrap();
        e.edit_apply(&l.id, &[EditOp { op: "update".into(), fid: Some(1), geometry: f.features[0].geometry.clone(), ..Default::default() }]).unwrap();
        e.edit_save(&l.id).unwrap();
        assert_eq!(write_wkb(&stored(&e, &l, 1)), write_wkb(&after));
    }
}

fn hex_to_vec(h: &str) -> Vec<u8> {
    (0..h.len()).step_by(2).map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn a_utm_layer_edited_from_a_web_mercator_map_gets_exact_coordinates() {
    let (e, _g) = engine();
    let l = e.create_layer("utm points", "point", "EPSG:32614", &[("name".into(), "text".into())]).unwrap();
    e.edit_begin(&l.id, "EPSG:3857", None).unwrap();
    // Longitude -99° is the central meridian of zone 14: on the equator that is easting 500000, northing 0.
    let x = -99.0 * 20037508.342789244 / 180.0;
    let r = e.edit_apply(&l.id, &[op("insert", None, Some(&format!("POINT({x} 0)")), Some(json!({"name": "cm"})))]).unwrap();
    assert!(r.invalid.is_empty());
    e.edit_save(&l.id).unwrap();
    let t = e.layer(&l.id).unwrap().table;
    let (ex, ny) = (q::<f64>(&e, &format!("SELECT ST_X(geom) FROM \"{t}\"")), q::<f64>(&e, &format!("SELECT ST_Y(geom) FROM \"{t}\"")));
    assert!((ex - 500000.0).abs() < 1e-6, "easting {ex}");
    assert!(ny.abs() < 1e-6, "northing {ny}");
    // And back out to the map it lands where it was drawn.
    e.edit_begin(&l.id, "EPSG:3857", None).unwrap();
    let f = e.edit_features_in(&l.id, [-180.0, -80.0, 180.0, 80.0], 10).unwrap();
    let g = parse_wkb(&hex_to_vec(f.features[0].geometry.as_ref().unwrap())).unwrap();
    assert!((g.parts[0][0][0].x - x).abs() < 1e-5);
    e.edit_discard(&l.id).unwrap();
}

// ---------------------------------------------------------------- atomic save, overviews

#[test]
fn a_save_failing_partway_leaves_the_table_overviews_and_version_unchanged() {
    let (e, _g) = engine();
    let l = points(&e, 50_100);
    assert!(!l.overviews.is_empty(), "a layer this big has overviews");
    let t = l.table.clone();
    let snap = |e: &Engine| -> (i64, i64, Vec<i64>, u64) {
        let ov: Vec<i64> = l.overviews.iter().map(|z| q::<i64>(e, &format!("SELECT count(*) FROM \"{t}__ov{z}\""))).collect();
        (q::<i64>(e, &format!("SELECT count(*) FROM \"{t}\"")), q::<i64>(e, &format!("SELECT sum(__fid) FROM \"{t}\"")), ov, e.layer(&l.id).unwrap().version)
    };
    let before = snap(&e);
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("update", Some(1), Some("POINT(-99.9 30.1)"), None), op("delete", Some(2), None, None), op("insert", None, Some("POINT(-99 31)"), None)]).unwrap();
    // A value that can't be stored, written straight into the log so it is only found while saving (after the others were applied).
    let lt = format!("__edits_{}", l.id);
    e.with_conn_for_tests(|c| {
        c.execute_batch(&format!("INSERT INTO \"{lt}\" VALUES (100, 9, 'update', 3, NULL, '{{\"area\": \"not a number\"}}', false)"))?;
        Ok(())
    })
    .unwrap();
    let err = e.edit_save(&l.id).unwrap_err().to_string();
    assert!(!err.is_empty());
    assert_eq!(snap(&e), before, "table, overviews and version are as before");
    assert!(e.layer(&l.id).unwrap().editing, "the session is still open, edits still pending");
    assert_eq!(q::<i64>(&e, &format!("SELECT count(*) FROM \"{lt}\"")), 4);
    // The layer is still usable: the index is back, tiles read it.
    assert!(e.tile(&l.id, 10, 200, 400, &[]).is_ok());
    // Remove the bad row and the same session saves, overviews included.
    e.with_conn_for_tests(|c| Ok(c.execute_batch(&format!("DELETE FROM \"{lt}\" WHERE seq = 100"))?)).unwrap();
    let r = e.edit_save(&l.id).unwrap();
    assert_eq!((r.added, r.changed, r.deleted), (1, 1, 1));
    for z in &l.overviews {
        let ov = format!("{t}__ov{z}");
        assert_eq!(q::<i64>(&e, &format!("SELECT count(*) FROM \"{ov}\" WHERE __fid = 2")), 0, "deleted feature gone from {ov}");
        assert_eq!(q::<f64>(&e, &format!("SELECT ST_X(geom) FROM \"{ov}\" WHERE __fid = 1")), -99.9, "changed feature updated in {ov}");
        assert_eq!(q::<i64>(&e, &format!("SELECT count(*) FROM \"{ov}\" WHERE __fid = 50101")), 1, "new feature in {ov}");
    }
}

// ---------------------------------------------------------------- what others see

fn run_rows(e: &Engine, layer: &str) -> (Option<i64>, Status) {
    let g: Graph = serde_json::from_value(json!({"nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}], "edges": []})).unwrap();
    let r = e.run_graph(&g, Some("in"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "in").unwrap();
    (r.rows, r.status)
}

#[test]
fn pending_edits_are_not_in_node_trees_and_the_tree_reruns_after_save() {
    let (e, _g) = engine();
    let l = points(&e, 4);
    assert_eq!(run_rows(&e, &l.id).0, Some(4));
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), None), op("delete", Some(1), None, None), op("delete", Some(2), None, None)]).unwrap();
    let (n, status) = run_rows(&e, &l.id);
    assert_eq!(n, Some(4), "the tree reads saved data");
    assert!(matches!(status, Status::Done | Status::Cached));
    // Nor does a copy or a table page see them.
    assert_eq!(e.copy_layer(&l.id, "copy", None).unwrap().feature_count, 4);
    assert_eq!(rows_of(&e, &l.id).len(), 4);
    e.edit_save(&l.id).unwrap();
    let (n, status) = run_rows(&e, &l.id);
    assert_eq!(n, Some(3), "the tree ran again on the new version");
    assert!(matches!(status, Status::Done), "not served from the cache: {status:?}");
}

#[test]
fn a_python_definition_query_runs_again_after_save() {
    if engine::resources::python_exe().is_none() {
        eprintln!("SKIPPED: no python3");
        return;
    }
    let (e, _g) = engine();
    let l = points(&e, 4);
    let script = "def calculate(row):\n    return row[\"area\"] > 2\n";
    e.set_definition_python(&l.id, Some(script)).unwrap();
    e.approve_script(script).unwrap();
    assert_eq!(e.layer(&l.id).unwrap().query_count, Some(2));
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), Some(json!({"id": 50, "area": 9.0})))]).unwrap();
    assert_eq!(e.layer(&l.id).unwrap().query_count, Some(2), "pending edits don't count");
    e.edit_save(&l.id).unwrap();
    let after = e.layer(&l.id).unwrap();
    assert_eq!(after.query_count, Some(3), "the script ran on the new data version");
    assert_eq!(rows_of(&e, &l.id).len(), 3);
    assert_eq!(after.definition_error, None);
}

#[test]
fn tools_that_would_write_the_layer_are_blocked_with_a_sentence_during_a_session() {
    let (e, _g) = engine();
    let l = points(&e, 3);
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let msg = e.update_cell(&l.id, 1, "kind", Some("x")).unwrap_err().to_string();
    assert!(msg.contains("is being edited") && msg.contains("Save or discard"), "{msg}");
    assert!(e.add_field(&l.id, "z", "text").unwrap_err().to_string().contains("being edited"));
    assert!(e.delete_rows(&l.id, &[1]).unwrap_err().to_string().contains("being edited"));
    assert!(e.define_projection(&l.id, "EPSG:3857", None).unwrap_err().to_string().contains("renumber"));
    assert!(e.edit_begin("nope", "EPSG:4326", None).is_err());
    e.edit_discard(&l.id).unwrap();
    assert!(e.update_cell(&l.id, 1, "kind", Some("x")).is_ok(), "allowed again after the session");
}

#[test]
fn layers_that_can_not_be_edited_say_why() {
    let (e, _g) = engine();
    let l = points(&e, 3);
    e.define_projection(&l.id, "EPSG:4326", None).unwrap();
    let t = e.create_table("t", &[("a".into(), "text".into())]).unwrap().id;
    let msg = e.edit_begin(&t, "EPSG:4326", None).unwrap_err().to_string();
    assert!(msg.contains("no shapes"), "{msg}");
    assert!(e.create_layer("x", "polygon", "", &[]).unwrap_err().to_string().contains("never left blank"));
    assert!(e.create_layer("x", "blob", "EPSG:4326", &[]).unwrap_err().to_string().contains("Unknown geometry type"));
    assert!(e.create_layer("", "point", "EPSG:4326", &[]).is_err());
    assert!(e.create_layer("x", "point", "EPSG:4326", &[("a".into(), "text".into()), ("A".into(), "text".into())]).is_err());
}

// ---------------------------------------------------------------- create layer, validity, errors

#[test]
fn a_new_layer_has_its_fields_an_id_and_a_citation_naming_the_user() {
    let (e, _g) = engine();
    let l = e.create_layer("Wells", "point", "EPSG:32614", &[("depth".into(), "double".into()), ("name".into(), "text".into())]).unwrap();
    assert_eq!((l.geom_type.as_str(), l.crs.as_deref(), l.feature_count), ("Point", Some("EPSG:32614"), 0));
    assert_eq!(l.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["id", "depth", "name"]);
    let line = e.create_layer("Roads", "line", "EPSG:4326", &[]).unwrap();
    assert_eq!(line.geom_type, "Line");
    assert_eq!(e.create_layer("Parcels", "polygon", "EPSG:4326", &[("id".into(), "text".into())]).unwrap().fields.len(), 1);
    let author: String = q(&e, &format!("SELECT citation::JSON->>'author' FROM __sources WHERE key = 'user:{}'", l.id));
    assert!(!author.is_empty());
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "the user".into());
    assert_eq!(author, user);
    // It can be edited at once, and a shape of the wrong kind is refused.
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let err = e.edit_apply(&l.id, &[op("insert", None, Some("LINESTRING(-99 31, -98 31)"), None)]).unwrap_err().to_string();
    assert!(err.contains("holds points"), "{err}");
    let err = e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), Some(json!({"depth": "deep"})))]).unwrap_err().to_string();
    assert!(err.contains("does not fit"), "{err}");
    let err = e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 95)"), None)]).unwrap_err().to_string();
    assert!(err.contains("latitude"), "{err}");
    assert!(e.edit_apply(&l.id, &[op("update", Some(77), Some("POINT(-99 31)"), None)]).unwrap_err().to_string().contains("does not exist"));
    assert!(e.edit_apply(&l.id, &[op("insert", None, None, None)]).unwrap_err().to_string().contains("needs a shape"));
    assert_eq!(e.edit_pending(&l.id).unwrap().steps, 0, "a refused batch writes nothing");
}

#[test]
fn invalid_shapes_are_flagged_fixed_on_request_and_reported_at_save() {
    let (e, _g) = engine();
    let l = e.create_layer("p", "polygon", "EPSG:4326", &[]).unwrap();
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    let bow = "POLYGON((0 0, 2 2, 2 0, 0 2, 0 0))";
    let r = e.edit_apply(&l.id, &[op("insert", None, Some(bow), None)]).unwrap();
    assert_eq!(r.invalid, r.fids);
    assert_eq!(r.status.invalid, r.fids);
    let mut fixed = op("insert", None, Some(bow), None);
    fixed.fix = true;
    let r2 = e.edit_apply(&l.id, &[fixed]).unwrap();
    assert!(r2.invalid.is_empty());
    let s = e.edit_save(&l.id).unwrap();
    assert_eq!(s.invalid, 1);
    assert!(s.note.unwrap().contains("not valid"));
    assert_eq!(s.layer.feature_count, 2);
}

// ---------------------------------------------------------------- crash recovery, old projects

#[test]
fn edits_survive_a_crash_and_the_next_open_can_resume_or_discard_them() {
    let (e, _g) = engine();
    let l = points(&e, 4);
    let project = tempfile::tempdir().unwrap();
    e.save_project(project.path(), "{}").unwrap();
    e.open_project(project.path()).unwrap();
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), Some(json!({"id": 77}))), op("delete", Some(1), None, None)]).unwrap();
    // The program dies here: no Save, no Discard.
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(project.path()).unwrap();
    let again = e2.layer(&l.id).unwrap();
    assert!(again.editing, "the layer is marked as being edited");
    assert_eq!(again.feature_count, 4, "saved data is as it was");
    let st = e2.edit_pending(&l.id).unwrap();
    assert!(st.editing);
    assert_eq!((st.added, st.deleted, st.steps), (1, 1, 1));
    // Resume: the pending edits are all there and can be saved.
    let st = e2.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    assert_eq!(st.added, 1);
    // The attribute table is blocked while they wait.
    assert!(e2.update_cell(&l.id, 2, "kind", Some("x")).is_err());
    let r = e2.edit_save(&l.id).unwrap();
    assert_eq!((r.added, r.deleted, r.layer.feature_count), (1, 1, 4));
    // And the other way: discard on the next open.
    e2.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e2.edit_apply(&l.id, &[op("delete", Some(2), None, None)]).unwrap();
    e2.save_project(project.path(), "{}").unwrap();
    drop(e2);
    let e3 = Engine::new().unwrap();
    e3.open_project(project.path()).unwrap();
    assert!(e3.layer(&l.id).unwrap().editing);
    e3.edit_discard(&l.id).unwrap();
    assert!(!e3.layer(&l.id).unwrap().editing);
    assert_eq!(fids(&e3, &l.id).len(), 4);
}

#[test]
fn an_old_project_without_the_edit_tables_opens_and_can_be_edited() {
    let (e, _g) = engine();
    let l = points(&e, 3);
    let project = tempfile::tempdir().unwrap();
    e.save_project(project.path(), "{}").unwrap();
    e.open_project(project.path()).unwrap();
    // As an older build left it: none of the new tables, and no `editing` key in the stored layer.
    e.with_conn_for_tests(|c| {
        c.execute_batch("DROP TABLE __edit_sessions; DROP TABLE __edit_fids; DROP TABLE __edit_log; UPDATE __layers SET info = json_merge_patch(info, '{\"editing\": null}')")?;
        Ok(())
    })
    .unwrap();
    e.save_project(project.path(), "{}").unwrap();
    drop(e);
    let e2 = Engine::new().unwrap();
    e2.open_project(project.path()).unwrap();
    let again = e2.layer(&l.id).unwrap();
    assert!(!again.editing);
    assert_eq!(again.feature_count, 3);
    e2.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e2.edit_apply(&l.id, &[op("insert", None, Some("POINT(-99 31)"), None)]).unwrap();
    assert_eq!(e2.edit_save(&l.id).unwrap().layer.feature_count, 4);
}

#[test]
fn points_helper_is_consistent() {
    // Guards the fixture the other tests lean on.
    let _ = pt(1.0, 2.0);
    let (e, _g) = engine();
    let l = points(&e, 3);
    assert_eq!(l.feature_count, 3);
    assert_eq!(l.fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["fid", "id", "kind", "area"]);
}

#[test]
fn tiles_leave_out_features_with_pending_edits_during_a_session() {
    let (e, _g) = engine();
    let l = points(&e, 3);
    let full = e.tile(&l.id, 0, 0, 0, &[]).unwrap();
    assert!(!full.is_empty());
    e.edit_begin(&l.id, "EPSG:4326", None).unwrap();
    e.edit_apply(&l.id, &[op("delete", Some(1), None, None), op("delete", Some(2), None, None), op("delete", Some(3), None, None)]).unwrap();
    assert!(e.tile(&l.id, 0, 0, 0, &[]).unwrap().is_empty(), "the edit overlay draws them, the tile doesn't");
    e.edit_undo(&l.id).unwrap();
    assert_eq!(e.tile(&l.id, 0, 0, 0, &[]).unwrap(), full, "undo puts them back in the tile");
    e.edit_discard(&l.id).unwrap();
    assert_eq!(e.tile(&l.id, 0, 0, 0, &[]).unwrap(), full);
}
