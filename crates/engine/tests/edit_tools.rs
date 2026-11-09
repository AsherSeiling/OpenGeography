//! Shape editing E3: reshape tools, exact input (geodesic circle, distance and bearing, typed coordinates) and paste between layers.
use engine::edit::{bytes_to_hex, parse_wkb, EditOp};
use engine::edit_tools::Reshape;
use engine::Engine;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde_json::{json, Value};
use std::sync::OnceLock;

fn engine() -> (Engine, std::sync::MutexGuard<'static, ()>) {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static D: OnceLock<tempfile::TempDir> = OnceLock::new();
    let guard = ONE.lock().unwrap_or_else(|p| p.into_inner());
    std::env::set_var("GIS_STUDIO_DATA", D.get_or_init(|| tempfile::tempdir().unwrap()).path());
    (Engine::new().unwrap(), guard)
}

fn wkb(wkt: &str) -> Vec<u8> {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.query_row(&format!("SELECT ST_AsWKB(ST_GeomFromText('{wkt}'))"), [], |r| r.get::<_, Vec<u8>>(0)).unwrap()
}

fn ins(wkt: &str, name: &str) -> EditOp {
    EditOp { op: "insert".into(), geometry: Some(bytes_to_hex(&wkb(wkt))), attributes: Some(json!({"name": name}).as_object().cloned().unwrap()), ..Default::default() }
}

const G: &str = "EPSG:4326";

/// A polygon layer with the given polygons already saved.
fn layer(e: &Engine, polys: &[(&str, &str)]) -> engine::layers::LayerInfo {
    let l = e.create_layer("p", "polygon", G, &[("name".into(), "text".into())]).unwrap();
    e.edit_begin(&l.id, G, None).unwrap();
    let ops: Vec<EditOp> = polys.iter().map(|(w, n)| ins(w, n)).collect();
    e.edit_apply(&l.id, G, &ops).unwrap();
    e.edit_save(&l.id).unwrap();
    e.edit_begin(&l.id, G, None).unwrap();
    l
}

fn q<T: for<'a> engine::duckdb::types::FromSql>(e: &Engine, sql: &str) -> T {
    e.with_conn_for_tests(|c| Ok(c.query_row(sql, [], |r| r.get::<_, T>(0)).unwrap_or_else(|er| panic!("{sql}: {er}")))).unwrap()
}

fn pending_wkt(e: &Engine, l: &engine::layers::LayerInfo) -> Vec<(i64, String, Option<String>)> {
    let f = e.edit_features_in(&l.id, G, [-180.0, -85.0, 180.0, 85.0], 100).unwrap();
    f.features
        .iter()
        .map(|x| {
            let b: Vec<u8> = (0..x.geometry.as_ref().unwrap().len()).step_by(2).map(|i| u8::from_str_radix(&x.geometry.as_ref().unwrap()[i..i + 2], 16).unwrap()).collect();
            let wkt = e.with_conn_for_tests(|c| Ok(c.query_row("SELECT ST_AsText(ST_GeomFromWKB(?))", [b], |r| r.get::<_, String>(0))?)).unwrap();
            (x.fid, wkt, x.attributes.get("name").and_then(Value::as_str).map(String::from))
        })
        .collect()
}

