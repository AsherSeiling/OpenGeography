//! Vector spatial adjustment (GR3): known transforms recovered, attributes and the source untouched, dropped features reported,
//! mixed coordinate systems, and the same error-table traps as the raster warp.

use engine::georef::adjust::{AdjustMethod, AdjustRequest};
use engine::georef::{ControlPoint, ControlSet};
use engine::Engine;
use std::path::Path;

fn write_fixture(path: &Path, select: &str, srs: &str) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS '{srs}', USE_TMP_FILE false)", path.display())).unwrap();
}

/// from (x, y) in the layer to (x, y) in `crs`.
fn pt(id: u32, fx: f64, fy: f64, tx: f64, ty: f64) -> ControlPoint {
    ControlPoint { id, pixel: fx, line: fy, x: tx, y: ty, enabled: true }
}

fn set(crs: &str, points: Vec<ControlPoint>) -> ControlSet {
    ControlSet { id: "s".into(), name: "adj".into(), target_crs: crs.into(), image: None, points }
}

/// A known similarity-plus-shear free map: rotate 20 degrees, scale 1.5, shift.
fn rot(x: f64, y: f64) -> (f64, f64) {
    let (c, s) = (20f64.to_radians().cos() * 1.5, 20f64.to_radians().sin() * 1.5);
    (c * (x - 600_000.0) - s * (y - 3_900_000.0) + 610_000.0, s * (x - 600_000.0) + c * (y - 3_900_000.0) + 3_910_000.0)
}

fn numbers(wkt: &str) -> Vec<f64> {
    wkt.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E')).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect()
}

fn geoms(e: &Engine, table: &str) -> Vec<(i64, Vec<f64>)> {
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&format!("SELECT id, ST_AsText(geom) FROM \"{table}\" ORDER BY id"))?;
        let v = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, numbers(&r.get::<_, String>(1)?))))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .unwrap()
}

fn setup(dir: &Path, srs: &str, select: &str) -> (Engine, engine::layers::LayerInfo) {
    let f = dir.join("in.gpkg");
    write_fixture(&f, select, srs);
    let e = Engine::new().unwrap();
    let l = e.import(&f).unwrap().remove(0);
    (e, l)
}

const SQUARES: &str = "SELECT 1 AS id, 'a' AS name, 10 AS pop, ST_MakeEnvelope(600000, 3900000, 600100, 3900100) AS geom
    UNION ALL SELECT 2, 'b', 20, ST_MakeEnvelope(600100, 3900000, 600200, 3900100)
    UNION ALL SELECT 3, 'c', 30, ST_MakeEnvelope(600000, 3900100, 600100, 3900200)";

fn req(layer: &str, s: ControlSet, m: AdjustMethod) -> AdjustRequest {
    AdjustRequest { layer: layer.into(), set: s, method: m, name: None }
}

#[test]
fn affine_and_similarity_recover_a_known_transform_and_leave_the_source_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", SQUARES);
    let before = geoms(&e, &l.table);
    let from = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0), (600_200.0, 3_900_200.0)];
    let pts: Vec<ControlPoint> = from.iter().enumerate().map(|(i, f)| { let t = rot(f.0, f.1); pt(i as u32 + 1, f.0, f.1, t.0, t.1) }).collect();

    for method in [AdjustMethod::Affine, AdjustMethod::Similarity] {
        let r = e.adjust_layer(&req(&l.id, set("EPSG:32614", pts.clone()), method)).unwrap();
        assert_eq!(r.layer.crs.as_deref(), Some("EPSG:32614"));
        assert_eq!(r.report.output_crs, "EPSG:32614");
        assert!(r.dropped.is_empty());
        assert_eq!(r.moved, 3);
        assert!(r.report.rms < 1e-6, "{method:?} rms {}", r.report.rms);
        for ((_, orig), (_, got)) in before.iter().zip(geoms(&e, &r.layer.table)) {
            for (o, g) in orig.chunks(2).zip(got.chunks(2)) {
                let want = rot(o[0], o[1]);
                assert!((want.0 - g[0]).abs() < 1e-5 && (want.1 - g[1]).abs() < 1e-5, "{method:?}: {o:?} -> {g:?}, want {want:?}");
            }
        }
        if method == AdjustMethod::Similarity {
            assert!((r.report.scale.unwrap() - 1.5).abs() < 1e-9 && (r.report.rotation_deg.unwrap() - 20.0).abs() < 1e-7, "{:?} {:?}", r.report.scale, r.report.rotation_deg);
        }
        // Untouched attributes are identical.
        let attrs = |t: &str| e.with_conn_for_tests(|c| {
            let mut st = c.prepare(&format!("SELECT id, name, pop FROM \"{t}\" ORDER BY id"))?;
            Ok(st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?.collect::<Result<Vec<_>, _>>()?)
        }).unwrap();
        assert_eq!(attrs(&r.layer.table), attrs(&l.table));
        // The source layer is unchanged.
        assert_eq!(geoms(&e, &l.table), before);
        assert_eq!(e.layer(&l.id).unwrap().feature_count, 3);
    }
    // Similarity needs only two points, and says an exact fit means nothing.
    let two = set("EPSG:32614", pts[..2].to_vec());
    let r = e.adjust_layer(&req(&l.id, two, AdjustMethod::Similarity)).unwrap();
    assert!(r.report.warnings.iter().any(|w| w.contains("no spare points")), "{:?}", r.report.warnings);
    let three = set("EPSG:32614", vec![pts[0].clone(), pts[1].clone(), pts[2].clone()]);
    assert!(e.adjust_errors(&l.id, &three, AdjustMethod::Affine).unwrap().warnings.iter().any(|w| w.contains("no spare points")));
    assert!(e.adjust_errors(&l.id, &set("EPSG:32614", pts.clone()), AdjustMethod::Affine).unwrap().warnings.is_empty());
}

