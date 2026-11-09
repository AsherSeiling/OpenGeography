//! Network analysis nodes of Wave 1 package T5 (Shortest Route, Service Area, Closest Facility, OD Cost Matrix) on a small network worked out by
//! hand: a square with one corner pulled in, a one-way street, speeds that make the fastest route differ from the shortest, and a separate
//! piece of road that nothing connects to.
//!
//! Corners (longitude, latitude): A (0, 0), B (0.002, 0), C (0.002, 0.002), D (0, 0.0015). Segments, as numbered by row:
//! 1 A-B (60 km/h), 2 B-C (60), 3 D-C (5 km/h, one-way, drawn D to C, field FT), 4 A-D (5), 5 E-F far away (60).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde_json::{json, Value};
use std::path::Path;

fn vlayer(e: &Engine, dir: &Path, name: &str, select: &str) -> String {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.join(format!("{name}.gpkg"));
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'l', SRS 'EPSG:4326', USE_TMP_FILE false)", gpkg.display())).unwrap();
    e.import(&gpkg).unwrap().remove(0).id
}

fn network(e: &Engine, dir: &Path) -> String {
    let l = |speed: f64, ow: &str, wkt: &str| format!("({speed:?}, CAST({ow} AS VARCHAR), ST_GeomFromText('{wkt}'))");
    let rows = [
        l(60.0, "NULL", "LINESTRING(0 0, 0.002 0)"),
        l(60.0, "NULL", "LINESTRING(0.002 0, 0.002 0.002)"),
        l(5.0, "'FT'", "LINESTRING(0 0.0015, 0.002 0.002)"),
        l(5.0, "NULL", "LINESTRING(0 0, 0 0.0015)"),
        l(60.0, "NULL", "LINESTRING(0.01 0, 0.012 0)"),
    ];
    vlayer(e, dir, "net", &format!("SELECT * FROM (VALUES {}) t(speed, ow, geom)", rows.join(",")))
}

fn stops(e: &Engine, dir: &Path, name: &str, pts: &[(&str, f64, f64)]) -> String {
    let rows: Vec<String> = pts.iter().map(|(n, x, y)| format!("('{n}', ST_GeomFromText('POINT({x:?} {y:?})'))")).collect();
    vlayer(e, dir, name, &format!("SELECT * FROM (VALUES {}) t(name, geom)", rows.join(",")))
}

fn try_tool(e: &Engine, tool: &str, params: Value, ports: &[(&str, &str)]) -> NodeResult {
    let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
    let mut edges = vec![];
    for (i, (port, layer)) in ports.iter().enumerate() {
        nodes.push(json!({"id": format!("in{i}"), "tool": "input_layer", "params": {"layer": layer}}));
        edges.push(json!({"source": format!("in{i}"), "target": "t", "targetHandle": port}));
    }
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn tool(e: &Engine, name: &str, params: Value, ports: &[(&str, &str)]) -> NodeResult {
    let r = try_tool(e, name, params, ports);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{name}: {:?} {:?}", r.status, r.error);
    r
}

/// Rows of `SELECT {cols}` as strings (NULL as "NULL").
fn rows(e: &Engine, r: &NodeResult, cols: &str, order: &str) -> Vec<Vec<String>> {
    let n = cols.split(',').count();
    let list: Vec<String> = cols.split(',').map(|c| format!("coalesce(CAST({} AS VARCHAR), 'NULL')", c.trim())).collect();
    let sql = format!("SELECT {} FROM \"{}\" ORDER BY {order}", list.join(", "), r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&sql)?;
        let v = st.query_map([], |row| (0..n).map(|k| row.get::<_, String>(k)).collect::<Result<Vec<_>, _>>())?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .unwrap()
}

fn len(a: (f64, f64), b: (f64, f64)) -> f64 {
    Geodesic::wgs84().inverse(a.1, a.0, b.1, b.0)
}

const A: (f64, f64) = (0.0, 0.0);
const B: (f64, f64) = (0.002, 0.0);
const C: (f64, f64) = (0.002, 0.002);
const D: (f64, f64) = (0.0, 0.0015);

fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("not a number: {s}"))
}

