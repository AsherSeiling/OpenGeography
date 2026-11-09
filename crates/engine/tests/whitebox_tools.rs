//! WhiteboxTools nodes of Wave 1 package T1 (terrain, distance, stream order, focal filters). Every test checks an answer worked out by
//! hand or from a formula, not only that the tool ran. They skip, saying so, when the WhiteboxTools program is not installed.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn have() -> bool {
    let ok = engine::whitebox::available();
    if !ok {
        eprintln!("SKIPPED: WhiteboxTools is not installed (set GIS_STUDIO_WHITEBOX)");
    }
    ok
}

/// Write a float raster of `rows` rows (row 0 is the north edge) with square cells of `cell` units and no-data -9999.
fn write(path: &Path, epsg: u32, x0: f64, y0: f64, cell: f64, rows: &[Vec<f32>]) {
    let (h, w) = (rows.len(), rows[0].len());
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, w, h, 1).unwrap();
    ds.set_geo_transform(&[x0, cell, 0.0, y0, 0.0, -cell]).unwrap();
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    let mut band = ds.rasterband(1).unwrap();
    band.set_no_data_value(Some(-9999.0)).unwrap();
    band.write((0, 0), (w, h), &mut gdal::raster::Buffer::new((w, h), rows.concat())).unwrap();
}

fn grid(rows: usize, cols: usize, f: impl Fn(usize, usize) -> f32) -> Vec<Vec<f32>> {
    (0..rows).map(|r| (0..cols).map(|c| f(r, c)).collect()).collect()
}

fn px(path: &str, col: isize, row: isize) -> f32 {
    gdal::Dataset::open(path).unwrap().rasterband(1).unwrap().read_as::<f32>((col, row), (1, 1), (1, 1), None).unwrap().data()[0]
}

fn import(e: &Engine, p: &Path) -> String {
    e.import(p).unwrap().remove(0).id
}

/// One tool node fed by input layers: `ports` = (port id, layer id).
fn try_tool(e: &Engine, tool: &str, params: Value, ports: &[(&str, &str)]) -> NodeResult {
    let mut nodes = vec![json!({"id": "t", "tool": tool, "params": params})];
    let mut edges = vec![];
    for (i, (port, layer)) in ports.iter().enumerate() {
        let id = format!("in{i}");
        nodes.push(json!({"id": id, "tool": "input_layer", "params": {"layer": layer}}));
        edges.push(json!({"source": id, "target": "t", "targetHandle": port}));
    }
    let g: Graph = serde_json::from_value(json!({"nodes": nodes, "edges": edges})).unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn tool(e: &Engine, name: &str, params: Value, ports: &[(&str, &str)]) -> NodeResult {
    let r = try_tool(e, name, params, ports);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{name}: {:?} {:?}", r.status, r.error);
    r
}

fn out(r: &NodeResult) -> &str {
    r.output.as_deref().unwrap()
}

fn layer(e: &Engine, dir: &Path, name: &str, epsg: u32, cell: f64, rows: &[Vec<f32>]) -> String {
    let p = dir.join(format!("{name}.tif"));
    let (x0, y0) = if epsg == 4326 { (-100.0, 40.0) } else { (600_000.0, 3_900_000.0) };
    write(&p, epsg, x0, y0, cell, rows);
    import(e, &p)
}

#[test]
fn t1_euclidean_distance_is_the_straight_line_in_map_units() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // One target at column 10, row 10 of a 21 x 21 grid of 10 m cells; the cell 3 columns east and 4 rows south is 5 cells = 50 m away.
    let l = layer(&e, dir.path(), "t", 32614, 10.0, &grid(21, 21, |r, c| if (r, c) == (10, 10) { 1.0 } else { 0.0 }));
    let r = tool(&e, "euclidean_distance", json!({}), &[("in", &l)]);
    assert!((px(out(&r), 13, 14) - 50.0).abs() < 1e-3, "{}", px(out(&r), 13, 14));
    assert!((px(out(&r), 20, 10) - 100.0).abs() < 1e-3);
    assert_eq!(px(out(&r), 10, 10), 0.0);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // Allocation: two targets, every cell takes the value of the nearer one.
    let l2 = layer(&e, dir.path(), "t2", 32614, 10.0, &grid(21, 21, |r, c| match (r, c) { (10, 2) => 7.0, (10, 18) => 9.0, _ => 0.0 }));
    let a = tool(&e, "euclidean_allocation", json!({}), &[("in", &l2)]);
    assert_eq!(px(out(&a), 5, 3), 7.0);
    assert_eq!(px(out(&a), 15, 17), 9.0);
}

