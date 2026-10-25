//! Model tools (`nodes/*.rs`) on small inline fixtures: output schema, a known
//! result, CRS preservation and the refusal paths (see docs/TEST_PLAN.md §Tools).
//!
//! Fixtures live in UTM 14N (EPSG:32614, metres) around (600000, 3900000):
//!   polys  id1 'a' pop 10  [0,0]-[100,100]      mask [50,0]-[150,100]
//!          id2 'b' pop 20  [100,0]-[200,100]
//!          id3 'a' pop 30  [0,100]-[100,200]
//!   pts    p1 (50,50) val 1 · p2 (150,50) val 2 · p3 (1000,1000) val 3
//!   lines  l1 (0,50)-(200,50) · l2 (50,-100)-(50,300)

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

const X0: f64 = 600_000.0;
const Y0: f64 = 3_900_000.0;
const UTM: &str = "EPSG:32614";

fn write_fixture(path: &Path, select: &str, srs: Option<&str>) {
    let driver = if path.extension().is_some_and(|e| e == "shp") { "ESRI Shapefile" } else { "GPKG" };
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let srs = srs.map(|s| format!(", SRS '{s}'")).unwrap_or_default();
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER '{driver}'{srs}, USE_TMP_FILE false)", path.display())).unwrap();
}

fn env(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    format!("ST_MakeEnvelope({}, {}, {}, {})", X0 + x0, Y0 + y0, X0 + x1, Y0 + y1)
}

struct W {
    e: Engine,
    dir: tempfile::TempDir,
    polys: String,
    mask: String,
    pts: String,
    lines: String,
}

impl W {
    fn add(&self, name: &str, select: &str, srs: Option<&str>) -> String {
        let p = self.dir.path().join(format!("{name}.{}", if srs.is_some() { "gpkg" } else { "shp" }));
        write_fixture(&p, select, srs);
        self.e.import(&p).unwrap().remove(0).id
    }
}

fn world() -> W {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let mut w = W { e, dir, polys: String::new(), mask: String::new(), pts: String::new(), lines: String::new() };
    w.polys = w.add(
        "polys",
        &format!(
            "SELECT 1 AS id, 'a' AS name, 10 AS pop, {} AS geom UNION ALL SELECT 2, 'b', 20, {} UNION ALL SELECT 3, 'a', 30, {}",
            env(0.0, 0.0, 100.0, 100.0),
            env(100.0, 0.0, 200.0, 100.0),
            env(0.0, 100.0, 100.0, 200.0)
        ),
        Some(UTM),
    );
    w.mask = w.add("mask", &format!("SELECT 9 AS mid, {} AS geom", env(50.0, 0.0, 150.0, 100.0)), Some(UTM));
    w.pts = w.add(
        "pts",
        &format!(
            "SELECT 1 AS id, 'p1' AS name, 1.0 AS val, ST_Point({a}, {b}) AS geom UNION ALL SELECT 2, 'p2', 2.0, ST_Point({c}, {b}) UNION ALL SELECT 3, 'p3', 3.0, ST_Point({d}, {d2})",
            a = X0 + 50.0, b = Y0 + 50.0, c = X0 + 150.0, d = X0 + 1000.0, d2 = Y0 + 1000.0
        ),
        Some(UTM),
    );
    w.lines = w.add(
        "lines",
        &format!(
            "SELECT 1 AS id, ST_GeomFromText('LINESTRING({a} {b}, {c} {b})') AS geom UNION ALL SELECT 2, ST_GeomFromText('LINESTRING({d} {e}, {d} {f})')",
            a = X0, b = Y0 + 50.0, c = X0 + 200.0, d = X0 + 50.0, e = Y0 - 100.0, f = Y0 + 300.0
        ),
        Some(UTM),
    );
    w
}

fn graph(v: Value) -> Graph {
    serde_json::from_value(v).unwrap()
}