#[test]
fn t5_shortest_route_respects_one_way_and_reports_disconnected_pieces() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    // S at A, T at C, U on the separate piece.
    let s = stops(&e, dir.path(), "st", &[("S", A.0, A.1), ("T", C.0, C.1), ("U", 0.011, 0.0), ("T2", C.0, C.1)]);
    let r = tool(&e, "shortest_route", json!({}), &[("net", &net), ("stops", &s)]);
    let t = rows(&e, &r, "leg, from_stop, to_stop, cost, length_m, from_snap_m, status", "leg");
    // Leg 1, S to T by length: through D (A-D then D-C, one-way allowed that way): 165.9 + 223 = about 389 m, not through B (443.7 m).
    let want = len(A, D) + len(D, C);
    assert!((num(&t[0][3]) - want).abs() < 1e-6 && t[0][6] == "ok", "{t:?} want {want}");
    assert!(want < len(A, B) + len(B, C));
    assert!((num(&t[0][4]) - want).abs() < 1e-6);
    assert_eq!(num(&t[0][5]), 0.0);
    // Leg 2, T to U: U is on the piece nothing connects to.
    assert_eq!(t[1][6], "unreachable", "{t:?}");
    // Leg 3, U to T2: also unreachable; the rows are kept, with a message.
    assert_eq!(t[2][6], "unreachable");
    assert_eq!(t.len(), 3);
    assert!(r.message.as_deref().unwrap_or("").contains("2 connected piece"), "{:?}", r.message);
    assert!(r.message.as_deref().unwrap_or("").contains("2 of 3 legs could not be routed"), "{:?}", r.message);
    assert_eq!(r.crs.as_deref(), Some("EPSG:4326"));
    // The way back, T to S, cannot use the one-way D-C against its direction: it goes through B (443.7 m).
    let ts = stops(&e, dir.path(), "ts", &[("T", C.0, C.1), ("S", A.0, A.1)]);
    let back = rows(&e, &tool(&e, "shortest_route", json!({"oneway_field": "ow"}), &[("net", &net), ("stops", &ts)]), "cost, length_m", "leg");
    assert!((num(&back[0][0]) - (len(A, B) + len(B, C))).abs() < 1e-6, "{back:?}");
    // Travel time: with the slow 5 km/h side streets the fastest route from S to T is through B at 60 km/h = 1000 m per minute.
    let ft = tool(&e, "shortest_route", json!({"cost": "time", "speed_field": "speed", "speed_unit": "kmh", "oneway_field": "ow"}), &[("net", &net), ("stops", &stops(&e, dir.path(), "st2", &[("S", A.0, A.1), ("T", C.0, C.1)]))]);
    let row = &rows(&e, &ft, "cost, cost_unit, length_m", "leg")[0];
    assert!((num(&row[0]) - (len(A, B) + len(B, C)) / 1000.0).abs() < 1e-9 && row[1] == "min", "{row:?}");
    assert!((num(&row[2]) - (len(A, B) + len(B, C))).abs() < 1e-6);
}

#[test]
fn t5_one_way_field_is_honoured_when_given() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    // Without the one-way field, T to S can use the short way through D: length route via D. With it, it cannot.
    let ts = stops(&e, dir.path(), "ts", &[("T", C.0, C.1), ("S", A.0, A.1)]);
    let free = rows(&e, &tool(&e, "shortest_route", json!({}), &[("net", &net), ("stops", &ts)]), "cost", "leg");
    assert!((num(&free[0][0]) - (len(A, D) + len(D, C))).abs() < 1e-6, "{free:?}");
    let owd = rows(&e, &tool(&e, "shortest_route", json!({"oneway_field": "ow"}), &[("net", &net), ("stops", &ts)]), "cost", "leg");
    assert!((num(&owd[0][0]) - (len(A, B) + len(B, C))).abs() < 1e-6, "{owd:?}");
}

#[test]
fn t5_stops_snap_into_the_middle_of_a_segment_and_far_stops_are_reported() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    // P is 20 m north of the middle of A-B; Q is 22 m east of the middle of B-C; Z is far from every line.
    let p = (0.001, 20.0 / 110_574.0);
    let q = (0.002 + 22.0 / 111_319.49, 0.001);
    let s = stops(&e, dir.path(), "m", &[("P", p.0, p.1), ("Q", q.0, q.1), ("Z", 0.05, 0.05)]);
    let r = tool(&e, "shortest_route", json!({"max_snap": 500}), &[("net", &net), ("stops", &s)]);
    let t = rows(&e, &r, "leg, cost, from_snap_m, to_snap_m, status", "leg");
    // P to Q: half of A-B plus half of B-C.
    assert!((num(&t[0][1]) - (len(A, B) + len(B, C)) / 2.0).abs() < 0.05, "{t:?}");
    assert!((num(&t[0][2]) - 20.0).abs() < 0.2 && (num(&t[0][3]) - 22.0).abs() < 0.2, "{t:?}");
    assert_eq!(t[1][4], "not on the network");
    assert!(r.message.as_deref().unwrap_or("").contains("1 of 3 stops are more than 500 m from any line"), "{:?}", r.message);
}