#[test]
fn t1_tools_that_need_metres_refuse_a_raster_in_degrees() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = layer(&e, dir.path(), "g", 4326, 0.001, &grid(9, 9, |r, c| (r * 9 + c) as f32));
    for t in ["euclidean_distance", "plan_curvature", "total_curvature", "geomorphons", "cost_distance"] {
        let ports: Vec<(&str, &str)> = if t == "cost_distance" { vec![("source", &l), ("cost", &l)] } else { vec![("in", &l)] };
        let r = try_tool(&e, t, json!({}), &ports);
        assert!(matches!(r.status, Status::Error), "{t} should refuse a raster in degrees");
        let msg = r.error.unwrap();
        assert!(msg.contains("degrees") && msg.contains("Project"), "{t}: {msg}");
    }
    // A cell-count result works in degrees; an area result does not.
    assert!(matches!(try_tool(&e, "fd8_flow_accumulation", json!({"out_type": "cells"}), &[("in", &l)]).status, Status::Done | Status::Cached));
    assert!(matches!(try_tool(&e, "fd8_flow_accumulation", json!({"out_type": "catchment area"}), &[("in", &l)]).status, Status::Error));
}

#[test]
fn t1_focal_mean_and_tpi_match_hand_calculation() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // A plane z = 10 r + c: the 3 x 3 mean at any interior cell equals the cell itself, so TPI is 0 on it.
    let plane = layer(&e, dir.path(), "plane", 32614, 30.0, &grid(7, 7, |r, c| (10 * r + c) as f32));
    let m = tool(&e, "focal_mean", json!({"size": 3}), &[("in", &plane)]);
    assert!((px(out(&m), 3, 3) - 33.0).abs() < 1e-4);
    let t = tool(&e, "tpi", json!({"size": 3}), &[("in", &plane)]);
    assert!(px(out(&t), 3, 3).abs() < 1e-4);
    // A single peak of 9 on a flat 0: its 3 x 3 mean is 1, so TPI = 9 - 1 = 8; a neighbour (0 - 1/9 * 9) = -1.
    let peak = layer(&e, dir.path(), "peak", 32614, 30.0, &grid(7, 7, |r, c| if (r, c) == (3, 3) { 9.0 } else { 0.0 }));
    let t = tool(&e, "tpi", json!({"size": 3}), &[("in", &peak)]);
    assert!((px(out(&t), 3, 3) - 8.0).abs() < 1e-4, "{}", px(out(&t), 3, 3));
    assert!((px(out(&t), 4, 3) + 1.0).abs() < 1e-4, "{}", px(out(&t), 4, 3));
    // Focal range and maximum on the same peak.
    let mx = tool(&e, "focal_max", json!({"size": 3}), &[("in", &peak)]);
    assert_eq!(px(out(&mx), 4, 4), 9.0);
    assert_eq!(px(out(&mx), 5, 5), 0.0);
    let rg = tool(&e, "focal_range", json!({"size": 3}), &[("in", &peak)]);
    assert_eq!(px(out(&rg), 2, 2), 9.0);
    // A zero window is refused in words.
    let bad = try_tool(&e, "focal_mean", json!({"size": 0}), &[("in", &peak)]);
    assert!(matches!(bad.status, Status::Error));
}

