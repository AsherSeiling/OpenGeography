//! Wave 1 package T6: Hex Bins (H3), Natural Neighbour and TIN with Breaklines (spade), DBSCAN, Supervised Classification (smartcore).
//! Answers are worked out by hand or by an independent route.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

const X0: f64 = 600_000.0;
const Y0: f64 = 3_900_000.0;

fn vlayer(e: &Engine, dir: &Path, name: &str, srs: &str, select: &str) -> String {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let gpkg = dir.join(format!("{name}.gpkg"));
    c.execute_batch(&format!("COPY ({select}) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'l', SRS '{srs}', USE_TMP_FILE false)", gpkg.display())).unwrap();
    e.import(&gpkg).unwrap().remove(0).id
}

/// Points with a name, a number and an integer class: (name, x, y, value, class).
fn pts(e: &Engine, dir: &Path, name: &str, srs: &str, rows: &[(&str, f64, f64, f64, i64)]) -> String {
    let v: Vec<String> = rows.iter().map(|(n, x, y, z, c)| format!("('{n}', {z:?}, {c}, ST_GeomFromText('POINT({x:?} {y:?})'))")).collect();
    vlayer(e, dir, name, srs, &format!("SELECT * FROM (VALUES {}) t(name, v, cls, geom)", v.join(",")))
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

fn table(e: &Engine, r: &NodeResult, cols: &str, order: &str) -> Vec<Vec<String>> {
    let n = cols.split(',').count();
    let list: Vec<String> = cols.split(',').map(|c| format!("coalesce(CAST({} AS VARCHAR), 'NULL')", c.trim())).collect();
    let sql = format!("SELECT {} FROM \"{}\" ORDER BY {order}", list.join(", "), r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&sql)?;
        Ok(st.query_map([], |row| (0..n).map(|k| row.get::<_, String>(k)).collect::<Result<Vec<_>, _>>())?.collect::<Result<Vec<_>, _>>()?)
    })
    .unwrap()
}

fn num(s: &str) -> f64 {
    s.parse().unwrap_or_else(|_| panic!("not a number: {s}"))
}

fn write_raster(path: &Path, epsg: u32, x0: f64, y0: f64, cell: f64, bands: &[Vec<Vec<f32>>]) {
    let (h, w) = (bands[0].len(), bands[0][0].len());
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, w, h, bands.len()).unwrap();
    ds.set_geo_transform(&[x0, cell, 0.0, y0, 0.0, -cell]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    for (i, b) in bands.iter().enumerate() {
        let mut band = ds.rasterband(i + 1).unwrap();
        band.set_no_data_value(Some(-9999.0)).unwrap();
        band.write((0, 0), (w, h), &mut gdal::raster::Buffer::new((w, h), b.concat())).unwrap();
    }
}

fn read(path: &str) -> (usize, usize, Vec<f32>, [f64; 6]) {
    let ds = gdal::Dataset::open(path).unwrap();
    let (w, h) = ds.raster_size();
    (w, h, ds.rasterband(1).unwrap().read_as::<f32>((0, 0), (w, h), (w, h), None).unwrap().data().to_vec(), ds.geo_transform().unwrap())
}

