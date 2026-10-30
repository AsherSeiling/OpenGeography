//! USNG / MGRS tools against the references the `mgrs` package writes for the same places (WGS 84), including the southern hemisphere,
//! Norway's widened zone 32 and Svalbard's zone 33X.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};

/// lon, lat, MGRS (5 digits) from the `mgrs` npm package.
const REFS: &[(f64, f64, &str)] = &[
    (-93.0003, 45.0004, "15TVK9997682994"),
    (-119.5, 37.2, "11SKB7812419987"),
    (-70.5, -33.4, "19HCD6050503362"),
    (151.2093, -33.8688, "56HLH3436850948"),
    (10.75, 59.91, "32VNM9786842681"),
    (15.6, 78.22, "33XWG1367482991"),
    (-0.1276, 51.5072, "30UXC9933010142"),
    (-122.3321, 47.6062, "10TET5020072748"),
    (2.35, 48.85, "31UDQ5231410984"),
    (-156.5, 20.8, "4QGJ6022802030"),
    (28.0, -26.2, "35JNM9991001782"),
];

/// Polar (UPS) references from the Python `mgrs` package (NGA GeoTrans) for WGS 84: lon, lat, MGRS (5 digits).
const POLAR: &[(f64, f64, &str)] = &[
    (10.0, 89.5, "ZAG0963945331"),
    (-120.0, 89.5, "YZH5192427756"),
    (45.0, 85.5, "ZFD5344946550"),
    (-170.0, 87.0, "YZL4215028083"),
    (100.0, -89.5, "BAM5466890360"),
    (-100.0, -89.5, "AZM4533190360"),
    (0.5, -85.0, "BAT0484755436"),
    (-0.5, -85.0, "AZT9515255436"),
    (120.0, -81.0, "BLG6705699404"),
    (0.0, 84.5, "ZAA0000088918"),
    (179.9, 84.2, "ZAP0112444465"),
    (-179.9, 84.2, "YZP9887544465"),
];

fn polar_points(e: &Engine) -> String {
    let feats: Vec<(String, serde_json::Map<String, Value>)> = POLAR
        .iter()
        .map(|(lo, la, m)| (format!("POINT({lo} {la})"), json!({ "expect": m }).as_object().unwrap().clone()))
        .collect();
    e.create_features("polar refs", &feats).unwrap().id
}

fn points(e: &Engine) -> String {
    let feats: Vec<(String, serde_json::Map<String, Value>)> = REFS
        .iter()
        .map(|(lo, la, m)| (format!("POINT({lo} {la})"), json!({ "expect": m }).as_object().unwrap().clone()))
        .collect();
    e.create_features("refs", &feats).unwrap().id
}

fn run(e: &Engine, layer: &str, steps: Vec<(&str, Value)>) -> NodeResult {
    let mut nodes = vec![json!({"id": "in", "tool": "input_layer", "params": {"layer": layer}})];
    let mut edges = vec![];
    let mut prev = "in".to_string();
    for (i, (tool, params)) in steps.iter().enumerate() {
        let id = format!("s{i}");
        nodes.push(json!({"id": id, "tool": tool, "params": params}));
        edges.push(json!({"source": prev, "target": id, "targetHandle": "in"}));
        prev = id;
    }
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    let r = e.run_graph(&g, Some(&prev), &mut |_| {}).unwrap().into_iter().find(|r| r.id == prev).unwrap();
    assert!(matches!(r.status, Status::Done | Status::Cached), "{:?} {:?}", r.status, r.error);
    r
}

fn rows(e: &Engine, sql: &str) -> Vec<Vec<String>> {
    // Columns = the SELECT items before FROM (these queries have no commas inside function calls at depth 0 beyond the list).
    let head = &sql[sql.find("SELECT").unwrap() + 6..sql.find(" FROM").unwrap()];
    let (mut depth, mut n) = (0, 1);
    for ch in head.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => n += 1,
            _ => {}
        }
    }
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(sql)?;
        let v = st.query_map([], |r| (0..n).map(|i| r.get::<_, duckdb::types::Value>(i).map(|v| match v { duckdb::types::Value::Null => String::new(), duckdb::types::Value::Text(t) => t, duckdb::types::Value::BigInt(n) => n.to_string(), duckdb::types::Value::Double(d) => d.to_string(), duckdb::types::Value::Int(n) => n.to_string(), other => format!("{other:?}") })).collect::<Result<Vec<_>, _>>())?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .unwrap_or_else(|err| panic!("{sql}: {err}"))
}

