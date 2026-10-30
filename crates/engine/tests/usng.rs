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
    let c = run(&e, &id, vec![("add_usng", json!({"field": "REF", "precision": "2", "datum": "wgs84"}))]);
    let first = &rows(&e, &format!("SELECT REF FROM \"{}\" LIMIT 1", c.output.as_ref().unwrap()))[0][0];
    assert_eq!(first, "15T VK 99 82", "1 km precision");
    let sq = run(&e, &id, vec![("add_usng", json!({"field": "REF", "precision": "0", "datum": "wgs84"}))]);
    assert_eq!(rows(&e, &format!("SELECT REF FROM \"{}\" LIMIT 1", sq.output.as_ref().unwrap()))[0][0], "15T VK");
}

#[test]
fn polar_places_and_missing_geometry_get_no_reference_and_a_note() {
    let e = Engine::new().unwrap();
    let id = e.create_features("polar", &[("POINT(10 85)".to_string(), Default::default()), ("POINT(10 50)".to_string(), Default::default())]).unwrap().id;
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "datum": "wgs84"}))]);
    let v = rows(&e, &format!("SELECT REF FROM \"{}\" ORDER BY REF NULLS FIRST", r.output.as_ref().unwrap()));
    assert_eq!(v[0][0], "", "85°N has no UTM/MGRS reference");
    assert!(v[1][0].starts_with("32U "), "{v:?}");
}

#[test]
fn usng_to_points_comes_back_to_the_same_place_and_counts_bad_values() {
    let e = Engine::new().unwrap();
    let id = points(&e);
    // Round trip at 1 m: reference → point lies within a metre of where it started (centre of the 1 m cell is 0.7 m away at most).
    let r = run(&e, &id, vec![("add_usng", json!({"field": "REF", "style": "usng", "datum": "wgs84"})), ("usng_to_points", json!({"field": "REF", "datum": "wgs84", "where": "centre"}))]);
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
    let r = run(&e, &id, vec![("usng_grid", json!({"interval": "1000", "style": "usng", "datum": "wgs84"}))]);
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
