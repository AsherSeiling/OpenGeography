//! Textbook analysis tools of Wave 1 package T4: kernel density, mean centre, standard distance, directional distribution, average nearest
//! neighbour, Ripley's K, tabulate area, profile along a line, cut and fill. Each test compares with an answer worked out by hand or from the
//! formula in an independent way (not by calling the code under test).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};
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

fn points(e: &Engine, dir: &Path, name: &str, srs: &str, pts: &[(f64, f64, f64)]) -> String {
    let rows: Vec<String> = pts.iter().map(|(x, y, w)| format!("({w:?}, ST_GeomFromText('POINT({x:?} {y:?})'))")).collect();
    vlayer(e, dir, name, srs, &format!("SELECT * FROM (VALUES {}) t(w, geom)", rows.join(",")))
}

fn polygon(e: &Engine, dir: &Path, name: &str, srs: &str, wkt: &str) -> String {
    vlayer(e, dir, name, srs, &format!("SELECT 1 AS z, ST_GeomFromText('{wkt}') AS geom"))
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

/// First row of the result table as numbers, for the given column expressions.
fn nums(e: &Engine, r: &NodeResult, cols: &str) -> Vec<f64> {
    let n = cols.split(',').count();
    let list: Vec<String> = cols.split(',').map(|c| format!("CAST({} AS DOUBLE)", c.trim())).collect();
    let sql = format!("SELECT {} FROM \"{}\" LIMIT 1", list.join(", "), r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| (0..n).map(|k| row.get::<_, f64>(k)).collect::<Result<Vec<f64>, _>>())?)).unwrap()
}

fn text(e: &Engine, r: &NodeResult, expr: &str) -> String {
    let sql = format!("SELECT {expr}::VARCHAR FROM \"{}\" LIMIT 1", r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| Ok(c.query_row(&sql, [], |row| row.get::<_, String>(0))?)).unwrap()
}

fn write_raster(path: &Path, epsg: u32, x0: f64, y0: f64, dx: f64, rows: &[Vec<f32>], nodata: Option<f32>, int: bool) {
    let (h, w) = (rows.len(), rows[0].len());
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let flat = rows.concat();
    macro_rules! mk {
        ($t:ty) => {{
            let mut ds = drv.create_with_band_type::<$t, _>(path, w, h, 1).unwrap();
            ds.set_geo_transform(&[x0, dx, 0.0, y0, 0.0, -dx]).unwrap();
            ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
            let mut b = ds.rasterband(1).unwrap();
            if let Some(n) = nodata {
                b.set_no_data_value(Some(n as f64)).unwrap();
            }
            b.write((0, 0), (w, h), &mut gdal::raster::Buffer::new((w, h), flat.iter().map(|v| *v as $t).collect())).unwrap();
        }};
    }
    if int { mk!(i32) } else { mk!(f32) }
}

fn raster(e: &Engine, dir: &Path, name: &str, epsg: u32, x0: f64, y0: f64, dx: f64, rows: &[Vec<f32>], nodata: Option<f32>, int: bool) -> String {
    let p = dir.join(format!("{name}.tif"));
    write_raster(&p, epsg, x0, y0, dx, rows, nodata, int);
    e.import(&p).unwrap().remove(0).id
}

fn ecef_mean_via_proj(pts: &[(f64, f64)]) -> (f64, f64) {
    use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
    let mut a = SpatialRef::from_epsg(4326).unwrap();
    a.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let b = SpatialRef::from_epsg(4978).unwrap();
    let t = CoordTransform::new(&a, &b).unwrap();
    let (mut x, mut y, mut z) = (pts.iter().map(|p| p.0).collect::<Vec<_>>(), pts.iter().map(|p| p.1).collect::<Vec<_>>(), vec![0.0; pts.len()]);
    t.transform_coords(&mut x, &mut y, &mut z).unwrap();
    let n = pts.len() as f64;
    let m = [x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n, z.iter().sum::<f64>() / n];
    let t2 = CoordTransform::new(&b, &a).unwrap();
    let (mut mx, mut my, mut mz) = ([m[0]], [m[1]], [m[2]]);
    t2.transform_coords(&mut mx, &mut my, &mut mz).unwrap();
    (mx[0], my[0])
}

#[test]
fn t4_features_that_surround_the_earth_have_no_mean_centre() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let g = points(&e, dir.path(), "anti", "EPSG:4326", &[(10.0, 20.0, 1.0), (-170.0, -20.0, 1.0), (50.0, 5.0, 1.0), (-130.0, -5.0, 1.0)]);
    for t in ["mean_centre", "standard_distance", "directional_distribution"] {
        let r = try_tool(&e, t, json!({}), &[("in", &g)]);
        assert!(matches!(r.status, Status::Error) && r.error.unwrap().contains("surround the Earth"), "{t}");
    }
}