/// Run `tool` with `inputs` wired to its ports in order; any status comes back.
fn run_raw(w: &W, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let spec = w.e.tool_specs().into_iter().find(|s| s.id == tool).unwrap_or_else(|| panic!("no tool {tool}"));
    let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
    let mut edges = vec![];
    for (i, l) in inputs.iter().enumerate() {
        nodes.push(json!({"id": format!("in{i}"), "tool": "input_layer", "params": {"layer": l}}));
        edges.push(json!({"source": format!("in{i}"), "target": "t", "targetHandle": spec.inputs[i].id}));
    }
    let res = w.e.run_graph(&graph(json!({"nodes": nodes, "edges": edges})), Some("t"), &mut |_| {}).unwrap();
    res.into_iter().find(|r| r.id == "t").unwrap()
}

fn run(w: &W, tool: &str, params: Value, inputs: &[&str]) -> NodeResult {
    let r = run_raw(w, tool, params, inputs);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{tool}: {:?} {:?}", r.status, r.error);
    r
}

fn table(r: &NodeResult) -> String {
    format!("\"{}\"", r.output.as_ref().expect("node has an output table"))
}

/// First column of the first row of `sql`; `{t}` stands for the node's output table.
fn qf(w: &W, r: &NodeResult, sql: &str) -> f64 {
    let sql = sql.replace("{t}", &table(r));
    w.e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| row.get::<_, f64>(0))?)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

fn qs(w: &W, r: &NodeResult, sql: &str) -> String {
    let sql = sql.replace("{t}", &table(r));
    w.e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| row.get::<_, String>(0))?)).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Output field names, without GeoPackage's `fid` row id that file imports carry.
fn fields(w: &W, r: &NodeResult) -> Vec<String> {
    w.e.preview_output(r.output.as_ref().unwrap(), r.crs.clone(), "check").unwrap().fields.iter().map(|f| f.name.clone()).filter(|n| n != "fid").collect()
}

fn close(a: f64, b: f64, rel: f64) {
    assert!((a - b).abs() <= b.abs() * rel + 1e-9, "{a} vs {b} (±{}%)", rel * 100.0);
}

// ---------------------------------------------------------------- overlay

#[test]
fn clip_erase_intersect_known_areas_and_crs() {
    let w = world();
    let (p, m) = (w.polys.as_str(), w.mask.as_str());
    let clip = run(&w, "clip", json!({}), &[p, m]);
    assert_eq!((clip.rows, clip.crs.as_deref()), (Some(2), Some(UTM)));
    close(qf(&w, &clip, "SELECT sum(ST_Area(geom)) FROM {t}"), 10_000.0, 1e-6);
    assert_eq!(fields(&w, &clip), ["id", "name", "pop"], "clip keeps the input's attributes only");

    let erase = run(&w, "erase", json!({}), &[p, m]);
    assert_eq!((erase.rows, erase.crs.as_deref()), (Some(3), Some(UTM)));
    close(qf(&w, &erase, "SELECT sum(ST_Area(geom)) FROM {t}"), 20_000.0, 1e-6);

    let x = run(&w, "intersect", json!({}), &[p, m]);
    assert_eq!(x.rows, Some(2));
    close(qf(&w, &x, "SELECT sum(ST_Area(geom)) FROM {t}"), 10_000.0, 1e-6);
    let f = fields(&w, &x);
    assert!(f.contains(&"pop".to_string()) && f.contains(&"mid".to_string()), "intersect carries both layers' fields: {f:?}");
}

#[test]
fn union_and_symmetrical_difference_areas() {
    let w = world();
    let (p, m) = (w.polys.as_str(), w.mask.as_str());
    // The mask lies wholly inside polys 1+2, so the union adds no area...
    let u = run(&w, "union", json!({}), &[p, m]);
    close(qf(&w, &u, "SELECT sum(ST_Area(geom)) FROM {t}"), 30_000.0, 1e-6);
    assert_eq!(u.crs.as_deref(), Some(UTM));
    // ...and the symmetrical difference is polys minus the mask.
    let s = run(&w, "symmetrical_difference", json!({}), &[p, m]);
    close(qf(&w, &s, "SELECT sum(ST_Area(geom)) FROM {t}"), 20_000.0, 1e-6);
}