#[test]
fn rubber_sheet_lands_control_points_exactly_and_warns_about_it() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", SQUARES);
    let warp = |x: f64, y: f64| (x + 20.0 * ((y - 3_900_000.0) / 60.0).sin(), y + 15.0 * ((x - 600_000.0) / 70.0).cos());
    let pts: Vec<ControlPoint> = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0), (600_200.0, 3_900_200.0), (600_100.0, 3_900_100.0), (600_100.0, 3_900_000.0)]
        .iter()
        .enumerate()
        .map(|(i, f)| { let t = warp(f.0, f.1); pt(i as u32 + 1, f.0, f.1, t.0, t.1) })
        .collect();
    let mut s = set("EPSG:32614", pts);
    s.points[5].enabled = false;
    let r = e.adjust_layer(&req(&l.id, s, AdjustMethod::RubberSheet)).unwrap();
    for p in r.report.points.iter().filter(|p| p.enabled) {
        assert!(p.length < 1e-6, "point {} error {}", p.id, p.length);
    }
    assert!(r.report.rms < 1e-6);
    let chk = r.report.points.iter().find(|p| p.check).unwrap();
    assert!(chk.length > 1e-3, "check point error {}", chk.length);
    assert!(r.report.warnings.iter().any(|w| w.contains("zero")), "{:?}", r.report.warnings);
    // The vertex at a control point lands at its "to" position.
    let out = geoms(&e, &r.layer.table);
    let (x, y) = warp(600_100.0, 3_900_100.0);
    assert!(out.iter().flat_map(|(_, c)| c.chunks(2)).any(|c| (c[0] - x).abs() < 1e-5 && (c[1] - y).abs() < 1e-5), "no vertex at {x},{y}");
}

#[test]
fn features_pushed_past_the_poles_are_dropped_and_reported_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(
        dir.path(),
        "EPSG:4326",
        "SELECT 1 AS id, 'a' AS name, ST_Point(10, 20) AS geom UNION ALL SELECT 2, 'b', ST_Point(15, 85) UNION ALL SELECT 3, 'c', ST_Point(20, 50)",
    );
    // A pure shift of 10 degrees north: the point at 85 would land at 95.
    let pts: Vec<ControlPoint> = [(0.0, 0.0), (60.0, 0.0), (0.0, 60.0), (60.0, 60.0)].iter().enumerate().map(|(i, f)| pt(i as u32 + 1, f.0, f.1, f.0, f.1 + 10.0)).collect();
    let r = e.adjust_layer(&req(&l.id, set("EPSG:4326", pts), AdjustMethod::Affine)).unwrap();
    assert_eq!(r.dropped.len(), 1);
    let fid2: i64 = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT __fid FROM \"{}\" WHERE id = 2", l.table), [], |r| r.get(0))?)).unwrap();
    assert_eq!(r.dropped[0].fid, fid2, "the dropped feature is reported by its id");
    assert!(r.dropped[0].reason.contains("poles"), "{}", r.dropped[0].reason);
    assert_eq!(r.moved, 2);
    assert_eq!(r.layer.feature_count, 2);
    assert!(r.notes.iter().any(|n| n.contains("left out") && n.contains('#')), "{:?}", r.notes);
    assert!(r.layer.warnings.iter().any(|w| w.contains("could not be adjusted")));
    let g = geoms(&e, &r.layer.table);
    assert_eq!(g.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![1, 3]);
    assert!((g[0].1[1] - 30.0).abs() < 1e-9 && (g[1].1[1] - 60.0).abs() < 1e-9);
}