#[test]
fn t4_mean_centre_is_weighted_in_a_plane_and_on_the_ellipsoid_in_degrees() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Planar weighted mean: (0, 0) with weight 1 and (6, 0) with weight 3 gives x = 4.5.
    let p = points(&e, dir.path(), "p", "EPSG:32614", &[(X0, Y0, 1.0), (X0 + 6.0, Y0, 3.0)]);
    let r = tool(&e, "mean_centre", json!({"weight": "w"}), &[("in", &p)]);
    let v = nums(&e, &r, "ST_X(geom), ST_Y(geom), n, weight_sum");
    assert!((v[0] - (X0 + 4.5)).abs() < 1e-9 && (v[1] - Y0).abs() < 1e-9 && v[2] == 2.0 && v[3] == 4.0, "{v:?}");
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // Two points at 60 N on opposite meridians: the ellipsoidal mean is the pole, not (90, 60).
    let g = points(&e, dir.path(), "g", "EPSG:4326", &[(0.0, 60.0, 1.0), (180.0, 60.0, 1.0)]);
    let v = nums(&e, &tool(&e, "mean_centre", json!({}), &[("in", &g)]), "ST_Y(geom)");
    assert!((v[0] - 90.0).abs() < 1e-6, "{v:?}");
    // Across the date line: 179 and -179 at the equator average to 180 (or -180), never to 0.
    let g2 = points(&e, dir.path(), "g2", "EPSG:4326", &[(179.0, 10.0, 1.0), (-179.0, 10.0, 1.0)]);
    let v = nums(&e, &tool(&e, "mean_centre", json!({}), &[("in", &g2)]), "abs(ST_X(geom))");
    assert!((v[0] - 180.0).abs() < 1e-6, "{v:?}");
    // Against PROJ's geocentric conversion (EPSG:4978) done independently.
    let pts = [(10.0, 50.0), (30.0, 52.0), (25.0, 40.0)];
    let g3 = points(&e, dir.path(), "g3", "EPSG:4326", &pts.map(|p| (p.0, p.1, 1.0)));
    let v = nums(&e, &tool(&e, "mean_centre", json!({}), &[("in", &g3)]), "ST_X(geom), ST_Y(geom)");
    let (lon, lat) = ecef_mean_via_proj(&pts);
    assert!((v[0] - lon).abs() < 1e-9 && (v[1] - lat).abs() < 1e-9, "{v:?} vs {lon} {lat}");
    // A weight field with a negative value is refused in words.
    let neg = points(&e, dir.path(), "neg", "EPSG:32614", &[(X0, Y0, 1.0), (X0 + 1.0, Y0, -2.0)]);
    assert!(matches!(try_tool(&e, "mean_centre", json!({"weight": "w"}), &[("in", &neg)]).status, Status::Error));
}

#[test]
fn t4_standard_distance_is_the_root_mean_square_distance_to_the_mean_centre() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let p = points(&e, dir.path(), "p", "EPSG:32614", &[(X0, Y0, 1.0), (X0 + 6.0, Y0, 3.0)]);
    // Unweighted: mean 3, both 3 away: 3.  Weighted: mean 4.5, sqrt((1 * 4.5^2 + 3 * 1.5^2) / 4) = sqrt(6.75).
    let u = nums(&e, &tool(&e, "standard_distance", json!({}), &[("in", &p)]), "std_distance_m, n");
    assert!((u[0] - 3.0).abs() < 1e-9 && u[1] == 2.0, "{u:?}");
    let w = nums(&e, &tool(&e, "standard_distance", json!({"weight": "w"}), &[("in", &p)]), "std_distance_m");
    assert!((w[0] - 6.75f64.sqrt()).abs() < 1e-9, "{w:?}");
    // The circle's radius from the centre to a vertex is the standard distance.
    let r = tool(&e, "standard_distance", json!({}), &[("in", &p)]);
    let v = nums(&e, &r, "ST_XMax(geom) - ST_XMin(geom)");
    assert!((v[0] - 6.0).abs() < 0.01, "{v:?}");
    // Geographic: four points 10 km from a centre toward N, E, S, W; the standard distance is 10 km (geodesic).
    let geod = Geodesic::wgs84();
    let (lo0, la0) = (-100.0, 40.0);
    let pts: Vec<(f64, f64, f64)> = [0.0, 90.0, 180.0, 270.0].iter().map(|az| { let (la, lo): (f64, f64) = geod.direct(la0, lo0, *az, 10_000.0); (lo, la, 1.0) }).collect();
    let g = points(&e, dir.path(), "g", "EPSG:4326", &pts);
    let v = nums(&e, &tool(&e, "standard_distance", json!({}), &[("in", &g)]), "std_distance_m");
    assert!((v[0] - 10_000.0).abs() < 1.0, "{v:?}");
}

/// Lattice of (i * a, j * b) for i in -m..=m, j in -k..=k, turned so its long axis points at azimuth `az` degrees clockwise from north.
fn lattice(m: i32, k: i32, a: f64, b: f64, az: f64) -> Vec<(f64, f64)> {
    let (s, c) = (az.to_radians().sin(), az.to_radians().cos());
    let mut v = vec![];
    for i in -m..=m {
        for j in -k..=k {
            let (u, w) = (i as f64 * a, j as f64 * b); // u along the long axis, w across
            // east / north offsets: the long axis direction is (s, c) east/north; the cross axis is (c, -s).
            v.push((u * s + w * c, u * c - w * s));
        }
    }
    v
}