#[test]
fn line_intersections_finds_the_crossing() {
    let w = world();
    let r = run(&w, "line_intersections", json!({}), &[&w.lines, &w.lines]);
    // l1×l2 and l2×l1; the collinear self-overlaps give no point.
    assert_eq!(r.rows, Some(2));
    assert_eq!(qs(&w, &r, "SELECT DISTINCT ST_AsText(geom) FROM {t}"), format!("POINT ({} {})", X0 + 50.0, Y0 + 50.0));
    assert_eq!(r.crs.as_deref(), Some(UTM));
}

// ---------------------------------------------------------------- proximity

#[test]
fn buffer_planar_dissolve_and_geodesic_areas() {
    let w = world();
    let b = run(&w, "buffer", json!({"distance": 10, "units": "m", "method": "planar"}), &[&w.pts]);
    assert_eq!((b.rows, b.crs.as_deref()), (Some(3), Some(UTM)));
    // 8 segments per quarter circle: 3.1214 r² ≈ 312 m².
    close(qf(&w, &b, "SELECT min(ST_Area(geom)) FROM {t}"), 312.14, 0.02);
    assert_eq!(fields(&w, &b), ["id", "name", "val"]);
    let d = run(&w, "buffer", json!({"distance": 10, "units": "m", "method": "planar", "dissolve": true}), &[&w.pts]);
    assert_eq!(d.rows, Some(1));
    assert!(fields(&w, &d).is_empty(), "a dissolved buffer has no attributes");
    let g = run(&w, "buffer", json!({"distance": 100, "units": "m"}), &[&w.pts]);
    close(qf(&w, &g, "SELECT min(ST_Area(geom)) FROM {t}"), 31_214.0, 0.03);
    // ft and US survey ft differ by 2 ppm: both must be converted, not treated as metres.
    let ft = run(&w, "buffer", json!({"distance": 100, "units": "usft", "method": "planar"}), &[&w.pts]);
    close(qf(&w, &ft, "SELECT min(ST_Area(geom)) FROM {t}"), 31_214.0 * 0.3048006096_f64.powi(2), 0.02);
}

#[test]
fn near_and_join_nearest_report_distance_and_respect_radius() {
    let w = world();
    let n = run(&w, "near", json!({"radius": 2, "units": "km"}), &[&w.pts, &w.polys]);
    assert_eq!((n.rows, n.crs.as_deref()), (Some(3), Some(UTM)));
    assert_eq!(fields(&w, &n), ["id", "name", "val", "NEAR_DIST", "NEAR_ROW"]);
    assert_eq!(qf(&w, &n, "SELECT NEAR_DIST FROM {t} WHERE id = 1"), 0.0);
    // p3 is √(900²+800²) ≈ 1204 m from the nearest polygon corner (geodesic, so ±0.5%);
    // NEAR_DIST is in the units chosen for the radius (km here).
    close(qf(&w, &n, "SELECT NEAR_DIST FROM {t} WHERE id = 3"), 1.2042, 0.005);
    let metres = run(&w, "near", json!({"radius": 2000, "units": "m"}), &[&w.pts, &w.polys]);
    close(qf(&w, &metres, "SELECT NEAR_DIST FROM {t} WHERE id = 3"), 1204.2, 0.005);
    let none = run(&w, "near", json!({"radius": 0.5, "units": "km"}), &[&w.pts, &w.polys]);
    assert_eq!(qf(&w, &none, "SELECT count(NEAR_DIST) FROM {t}"), 2.0, "p3 is outside the radius: no match");

    let all = run(&w, "join_nearest", json!({"radius": 0.5, "units": "km", "keep_all": true}), &[&w.pts, &w.polys]);
    assert_eq!(all.rows, Some(3));
    let hit = run(&w, "join_nearest", json!({"radius": 0.5, "units": "km", "keep_all": false}), &[&w.pts, &w.polys]);
    assert_eq!(hit.rows, Some(2));
}