#[test]
fn to_points_in_another_crs_are_transformed_explicitly_and_the_output_crs_is_the_layers() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", SQUARES);
    // From points in UTM 14N; to points given as lon/lat (WGS 84): the same ground positions shifted by (+50, +70) m.
    let from = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0), (600_200.0, 3_900_200.0)];
    let to_utm: Vec<(f64, f64)> = from.iter().map(|f| (f.0 + 50.0, f.1 + 70.0)).collect();
    let to_ll: Vec<(f64, f64)> = to_utm.iter().map(|t| e.transform_point(t.0, t.1, "EPSG:32614", "EPSG:4326").unwrap()).collect();
    let pts: Vec<ControlPoint> = from.iter().zip(&to_ll).enumerate().map(|(i, (f, t))| pt(i as u32 + 1, f.0, f.1, t.0, t.1)).collect();
    let r = e.adjust_layer(&req(&l.id, set("EPSG:4326", pts), AdjustMethod::Affine)).unwrap();
    assert_eq!(r.layer.crs.as_deref(), Some("EPSG:32614"), "the adjusted layer stays in the layer's CRS");
    assert!(r.notes.iter().any(|n| n.contains("transformed into the layer's coordinate system")), "{:?}", r.notes);
    let out = geoms(&e, &r.layer.table);
    let orig = geoms(&e, &l.table);
    for ((_, o), (_, g)) in orig.iter().zip(&out) {
        for (a, b) in o.chunks(2).zip(g.chunks(2)) {
            assert!((b[0] - a[0] - 50.0).abs() < 1e-4 && (b[1] - a[1] - 70.0).abs() < 1e-4, "{a:?} -> {b:?}");
        }
    }
    assert!(r.report.rms_m < 1e-4);
}

#[test]
fn z_values_are_carried_through_and_a_folded_shape_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", "SELECT 1 AS id, ST_Force3DZ(ST_Point(600010, 3900010), 12.5) AS geom UNION ALL SELECT 2, ST_Force3DZ(ST_Point(600150, 3900150), -3.25)");
    let pts: Vec<ControlPoint> = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0)].iter().enumerate().map(|(i, f)| pt(i as u32 + 1, f.0, f.1, f.0 + 100.0, f.1 - 40.0)).collect();
    let r = e.adjust_layer(&req(&l.id, set("EPSG:32614", pts), AdjustMethod::Similarity)).unwrap();
    let zs: Vec<(f64, f64, f64)> = e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&format!("SELECT ST_X(geom), ST_Y(geom), ST_Z(geom) FROM \"{}\" ORDER BY id", r.layer.table))?;
        Ok(st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>, _>>()?)
    }).unwrap();
    assert!((zs[0].0 - 600_110.0).abs() < 1e-6 && (zs[0].1 - 3_899_970.0).abs() < 1e-6 && zs[0].2 == 12.5, "{zs:?}");
    assert_eq!(zs[1].2, -3.25);

    // A rubber sheet that swaps two corners folds a square across itself: it was valid, now it is not, so it is dropped and reported.
    let dir2 = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir2.path(), "EPSG:32614", "SELECT 1 AS id, ST_MakeEnvelope(600000, 3900000, 600100, 3900100) AS geom UNION ALL SELECT 2, ST_MakeEnvelope(600500, 3900500, 600510, 3900510)");
    let c = |i, fx: f64, fy: f64, tx: f64, ty: f64| pt(i, fx, fy, tx, ty);
    // The first square's corners are mapped crosswise (a twist); the second square is far from the twist and stays valid.
    let pts = vec![
        c(1, 600_000.0, 3_900_000.0, 600_000.0, 3_900_000.0),
        c(2, 600_100.0, 3_900_000.0, 600_100.0, 3_900_100.0),
        c(3, 600_100.0, 3_900_100.0, 600_100.0, 3_900_000.0),
        c(4, 600_000.0, 3_900_100.0, 600_000.0, 3_900_100.0),
        c(5, 600_500.0, 3_900_500.0, 600_500.0, 3_900_500.0),
        c(6, 600_510.0, 3_900_510.0, 600_510.0, 3_900_510.0),
        c(7, 600_700.0, 3_900_000.0, 600_700.0, 3_900_000.0),
    ];
    let r = e.adjust_layer(&req(&l.id, set("EPSG:32614", pts), AdjustMethod::RubberSheet)).unwrap();
    // The first square's right side is swapped top for bottom: its ring becomes a bow tie. The far square is valid before and after.
    assert_eq!(r.dropped.len(), 1, "{:?}", r.dropped);
    assert!(r.dropped[0].reason.contains("invalid"), "{}", r.dropped[0].reason);
    assert_eq!(r.layer.feature_count, 1);
    assert_eq!(r.moved, 1);
    let invalid: i64 = e.with_conn_for_tests(|cn| Ok(cn.query_row(&format!("SELECT count(*) FROM \"{}\" WHERE NOT ST_IsValid(geom)", r.layer.table), [], |x| x.get(0))?)).unwrap();
    assert_eq!(invalid, 0);
}