#[test]
fn t6_dbscan_finds_clusters_borders_and_noise() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let mut rows: Vec<(String, f64, f64, f64, i64)> = vec![];
    for k in 0..5 {
        rows.push((format!("a{k}"), X0 + k as f64 * 8.0, Y0, 0.0, 0)); // a tight line of 5 points, 8 m apart
        rows.push((format!("b{k}"), X0 + 2000.0 + k as f64 * 8.0, Y0 + 500.0, 0.0, 0));
    }
    rows.push(("border".into(), X0 + 32.0 + 45.0, Y0, 0.0, 0)); // 45 m from the last point of A: within eps of one point only
    rows.push(("lone1".into(), X0 + 1000.0, Y0 + 1000.0, 0.0, 0));
    rows.push(("lone2".into(), X0 + 5000.0, Y0, 0.0, 0));
    let refs: Vec<(&str, f64, f64, f64, i64)> = rows.iter().map(|r| (r.0.as_str(), r.1, r.2, r.3, r.4)).collect();
    let l = pts(&e, dir.path(), "p", "EPSG:32614", &refs);
    let r = tool(&e, "dbscan", json!({"eps": 50, "min_points": 4}), &[("in", &l)]);
    let t = table(&e, &r, "name, cluster, label", "name");
    let get = |n: &str| t.iter().find(|r| r[0] == n).unwrap().clone();
    // A and B are clusters 1 and 2 (numbered in row order), the border point joins A without being a core point, the others are noise.
    for k in 0..5 {
        assert_eq!(get(&format!("a{k}"))[1], "1", "{t:?}");
        assert_eq!(get(&format!("b{k}"))[1], "2");
    }
    assert_eq!(get("border")[1], "1");
    assert_eq!((get("lone1")[1].as_str(), get("lone1")[2].as_str()), ("0", "noise"));
    assert_eq!(get("lone2")[1], "0");
    assert!(r.message.as_deref().unwrap_or("").contains("2 cluster(s), 2 noise point(s) of 13"), "{:?}", r.message);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // Geographic: eps is in metres. One degree of longitude is 55.8 km at 60 N and 111.3 km at the equator, so eps = 60 km joins the pair
    // at 60 N and not the pair on the equator.
    let g = pts(&e, dir.path(), "g", "EPSG:4326", &[("n1", 0.0, 60.0, 0.0, 0), ("n2", 1.0, 60.0, 0.0, 0), ("q1", 0.0, 0.0, 0.0, 0), ("q2", 1.0, 0.0, 0.0, 0)]);
    let r = tool(&e, "dbscan", json!({"eps": 60000, "min_points": 2}), &[("in", &g)]);
    let t = table(&e, &r, "name, cluster", "name");
    assert_eq!(t, vec![vec!["n1", "1"], vec!["n2", "1"], vec!["q1", "0"], vec!["q2", "0"]], "{t:?}");
    assert!(matches!(try_tool(&e, "dbscan", json!({"eps": 0}), &[("in", &g)]).status, Status::Error));
}