#[test]
fn select_by_location_tool_known_result() {
    let w = world();
    let inside = run(&w, "select_by_location", json!({"relation": "intersects", "invert": false}), &[&w.pts, &w.polys]);
    let outside = run(&w, "select_by_location", json!({"relation": "intersects", "invert": true}), &[&w.pts, &w.polys]);
    assert_eq!((inside.rows, outside.rows), (Some(2), Some(1)));
    assert_eq!(qs(&w, &outside, "SELECT name FROM {t}"), "p3");
    assert_eq!(inside.crs.as_deref(), Some(UTM));
}

#[test]
fn spatial_join_predicates_and_keep_all() {
    let w = world();
    let all = run(&w, "spatial_join", json!({"predicate": "intersects", "keep_all": true}), &[&w.pts, &w.polys]);
    assert_eq!(all.rows, Some(3));
    let f = fields(&w, &all);
    assert!(f.contains(&"val".to_string()) && f.contains(&"pop".to_string()), "{f:?}");
    assert_eq!(qf(&w, &all, "SELECT count(pop) FROM {t}"), 2.0, "p3 keeps its row with NULL join fields");
    let some = run(&w, "spatial_join", json!({"predicate": "intersects", "keep_all": false}), &[&w.pts, &w.polys]);
    assert_eq!(some.rows, Some(2));
    // Contains: polygons as targets, points as join features.
    let c = run(&w, "spatial_join", json!({"predicate": "contains", "keep_all": false}), &[&w.polys, &w.pts]);
    assert_eq!(c.rows, Some(2));
}

#[test]
fn summarize_within_counts_and_sums() {
    let w = world();
    let r = run(&w, "summarize_within", json!({"field": "val", "stat": "sum"}), &[&w.polys, &w.pts]);
    assert_eq!(r.rows, Some(3));
    assert!(fields(&w, &r).contains(&"COUNT".to_string()));
    assert_eq!(qf(&w, &r, "SELECT COUNT FROM {t} WHERE id = 1"), 1.0);
    assert_eq!(qf(&w, &r, "SELECT COUNT FROM {t} WHERE id = 3"), 0.0);
    assert_eq!(r.crs.as_deref(), Some(UTM));
}

// ---------------------------------------------------------------- geometry

#[test]
fn project_changes_crs_and_round_trips() {
    let w = world();
    let g = run(&w, "project", json!({"crs": "EPSG:4326"}), &[&w.pts]);
    assert_eq!(g.crs.as_deref(), Some("EPSG:4326"));
    assert_eq!(g.rows, Some(3));
    let lon = qf(&w, &g, "SELECT ST_X(geom) FROM {t} WHERE id = 1");
    let lat = qf(&w, &g, "SELECT ST_Y(geom) FROM {t} WHERE id = 1");
    assert!((-98.1..-97.7).contains(&lon) && (35.0..35.4).contains(&lat), "{lon},{lat}");
    // Chain the project tool back: the original metres return to the mm.
    let g_layer = w.e.preview_output(g.output.as_ref().unwrap(), g.crs.clone(), "g").unwrap();
    let back = w.e.transform_point(lon, lat, "EPSG:4326", UTM).unwrap();
    assert!((back.0 - (X0 + 50.0)).abs() < 1e-3 && (back.1 - (Y0 + 50.0)).abs() < 1e-3, "{back:?}");
    assert_eq!(g_layer.crs.as_deref(), Some("EPSG:4326"));
}

#[test]
fn dissolve_and_collect_group_by_field() {
    let w = world();
    let all = run(&w, "dissolve", json!({}), &[&w.polys]);
    assert_eq!(all.rows, Some(1));
    close(qf(&w, &all, "SELECT sum(ST_Area(geom)) FROM {t}"), 30_000.0, 1e-6);
    let by = run(&w, "dissolve", json!({"field": "name"}), &[&w.polys]);
    assert_eq!((by.rows, by.crs.as_deref()), (Some(2), Some(UTM)));
    close(qf(&w, &by, "SELECT ST_Area(geom) FROM {t} WHERE name = 'a'"), 20_000.0, 1e-6);
    assert_eq!(run(&w, "collect", json!({"field": "name"}), &[&w.polys]).rows, Some(2));
    assert_eq!(run(&w, "collect", json!({}), &[&w.polys]).rows, Some(1));
}