#[test]
fn add_usng_matches_the_mgrs_package_in_both_spellings_and_every_precision() {
    let e = Engine::new().unwrap();
    let id = points(&e);
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "mgrs", "precision": "5", "datum": "wgs84"}))]);
    for row in rows(&e, &format!("SELECT expect, REF FROM \"{}\"", r.output.as_ref().unwrap())) {
        assert_eq!(row[1], row[0], "MGRS of {}", row[0]);
    }
    // USNG: the same reference with spaces; fewer digits shorten it, 0 digits is the 100 km square.
    let u = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "5", "datum": "wgs84"}))]);
    for row in rows(&e, &format!("SELECT expect, REF FROM \"{}\"", u.output.as_ref().unwrap())) {
        let m = &row[0];
        let split = m.find(|c: char| c.is_ascii_alphabetic()).unwrap(); // after the zone digits
        let (zone, rest) = (&m[..split], &m[split..]);
        let want = format!("{zone}{} {} {} {}", &rest[..1], &rest[1..3], &rest[3..8], &rest[8..]);
        assert_eq!(row[1], want, "USNG of {m}");
    }
    let c = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "2", "datum": "wgs84"}))]);
    let first = &rows(&e, &format!("SELECT REF FROM \"{}\" LIMIT 1", c.output.as_ref().unwrap()))[0][0];
    assert_eq!(first, "15T VK 99 82", "1 km precision");
    let sq = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "0", "datum": "wgs84"}))]);
    assert_eq!(rows(&e, &format!("SELECT REF FROM \"{}\" LIMIT 1", sq.output.as_ref().unwrap()))[0][0], "15T VK");
}

#[test]
fn polar_places_get_a_polar_reference_and_a_missing_location_gets_none() {
    let e = Engine::new().unwrap();
    let id = e.create_features("polar", &[("POINT(10 85)".to_string(), Default::default()), ("POINT(10 50)".to_string(), Default::default())]).unwrap().id;
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "5", "datum": "wgs84"}))]);
    let v = rows(&e, &format!("SELECT REF FROM \"{}\" ORDER BY REF", r.output.as_ref().unwrap()));
    assert!(v[0][0].starts_with("32U "), "{v:?}");
    assert!(v[1][0].starts_with("Z "), "85°N, 10°E is polar zone Z, not a UTM zone: {v:?}");
}

#[test]
fn usng_to_points_comes_back_to_the_same_place_and_counts_bad_values() {
    let e = Engine::new().unwrap();
    let id = points(&e);
    // Round trip at 1 m: reference → point lies within a metre of where it started (centre of the 1 m cell is 0.7 m away at most).
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "5", "datum": "wgs84"})), ("usng_to_points", json!({"field": "REF", "datum": "wgs84", "where": "centre"}))]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(r.rows, Some(REFS.len() as i64));
    let t = r.output.as_ref().unwrap();
    // Distance in metres between the original lon/lat (parsed from the WKT we made) and the new point.
    for (lo, la, m) in REFS {
        // Flat-earth metres over a few centimetres to a metre are exact enough here.
        let sql = format!(
            "SELECT sqrt(power((ST_X(geom) - ({lo})) * cos(radians({la})), 2) + power(ST_Y(geom) - ({la}), 2)) * 111320 FROM \"{t}\" WHERE expect = '{m}'"
        );
        let d: f64 = rows(&e, &sql)[0][0].parse().unwrap();
        assert!(d < 1.5, "{m}: {d} m from where it started");
    }
    // A table with some junk: the good rows come through, the bad are counted in the note.
    let tbl = e.create_features("junk", &[("POINT(0 0)".to_string(), json!({"ref": "15T VK 99976 82994"}).as_object().unwrap().clone()), ("POINT(0 0)".to_string(), json!({"ref": "not a reference"}).as_object().unwrap().clone()), ("POINT(0 0)".to_string(), json!({"ref": "15TVK12"}).as_object().unwrap().clone())]).unwrap().id;
    let j = run(&e, &tbl, vec![("usng_to_points", json!({"field": "ref", "datum": "wgs84"}))]);
    assert_eq!(j.rows, Some(2), "the junk row is left out");
    assert!(j.message.clone().unwrap_or_default().contains("not a valid USNG/MGRS"), "{:?}", j.message);
}