#[test]
fn t5_od_matrix_cost_snap_and_status() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    let o = stops(&e, dir.path(), "o", &[("S", A.0, A.1), ("U", 0.011, 0.0)]);
    let d = stops(&e, dir.path(), "d", &[("B", B.0, B.1), ("T", C.0, C.1)]);
    let r = tool(&e, "od_matrix", json!({"origin_id": "name", "dest_id": "name"}), &[("net", &net), ("origins", &o), ("destinations", &d)]);
    let t = rows(&e, &r, "origin, destination, cost, length_m, status", "origin, destination");
    // Order: (S, B), (S, T), (U, B), (U, T).
    assert_eq!((t[0][0].as_str(), t[0][1].as_str()), ("S", "B"));
    assert!((num(&t[0][2]) - len(A, B)).abs() < 1e-6 && t[0][4] == "ok");
    assert!((num(&t[1][2]) - (len(A, D) + len(D, C))).abs() < 1e-6, "{t:?}");
    assert_eq!(t[2][4], "unreachable");
    assert_eq!(t[3][4], "unreachable");
    assert_eq!(t[2][2], "NULL");
    // A cutoff shorter than the way to B makes that pair unreachable too.
    let c = tool(&e, "od_matrix", json!({"origin_id": "name", "dest_id": "name", "cutoff": 100}), &[("net", &net), ("origins", &o), ("destinations", &d)]);
    assert_eq!(rows(&e, &c, "status", "origin, destination")[0][0], "unreachable");
}

#[test]
fn t5_service_area_gives_the_reachable_stretches_and_a_polygon() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    let f = stops(&e, dir.path(), "f", &[("F1", A.0, A.1)]);
    let r = tool(&e, "service_area", json!({"cutoff": 300, "facility_id": "name"}), &[("net", &net), ("facilities", &f)]);
    let t = rows(&e, &r, "edge_row, fraction_reached", "edge_row");
    // From A within 300 m: A-B in full, B-C partly, A-D in full, D-C partly (D to C only, as it is one-way).
    let (ab, bc, ad, dc) = (len(A, B), len(B, C), len(A, D), len(D, C));
    let want = [(1.0, 1.0), (2.0, (300.0 - ab) / bc), (3.0, (300.0 - ad) / dc), (4.0, 1.0)];
    assert_eq!(t.len(), 4, "{t:?}");
    for (row, (rn, frac)) in t.iter().zip(want) {
        assert_eq!(num(&row[0]), rn, "{t:?}");
        assert!((num(&row[1]) - frac).abs() < 1e-6, "{t:?} want {frac} for row {rn}");
    }
    // The far piece is not reached. A cutoff in minutes at 60 km/h reaches 1000 m per minute.
    let tm = tool(&e, "service_area", json!({"cutoff": 0.2, "cost": "time", "speed_field": "speed"}), &[("net", &net), ("facilities", &f)]);
    let tt = rows(&e, &tm, "edge_row, fraction_reached, cost_unit", "edge_row");
    assert_eq!(tt[0][0], "1"); // A-B is reached in 0.2 min = 200 m of 222.6 m
    assert!((num(&tt[0][1]) - 200.0 / ab).abs() < 1e-6 && tt[0][2] == "min", "{tt:?}");
    // The polygon variant: one polygon per facility containing the facility, with its buffer stated.
    let pg = tool(&e, "service_area", json!({"cutoff": 300, "output": "polygon", "buffer": 50}), &[("net", &net), ("facilities", &f)]);
    let sql = format!("SELECT count(*)::VARCHAR, any_value(ST_GeometryType(geom)::VARCHAR), any_value(ST_Contains(geom, ST_Point(0.0, 0.0))::VARCHAR), any_value(ST_IsValid(geom)::VARCHAR), any_value(buffer_m::VARCHAR) FROM \"{}\"", pg.output.as_ref().unwrap());
    let v: Vec<String> = e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| (0..5).map(|k| row.get::<_, String>(k)).collect::<Result<Vec<_>, _>>())?)).unwrap();
    assert_eq!(v[0], "1");
    assert!(v[1].starts_with("POLYGON") && v[2] == "true" && v[3] == "true" && v[4] == "50.0", "{v:?}");
    assert!(matches!(try_tool(&e, "service_area", json!({"cutoff": 0}), &[("net", &net), ("facilities", &f)]).status, Status::Error));
}