#[test]
fn feature_to_point_and_bounding_geometry() {
    let w = world();
    let c = run(&w, "feature_to_point", json!({"method": "centroid"}), &[&w.polys]);
    assert_eq!(c.rows, Some(3));
    assert_eq!(qs(&w, &c, "SELECT ST_GeometryType(geom) FROM {t} LIMIT 1"), "POINT");
    assert_eq!(qs(&w, &c, "SELECT ST_AsText(geom) FROM {t} WHERE id = 1"), format!("POINT ({} {})", X0 + 50.0, Y0 + 50.0));
    let env = run(&w, "bounding_geometry", json!({"shape": "envelope", "group": "all"}), &[&w.polys]);
    close(qf(&w, &env, "SELECT ST_Area(geom) FROM {t}"), 40_000.0, 1e-6);
    // The L-shape's convex hull cuts a 100×100 right triangle (5,000 m²) off the square.
    let hull = run(&w, "bounding_geometry", json!({"shape": "hull", "group": "all"}), &[&w.polys]);
    close(qf(&w, &hull, "SELECT ST_Area(geom) FROM {t}"), 35_000.0, 1e-6);
    assert_eq!(hull.crs.as_deref(), Some(UTM));
    let ch = run(&w, "concave_hull", json!({"ratio": 1.0}), &[&w.pts]);
    close(qf(&w, &ch, "SELECT ST_Area(geom) FROM {t}"), 47_500.0, 0.01); // ratio 1 = convex hull
}

#[test]
fn simplify_explode_repair_delaunay_voronoi() {
    let w = world();
    let wig = w.add("wig", &format!("SELECT 1 AS id, ST_GeomFromText('LINESTRING({a} {b}, {c} {d}, {e} {b})') AS geom", a = X0, b = Y0, c = X0 + 50.0, d = Y0 + 1.0, e = X0 + 100.0), Some(UTM));
    let s = run(&w, "simplify", json!({"tolerance": 5, "units": "m", "topology": false}), &[&wig]);
    assert_eq!(qf(&w, &s, "SELECT ST_NPoints(geom) FROM {t}"), 2.0, "a 1 m wiggle vanishes at 5 m tolerance");
    let keep = run(&w, "simplify", json!({"tolerance": 0.1, "units": "m", "topology": false}), &[&wig]);
    assert_eq!(qf(&w, &keep, "SELECT ST_NPoints(geom) FROM {t}"), 3.0);

    let multi = w.add("multi", &format!("SELECT 1 AS id, ST_Union({}, {}) AS geom", env(0.0, 0.0, 10.0, 10.0), env(50.0, 0.0, 60.0, 10.0)), Some(UTM));
    let x = run(&w, "explode", json!({}), &[&multi]);
    assert_eq!(x.rows, Some(2));
    close(qf(&w, &x, "SELECT max(ST_Area(geom)) FROM {t}"), 100.0, 1e-6);

    // A bow-tie is invalid; Repair returns something valid with the same row.
    let bow = w.add("bow", &format!("SELECT 1 AS id, ST_GeomFromText('POLYGON(({a} {b}, {c} {d}, {c} {b}, {a} {d}, {a} {b}))') AS geom", a = X0, b = Y0, c = X0 + 100.0, d = Y0 + 100.0), Some(UTM));
    let fixed = run(&w, "repair", json!({}), &[&bow]);
    assert_eq!(fixed.rows, Some(1));
    assert_eq!(qs(&w, &fixed, "SELECT CAST(ST_IsValid(geom) AS VARCHAR) FROM {t}"), "true");

    let tri = run(&w, "delaunay", json!({}), &[&w.pts]);
    assert_eq!((tri.rows, tri.crs.as_deref()), (Some(1), Some(UTM)), "three points make one triangle");
    close(qf(&w, &tri, "SELECT ST_Area(geom) FROM {t}"), 47_500.0, 1e-6);
    let v = run(&w, "voronoi", json!({"buffer": 10}), &[&w.pts]);
    assert_eq!(v.rows, Some(3));
    assert!(fields(&w, &v).contains(&"name".to_string()), "cells carry the point's attributes");
}