#[test]
fn merge_explode_and_make_multipart_are_single_undo_steps() {
    let (e, _g) = engine();
    let l = layer(&e, &[("POLYGON((0 0, 1 0, 1 1, 0 1, 0 0))", "a"), ("POLYGON((1 0, 2 0, 2 1, 1 1, 1 0))", "b"), ("POLYGON((5 5, 6 5, 6 6, 5 6, 5 5))", "c")]);
    // Merge a and b: one feature with a's attributes, b is gone, and one Undo puts both back.
    let r = e.edit_reshape(&l.id, G, &Reshape::Merge { fids: vec![1, 2], keep: None }).unwrap();
    assert_eq!((r.status.changed, r.status.deleted, r.status.steps), (1, 1, 1));
    let now = pending_wkt(&e, &l);
    assert_eq!(now.len(), 2);
    assert!(now.iter().any(|(f, w, n)| *f == 1 && w.starts_with("POLYGON") && n.as_deref() == Some("a")));
    let s = e.edit_undo(&l.id).unwrap();
    assert_eq!((s.changed, s.deleted), (0, 0));
    assert!(e.edit_reshape(&l.id, G, &Reshape::Merge { fids: vec![1], keep: None }).unwrap_err().to_string().contains("2 or more"));
    // Make multipart from a and c, then explode it: three features' worth again, attributes copied to the new part.
    e.edit_reshape(&l.id, G, &Reshape::MakeMultipart { fids: vec![1, 3] }).unwrap();
    assert!(pending_wkt(&e, &l).iter().any(|(f, w, _)| *f == 1 && w.starts_with("MULTIPOLYGON")));
    let r = e.edit_reshape(&l.id, G, &Reshape::Explode { fid: 1 }).unwrap();
    assert_eq!(r.status.added, 1, "the second part is a new feature");
    let all = pending_wkt(&e, &l);
    assert_eq!(all.iter().filter(|(_, _, n)| n.as_deref() == Some("a")).count(), 2, "the new part copied the attributes");
    assert!(e.edit_reshape(&l.id, G, &Reshape::Explode { fid: 2 }).unwrap_err().to_string().contains("nothing to explode"));
    e.edit_discard(&l.id).unwrap();
}

#[test]
fn holes_are_cut_inside_a_polygon_only_and_filled_again() {
    let (e, _g) = engine();
    let l = layer(&e, &[("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))", "a")]);
    let inside = bytes_to_hex(&wkb("POLYGON((2 2, 4 2, 4 4, 2 4, 2 2))"));
    let outside = bytes_to_hex(&wkb("POLYGON((8 8, 12 8, 12 12, 8 12, 8 8))"));
    assert!(e.edit_reshape(&l.id, G, &Reshape::CutHole { fid: 1, hole: outside }).unwrap_err().to_string().contains("completely inside"));
    e.edit_reshape(&l.id, G, &Reshape::CutHole { fid: 1, hole: inside }).unwrap();
    let w = pending_wkt(&e, &l).remove(0).1;
    assert_eq!(w.matches('(').count(), 3, "one outer ring and one hole: {w}");
    e.edit_reshape(&l.id, G, &Reshape::FillHoles { fids: vec![1] }).unwrap();
    assert_eq!(pending_wkt(&e, &l).remove(0).1.matches('(').count(), 2);
    assert!(e.edit_reshape(&l.id, G, &Reshape::FillHoles { fids: vec![1] }).unwrap_err().to_string().contains("no holes"));
    e.edit_discard(&l.id).unwrap();
}

#[test]
fn split_cuts_a_polygon_by_a_line_that_crosses_it() {
    let (e, _g) = engine();
    let l = layer(&e, &[("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))", "a")]);
    let short = bytes_to_hex(&wkb("LINESTRING(5 -1, 5 5)"));
    assert!(e.edit_reshape(&l.id, G, &Reshape::Split { fid: 1, line: short }).unwrap_err().to_string().contains("all the way across"));
    let line = bytes_to_hex(&wkb("LINESTRING(5 -1, 5 11)"));
    let r = e.edit_reshape(&l.id, G, &Reshape::Split { fid: 1, line }).unwrap();
    assert_eq!((r.status.changed, r.status.added), (1, 1));
    let all = pending_wkt(&e, &l);
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|(_, _, n)| n.as_deref() == Some("a")), "both halves keep the attributes");
    e.edit_save(&l.id).unwrap();
    let t = e.layer(&l.id).unwrap().table;
    assert!((q::<f64>(&e, &format!("SELECT sum(ST_Area(geom)) FROM \"{t}\"")) - 100.0).abs() < 1e-9, "the halves add up to the original");
}