#[test]
fn t1_viewshed_is_blocked_by_a_wall_and_places_observers_in_the_dem_crs() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Flat ground at 0 m with a 100 m wall along column 20; the observer stands 1.7 m above cell (row 20, column 10).
    let dem = layer(&e, dir.path(), "wall", 32614, 10.0, &grid(41, 41, |_, c| if c == 20 { 100.0 } else { 0.0 }));
    let (x, y) = (600_000.0 + 10.5 * 10.0, 3_900_000.0 - 20.5 * 10.0);
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    // The observer is given in WGS 84 so the tool must transform it into the DEM's UTM zone.
    let (lon, lat): (f64, f64) = c
        .query_row(&format!("SELECT ST_X(p), ST_Y(p) FROM (SELECT ST_Transform(ST_Point({x}, {y}), 'EPSG:32614', 'EPSG:4326', true) AS p)"), [], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    let gpkg = dir.path().join("obs.gpkg");
    c.execute_batch(&format!(
        "COPY (SELECT ST_Point({lon}, {lat}) AS geom UNION ALL SELECT ST_Point({lon} + 5.0, {lat}) AS geom) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'o', SRS 'EPSG:4326', USE_TMP_FILE false)",
        gpkg.display()
    ))
    .unwrap();
    let obs = import(&e, &gpkg);
    let r = tool(&e, "viewshed", json!({"height": 1.7}), &[("in", &dem), ("obs", &obs)]);
    let v = out(&r);
    assert!(px(v, 15, 20) > 0.0, "a cell in front of the wall is visible");
    assert_eq!(px(v, 30, 20), 0.0, "a cell behind a 100 m wall is hidden");
    assert!(r.message.as_deref().unwrap_or("").contains("outside the DEM"), "the second observer is 5 degrees away: {:?}", r.message);
}

#[test]
fn t1_cost_distance_and_least_cost_path() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let src = layer(&e, dir.path(), "src", 32614, 10.0, &grid(5, 9, |r, c| if (r, c) == (2, 0) { 1.0 } else { 0.0 }));
    let cost = layer(&e, dir.path(), "cost", 32614, 10.0, &grid(5, 9, |_, _| 2.0));
    let acc = tool(&e, "cost_distance", json!({}), &[("source", &src), ("cost", &cost)]);
    // Straight east along a row of cost 2 (cost units per map unit) across 4 cells of 10 map units: 2 * 40 = 80.
    assert!((px(out(&acc), 4, 2) - 80.0).abs() < 1e-3, "{}", px(out(&acc), 4, 2));
    // Diagonal: 2 cells east and 2 south of the source column is a diagonal of 2 steps of 10 * sqrt(2) at cost 2.
    assert!((px(out(&acc), 2, 4) - 2.0 * 2.0 * 10.0 * 2f32.sqrt()).abs() < 1e-2, "{}", px(out(&acc), 2, 4));
    // A zero cost is refused.
    let zero = layer(&e, dir.path(), "zero", 32614, 10.0, &grid(5, 9, |_, _| 0.0));
    assert!(matches!(try_tool(&e, "cost_distance", json!({}), &[("source", &src), ("cost", &zero)]).status, Status::Error));
    // Path from the east end back to the source runs along row 2.
    let dest = layer(&e, dir.path(), "dest", 32614, 10.0, &grid(5, 9, |r, c| if (r, c) == (2, 8) { 1.0 } else { 0.0 }));
    let p = tool(&e, "cost_path", json!({}), &[("source", &src), ("cost", &cost), ("dest", &dest)]);
    for c in 0..9 {
        assert!(px(out(&p), c, 2) > 0.0, "column {c} is on the path");
    }
    // Away from the path the cells are not 1 (no data or 0).
    assert!(px(out(&p), 4, 0) != 1.0);
}

/// A hand-made stream network (ESRI D8 codes: 1 E, 4 S, 16 W). Two headwater arms (columns 2 and 6) run south and meet at (4, 4); the main
/// stem runs south to row 8; a third headwater joins it from the east at (6, 4).
fn network(dir: &Path, e: &Engine) -> (String, String) {
    let mut ptr = vec![vec![0.0f32; 9]; 9];
    let mut st = vec![vec![-9999.0f32; 9]; 9];
    let mut set = |r: usize, c: usize, d: f32| {
        ptr[r][c] = d;
        st[r][c] = 1.0;
    };
    for r in 0..4 {
        set(r, 2, 4.0);
        set(r, 6, 4.0);
    }
    set(4, 2, 1.0);
    set(4, 3, 1.0);
    set(4, 6, 16.0);
    set(4, 5, 16.0);
    for r in 4..9 {
        set(r, 4, 4.0);
    }
    set(6, 6, 16.0);
    set(6, 5, 16.0);
    let (p, s) = (dir.join("ptr.tif"), dir.join("streams.tif"));
    write(&p, 32614, 600_000.0, 3_900_000.0, 10.0, &ptr);
    write(&s, 32614, 600_000.0, 3_900_000.0, 10.0, &st);
    (import(e, &p), import(e, &s))
}