#[test]
fn lines_and_vertices_tools() {
    let w = world();
    let rings = run(&w, "polygons_to_lines", json!({}), &[&w.polys]);
    assert_eq!(rings.rows, Some(3));
    assert!(qs(&w, &rings, "SELECT ST_GeometryType(geom) FROM {t} LIMIT 1").contains("LINE"));
    // Rebuild polygons from those rings: round trip.
    let g = graph(json!({
        "nodes": [
            {"id": "i", "tool": "input_layer", "params": {"layer": w.polys}},
            {"id": "l", "tool": "polygons_to_lines", "params": {}},
            {"id": "p", "tool": "lines_to_polygons", "params": {}}
        ],
        "edges": [{"source": "i", "target": "l", "targetHandle": "in"}, {"source": "l", "target": "p", "targetHandle": "in"}]
    }));
    let res = w.e.run_graph(&g, Some("p"), &mut |_| {}).unwrap();
    let p = res.iter().find(|r| r.id == "p").unwrap();
    assert_eq!((p.status.clone(), p.rows), (Status::Done, Some(3)), "{:?}", p.error);
    close(qf(&w, p, "SELECT sum(ST_Area(geom)) FROM {t}"), 30_000.0, 1e-6);

    let v = run(&w, "extract_vertices", json!({}), &[&w.pts]);
    assert_eq!(v.rows, Some(3));
    assert_eq!(fields(&w, &v).last().map(String::as_str), Some("VERTEX_INDEX"));
    assert_eq!(qf(&w, &v, "SELECT max(VERTEX_INDEX) FROM {t}"), 0.0);

    let path = run(&w, "points_to_path", json!({"order": "id"}), &[&w.pts]);
    assert_eq!(path.rows, Some(1));
    // p1→p2 is 100 m, p2→p3 is √(850²+950²).
    close(qf(&w, &path, "SELECT ST_Length(geom) FROM {t}"), 100.0 + (850f64.powi(2) + 950f64.powi(2)).sqrt(), 1e-6);

    let along = run(&w, "points_along_lines", json!({"distance": 50, "units": "m"}), &[&w.lines]);
    assert!(along.rows.unwrap() >= 10, "200 m + 400 m of line at 50 m: {:?}", along.rows);
    assert_eq!(qs(&w, &along, "SELECT ST_GeometryType(geom) FROM {t} LIMIT 1"), "POINT");
}

#[test]
fn point_utilities_random_duplicates_nulls() {
    let w = world();
    let r = run(&w, "random_points", json!({"count": 5}), &[&w.polys]);
    assert_eq!(r.rows, Some(15), "5 points in each of the 3 polygons");
    assert_eq!(r.crs.as_deref(), Some(UTM));
    let dup = w.add("dup", &format!("SELECT x AS id, ST_Point({}, {}) AS geom FROM range(3) t(x) UNION ALL SELECT 9, ST_Point({}, {})", X0, Y0, X0, Y0), Some(UTM));
    assert_eq!(run(&w, "delete_duplicates", json!({}), &[&dup]).rows, Some(1), "four identical points collapse");
    assert_eq!(run(&w, "delete_duplicates", json!({}), &[&w.pts]).rows, Some(3));
    let nul = w.add("nul", &format!("SELECT 1 AS id, ST_Point({}, {}) AS geom UNION ALL SELECT 2, NULL", X0, Y0), Some(UTM));
    let n = run(&w, "remove_null", json!({}), &[&nul]);
    assert_eq!(n.rows, Some(1), "the feature without a shape is dropped");
}