#[test]
fn a_geodesic_circle_has_its_radius_on_the_ellipsoid_and_a_rectangle_is_straight_in_the_map_crs() {
    let (e, _g) = engine();
    for map in ["EPSG:4326", "EPSG:32614"] {
        let center = if map == "EPSG:4326" { [-99.0, 40.0] } else { e.with_conn_for_tests(|c| Ok(engine::crs::transform_point(c, -99.0, 40.0, G, map)?)).map(|p| [p.0, p.1]).unwrap() };
        let c = e.edit_circle(map, center, 1000.0, 48).unwrap();
        let g = parse_wkb(&(0..c.geometry.len()).step_by(2).map(|i| u8::from_str_radix(&c.geometry[i..i + 2], 16).unwrap()).collect::<Vec<_>>()).unwrap();
        let ring = &g.parts[0][0];
        assert_eq!(ring.len(), 49);
        assert_eq!((ring[0].x, ring[0].y), (ring[48].x, ring[48].y), "closed");
        for v in ring.iter().take(48) {
            let (lon, lat) = e.with_conn_for_tests(|cn| Ok(engine::crs::transform_point(cn, v.x, v.y, map, G)?)).unwrap();
            let d: f64 = Geodesic::wgs84().inverse(40.0, -99.0, lat, lon);
            assert!((d - 1000.0).abs() < 0.05, "{map}: vertex {d} m from the centre");
        }
    }
    assert!(e.edit_circle(G, [0.0, 0.0], 0.0, 48).is_err());
    assert!(e.edit_circle(G, [0.0, 0.0], 10.0, 2).unwrap_err().to_string().contains("3 to 720"));
    let r = e.edit_rectangle([1.0, 2.0], [5.0, 4.0]).unwrap();
    assert!(r.geometry.len() > 10);
    assert!(e.edit_rectangle([1.0, 2.0], [1.0, 4.0]).is_err());
}

#[test]
fn distance_and_bearing_are_geodesic_and_typed_coordinates_become_map_positions() {
    let (e, _g) = engine();
    let p = e.edit_destination(G, [-99.0, 40.0], 1000.0, 90.0).unwrap();
    let d: f64 = Geodesic::wgs84().inverse(40.0, -99.0, p[1], p[0]);
    assert!((d - 1000.0).abs() < 0.05, "{d}");
    assert!(p[0] > -99.0 && (p[1] - 40.0).abs() < 1e-3, "due east: {p:?}");
    let north = e.edit_destination(G, [-99.0, 40.0], 111_000.0, 0.0).unwrap();
    assert!((north[1] - 41.0).abs() < 0.01, "{north:?}");
    assert!(e.edit_destination(G, [0.0, 0.0], -1.0, 0.0).is_err());

    let l = e.create_layer("utm", "point", "EPSG:32614", &[]).unwrap();
    // x/y in the layer's CRS, seen on a geographic map.
    let xy = e.edit_parse_coords(&l.id, G, "xy", "500000, 4000000\n600000 4100000").unwrap();
    assert!((xy[0][0] + 99.0).abs() < 1e-9 && (xy[0][1] - 36.1).abs() < 0.1, "{xy:?}");
    let ll = e.edit_parse_coords(&l.id, "EPSG:32614", "lonlat", "-99, 0").unwrap();
    assert!((ll[0][0] - 500000.0).abs() < 1e-6 && ll[0][1].abs() < 1e-6, "{ll:?}");
    // USNG: the Washington Monument, to within a couple of metres of its cell centre.
    let u = e.edit_parse_coords(&l.id, G, "usng", "18S UJ 23407 06483").unwrap();
    assert!((u[0][0] + 77.0353).abs() < 0.001 && (u[0][1] - 38.8895).abs() < 0.001, "{u:?}");
    let err = e.edit_parse_coords(&l.id, G, "xy", "1, 2\nnot numbers").unwrap_err().to_string();
    assert!(err.starts_with("Line 2:"), "{err}");
    assert!(e.edit_parse_coords(&l.id, G, "lonlat", "200, 0").unwrap_err().to_string().contains("±180"));
    assert!(e.edit_parse_coords(&l.id, G, "usng", "nonsense").unwrap_err().to_string().contains("Line 1"));
    assert!(e.edit_parse_coords(&l.id, G, "xy", "  \n").is_err());
}