#[test]
fn t6_hex_bins_count_sum_mean_and_geodesic_area() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = pts(&e, dir.path(), "p", "EPSG:4326", &[("a", -100.0, 40.0, 1.0, 0), ("b", -100.0001, 40.0001, 2.0, 0), ("c", -100.0002, 40.0, 6.0, 0), ("d", -90.0, 35.0, 10.0, 0)]);
    let r = tool(&e, "hex_bins", json!({"resolution": 5, "field": "v"}), &[("in", &l)]);
    let t = table(&e, &r, "h3, count, sum, mean, values_used, area_m2", "count DESC");
    assert_eq!(t.len(), 2, "{t:?}");
    // The cell of (-100, 40) at resolution 5, by the H3 library directly.
    let cell = h3o::LatLng::new(40.0, -100.0).unwrap().to_cell(h3o::Resolution::Five);
    assert_eq!(t[0][0], cell.to_string());
    assert_eq!((t[0][1].as_str(), num(&t[0][2]), num(&t[0][3])), ("3", 9.0, 3.0));
    assert_eq!(t[1][1], "1");
    // The ellipsoidal area is within 1 % of H3's own spherical area of the same cell.
    let sph = cell.area_m2();
    assert!((num(&t[0][5]) - sph).abs() / sph < 0.01, "{} vs {sph}", t[0][5]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:4326"));
    // A layer in UTM gives the same cell: the explicit transform to longitude/latitude is used.
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let (x, y): (f64, f64) = c.query_row("SELECT ST_X(p), ST_Y(p) FROM (SELECT ST_Transform(ST_Point(-100.0, 40.0), 'EPSG:4326', 'EPSG:32614', true) AS p)", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    let u = pts(&e, dir.path(), "u", "EPSG:32614", &[("a", x, y, 5.0, 0)]);
    let ru = tool(&e, "hex_bins", json!({"resolution": 5}), &[("in", &u)]);
    assert_eq!(table(&e, &ru, "h3", "h3")[0][0], cell.to_string());
    assert_eq!(ru.crs.as_deref(), Some("EPSG:32614"));
    // A resolution outside 0-15 is refused.
    assert!(matches!(try_tool(&e, "hex_bins", json!({"resolution": 16}), &[("in", &l)]).status, Status::Error));
}

#[test]
fn t6_natural_neighbour_reproduces_a_plane_and_stays_within_the_data() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // z = 2 dx + 3 dy + 1 on an irregular set of points (local offsets from the origin). Natural neighbour has linear precision.
    let xy = [(0.0, 0.0), (100.0, 10.0), (200.0, 0.0), (30.0, 90.0), (160.0, 120.0), (60.0, 200.0), (210.0, 190.0), (110.0, 95.0)];
    let rows: Vec<(String, f64, f64, f64, i64)> = xy.iter().enumerate().map(|(i, (x, y))| (format!("p{i}"), X0 + x, Y0 + y, 2.0 * x + 3.0 * y + 1.0, 0)).collect();
    let refs: Vec<(&str, f64, f64, f64, i64)> = rows.iter().map(|r| (r.0.as_str(), r.1, r.2, r.3, r.4)).collect();
    let l = pts(&e, dir.path(), "p", "EPSG:32614", &refs);
    let r = tool(&e, "natural_neighbour", json!({"field": "v", "cell": 10}), &[("in", &l)]);
    let (w, h, data, gt) = read(r.output.as_deref().unwrap());
    let (mut inside, mut outside) = (0, 0);
    for row in 0..h {
        for col in 0..w {
            let (cx, cy) = (gt[0] + (col as f64 + 0.5) * gt[1] - X0, gt[3] + (row as f64 + 0.5) * gt[5] - Y0);
            let v = data[row * w + col];
            if v == -9999.0 {
                outside += 1;
            } else {
                inside += 1;
                assert!((v as f64 - (2.0 * cx + 3.0 * cy + 1.0)).abs() < 0.01, "cell {col},{row}: {v} vs plane");
            }
        }
    }
    assert!(inside > 100 && outside > 10, "{inside} {outside}");
    assert!(r.message.as_deref().unwrap_or("").contains("outside the convex hull"), "{:?}", r.message);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // A bumpy surface: no value goes above the highest or below the lowest value of the points, and a cell centred on a point has its value.
    let z = [5.0, 9.0, 2.0, 7.0, 3.0, 8.0, 1.0, 6.0];
    let rows: Vec<(String, f64, f64, f64, i64)> = xy.iter().zip(z).enumerate().map(|(i, ((x, y), z))| (format!("p{i}"), X0 + x, Y0 + y, z, 0)).collect();
    let refs: Vec<(&str, f64, f64, f64, i64)> = rows.iter().map(|r| (r.0.as_str(), r.1, r.2, r.3, r.4)).collect();
    let l = pts(&e, dir.path(), "q", "EPSG:32614", &refs);
    let r = tool(&e, "natural_neighbour", json!({"field": "v", "cell": 10}), &[("in", &l)]);
    let (w, _, data, gt) = read(r.output.as_deref().unwrap());
    assert!(data.iter().filter(|v| **v != -9999.0).all(|v| (1.0 - 1e-5..=9.0 + 1e-5).contains(v)));
    // (110, 95) is not on a cell centre; (100, 10) is not either (centres sit at 5 mod 10), so use the (200, 0) cell neighbour check instead:
    // the cell whose centre is nearest (205, -5 is outside the hull) is skipped; check the cell around (105, 95) lies between its neighbours' range.
    let (col, row) = (((X0 + 105.0 - gt[0]) / 10.0) as usize, ((gt[3] - (Y0 + 95.0)) / 10.0) as usize);
    assert!((4.0..=7.0).contains(&data[row * w + col]), "{}", data[row * w + col]);
    // Degrees and a line of points are refused in words.
    let g = pts(&e, dir.path(), "g", "EPSG:4326", &[("a", 0.0, 0.0, 1.0, 0), ("b", 1.0, 0.0, 1.0, 0), ("c", 0.0, 1.0, 1.0, 0)]);
    assert!(matches!(try_tool(&e, "natural_neighbour", json!({"field": "v", "cell": 0.1}), &[("in", &g)]).status, Status::Error));
    let line = pts(&e, dir.path(), "line", "EPSG:32614", &[("a", X0, Y0, 1.0, 0), ("b", X0 + 10.0, Y0, 2.0, 0), ("c", X0 + 20.0, Y0, 3.0, 0)]);
    let bad = try_tool(&e, "natural_neighbour", json!({"field": "v", "cell": 1}), &[("in", &line)]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("one line"));
}