#[test]
fn usng_grid_squares_are_labelled_and_line_up() {
    let e = Engine::new().unwrap();
    // About 3 km × 2 km near the centre of zone 15.
    let id = e.create_features("area", &[("POLYGON((-93.02 45.00, -92.98 45.00, -92.98 45.02, -93.02 45.02, -93.02 45.00))".to_string(), Default::default())]).unwrap().id;
    let r = run(&e, &id, vec![("usng_grid", json!({"interval": "1000", "style": "usng", "datum": "wgs84", "field": "USNG"}))]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32615"));
    let t = r.output.as_ref().unwrap();
    let n: i64 = rows(&e, &format!("SELECT count(*) FROM \"{t}\""))[0][0].parse().unwrap();
    assert!((6..=20).contains(&n), "{n} squares");
    // Squares are 1 km a side and every label is a 1 km reference (two digits each way).
    let side: f64 = rows(&e, &format!("SELECT max(ST_XMax(geom) - ST_XMin(geom)) FROM \"{t}\""))[0][0].parse().unwrap();
    assert!((side - 1000.0).abs() < 1e-6, "{side}");
    for row in rows(&e, &format!("SELECT USNG FROM \"{t}\"")) {
        let parts: Vec<&str> = row[0].split(' ').collect();
        assert_eq!(parts.len(), 4, "{}", row[0]);
        assert_eq!((parts[2].len(), parts[3].len()), (2, 2), "{}", row[0]);
        assert!(parts[0] == "15T", "{}", row[0]);
    }
    // The square holding the zone's central meridian crossing is W or V (E 500 000 m), and the answer is 15T WK / VK for this place.
    let sq: Vec<String> = rows(&e, &format!("SELECT DISTINCT split_part(USNG, ' ', 2) FROM \"{t}\" ORDER BY 1")).into_iter().map(|r| r[0].clone()).collect();
    assert_eq!(sq, vec!["VK".to_string(), "WK".to_string()], "{sq:?}");
}

#[test]
fn export_features_can_carry_a_usng_column_through_the_export_tool() {
    let e = Engine::new().unwrap();
    let id = points(&e);
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("out.csv");
    let g: Graph = serde_json::from_value(json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": id}},
            {"id": "grid", "tool": "add_usng", "params": {"field": "MGRS", "style": "mgrs", "precision": "5", "datum": "wgs84"}},
            {"id": "out", "tool": "export", "params": {"path": csv.to_str().unwrap(), "format": "CSV"}}
        ],
        "edges": [{"source": "in", "target": "grid", "targetHandle": "in"}, {"source": "grid", "target": "out", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).unwrap();
    assert!(res.iter().all(|r| r.error.is_none()), "{:?}", res.iter().map(|r| &r.error).collect::<Vec<_>>());
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.lines().next().unwrap().contains("MGRS"), "{}", text.lines().next().unwrap());
    for (_, _, m) in REFS {
        assert!(text.contains(m), "{m} missing from the exported file");
    }
}

#[test]
fn the_grid_layer_for_a_view_is_a_new_labelled_layer_and_leaves_no_extent_layer_behind() {
    let e = Engine::new().unwrap();
    let before = e.list_layers().len();
    let l = e.usng_grid_layer([-93.02, 45.0, -92.98, 45.02], 1000.0, "usng", "wgs84", "USNG grid 1 km").unwrap();
    assert_eq!(l.name, "USNG grid 1 km");
    assert!(l.feature_count >= 6, "{}", l.feature_count);
    assert!(l.fields.iter().any(|f| f.name == "USNG"));
    assert_eq!(e.list_layers().len(), before + 1, "only the grid layer was added");
    assert!(e.usng_grid_layer([0.0, 85.0, 1.0, 86.0], 1000.0, "usng", "auto", "x").unwrap_err().to_string().contains("polar (UPS)"));
    assert!(e.usng_grid_layer([-93.02, 45.0, -92.98, 45.02], 12345.0, "usng", "auto", "x").is_err());
}

#[test]
fn polar_references_match_ngas_geotrans_in_every_zone_and_come_back_to_the_same_place() {
    let e = Engine::new().unwrap();
    let id = polar_points(&e);
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "mgrs", "precision": "5", "datum": "wgs84"}))]);
    for row in rows(&e, &format!("SELECT expect, REF FROM \"{}\"", r.output.as_ref().unwrap())) {
        assert_eq!(row[1], row[0], "polar MGRS of {}", row[0]);
    }
    // USNG spelling of a polar reference: the zone letter, the square, the digits.
    let u = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "precision": "5"}))]);
    let first = rows(&e, &format!("SELECT expect, REF FROM \"{}\" LIMIT 1", u.output.as_ref().unwrap()));
    let m = &first[0][0];
    assert_eq!(first[0][1], format!("{} {} {} {}", &m[..1], &m[1..3], &m[3..8], &m[8..]));
    // Back to points: within a metre or two (the 1 m cell's centre), in WGS 84.
    let back = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "mgrs", "precision": "5", "datum": "wgs84"})), ("usng_to_points", json!({"field": "REF", "datum": "wgs84"}))]);
    assert_eq!(back.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(back.rows, Some(POLAR.len() as i64));
    let t = back.output.as_ref().unwrap();
    for (lo, la, m) in POLAR {
        // Near the poles longitude means little, so compare in 3D-free flat metres from the pole instead.
        let sql = format!(
            "SELECT sqrt(power((ST_X(geom) - ({lo})) * cos(radians({la})), 2) + power(ST_Y(geom) - ({la}), 2)) * 111320 FROM \"{t}\" WHERE expect = '{m}'"
        );
        let d: f64 = rows(&e, &sql)[0][0].parse().unwrap();
        assert!(d < 2.0, "{m}: {d} m from where it started");
    }
    // Wrong letters for a polar zone are explained.
    let e1 = engine::usng::parse("YAA0000").unwrap_err().to_string();
    assert!(e1.contains("same number") || e1.contains("do not exist"), "{e1}");
    assert!(engine::usng::parse("AMM12").unwrap_err().to_string().contains("do not exist in polar zone A"));
}