#[test]
fn t4_directional_distribution_recovers_a_known_axis_and_rotation() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Variance of integers -m..m is m(m+1)/3, so the standard deviations along the axes are a sqrt(m(m+1)/3) and b sqrt(k(k+1)/3).
    let (m, k, a, b, az) = (10, 3, 100.0, 80.0, 60.0);
    let (sa, sb) = (a * ((m * (m + 1)) as f64 / 3.0).sqrt(), b * ((k * (k + 1)) as f64 / 3.0).sqrt());
    let (want_major, want_minor) = (2f64.sqrt() * sa, 2f64.sqrt() * sb);
    // Projected.
    let pts: Vec<(f64, f64, f64)> = lattice(m, k, a, b, az).into_iter().map(|(x, y)| (X0 + x, Y0 + y, 1.0)).collect();
    let l = points(&e, dir.path(), "proj", "EPSG:32614", &pts);
    let r = tool(&e, "directional_distribution", json!({}), &[("in", &l)]);
    let v = nums(&e, &r, "semi_major_m, semi_minor_m, azimuth_deg, centre_x, centre_y, n");
    assert!((v[0] - want_major).abs() < 1e-6 * want_major, "{v:?} vs {want_major}");
    assert!((v[1] - want_minor).abs() < 1e-6 * want_minor, "{v:?} vs {want_minor}");
    assert!((v[2] - az).abs() < 1e-6, "{v:?}");
    assert!((v[3] - X0).abs() < 1e-6 && (v[4] - Y0).abs() < 1e-6 && v[5] == pts.len() as f64);
    // Two standard deviations double the axes.
    let r2 = tool(&e, "directional_distribution", json!({"sd": "2"}), &[("in", &l)]);
    assert!((nums(&e, &r2, "semi_major_m")[0] - 2.0 * want_major).abs() < 1e-5 * want_major);
    // Geographic: place the same lattice by true distance and azimuth from a centre; in the azimuthal equidistant projection used for the
    // ellipse these are exact, so the axes and azimuth match the same numbers.
    let geod = Geodesic::wgs84();
    let (lo0, la0) = (-100.0, 40.0);
    let gp: Vec<(f64, f64, f64)> = lattice(m, k, a, b, az)
        .into_iter()
        .map(|(x, y)| {
            let (r, bearing) = (x.hypot(y), x.atan2(y).to_degrees());
            let (la, lo): (f64, f64) = geod.direct(la0, lo0, bearing, r);
            (lo, la, 1.0)
        })
        .collect();
    let g = points(&e, dir.path(), "geo", "EPSG:4326", &gp);
    let r = tool(&e, "directional_distribution", json!({}), &[("in", &g)]);
    let v = nums(&e, &r, "semi_major_m, semi_minor_m, azimuth_deg");
    assert!((v[0] - want_major).abs() < 2e-3 * want_major, "{v:?} vs {want_major}");
    assert!((v[1] - want_minor).abs() < 2e-3 * want_minor, "{v:?} vs {want_minor}");
    assert!((v[2] - az).abs() < 0.05, "{v:?}");
    assert!(text(&e, &r, "plane").contains("azimuthal equidistant"));
    assert_eq!(r.crs.as_deref(), Some("EPSG:4326"));
    // Points on one line are refused in words.
    let line: Vec<(f64, f64, f64)> = (0..5).map(|i| (X0 + i as f64, Y0, 1.0)).collect();
    let bad = try_tool(&e, "directional_distribution", json!({}), &[("in", &points(&e, dir.path(), "line", "EPSG:32614", &line))]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("two directions"));
}

fn square(x0: f64, y0: f64, side: f64) -> String {
    format!("POLYGON(({x0:?} {y0:?}, {:?} {y0:?}, {:?} {:?}, {x0:?} {:?}, {x0:?} {y0:?}))", x0 + side, x0 + side, y0 + side, y0 + side)
}