#[test]
fn paste_moves_shapes_into_the_target_crs_matches_fields_by_name_and_reports_the_rest() {
    let (e, _g) = engine();
    let src = e.create_layer("src", "point", G, &[("kind".into(), "text".into()), ("extra".into(), "text".into())]).unwrap();
    e.edit_begin(&src.id, G, None).unwrap();
    let mk = |wkt: &str, kind: &str| EditOp { op: "insert".into(), geometry: Some(bytes_to_hex(&wkb(wkt))), attributes: Some(json!({"id": 7, "kind": kind, "extra": "x"}).as_object().cloned().unwrap()), ..Default::default() };
    e.edit_apply(&src.id, G, &[mk("POINT(-99 0)", "12"), mk("POINT(-98 1)", "oops")]).unwrap();
    e.edit_save(&src.id).unwrap();
    // The target has `KIND` as a whole number (case differs; 'oops' can't fit) and no `extra`.
    let dst = e.create_layer("dst", "point", "EPSG:32614", &[("KIND".into(), "integer".into())]).unwrap();
    e.edit_begin(&dst.id, "EPSG:32614", None).unwrap();
    let r = e.edit_paste(&src.id, &[1, 2], &dst.id).unwrap();
    assert_eq!(r.pasted, 2);
    assert_eq!(r.unmatched_fields, ["extra"]);
    assert_eq!(r.failed_values, 1);
    assert_eq!((r.apply.status.added, r.apply.status.steps), (2, 1), "one undo step");
    e.edit_save(&dst.id).unwrap();
    let t = e.layer(&dst.id).unwrap().table;
    // Exactly the engine's own transform of the source coordinates.
    let want = e.with_conn_for_tests(|c| Ok(engine::crs::transform_point(c, -99.0, 0.0, G, "EPSG:32614")?)).unwrap();
    assert!((q::<f64>(&e, &format!("SELECT ST_X(geom) FROM \"{t}\" ORDER BY __fid LIMIT 1")) - want.0).abs() < 1e-6);
    assert_eq!(q::<i64>(&e, &format!("SELECT \"KIND\" FROM \"{t}\" ORDER BY __fid LIMIT 1")), 12);
    assert_eq!(q::<i64>(&e, &format!("SELECT count(*) FROM \"{t}\" WHERE \"KIND\" IS NULL")), 1, "the value that did not fit was pasted empty");
    // Sentences for the refusals.
    assert!(e.edit_paste(&src.id, &[], &dst.id).unwrap_err().to_string().contains("Select features"));
    assert!(e.edit_paste(&src.id, &[1], &dst.id).unwrap_err().to_string().contains("not being edited"));
    let _ = EditOp::default();
}