#[test]
fn t6_tin_follows_a_breakline_exactly() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Four corners at height 0 and a point of 100 close to the middle of the breakline from (2, 5) to (8, 5), height 7.
    let l = pts(&e, dir.path(), "p", "EPSG:32614", &[("a", X0, Y0, 0.0, 0), ("b", X0 + 10.0, Y0, 0.0, 0), ("c", X0 + 10.0, Y0 + 10.0, 0.0, 0), ("d", X0, Y0 + 10.0, 0.0, 0), ("w", X0 + 5.0, Y0 + 5.4, 100.0, 0)]);
    let bl = vlayer(&e, dir.path(), "bl", "EPSG:32614", &format!("SELECT 7.0 AS h, ST_GeomFromText('LINESTRING({} {}, {} {})') AS geom", X0 + 2.0, Y0 + 5.0, X0 + 8.0, Y0 + 5.0));
    let r = tool(&e, "tin_breaklines", json!({"field": "v", "line_field": "h", "cell": 2}), &[("in", &l), ("lines", &bl)]);
    let (w, h, data, gt) = read(r.output.as_deref().unwrap());
    // Cell centres on y = 5 at x = 3, 5, 7 lie on the breakline: exactly its height, whatever the point of 100 does nearby.
    let at = |x: f64, y: f64| {
        let (c, rr) = (((X0 + x - gt[0]) / gt[1]).floor() as usize, ((gt[3] - (Y0 + y)) / 2.0).floor() as usize);
        assert!(c < w && rr < h);
        data[rr * w + c] as f64
    };
    for x in [3.0, 5.0, 7.0] {
        assert!((at(x, 5.0) - 7.0).abs() < 1e-4, "x = {x}: {}", at(x, 5.0));
    }
    // Away from the line the point of 100 and the corners shape the surface; the corner cells stay low.
    assert!(at(1.0, 9.0) < 50.0 && at(5.0, 5.4 + 1.0) > 7.0, "{} {}", at(1.0, 9.0), at(5.0, 6.5));
    // A line without a height (no Z, no field) is reported and not used.
    let bad = vlayer(&e, dir.path(), "bad", "EPSG:32614", &format!("SELECT 1 AS k, ST_GeomFromText('LINESTRING({} {}, {} {})') AS geom", X0 + 2.0, Y0 + 5.0, X0 + 8.0, Y0 + 5.0));
    let r2 = tool(&e, "tin_breaklines", json!({"field": "v", "cell": 2}), &[("in", &l), ("lines", &bad)]);
    assert!(r2.message.as_deref().unwrap_or("").contains("no height"), "{:?}", r2.message);
    // A breakline that crosses another is counted.
    let x2 = vlayer(&e, dir.path(), "x2", "EPSG:32614", &format!("SELECT * FROM (VALUES (7.0, ST_GeomFromText('LINESTRING({} {}, {} {})')), (3.0, ST_GeomFromText('LINESTRING({} {}, {} {})'))) t(h, geom)", X0 + 2.0, Y0 + 5.0, X0 + 8.0, Y0 + 5.0, X0 + 5.0, Y0 + 1.0, X0 + 5.0, Y0 + 9.0));
    let r3 = tool(&e, "tin_breaklines", json!({"field": "v", "line_field": "h", "cell": 2}), &[("in", &l), ("lines", &x2)]);
    assert!(r3.message.as_deref().unwrap_or("").contains("cross another breakline"), "{:?}", r3.message);
}

fn two_band_image(e: &Engine, dir: &Path) -> String {
    // 20 x 20 cells of 10 m: band 1 is the column number, band 2 the row number. The classes follow band 1: left half 1, right half 2.
    let b1: Vec<Vec<f32>> = (0..20).map(|_| (0..20).map(|c| c as f32).collect()).collect();
    let b2: Vec<Vec<f32>> = (0..20).map(|r| (0..20).map(|_| r as f32).collect()).collect();
    let p = dir.join("img.tif");
    write_raster(&p, 32614, X0, Y0, 10.0, &[b1, b2]);
    e.import(&p).unwrap().remove(0).id
}

fn training_points(e: &Engine, dir: &Path, name: &str) -> String {
    let mut rows: Vec<(String, f64, f64, f64, i64)> = vec![];
    for k in 0..10 {
        // Left: columns 1-8, class 1. Right: columns 11-18, class 2. Cell centres are at (col + 0.5) * 10 east and (row + 0.5) * 10 south.
        rows.push((format!("l{k}"), X0 + ((1 + k % 8) as f64 + 0.5) * 10.0, Y0 - ((2 * k) as f64 % 18.0 + 0.5) * 10.0, 0.0, 1));
        rows.push((format!("r{k}"), X0 + ((11 + k % 8) as f64 + 0.5) * 10.0, Y0 - ((2 * k + 1) as f64 % 18.0 + 0.5) * 10.0, 0.0, 2));
    }
    let refs: Vec<(&str, f64, f64, f64, i64)> = rows.iter().map(|r| (r.0.as_str(), r.1, r.2, r.3, r.4)).collect();
    pts(e, dir, name, "EPSG:32614", &refs)
}

