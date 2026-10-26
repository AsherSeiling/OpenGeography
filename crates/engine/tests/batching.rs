//! Buffer and Dissolve split big layers into pieces so Cancel can stop between
//! them (B-T4). The pieces must give the same answer as one query: these tests
//! run the tools on layers just over the batching thresholds and compare with
//! the original single-query SQL (docs/TEST_PLAN.md §Progress/cancel).

use engine::graph::{Graph, NodeResult, Status};
use engine::nodes::aeqd_for;
use engine::Engine;
use serde_json::{json, Value};

const BBOX: [f64; 4] = [-100.0, 30.0, -98.0, 32.0];

fn run(e: &Engine, layer: &str, tool: &str, params: Value) -> NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "t", "tool": tool, "params": params}],
        "edges": [{"source": "in", "target": "t", "targetHandle": "in"}]
    }))
    .unwrap();
    let r = e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap();
    assert!(matches!(r.status, Status::Done | Status::Cached), "{tool}: {:?} {:?}", r.status, r.error);
    r
}

fn cell(e: &Engine, sql: &str) -> f64 {
    e.with_conn_for_tests(|c| Ok(c.query_row(sql, [], |r| r.get::<_, f64>(0))?)).unwrap_or_else(|err| panic!("{sql}: {err}"))
}

fn columns(e: &Engine, table: &str) -> Vec<String> {
    e.with_conn_for_tests(|c| {
        let mut s = c.prepare(&format!("SELECT column_name FROM information_schema.columns WHERE table_name = '{table}' ORDER BY ordinal_position"))?;
        let v = s.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .unwrap()
}

/// Dissolve (partitioned union) equals one `ST_Union_Agg`, per group and in total.
#[test]
fn dissolve_in_tiles_equals_one_union() {
    let e = Engine::new().unwrap();
    // Over the 20 000-feature threshold.
    let layer = e.generate_test_layer("polygons", 21_000, BBOX).unwrap();
    for field in [None, Some("class")] {
        let params = field.map_or(json!({}), |f| json!({"field": f}));
        let r = run(&e, &layer.id, "dissolve", params);
        let out = r.output.clone().unwrap();
        let (lead, group) = field.map_or((String::new(), String::new()), |f| (format!("\"{f}\", "), format!(" GROUP BY \"{f}\"")));
        let reference = format!("SELECT {lead}count(*) AS COUNT, ST_Union_Agg(geom) AS geom FROM \"{}\"{group}", layer.table);
        let join = field.map_or("true".to_string(), |f| format!("a.\"{f}\" = b.\"{f}\""));
        let cols = columns(&e, &out);
        assert_eq!(cols, field.into_iter().map(String::from).chain(["COUNT".into(), "geom".into()]).collect::<Vec<_>>());
        let sql = |what: &str| format!("SELECT {what} FROM \"{out}\" a JOIN ({reference}) b ON {join}");
        assert_eq!(cell(&e, &sql("count(*)")) as usize, field.map_or(1, |_| 11));
        assert_eq!(cell(&e, &sql("sum(abs(a.COUNT - b.COUNT))")), 0.0, "COUNT per group");
        let worst = cell(&e, &sql("max(ST_Area(ST_SymDifference(a.geom, b.geom)) / ST_Area(b.geom))"));
        assert!(worst < 1e-9, "dissolved shapes differ by {worst} of their area");
        assert_eq!(cell(&e, &sql("sum(ST_NumGeometries(a.geom) - ST_NumGeometries(b.geom))")), 0.0, "same number of parts");
    }
}

/// A big geodesic Buffer (sorted by projection centre) equals one query, row for row, in the layer's row order.
#[test]
fn big_geodesic_buffer_equals_one_query() {
    let e = Engine::new().unwrap();
    // Over the 50 000-feature piece size.
    let layer = e.generate_test_layer("points", 60_000, [-100.0, 30.0, -97.0, 33.0]).unwrap();
    let c = format!("'{}'", engine::crs::resolve(engine::crs::WGS84));
    let a = aeqd_for("geom", engine::crs::WGS84);
    let buffered = format!("ST_Transform(ST_Buffer(ST_Transform(geom, {c}, {a}, true), 500, 32), {a}, {c}, true)");

    let r = run(&e, &layer.id, "buffer", json!({"distance": 500, "units": "m", "method": "geodesic"}));
    let out = r.output.clone().unwrap();
    assert_eq!(r.rows, Some(60_000));
    assert_eq!(columns(&e, &out), ["id", "class", "value", "geom"], "same columns in the same order");
    let reference = format!("SELECT id, class, value, {buffered} AS geom FROM \"{}\"", layer.table);
    let sql = |what: &str| format!("SELECT {what} FROM \"{out}\" a JOIN ({reference}) b ON a.id = b.id");
    assert_eq!(cell(&e, &sql("count(*)")), 60_000.0);
    let worst = cell(&e, &sql("max(ST_Area(ST_SymDifference(a.geom, b.geom)))"));
    assert!(worst < 1e-12, "buffer shapes differ by {worst} square degrees");
}

/// Buffer + Dissolve of a big layer: tiled union equals one union.
#[test]
fn dissolved_geodesic_buffer_in_tiles_equals_one_query() {
    let e = Engine::new().unwrap();
    let layer = e.generate_test_layer("points", 60_000, [-100.0, 30.0, -99.0, 31.0]).unwrap();
    let c = format!("'{}'", engine::crs::resolve(engine::crs::WGS84));
    let a = aeqd_for("geom", engine::crs::WGS84);
    let buffered = format!("ST_Transform(ST_Buffer(ST_Transform(geom, {c}, {a}, true), 300, 32), {a}, {c}, true)");
    let d = run(&e, &layer.id, "buffer", json!({"distance": 300, "units": "m", "method": "geodesic", "dissolve": true}));
    let dout = d.output.clone().unwrap();
    assert_eq!(columns(&e, &dout), vec!["geom".to_string()]);
    assert_eq!(cell(&e, &format!("SELECT count(*) FROM \"{dout}\"")), 1.0);
    let dref = format!("SELECT ST_Union_Agg({buffered}) AS geom FROM \"{}\"", layer.table);
    let sql = format!("SELECT ST_Area(ST_SymDifference(a.geom, b.geom)) / ST_Area(b.geom) FROM \"{dout}\" a, ({dref}) b");
    assert!(cell(&e, &sql) < 1e-9);
}