#[test]
fn reshape_edge_replaces_the_shorter_stretch_of_edge_with_the_line() {
    let (e, _g) = engine();
    let l = layer(&e, &[("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))", "a")]);
    // A bump below the bottom edge: its ends (2, 0.3) and (8, 0.2) are off the edge and are moved onto it.
    let line = bytes_to_hex(&wkb("LINESTRING(2 0.3, 3 -3, 7 -3, 8 0.2)"));
    let r = e.edit_reshape(&l.id, G, &Reshape::ReshapeEdge { fid: 1, line }).unwrap();
    assert_eq!((r.status.changed, r.status.steps), (1, 1));
    let w = pending_wkt(&e, &l).remove(0).1;
    assert!(w.contains("3 -3") && w.contains("7 -3"), "{w}");
    // The untouched corners and the rest of the edge are exactly as they were.
    assert!(w.contains("10 0") && w.contains("10 10") && w.contains("0 10"), "{w}");
    e.edit_save(&l.id).unwrap();
    let t = e.layer(&l.id).unwrap().table;
    let area: f64 = q(&e, &format!("SELECT ST_Area(geom) FROM \"{t}\""));
    // The bump is a trapezoid of 15 square units (bases 6 and 4, height 3) on the 100-unit square.
    assert!((area - 115.0).abs() < 1e-6, "bulged outward by the drawn area: {area}");
    assert!(q::<bool>(&e, &format!("SELECT ST_IsValid(geom) FROM \"{t}\"")));
    // A stretch that runs through the ring's start point (the corner at 0 0) is replaced too: cutting that corner off leaves 99.
    let corner = bytes_to_hex(&wkb("LINESTRING(1 0, 1 1, 0 1)"));
    e.edit_begin(&l.id, G, None).unwrap();
    e.edit_reshape(&l.id, G, &Reshape::ReshapeEdge { fid: 1, line: corner }).unwrap();
    e.edit_save(&l.id).unwrap();
    let a2: f64 = q(&e, &format!("SELECT ST_Area(geom) FROM \"{t}\""));
    assert!((a2 - 114.0).abs() < 1e-6, "corner cut off the bumped square: {a2}");
    // A line that comes back to the same place is refused with a sentence; a one-point line too.
    e.edit_begin(&l.id, G, None).unwrap();
    assert!(e.edit_reshape(&l.id, G, &Reshape::ReshapeEdge { fid: 1, line: bytes_to_hex(&wkb("LINESTRING(5 0, 5 -2, 5 0)")) }).unwrap_err().to_string().contains("same place"));
    assert!(e.edit_reshape(&l.id, G, &Reshape::ReshapeEdge { fid: 1, line: bytes_to_hex(&wkb("POINT(1 1)")) }).unwrap_err().to_string().contains("at least two"));
    e.edit_discard(&l.id).unwrap();
}

/// A UTM 14N elevation raster: 20 × 20 cells of 100 m from (500000, 4000000); the cell in column c holds c + 0.5, except one NoData cell.
fn dem_file(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("dem.tif");
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(&path, 20, 20, 1).unwrap();
    ds.set_geo_transform(&[500000.0, 100.0, 0.0, 4000000.0, 0.0, -100.0]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614).unwrap()).unwrap();
    let data: Vec<f32> = (0..400).map(|i| if i == 5 * 20 + 5 { -9999.0 } else { (i % 20) as f32 + 0.5 }).collect();
    let mut band = ds.rasterband(1).unwrap();
    band.set_no_data_value(Some(-9999.0)).unwrap();
    band.write((0, 0), (20, 20), &mut gdal::raster::Buffer::new((20, 20), data)).unwrap();
    path
}

fn utm_to_ll(e: &Engine, x: f64, y: f64) -> (f64, f64) {
    e.with_conn_for_tests(|c| {
        Ok(c.query_row("SELECT ST_X(g), ST_Y(g) FROM (SELECT ST_Transform(ST_Point(?, ?), 'EPSG:32614', 'EPSG:4326', true) AS g)", [x, y], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)))?)
    })
    .unwrap()
}

fn tri(e: &Engine, p: [(f64, f64); 3]) -> String {
    let v: Vec<(f64, f64)> = p.iter().map(|&(x, y)| utm_to_ll(e, x, y)).collect();
    format!("POLYGON(({} {}, {} {}, {} {}, {} {}))", v[0].0, v[0].1, v[1].0, v[1].1, v[2].0, v[2].1, v[0].0, v[0].1)
}