#[test]
fn t5_closest_facility_ranks_by_cost_and_keeps_incidents_that_reach_nothing() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    let inc = stops(&e, dir.path(), "i", &[("S", A.0, A.1), ("U", 0.011, 0.0)]);
    let fac = stops(&e, dir.path(), "f", &[("FC", C.0, C.1), ("FB", B.0, B.1)]);
    let r = tool(&e, "closest_facility", json!({"incident_id": "name", "facility_id": "name", "count": 2}), &[("net", &net), ("incidents", &inc), ("facilities", &fac)]);
    let t = rows(&e, &r, "incident, facility, rank, cost, status", "incident, rank");
    // S: nearest is B (222.6 m), then C (389 m, through D). U reaches no facility: a row with a status.
    assert_eq!(&t[0][..3], &["S", "FB", "1"]);
    assert!((num(&t[0][3]) - len(A, B)).abs() < 1e-6);
    assert_eq!(&t[1][..3], &["S", "FC", "2"]);
    assert!((num(&t[1][3]) - (len(A, D) + len(D, C))).abs() < 1e-6, "{t:?}");
    assert_eq!(t[2][0], "U");
    assert_eq!(t[2][4], "no facility reachable");
    assert!(r.message.as_deref().unwrap_or("").contains("1 of 2 incidents have no route"), "{:?}", r.message);
    // Travelling from the facility to the incident uses the one-way the other way round: from C to S it must go through B.
    let rf = tool(&e, "closest_facility", json!({"incident_id": "name", "facility_id": "name", "count": 2, "direction": "from", "oneway_field": "ow"}), &[("net", &net), ("incidents", &inc), ("facilities", &fac)]);
    let tf = rows(&e, &rf, "facility, cost", "incident, rank");
    let fc = tf.iter().find(|r| r[0] == "FC").unwrap();
    assert!((num(&fc[1]) - (len(B, C) + len(A, B))).abs() < 1e-6, "{tf:?}");
}

#[test]
fn t5_inputs_are_checked_in_words() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let net = network(&e, dir.path());
    let s = stops(&e, dir.path(), "s", &[("S", A.0, A.1), ("T", C.0, C.1)]);
    // Travel time without a speed field.
    let r = try_tool(&e, "shortest_route", json!({"cost": "time"}), &[("net", &net), ("stops", &s)]);
    assert!(matches!(r.status, Status::Error) && r.error.unwrap().contains("speed field"));
    // Points given as the network.
    let r = try_tool(&e, "shortest_route", json!({}), &[("net", &s), ("stops", &s)]);
    assert!(matches!(r.status, Status::Error) && r.error.unwrap().contains("line layer"));
    // One stop is not a route.
    let one = stops(&e, dir.path(), "one", &[("S", A.0, A.1)]);
    assert!(matches!(try_tool(&e, "shortest_route", json!({}), &[("net", &net), ("stops", &one)]).status, Status::Error));
    // Speed missing on a line: that line is reported, not silently dropped.
    let l = |speed: &str, wkt: &str| format!("({speed}, ST_GeomFromText('{wkt}'))");
    let rows_sql = [l("60.0", "LINESTRING(0 0, 0.002 0)"), l("CAST(NULL AS DOUBLE)", "LINESTRING(0.002 0, 0.002 0.002)")];
    let n2 = vlayer(&e, dir.path(), "n2", &format!("SELECT * FROM (VALUES {}) t(speed, geom)", rows_sql.join(",")));
    let r = tool(&e, "shortest_route", json!({"cost": "time", "speed_field": "speed"}), &[("net", &n2), ("stops", &stops(&e, dir.path(), "s2", &[("S", 0.0, 0.0), ("T", 0.002, 0.0)]))]);
    assert!(r.message.as_deref().unwrap_or("").contains("no speed greater than 0 on the line (row numbers: 2)"), "{:?}", r.message);
}

#[test]
fn t5_every_tool_is_registered() {
    let e = Engine::new().unwrap();
    for id in ["shortest_route", "service_area", "closest_facility", "od_matrix"] {
        let s = e.tool_specs().into_iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!(s.category, "Analysis › Network");
        assert!(s.description.contains("snap") && s.description.contains("end points"), "{id}");
        assert!(!engine::nodes::keywords::keywords(id).is_empty());
    }
}