#[test]
fn fishnet_grid_cells() {
    let w = world();
    let all = run(&w, "fishnet", json!({"shape": "square", "size": 100, "units": "m", "clip": false}), &[&w.polys]);
    let hit = run(&w, "fishnet", json!({"shape": "square", "size": 100, "units": "m", "clip": true}), &[&w.polys]);
    assert!(all.rows.unwrap() >= 4 && hit.rows.unwrap() >= 3 && hit.rows <= all.rows, "{:?} {:?}", all.rows, hit.rows);
    let hex = run(&w, "fishnet", json!({"shape": "hexagon", "size": 50, "units": "m", "clip": false}), &[&w.polys]);
    assert!(hex.rows.unwrap() > 4);
    assert!(all.crs.is_some());
}

// ---------------------------------------------------------------- tables

#[test]
fn calculate_field_and_geometry_known_values() {
    let w = world();
    let r = run(&w, "calculate_field", json!({"field": "TWICE", "expression": "pop * 2"}), &[&w.polys]);
    assert_eq!(r.rows, Some(3));
    assert_eq!(qf(&w, &r, "SELECT sum(TWICE) FROM {t}"), 120.0);
    assert_eq!(fields(&w, &r), ["id", "name", "pop", "TWICE"]);
    assert_eq!(r.crs.as_deref(), Some(UTM), "a table op keeps the CRS");

    // Geodesic areas on the ellipsoid: 30,000 m² on the grid is ~30,024 m² on the ground at UTM's 0.9996 scale.
    let a = run(&w, "calculate_geometry", json!({"field": "AREA_M2", "property": "area", "area_units": "sqm"}), &[&w.polys]);
    close(qf(&w, &a, "SELECT sum(AREA_M2) FROM {t}"), 30_000.0 / 0.9996f64.powi(2), 0.003);
    let len = run(&w, "calculate_geometry", json!({"field": "LEN", "property": "length", "length_units": "m"}), &[&w.lines]);
    close(qf(&w, &len, "SELECT LEN FROM {t} WHERE id = 1"), 200.0 / 0.9996, 0.003);
    let acres = run(&w, "calculate_geometry", json!({"field": "AC", "property": "area", "area_units": "acres"}), &[&w.polys]);
    close(qf(&w, &acres, "SELECT sum(AC) FROM {t}") / qf(&w, &a, "SELECT sum(AREA_M2) FROM {t}"), 1.0 / 4046.8564224, 1e-6);
}

#[test]
fn filter_and_summary_statistics() {
    let w = world();
    assert_eq!(run(&w, "filter", json!({"where": "pop > 15"}), &[&w.polys]).rows, Some(2));
    assert_eq!(run(&w, "filter", json!({"where": "pop > 1000"}), &[&w.polys]).rows, Some(0));
    let bad = run_raw(&w, "filter", json!({"where": "pop >> ??"}), &[&w.polys]);
    assert_eq!(bad.status, Status::Error);
    assert!(!bad.error.unwrap().contains("__node_"), "internal table names stay out of messages");
    let all = run(&w, "summary_statistics", json!({"field": "pop"}), &[&w.polys]);
    assert_eq!(all.rows, Some(1));
    let by = run(&w, "summary_statistics", json!({"field": "pop", "case": "name"}), &[&w.polys]);
    assert_eq!(by.rows, Some(2));
}

#[test]
fn attribute_join_and_merge() {
    let w = world();
    let csv = w.dir.path().join("lookup.csv");
    std::fs::write(&csv, "name,label\na,Alpha\n").unwrap();
    let t = w.e.import(&csv).unwrap().remove(0);
    let left = run(&w, "attribute_join", json!({"target_field": "name", "join_field": "name", "keep_all": true}), &[&w.polys, &t.id]);
    assert_eq!(left.rows, Some(3));
    assert!(fields(&w, &left).contains(&"label".to_string()));
    assert_eq!(qf(&w, &left, "SELECT count(label) FROM {t}"), 2.0, "'b' has no match: NULL label, row kept");
    assert_eq!(left.crs.as_deref(), Some(UTM));
    let inner = run(&w, "attribute_join", json!({"target_field": "name", "join_field": "name", "keep_all": false}), &[&w.polys, &t.id]);
    assert_eq!(inner.rows, Some(2));

    let m = run(&w, "merge", json!({"source_field": true}), &[&w.polys, &w.mask]);
    assert_eq!(m.rows, Some(4));
    let f = fields(&w, &m);
    assert!(f.contains(&"pop".to_string()) && f.contains(&"mid".to_string()), "union of both layers' fields: {f:?}");
}