#[test]
fn t4_average_nearest_neighbour_matches_the_clark_evans_arithmetic() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Four points at the corners of a 10 x 10 study area: every nearest neighbour is 10 away.
    // Expected = 0.5 / sqrt(4 / 100) = 2.5; ratio 4; SE = 0.26136 / sqrt(16 / 100) = 0.6534; z = 7.5 / 0.6534.
    let p = points(&e, dir.path(), "p", "EPSG:32614", &[(X0, Y0, 1.0), (X0 + 10.0, Y0, 1.0), (X0, Y0 + 10.0, 1.0), (X0 + 10.0, Y0 + 10.0, 1.0)]);
    let a = polygon(&e, dir.path(), "a", "EPSG:32614", &square(X0, Y0, 10.0));
    let r = tool(&e, "average_nearest_neighbour", json!({}), &[("in", &p), ("area", &a)]);
    let v = nums(&e, &r, "points, study_area_m2, observed_mean_distance_m, expected_mean_distance_m, nn_ratio, z_score");
    assert_eq!(v[0], 4.0);
    assert!((v[1] - 100.0).abs() < 1e-9 && (v[2] - 10.0).abs() < 1e-9 && (v[3] - 2.5).abs() < 1e-9 && (v[4] - 4.0).abs() < 1e-9, "{v:?}");
    assert!((v[5] - 7.5 / (0.26136 / (16.0f64 / 100.0).sqrt())).abs() < 1e-9, "{v:?}");
    assert_eq!(text(&e, &r, "pattern"), "dispersed");
    // The study area is required.
    assert!(matches!(try_tool(&e, "average_nearest_neighbour", json!({}), &[("in", &p)]).status, Status::Error));
    // Geographic: nearest neighbours by geodesic distance and the area on the ellipsoid.
    let geod = Geodesic::wgs84();
    let (lo0, la0) = (-100.0, 40.0);
    let pts: Vec<(f64, f64)> = [0.0, 90.0, 180.0, 270.0, 45.0].iter().map(|az| { let (la, lo): (f64, f64) = geod.direct(la0, lo0, *az, 5_000.0); (lo, la) }).collect();
    let mut sum = 0.0;
    for i in 0..pts.len() {
        sum += (0..pts.len()).filter(|j| *j != i).map(|j| -> f64 { geod.inverse(pts[i].1, pts[i].0, pts[j].1, pts[j].0) }).fold(f64::MAX, f64::min);
    }
    let g = points(&e, dir.path(), "g", "EPSG:4326", &pts.iter().map(|p| (p.0, p.1, 1.0)).collect::<Vec<_>>());
    // A 0.2 x 0.2 degree study area around the centre; its true area is about 0.2 * 85.4 km * 0.2 * 111.0 km.
    let ga = polygon(&e, dir.path(), "ga", "EPSG:4326", &square(lo0 - 0.1, la0 - 0.1, 0.2));
    let r = tool(&e, "average_nearest_neighbour", json!({}), &[("in", &g), ("area", &ga)]);
    let v = nums(&e, &r, "observed_mean_distance_m, study_area_m2");
    assert!((v[0] - sum / pts.len() as f64).abs() < 1e-6, "{v:?} vs {}", sum / pts.len() as f64);
    let want_area = 0.2f64.to_radians() * 6_378_137.0 * (la0.to_radians().cos()) * 0.2f64.to_radians() * 6_335_439.0;
    assert!((v[1] - want_area).abs() / want_area < 0.01, "{v:?} vs {want_area}");
}

/// Uniform points in [0, side) squared from a small generator that the tool does not use.
fn lcg_points(n: usize, side: f64, seed: u64) -> Vec<(f64, f64, f64)> {
    let mut s = seed;
    let mut next = move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 33) as f64) / (1u64 << 31) as f64
    };
    (0..n).map(|_| (X0 + next() * side, Y0 + next() * side, 1.0)).collect()
}