fn zs(e: &Engine, l: &engine::layers::LayerInfo, fid: i64) -> Vec<f64> {
    let f = e.edit_features_in(&l.id, G, [-180.0, -85.0, 180.0, 85.0], 100).unwrap();
    let g = f.features.iter().find(|x| x.fid == fid).unwrap().geometry.clone().unwrap();
    let b: Vec<u8> = (0..g.len()).step_by(2).map(|i| u8::from_str_radix(&g[i..i + 2], 16).unwrap()).collect();
    parse_wkb(&b).unwrap().coords().map(|c| c.z).collect()
}

#[test]
fn new_vertices_take_z_from_a_dem_in_another_crs_and_missing_ones_are_reported_not_filled() {
    let (e, _g) = engine();
    let dir = tempfile::tempdir().unwrap();
    let dem = e.import_raster(&dem_file(dir.path()), Some("dem")).unwrap();
    let l = e.create_layer("zp", "polygon", G, &[("name".into(), "text".into())]).unwrap();
    e.edit_begin(&l.id, G, None).unwrap();
    // Refused: a layer that is not a raster.
    assert!(e.edit_set_z(&l.id, None, Some(&l.id)).unwrap_err().to_string().contains("not a raster"));
    let st = e.edit_set_z(&l.id, Some(5.0), Some(&dem.id)).unwrap();
    assert_eq!((st.z_dem.as_deref(), st.z_value), (Some(dem.id.as_str()), None), "the raster wins over a fixed value, and the status says so");
    // All three vertices inside the raster: Z is the value of the cell each falls in (column index + 0.5), looked up in the raster's own CRS.
    let a = tri(&e, [(500250.0, 3999750.0), (501650.0, 3999750.0), (501650.0, 3998350.0)]);
    let r = e.edit_apply(&l.id, G, &[ins(&a, "a")]).unwrap();
    assert!(r.z_note.is_none(), "{:?}", r.z_note);
    assert_eq!(zs(&e, &l, r.fids[0]), vec![2.5, 16.5, 16.5, 2.5]);
    // One vertex outside the raster and one on the NoData cell: reported, and left without Z.
    let b = tri(&e, [(503000.0, 3999750.0), (500550.0, 3999450.0), (501250.0, 3998350.0)]);
    let r = e.edit_apply(&l.id, G, &[ins(&b, "b")]).unwrap();
    let note = r.z_note.expect("missing Z is reported");
    assert!(note.contains("2 new vertices") && note.contains("1 outside \"dem\"") && note.contains("1 on NoData"), "{note}");
    let z = zs(&e, &l, r.fids[0]);
    assert!(z[0].is_nan() && z[1].is_nan() && z[2] == 12.5 && z[3].is_nan(), "no value was made up: {z:?}");
    // Adding a vertex to b later does not count its stored vertices again: only the new one is looked up.
    let p = [(503000.0, 3999750.0), (500550.0, 3999450.0), (501250.0, 3998350.0), (501850.0, 3998950.0)].map(|(x, y)| utm_to_ll(&e, x, y));
    let w = format!("POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))", p[0].0, p[0].1, p[1].0, p[1].1, p[2].0, p[2].1, p[3].0, p[3].1, p[0].0, p[0].1);
    let r2 = e.edit_apply(&l.id, G, &[EditOp { op: "update".into(), fid: Some(r.fids[0]), geometry: Some(bytes_to_hex(&wkb(&w))), ..Default::default() }]).unwrap();
    assert!(r2.z_note.is_none(), "{:?}", r2.z_note);
    assert_eq!(zs(&e, &l, r.fids[0]).iter().filter(|v| v.is_finite()).count(), 2);
    // Back to a fixed value: new vertices get it; the raster is no longer used.
    let st = e.edit_set_z(&l.id, Some(7.0), None).unwrap();
    assert_eq!((st.z_dem, st.z_value), (None, Some(7.0)));
    let c = e.edit_apply(&l.id, G, &[ins("POLYGON((1 1, 2 1, 2 2, 1 1))", "c")]).unwrap();
    assert_eq!(zs(&e, &l, c.fids[0]), vec![7.0; 4]);
    e.edit_discard(&l.id).unwrap();
}