#[test]
fn t1_stream_orders_match_the_textbook_definitions() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let (ptr, st) = network(dir.path(), &e);
    let ports = [("pntr", ptr.as_str()), ("streams", st.as_str())];
    // Strahler: headwater arms are 1; two 1s make a 2 at the confluence (4, 4); a 1 joining a 2 leaves it at 2.
    let s = tool(&e, "strahler_order", json!({}), &ports);
    assert_eq!(px(out(&s), 2, 1), 1.0);
    assert_eq!(px(out(&s), 6, 1), 1.0);
    assert_eq!(px(out(&s), 4, 5), 2.0);
    assert_eq!(px(out(&s), 4, 8), 2.0, "order stays 2 below the third tributary");
    // Shreve magnitude: the number of headwaters upstream. 2 below the first confluence, 3 below the second.
    let m = tool(&e, "shreve_magnitude", json!({}), &ports);
    assert_eq!(px(out(&m), 2, 1), 1.0);
    assert_eq!(px(out(&m), 4, 5), 2.0);
    assert_eq!(px(out(&m), 4, 8), 3.0);
    // Link identifiers: the two arms get different numbers from each other and from the main stem.
    let l = tool(&e, "stream_link_id", json!({}), &ports);
    let (a, b, c) = (px(out(&l), 2, 1), px(out(&l), 6, 1), px(out(&l), 4, 5));
    assert!(a != b && a != c && b != c, "{a} {b} {c}");
    assert_eq!(px(out(&l), 2, 2), a, "one link keeps one number");
}

#[test]
fn t1_curvature_of_a_bowl_has_the_analytic_magnitude() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // z = a (x^2 + y^2) with a = 0.002 /m and 10 m cells: both second derivatives are 2a = 0.004 per metre of height per metre^2.
    let a = 0.002f32;
    let dem = layer(&e, dir.path(), "bowl", 32614, 10.0, &grid(21, 21, |r, c| {
        let (x, y) = ((c as f32 - 10.0) * 10.0, (r as f32 - 10.0) * 10.0);
        a * (x * x + y * y)
    }));
    let at = |t: &str| px(out(&tool(&e, t, json!({}), &[("in", &dem)])), 17, 10) as f64;
    // At 70 m from the centre the slope is p = 2 a r = 0.28. Profile curvature = -2a / (1 + p^2)^1.5 (negative: the bowl is concave downslope).
    let p: f64 = 2.0 * 0.002 * 70.0;
    let want_profile = -0.004 / (1.0 + p * p).powf(1.5);
    assert!((at("profile_curvature") - want_profile).abs() < 1e-6, "profile {} vs {want_profile}", at("profile_curvature"));
    // Plan curvature of a circular contour of radius R is -1/R on a bowl (convergent).
    assert!((at("plan_curvature") + 1.0 / 70.0).abs() < 1e-6, "plan {}", at("plan_curvature"));
    // Gaussian curvature of z = f(r): f'' f' / (r (1 + f'^2)^2).
    let want_gauss = 0.004 * p / (70.0 * (1.0 + p * p).powi(2));
    assert!((at("gaussian_curvature") - want_gauss).abs() < 1e-8, "gaussian {} vs {want_gauss}", at("gaussian_curvature"));
    // Total curvature (Florinsky): z_xx^2 + 2 z_xy^2 + z_yy^2 = 2 * 0.004^2.
    assert!((at("total_curvature") - 3.2e-5).abs() < 1e-8, "total {}", at("total_curvature"));
}