fn k_table(e: &Engine, r: &NodeResult) -> Vec<(f64, f64, String)> {
    let sql = format!("SELECT distance_m, k, verdict FROM \"{}\" ORDER BY distance_m", r.output.as_ref().unwrap());
    e.with_conn_for_tests(|c| {
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map([], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .unwrap()
}

#[test]
fn t4_ripleys_k_matches_hand_counts_and_separates_clustered_from_random_patterns() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let area = polygon(&e, dir.path(), "area", "EPSG:32614", &square(X0, Y0, 10.0));
    // Three points 1 and 2 apart on a line in a 10 x 10 area (no edge correction): K(d) = A / (n (n-1)) * ordered pairs within d.
    let p = points(&e, dir.path(), "p", "EPSG:32614", &[(X0 + 2.0, Y0 + 5.0, 1.0), (X0 + 3.0, Y0 + 5.0, 1.0), (X0 + 5.0, Y0 + 5.0, 1.0)]);
    let r = tool(&e, "ripleys_k", json!({"distances": "1, 2, 3", "edge": "none", "permutations": 0}), &[("in", &p), ("area", &area)]);
    let t = k_table(&e, &r);
    for (got, want) in t.iter().zip([100.0 / 6.0 * 2.0, 100.0 / 6.0 * 4.0, 100.0]) {
        assert!((got.1 - want).abs() < 1e-9, "{t:?}");
    }
    assert_eq!(text(&e, &r, "permutations"), "0");
    // Border correction on the same points: only centres at least d from the edge count. The distances to the edge are 2, 3 and 5, so at
    // d = 3 the points at x = 3 and x = 5 are centres (2 are used). Each has 2 neighbours within 3 (x = 3: the points at 1 and 2 away;
    // x = 5: those at 2 and 3 away), so K = 100 / ((3 - 1) * 2) * 4 = 100.
    let rb = tool(&e, "ripleys_k", json!({"distances": "3", "edge": "border", "permutations": 0}), &[("in", &p), ("area", &area)]);
    assert!((k_table(&e, &rb)[0].1 - 100.0).abs() < 1e-9, "{:?}", k_table(&e, &rb));
    // A bigger study area with a random and a clustered pattern, the same seed and permutations.
    let big = polygon(&e, dir.path(), "big", "EPSG:32614", &square(X0, Y0, 1000.0));
    let csr = points(&e, dir.path(), "csr", "EPSG:32614", &lcg_points(200, 1000.0, 42));
    let params = json!({"distances": "25, 50, 75, 100", "edge": "border", "permutations": 99, "seed": 7});
    let r1 = tool(&e, "ripleys_k", params.clone(), &[("in", &csr), ("area", &big)]);
    let t1 = k_table(&e, &r1);
    let flagged = t1.iter().filter(|x| x.2 != "consistent with random").count();
    assert!(flagged <= 1, "a random pattern should sit inside its envelope: {t1:?}");
    // Clustered: 10 clusters of 20 points within about 15 m of a centre.
    let centres = lcg_points(10, 800.0, 5);
    let mut cl = vec![];
    for (ci, c) in centres.iter().enumerate() {
        for (x, y, _) in lcg_points(20, 30.0, 100 + ci as u64) {
            cl.push((c.0 + 100.0 + (x - X0) - 15.0, c.1 + 100.0 + (y - Y0) - 15.0, 1.0));
        }
    }
    let cl_l = points(&e, dir.path(), "cl", "EPSG:32614", &cl);
    let r2 = tool(&e, "ripleys_k", params.clone(), &[("in", &cl_l), ("area", &big)]);
    let t2 = k_table(&e, &r2);
    assert!(t2.iter().all(|x| x.2 == "clustered"), "{t2:?}");
    // Same seed: identical table; different seed: the envelope differs (the observed K does not).
    let again = k_table(&e, &tool(&e, "ripleys_k", params.clone(), &[("in", &csr), ("area", &big)]));
    assert_eq!(t1, again);
    let sql = |r: &NodeResult| format!("SELECT k_sim_max FROM \"{}\" ORDER BY distance_m LIMIT 1", r.output.as_ref().unwrap());
    let hi = |r: &NodeResult| e.with_conn_for_tests(|c| Ok(c.query_row(&sql(r), [], |row| row.get::<_, f64>(0))?)).unwrap();
    let other = tool(&e, "ripleys_k", json!({"distances": "25, 50, 75, 100", "edge": "border", "permutations": 99, "seed": 8}), &[("in", &csr), ("area", &big)]);
    assert!(hi(&r1) != hi(&other));
    assert_eq!(text(&e, &other, "seed"), "8");
    // Refusals: degrees, empty distance list.
    let g = points(&e, dir.path(), "g", "EPSG:4326", &[(0.0, 0.0, 1.0), (0.1, 0.0, 1.0), (0.0, 0.1, 1.0)]);
    let ga = polygon(&e, dir.path(), "ga", "EPSG:4326", &square(-1.0, -1.0, 2.0));
    let bad = try_tool(&e, "ripleys_k", json!({"distances": "100"}), &[("in", &g), ("area", &ga)]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("Project"));
    assert!(matches!(try_tool(&e, "ripleys_k", json!({"distances": ""}), &[("in", &p), ("area", &area)]).status, Status::Error));
}

#[test]
fn t4_kernel_density_integrates_to_the_total_weight() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let p = points(&e, dir.path(), "p", "EPSG:32614", &[(X0 + 500.0, Y0 + 500.0, 1.0), (X0 + 900.0, Y0 + 650.0, 2.0), (X0 + 300.0, Y0 + 1000.0, 3.0)]);
    for (field, total) in [(json!(null), 3.0), (json!("w"), 6.0)] {
        let r = tool(&e, "kernel_density", json!({"weight": field, "bandwidth": 200, "cell": 10, "per": "km2"}), &[("in", &p)]);
        let ds = gdal::Dataset::open(r.output.as_deref().unwrap()).unwrap();
        let (w, h) = ds.raster_size();
        let data = ds.rasterband(1).unwrap().read_as::<f64>((0, 0), (w, h), (w, h), None).unwrap();
        // Weight per square kilometre times the cell area (100 m^2 = 1e-4 km^2) summed over the surface.
        let sum: f64 = data.data().iter().sum::<f64>() * 100.0 / 1.0e6;
        assert!((sum - total).abs() / total < 2e-3, "sum {sum} vs total {total}");
        // The surface extends one bandwidth beyond the points: 200 m on each side of the 600 m by 500 m extent.
        assert_eq!((w, h), (((600.0 + 400.0) / 10.0) as usize + 1, ((500.0 + 400.0) / 10.0) as usize + 1));
        assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    }
    // One point of weight 1: the peak is 3 / (pi h^2) per square metre at the point (u = 0), by the quartic kernel's formula.
    let one = points(&e, dir.path(), "one", "EPSG:32614", &[(X0 + 505.0, Y0 + 505.0, 1.0)]);
    let r = tool(&e, "kernel_density", json!({"bandwidth": 200, "cell": 10, "per": "m2"}), &[("in", &one)]);
    let ds = gdal::Dataset::open(r.output.as_deref().unwrap()).unwrap();
    let (w, h) = ds.raster_size();
    let max = ds.rasterband(1).unwrap().read_as::<f64>((0, 0), (w, h), (w, h), None).unwrap().data().iter().cloned().fold(0.0, f64::max);
    assert!((max - 3.0 / (std::f64::consts::PI * 200.0 * 200.0)).abs() / max < 1e-3, "{max}");
    // Geographic points and a zero bandwidth are refused.
    let g = points(&e, dir.path(), "g", "EPSG:4326", &[(0.0, 0.0, 1.0)]);
    let bad = try_tool(&e, "kernel_density", json!({"bandwidth": 200, "cell": 10}), &[("in", &g)]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("Project"));
    assert!(matches!(try_tool(&e, "kernel_density", json!({"bandwidth": 0, "cell": 10}), &[("in", &p)]).status, Status::Error));
}

#[test]
fn t4_tabulate_area_counts_cells_by_centre_and_uses_true_cell_areas() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // 10 x 10 cells of 10 m: columns 0-4 are class 1, columns 5-9 class 2. One cell has no data (class -1 = no data).
    let rows: Vec<Vec<f32>> = (0..10).map(|r| (0..10).map(|c| if (r, c) == (0, 6) { -1.0 } else if c < 5 { 1.0 } else { 2.0 }).collect()).collect();
    let cls = raster(&e, dir.path(), "cls", 32614, X0, Y0, 10.0, &rows, Some(-1.0), true);
    // Zone A covers x from 0 to 58 m: columns 0-5 by centre (the centre of column 5 is at 55 m). Zone B covers 62 to 100 m: columns 6-9.
    let zones = vlayer(&e, dir.path(), "zones", "EPSG:32614", &format!(
        "SELECT * FROM (VALUES ('A', ST_GeomFromText('{}')), ('B', ST_GeomFromText('{}'))) t(zone, geom)",
        format!("POLYGON(({X0:?} {:?}, {:?} {:?}, {:?} {Y0:?}, {X0:?} {Y0:?}, {X0:?} {:?}))", Y0 - 100.0, X0 + 58.0, Y0 - 100.0, X0 + 58.0, Y0 - 100.0),
        format!("POLYGON(({:?} {:?}, {:?} {:?}, {:?} {Y0:?}, {:?} {Y0:?}, {:?} {:?}))", X0 + 62.0, Y0 - 100.0, X0 + 100.0, Y0 - 100.0, X0 + 100.0, X0 + 62.0, X0 + 62.0, Y0 - 100.0)
    ));
    let r = tool(&e, "tabulate_area", json!({"zone_field": "zone"}), &[("zones", &zones), ("classes", &cls)]);
    let sql = format!("SELECT zone, class, cells, area_m2 FROM \"{}\" ORDER BY zone, class NULLS LAST", r.output.as_ref().unwrap());
    let rows: Vec<(String, Option<i64>, i64, f64)> = e
        .with_conn_for_tests(|c| {
            let mut st = c.prepare(&sql)?;
            let v = st.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        })
        .unwrap();
    // A: 5 columns of class 1 = 50 cells; column 5 = 10 cells of class 2 (one is column 5, no-data cell is in column 6, so not here).
    assert_eq!(rows[0], ("A".into(), Some(1), 50, 5000.0), "{rows:?}");
    assert_eq!(rows[1], ("A".into(), Some(2), 10, 1000.0), "{rows:?}");
    // B: columns 6-9 = 40 cells, one with no data: 39 of class 2 and 1 no-data row.
    assert_eq!(rows[2], ("B".into(), Some(2), 39, 3900.0), "{rows:?}");
    assert_eq!(rows[3], ("B".into(), None, 1, 100.0), "{rows:?}");
    assert!(r.message.as_deref().unwrap_or("").contains("no data"), "{:?}", r.message);
    // Geographic raster: a 1 x 1 degree cell at the equator is about 12 308.8 km^2 on WGS 84, and shrinks toward the pole.
    let eq = vec![vec![1.0f32, 1.0], vec![1.0, 1.0]];
    let geo = raster(&e, dir.path(), "geo", 4326, 0.0, 61.0, 1.0, &eq, None, true); // rows at latitudes 61-60 and 60-59
    let z = polygon(&e, dir.path(), "gz", "EPSG:4326", &square(0.0, 59.0, 2.0));
    let r = tool(&e, "tabulate_area", json!({"zone_field": "z"}), &[("zones", &z), ("classes", &geo)]);
    let v = nums(&e, &r, "area_m2");
    // Spherical check with the authalic radius, within 1 %: 4 cells of 1 degree between 59 and 61 north, 2 degrees wide.
    let rr = 6_371_007.0f64;
    let sph = rr * rr * 2.0f64.to_radians() * (61.0f64.to_radians().sin() - 59.0f64.to_radians().sin());
    assert!((v[0] - sph).abs() / sph < 0.01, "{v:?} vs {sph}");
    let eqr = raster(&e, dir.path(), "eq", 4326, 0.0, 1.0, 1.0, &[vec![1.0f32]], None, true);
    let ez = polygon(&e, dir.path(), "ez", "EPSG:4326", &square(0.0, 0.0, 1.0));
    let v = nums(&e, &tool(&e, "tabulate_area", json!({"zone_field": "z"}), &[("zones", &ez), ("classes", &eqr)]), "area_m2");
    assert!((v[0] / 1e6 - 12_308.8).abs() < 2.0, "{v:?}");
    // A decimal raster is refused in words.
    let fl = raster(&e, dir.path(), "fl", 32614, X0, Y0, 10.0, &rows_f(), None, false);
    assert!(matches!(try_tool(&e, "tabulate_area", json!({"zone_field": "zone"}), &[("zones", &zones), ("classes", &fl)]).status, Status::Error));
}