#[test]
fn export_writes_a_file_that_reimports_with_its_crs() {
    let w = world();
    let out = w.dir.path().join("out.gpkg");
    let r = run_raw(&w, "export", json!({"path": out.display().to_string(), "format": "GPKG"}), &[&w.polys]);
    assert_eq!(r.status, Status::Done, "{:?}", r.error);
    let back = w.e.import(&out).unwrap().remove(0);
    assert_eq!((back.feature_count, back.crs.as_deref()), (3, Some(UTM)));
}

// ---------------------------------------------------------------- refusal paths

#[test]
fn layer_without_crs_is_refused_not_guessed() {
    let w = world();
    let nocrs = w.add("nocrs", "SELECT 1 AS id, ST_Point(1, 2) AS geom", None);
    for (tool, params, inputs) in [
        ("buffer", json!({"distance": 10, "units": "m"}), vec![nocrs.as_str()]),
        ("project", json!({"crs": "EPSG:4326"}), vec![nocrs.as_str()]),
        ("near", json!({"radius": 1, "units": "km"}), vec![nocrs.as_str(), w.polys.as_str()]),
        ("spatial_join", json!({}), vec![nocrs.as_str(), w.polys.as_str()]),
    ] {
        let r = run_raw(&w, tool, params, &inputs);
        assert_eq!(r.status, Status::Error, "{tool} must not guess a CRS (got {:?}, rows {:?})", r.status, r.rows);
        let msg = r.error.unwrap().to_lowercase();
        assert!(msg.contains("coordinate system") || msg.contains("crs"), "{tool}: unreadable message {msg}");
    }
}

#[test]
fn mixed_crs_inputs_are_reprojected_and_flagged() {
    let w = world();
    // The same mask expressed in WGS 84 degrees: clip must reproject it to the input's CRS.
    let m = w.e.preview_output(&run(&w, "project", json!({"crs": "EPSG:4326"}), &[&w.mask]).output.unwrap(), Some("EPSG:4326".into()), "m").unwrap();
    assert_eq!(m.crs.as_deref(), Some("EPSG:4326"));
    let id = w.e.copy_layer(&m.id, "mask4326", None).unwrap().id;
    let r = run(&w, "clip", json!({}), &[&w.polys, &id]);
    assert!(r.reprojected, "the executor flags an input that was reprojected");
    assert_eq!(r.crs.as_deref(), Some(UTM), "the output stays in the first input's CRS");
    close(qf(&w, &r, "SELECT sum(ST_Area(geom)) FROM {t}"), 10_000.0, 0.01);
}

#[test]
fn every_tool_spec_is_well_formed() {
    let w = world();
    let specs = w.e.tool_specs();
    assert!(specs.len() >= 45, "{} tools", specs.len());
    let mut seen = std::collections::HashSet::new();
    for s in &specs {
        assert!(seen.insert(s.id), "duplicate tool id {}", s.id);
        assert!(!s.name.is_empty() && !s.description.is_empty() && !s.category.is_empty(), "{} lacks name/description/category", s.id);
        for p in &s.params {
            if matches!(p.kind, engine::nodes::ParamKind::Enum) {
                assert!(!p.options.is_empty(), "{}.{} is an enum without options", s.id, p.id);
                if let Some(d) = p.default.as_str() {
                    assert!(p.options.iter().any(|(v, _)| *v == d), "{}.{} default {d} is not an option", s.id, p.id);
                }
            }
            if let Some(port) = p.field_of {
                assert!(s.inputs.iter().any(|i| i.id == port), "{}.{} refers to missing port {port}", s.id, p.id);
            }
        }
    }
}