#[test]
fn t6_classification_reports_holdout_accuracy_and_a_confusion_matrix() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let img = two_band_image(&e, dir.path());
    let tr = training_points(&e, dir.path(), "tr");
    for method in ["random_forest", "knn"] {
        let r = tool(&e, "classify_supervised", json!({"class_field": "cls", "method": method, "holdout": 0.3, "seed": 7, "k": 3, "trees": 50}), &[("in", &img), ("train", &tr)]);
        let (w, h, data, _) = read(r.output.as_deref().unwrap());
        let right = (0..h).flat_map(|row| (0..w).map(move |c| (row, c))).filter(|(row, c)| data[row * w + c] == if *c < 10 { 1.0 } else { 2.0 }).count();
        assert!(right as f64 / (w * h) as f64 > 0.9, "{method}: {right} of {} cells follow the rule", w * h);
        // Cells far from the boundary are certain.
        assert_eq!(data[5 * w + 2], 1.0);
        assert_eq!(data[5 * w + 17], 2.0);
        let msg = r.message.as_deref().unwrap_or("");
        assert!(msg.contains("hold-out accuracy 100.0 %") && msg.contains("seed 7") && msg.contains("class 1: 3 0; class 2: 0 3"), "{method}: {msg}");
        let ds = gdal::Dataset::open(r.output.as_deref().unwrap()).unwrap();
        use gdal::Metadata;
        assert_eq!(ds.metadata_item("HOLDOUT_ACCURACY", "").as_deref(), Some("1"));
        assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    }
    // The same seed gives the same map; the seed is part of the answer.
    let params = json!({"class_field": "cls", "seed": 7, "trees": 50});
    let a = read(tool(&e, "classify_supervised", params.clone(), &[("in", &img), ("train", &tr)]).output.as_deref().unwrap()).2;
    let b = read(tool(&e, "classify_supervised", params, &[("in", &img), ("train", &tr)]).output.as_deref().unwrap()).2;
    assert_eq!(a, b);
    // Polygons as training areas: every cell whose centre is inside.
    let polys = vlayer(&e, dir.path(), "polys", "EPSG:32614", &format!(
        "SELECT * FROM (VALUES (1, ST_GeomFromText('POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))')), (2, ST_GeomFromText('POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))'))) t(cls, geom)",
        X0, Y0, X0 + 80.0, Y0, X0 + 80.0, Y0 - 200.0, X0, Y0 - 200.0, X0, Y0,
        X0 + 120.0, Y0, X0 + 200.0, Y0, X0 + 200.0, Y0 - 200.0, X0 + 120.0, Y0 - 200.0, X0 + 120.0, Y0
    ));
    let r = tool(&e, "classify_supervised", json!({"class_field": "cls", "seed": 3, "trees": 20}), &[("in", &img), ("train", &polys)]);
    assert!(r.message.as_deref().unwrap_or("").contains("with 320 training samples"), "{:?}", r.message); // 8 columns x 20 rows x 2 classes
    // One class, or a class with a single sample, is refused in words.
    let one = pts(&e, dir.path(), "one", "EPSG:32614", &[("a", X0 + 15.0, Y0 - 15.0, 0.0, 1), ("b", X0 + 25.0, Y0 - 25.0, 0.0, 1)]);
    assert!(matches!(try_tool(&e, "classify_supervised", json!({"class_field": "cls"}), &[("in", &img), ("train", &one)]).status, Status::Error));
    let thin = pts(&e, dir.path(), "thin", "EPSG:32614", &[("a", X0 + 15.0, Y0 - 15.0, 0.0, 1), ("b", X0 + 25.0, Y0 - 25.0, 0.0, 1), ("c", X0 + 155.0, Y0 - 15.0, 0.0, 2)]);
    let bad = try_tool(&e, "classify_supervised", json!({"class_field": "cls"}), &[("in", &img), ("train", &thin)]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("Class 2 has only 1"));
}

#[test]
fn t6_every_tool_is_registered() {
    let e = Engine::new().unwrap();
    for id in ["hex_bins", "natural_neighbour", "tin_breaklines", "dbscan", "classify_supervised"] {
        let s = e.tool_specs().into_iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id}"));
        assert!(s.category.contains('›') && s.description.len() > 100, "{id}");
        assert!(!engine::nodes::keywords::keywords(id).is_empty(), "{id}");
    }
}