fn rows_f() -> Vec<Vec<f32>> {
    vec![vec![1.5; 10]; 10]
}

#[test]
fn t4_cut_and_fill_matches_a_hand_computed_block() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let before: Vec<Vec<f32>> = vec![vec![100.0; 4]; 4];
    // After: a 2 x 2 block raised by 3, three cells lowered by 2, one cell without data, the rest unchanged.
    let after: Vec<Vec<f32>> = (0..4).map(|r| (0..4).map(|c| match (r, c) { (0..=1, 0..=1) => 103.0, (3, 0..=2) => 98.0, (3, 3) => -9999.0, _ => 100.0 }).collect()).collect();
    let b = raster(&e, dir.path(), "before", 32614, X0, Y0, 10.0, &before, Some(-9999.0), false);
    let a = raster(&e, dir.path(), "after", 32614, X0, Y0, 10.0, &after, Some(-9999.0), false);
    let r = tool(&e, "cut_fill", json!({"z_unit": "m"}), &[("before", &b), ("after", &a)]);
    let v = nums(&e, &r, "fill_volume_m3, cut_volume_m3, net_volume_m3, fill_area_m2, cut_area_m2, unchanged_area_m2, cells_compared, cells_no_data");
    // Fill 4 cells * 3 m * 100 m^2 = 1200; cut 3 cells * 2 m * 100 = 600; net 600; areas 400, 300, 800 (8 unchanged cells); 15 compared, 1 no data.
    assert_eq!(v, vec![1200.0, 600.0, 600.0, 400.0, 300.0, 800.0, 15.0, 1.0]);
    assert!(r.message.as_deref().unwrap_or("").contains("1 of 16 cells have no data"), "{:?}", r.message);
    // Heights in feet: the same block gives volumes 0.3048 times as large.
    let rf = tool(&e, "cut_fill", json!({"z_unit": "ft"}), &[("before", &b), ("after", &a)]);
    assert!((nums(&e, &rf, "fill_volume_m3")[0] - 1200.0 * 0.3048).abs() < 1e-9);
    // Different grids are refused with the Align Raster sentence.
    let other = raster(&e, dir.path(), "other", 32614, X0 + 5.0, Y0, 10.0, &before, None, false);
    let bad = try_tool(&e, "cut_fill", json!({}), &[("before", &b), ("after", &other)]);
    assert!(matches!(bad.status, Status::Error) && bad.error.unwrap().contains("Align Raster"));
    // Geographic cells use the true area: 1 degree cells at the equator, raised 1 m, give 1 m times about 12 308.8 km^2.
    let gb = raster(&e, dir.path(), "gb", 4326, 0.0, 1.0, 1.0, &[vec![0.0f32]], None, false);
    let ga = raster(&e, dir.path(), "ga", 4326, 0.0, 1.0, 1.0, &[vec![1.0f32]], None, false);
    let v = nums(&e, &tool(&e, "cut_fill", json!({}), &[("before", &gb), ("after", &ga)]), "fill_volume_m3");
    assert!((v[0] / 1e6 - 12_308.8).abs() < 2.0, "{v:?}");
}