#[test]
fn refusals_read_as_sentences() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", SQUARES);
    let a = |s: ControlSet, m| e.adjust_errors(&l.id, &s, m).unwrap_err().to_string();
    let p = vec![pt(1, 0.0, 0.0, 1.0, 1.0), pt(2, 10.0, 10.0, 11.0, 11.0)];
    assert!(a(set("EPSG:32614", p.clone()), AdjustMethod::Affine).contains("at least 3"));
    assert!(a(set("EPSG:32614", p[..1].to_vec()), AdjustMethod::Similarity).contains("at least 2"));
    assert!(a(set("", p.clone()), AdjustMethod::Similarity).contains("never guessed"));
    let line = vec![pt(1, 0.0, 0.0, 0.0, 0.0), pt(2, 10.0, 10.0, 10.0, 10.0), pt(3, 20.0, 20.0, 20.0, 20.0)];
    assert!(a(set("EPSG:32614", line), AdjustMethod::Affine).contains("one straight line"));
    let same = vec![pt(1, 5.0, 5.0, 0.0, 0.0), pt(2, 5.0, 5.0, 10.0, 10.0)];
    assert!(a(set("EPSG:32614", same), AdjustMethod::Similarity).contains("same place"));
    // A geographic layer: points spread over more than 180 degrees of longitude are refused with advice.
    let g = setup(dir.path(), "EPSG:4326", "SELECT 1 AS id, ST_Point(0, 0) AS geom");
    let wide = vec![pt(1, -170.0, 0.0, -170.0, 0.0), pt(2, 20.0, 0.0, 20.0, 0.0), pt(3, 0.0, 40.0, 0.0, 40.0)];
    let err = g.0.adjust_errors(&g.1.id, &set("EPSG:4326", wide), AdjustMethod::Affine).unwrap_err().to_string();
    assert!(err.contains("projected coordinate system"), "{err}");
    // A table has no shapes.
    let csv = dir.path().join("t.csv");
    std::fs::write(&csv, "a,b\n1,2\n").unwrap();
    let t = e.import(&csv).unwrap().remove(0);
    assert!(e.adjust_layer(&req(&t.id, set("EPSG:32614", p), AdjustMethod::Similarity)).unwrap_err().to_string().contains("no shapes"));
}

#[test]
fn citations_keep_the_source_and_record_the_points_and_method() {
    let dir = tempfile::tempdir().unwrap();
    let (e, l) = setup(dir.path(), "EPSG:32614", SQUARES);
    let pts: Vec<ControlPoint> = [(600_000.0, 3_900_000.0), (600_200.0, 3_900_000.0), (600_000.0, 3_900_200.0)].iter().enumerate().map(|(i, f)| pt(i as u32 + 1, f.0, f.1, f.0 + 5.0, f.1)).collect();
    let r = e.adjust_layer(&AdjustRequest { name: Some("Shifted".into()), ..req(&l.id, set("EPSG:32614", pts), AdjustMethod::Affine) }).unwrap();
    assert_eq!(r.layer.name, "Shifted");
    let srcs = e.sources().unwrap();
    let c = srcs.iter().find(|s| s.citation.key.starts_with("adjust:")).expect("an adjustment citation");
    assert!(c.citation.annotation.contains("affine") && c.citation.annotation.contains("#1 (600000, 3900000)"), "{}", c.citation.annotation);
    assert!(srcs.iter().any(|s| s.citation.key.starts_with("file:") && s.uses.iter().any(|u| u.layer_id == r.layer.id)), "the source's citation is kept");
}