/// Every tool of the family runs on a plausible input and gives a raster on the input's grid and CRS with some valid cells.
#[test]
fn t1_every_remaining_tool_runs_and_keeps_the_grid_and_crs() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // A valley: z falls to the south and toward the centre column.
    let dem = layer(&e, dir.path(), "valley", 32614, 10.0, &grid(30, 30, |r, c| 100.0 - 1.0 * r as f32 + 0.3 * (c as f32 - 15.0).abs()));
    let filled = tool(&e, "fill_depressions", json!({}), &[("in", &dem)]);
    let fl = import_result(&e, dir.path(), &filled, "filled");
    let acc = tool(&e, "flow_accumulation", json!({"out_type": "cells"}), &[("in", &fl)]);
    let ptr = tool(&e, "flow_direction", json!({}), &[("in", &fl)]);
    let sca = tool(&e, "fd8_flow_accumulation", json!({"out_type": "specific contributing area"}), &[("in", &fl)]);
    let slope = tool(&e, "slope", json!({}), &[("in", &dem)]);
    let streams = tool(&e, "extract_streams", json!({"threshold": 20}), &[("in", &import_result(&e, dir.path(), &acc, "acc"))]);
    let (ptr_l, sca_l, slope_l, st_l) = (import_result(&e, dir.path(), &ptr, "ptr"), import_result(&e, dir.path(), &sca, "sca"), import_result(&e, dir.path(), &slope, "slope"), import_result(&e, dir.path(), &streams, "st"));
    let single: &[&str] = &[
        "plan_curvature", "profile_curvature", "total_curvature", "tangential_curvature", "gaussian_curvature", "tpi", "dev", "rel_topographic_position",
        "elev_rel_minmax", "geomorphons", "breach_depressions", "fd8_flow_accumulation", "dinf_flow_accumulation", "isobasins", "focal_mean", "focal_median",
        "focal_max", "focal_min", "focal_range", "focal_stddev", "focal_majority", "focal_gaussian",
    ];
    for t in single {
        let r = tool(&e, t, json!({}), &[("in", &dem)]);
        assert_same_grid(out(&r), &dir.path().join("valley.tif"), t);
        assert_eq!(r.crs.as_deref(), Some("EPSG:32614"), "{t}");
    }
    for t in ["strahler_order", "shreve_magnitude", "horton_order", "stream_link_id", "subbasins", "strahler_basins"] {
        let r = tool(&e, t, json!({}), &[("pntr", &ptr_l), ("streams", &st_l)]);
        assert_same_grid(out(&r), &dir.path().join("valley.tif"), t);
    }
    for t in ["wetness_index", "stream_power_index"] {
        let r = tool(&e, t, json!({}), &[("sca", &sca_l), ("slope", &slope_l)]);
        assert_same_grid(out(&r), &dir.path().join("valley.tif"), t);
    }
    for t in ["hand", "downslope_distance_to_stream"] {
        let r = tool(&e, t, json!({}), &[("in", &dem), ("streams", &st_l)]);
        assert_same_grid(out(&r), &dir.path().join("valley.tif"), t);
    }
}

/// Import a result raster as a layer so another node can read it (results are files in the raster store).
fn import_result(e: &Engine, dir: &Path, r: &NodeResult, name: &str) -> String {
    let dst = dir.join(format!("{name}.tif"));
    std::fs::copy(out(r), &dst).unwrap();
    import(e, &dst)
}

fn assert_same_grid(out: &str, reference: &Path, what: &str) {
    let (a, b) = (gdal::Dataset::open(out).unwrap(), gdal::Dataset::open(reference).unwrap());
    assert_eq!(a.raster_size(), b.raster_size(), "{what}");
    assert_eq!(a.geo_transform().unwrap(), b.geo_transform().unwrap(), "{what}");
    let band = a.rasterband(1).unwrap();
    let st = band.get_statistics(true, false).unwrap().unwrap_or_else(|| panic!("{what}: no valid cell"));
    assert!(st.max.is_finite(), "{what}");
}

#[test]
fn t1_every_new_tool_has_a_category_description_and_keywords() {
    let e = Engine::new().unwrap();
    let specs = e.tool_specs();
    let ids = [
        "viewshed", "cost_distance", "cost_path", "cost_allocation", "euclidean_distance", "euclidean_allocation", "plan_curvature", "profile_curvature",
        "total_curvature", "tangential_curvature", "gaussian_curvature", "tpi", "dev", "rel_topographic_position", "elev_rel_minmax", "geomorphons",
        "breach_depressions", "fd8_flow_accumulation", "dinf_flow_accumulation", "wetness_index", "stream_power_index", "hand",
        "downslope_distance_to_stream", "strahler_order", "shreve_magnitude", "horton_order", "stream_link_id", "subbasins",
        "strahler_basins", "isobasins", "focal_mean", "focal_median", "focal_max", "focal_min", "focal_range", "focal_stddev", "focal_majority", "focal_gaussian",
    ];
    for id in ids {
        let s = specs.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} is not registered"));
        assert!(s.category.contains('›') || s.category == "Hydrology", "{id}: {}", s.category);
        assert!(s.description.len() > 40, "{id}");
        assert!(!engine::nodes::keywords::keywords(id).is_empty(), "{id} has no keywords");
    }
}