#[test]
fn t4_profile_samples_the_raster_at_equal_geodesic_spacing() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Plane z = 0.01 (x - X0) + 0.02 (y - Y0) in UTM, 10 m cells: bilinear interpolation reproduces a plane exactly.
    let dem: Vec<Vec<f32>> = (0..100).map(|r| (0..100).map(|c| (0.01 * (c as f64 * 10.0 + 5.0) - 0.02 * (r as f64 * 10.0 + 5.0)) as f32).collect()).collect();
    let d = raster(&e, dir.path(), "dem", 32614, X0, Y0, 10.0, &dem, Some(-9999.0), false);
    // A line from inside the raster off its east edge: some stations have no elevation.
    let line = vlayer(&e, dir.path(), "line", "EPSG:32614", &format!("SELECT 1 AS id, ST_GeomFromText('LINESTRING({} {}, {} {}, {} {})') AS geom", X0 + 100.0, Y0 - 800.0, X0 + 500.0, Y0 - 400.0, X0 + 1100.0, Y0 - 400.0));
    let r = tool(&e, "profile_line", json!({"spacing": 100}), &[("in", &line), ("dem", &d)]);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    let sql = format!("SELECT dist_m, elevation, status, ST_X(geom), ST_Y(geom), lon, lat FROM \"{}\" ORDER BY dist_m", r.output.as_ref().unwrap());
    let rows: Vec<(f64, Option<f64>, String, f64, f64, f64, f64)> = e
        .with_conn_for_tests(|c| {
            let mut st = c.prepare(&sql)?;
            Ok(st.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))?.collect::<Result<Vec<_>, _>>()?)
        })
        .unwrap();
    assert!(rows.len() > 10);
    // Stations are 100 m apart by geodesic distance (the last one is the end of the line), starting at 0.
    assert_eq!(rows[0].0, 0.0);
    for w in rows.windows(2).take(rows.len() - 2) {
        assert!((w[1].0 - w[0].0 - 100.0).abs() < 1e-9);
    }
    // Where there is a value it is the plane at the station's own coordinates; where the station is past the east edge it says so.
    let geod = Geodesic::wgs84();
    let mut inside = 0;
    for w in rows.windows(2).filter(|w| w[1].0 <= 565.0 || w[0].0 >= 566.0) {
        // Pairs on one straight segment (the corner is at 565.7 m): the geodesic between stations is the distance along the line.
        let d: f64 = geod.inverse(w[0].6, w[0].5, w[1].6, w[1].5);
        assert!((d - (w[1].0 - w[0].0)).abs() < 1e-3, "geodesic spacing {d} vs {}", w[1].0 - w[0].0);
    }
    for (_, z, st, x, y, _, _) in &rows {
        if let Some(z) = z {
            inside += 1;
            assert!((z - (0.01 * (x - X0) + 0.02 * (y - Y0))).abs() < 1e-3, "{z} at {x} {y}");
            assert_eq!(st, "ok");
        } else {
            assert_eq!(st, "outside the raster");
            assert!(*x >= X0 + 1000.0);
        }
    }
    assert!(inside > 5 && inside < rows.len());
    assert!(r.message.as_deref().unwrap_or("").contains("outside the raster"), "{:?}", r.message);
    // A geographic DEM with z = 1000 (lon - lon0): elevation at a station depends only on its longitude, which tests the explicit
    // line-to-DEM transform (the line is in UTM, the DEM in degrees) and the bilinear sampling together.
    let gdem: Vec<Vec<f32>> = (0..50).map(|_| (0..50).map(|c| (1000.0 * ((-100.0 + (c as f64 + 0.5) * 0.01) + 100.0)) as f32).collect()).collect();
    let gd = raster(&e, dir.path(), "gdem", 4326, -100.0, 40.5, 0.01, &gdem, None, false);
    // A point in UTM zone 14N near (-100, 40.25): the zone's central meridian is -99, so the line is in UTM but spans the DEM.
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    let (x, y): (f64, f64) = c.query_row("SELECT ST_X(p), ST_Y(p) FROM (SELECT ST_Transform(ST_Point(-99.9, 40.25), 'EPSG:4326', 'EPSG:32614', true) AS p)", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    let l2 = vlayer(&e, dir.path(), "l2", "EPSG:32614", &format!("SELECT 1 AS id, ST_GeomFromText('LINESTRING({x} {y}, {} {y})') AS geom", x + 20_000.0));
    let r = tool(&e, "profile_line", json!({"spacing": 2000, "method": "bilinear"}), &[("in", &l2), ("dem", &gd)]);
    let sql = format!("SELECT elevation, lon FROM \"{}\" WHERE status = 'ok'", r.output.as_ref().unwrap());
    let pairs: Vec<(f64, f64)> = e.with_conn_for_tests(|c| { let mut st = c.prepare(&sql)?; Ok(st.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<Vec<_>, _>>()?) }).unwrap();
    assert!(pairs.len() >= 10);
    for (z, lon) in pairs {
        assert!((z - 1000.0 * (lon + 100.0)).abs() < 0.01, "{z} at lon {lon}");
    }
    // Points are refused as input in words.
    let pts = points(&e, dir.path(), "pp", "EPSG:32614", &[(X0, Y0, 1.0)]);
    assert!(matches!(try_tool(&e, "profile_line", json!({}), &[("in", &pts), ("dem", &d)]).status, Status::Error));
}

#[test]
fn t4_every_tool_is_registered_with_category_description_and_keywords() {
    let e = Engine::new().unwrap();
    let specs = e.tool_specs();
    for id in ["kernel_density", "mean_centre", "standard_distance", "directional_distribution", "average_nearest_neighbour", "ripleys_k", "tabulate_area", "profile_line", "cut_fill"] {
        let s = specs.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} is not registered"));
        assert!(s.category.contains('›'), "{id}");
        assert!(s.description.len() > 80, "{id}");
        assert!(!engine::nodes::keywords::keywords(id).is_empty(), "{id}");
    }
}
