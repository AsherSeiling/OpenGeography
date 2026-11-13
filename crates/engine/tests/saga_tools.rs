//! SAGA nodes of package T2 (valley bottom flatness, TPI landforms, solar radiation, channel network). Each test checks an answer that follows
//! from the shape of a synthetic surface, not only that the tool ran. They skip, saying so, when `saga_cmd` is not installed (set GIS_STUDIO_SAGA).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn have() -> bool {
    let ok = engine::saga::available();
    if !ok {
        eprintln!("SKIPPED: SAGA is not installed (set GIS_STUDIO_SAGA)");
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


fn srs_code(path: &str) -> Option<String> {
    gdal::Dataset::open(path).unwrap().spatial_ref().ok().and_then(|s| s.auth_code().ok().map(|c| c.to_string()))
}

#[test]
fn t2_mrvbf_is_high_on_a_flat_valley_floor_and_zero_on_steep_walls() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // 201 x 201 cells of 10 m: a flat floor 40 columns wide in the middle, walls rising 3 m per cell (30 %, above the 16 % threshold).
    let dem = grid(201, 201, |_, c| { let d = (c as f32 - 100.0).abs(); if d <= 20.0 { 0.0 } else { 3.0 * (d - 20.0) } });
    let l = layer(&e, dir.path(), "v", 32614, 10.0, &dem);
    let r = tool(&e, "saga_mrvbf", json!({}), &[("in", &l)]);
    let (floor, wall) = (px(out(&r), 100, 100), px(out(&r), 175, 100));
    assert!(floor >= 1.0, "valley floor MRVBF {floor}");
    assert!(wall < 0.01, "steep wall MRVBF {wall}");
    assert_eq!(srs_code(out(&r)).as_deref(), Some("32614"));
    // Ridge top flatness of the same surface is 0 on the floor.
    let rt = tool(&e, "saga_mrvbf", json!({"result": "mrrtf"}), &[("in", &l)]);
    assert!(px(out(&rt), 100, 100) < 0.01);
}

#[test]
fn t2_mrvbf_refuses_a_geographic_dem() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = layer(&e, dir.path(), "g", 4326, 0.001, &grid(30, 30, |r, c| (r + c) as f32));
    let r = try_tool(&e, "saga_mrvbf", json!({}), &[("in", &l)]);
    assert!(matches!(r.status, Status::Error), "{:?}", r.status);
    assert!(r.error.unwrap().contains("degrees"));
}

#[test]
fn t2_tpi_landforms_give_plains_on_flat_land_and_a_top_class_on_the_summit() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // One round hill (height 100 m, sigma 300 m = 30 cells) on flat ground; 10 m cells.
    let dem = grid(201, 201, |r, c| { let (dy, dx) = (r as f32 - 100.0, c as f32 - 100.0); 100.0 * (-(dx * dx + dy * dy) / (2.0 * 30.0 * 30.0)).exp() });
    let l = layer(&e, dir.path(), "h", 32614, 10.0, &dem);
    let r = tool(&e, "saga_tpi_landforms", json!({}), &[("in", &l)]);
    assert_eq!(px(out(&r), 195, 195), 5.0, "flat corner should be class 5 (plains)");
    let top = px(out(&r), 100, 100);
    assert!(top >= 8.0, "summit class {top} (8 to 10 are ridges and tops)");
    // A radius order that cannot work is refused in words.
    let bad = try_tool(&e, "saga_tpi_landforms", json!({"small_min": 200, "small_max": 100}), &[("in", &l)]);
    assert!(matches!(bad.status, Status::Error));
    assert!(bad.error.unwrap().contains("inner radius"));
}

#[test]
fn t2_solar_radiation_follows_the_sun_slope_and_aspect() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // 60 x 60 cells of 30 m at about 35 degrees north (UTM 14N, y = 3.9 million m). Flat, then a 20 % slope up to the north (faces south),
    // then a 20 % slope up to the south (faces north).
    let flat = layer(&e, dir.path(), "flat", 32614, 30.0, &grid(60, 60, |_, _| 500.0));
    let south = layer(&e, dir.path(), "sfacing", 32614, 30.0, &grid(60, 60, |r, _| 500.0 + 6.0 * (59 - r) as f32));
    let north = layer(&e, dir.path(), "nfacing", 32614, 30.0, &grid(60, 60, |r, _| 500.0 + 6.0 * r as f32));
    let rf = tool(&e, "saga_solar_radiation", json!({"first_day": 172, "last_day": 172}), &[("in", &flat)]);
    let summer = px(out(&rf), 30, 30);
    // A clear June day at 35 N delivers roughly 7 to 10 kWh/m2 on a flat surface; the lumped 70 % atmosphere puts it inside that band.
    assert!((6.0..=11.0).contains(&summer), "flat 21 June total {summer} kWh/m2");
    assert!(rf.message.as_deref().is_some_and(|m| m.contains("latitude 35")), "{:?}", rf.message);
    let w = |l: &str| px(out(&tool(&e, "saga_solar_radiation", json!({"first_day": 355, "last_day": 355}), &[("in", l)])), 30, 30);
    let (wf, ws, wn) = (w(&flat), w(&south), w(&north));
    assert!(ws > wf && wf > wn, "on 21 December a south-facing slope ({ws}) beats flat ({wf}) beats north-facing ({wn})");
    assert!(wf < summer, "winter {wf} is below summer {summer}");
    assert_eq!(srs_code(out(&rf)).as_deref(), Some("32614"));
    // Direct plus diffuse is the total.
    let d = tool(&e, "saga_solar_radiation", json!({"first_day": 172, "last_day": 172, "result": "direct"}), &[("in", &flat)]);
    let f = tool(&e, "saga_solar_radiation", json!({"first_day": 172, "last_day": 172, "result": "diffuse"}), &[("in", &flat)]);
    assert!((px(out(&d), 30, 30) + px(out(&f), 30, 30) - summer).abs() < 0.01 * summer);
}

#[test]
fn t2_solar_radiation_refuses_a_bad_day_range() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let l = layer(&e, dir.path(), "f", 32614, 30.0, &grid(20, 20, |_, _| 10.0));
    let r = try_tool(&e, "saga_solar_radiation", json!({"first_day": 200, "last_day": 100}), &[("in", &l)]);
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.unwrap().contains("first day"));
}

#[test]
fn t2_channel_network_marks_the_valley_axis_and_nothing_on_the_walls() {
    if !have() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // A V-shaped valley draining south: the axis is column 50; cells on the walls are not channel cells.
    let dem = grid(101, 101, |r, c| 2.0 * (c as f32 - 50.0).abs() + 0.5 * (100 - r) as f32 + 0.0001 * (c as f32));
    let l = layer(&e, dir.path(), "ch", 32614, 10.0, &dem);
    let r = tool(&e, "saga_channel_network", json!({"threshold": 2}), &[("in", &l)]);
    let axis = px(out(&r), 50, 60);
    assert!(axis >= 1.0, "axis order {axis}");
    let ds = gdal::Dataset::open(out(&r)).unwrap();
    let nd = ds.rasterband(1).unwrap().no_data_value();
    let wall = px(out(&r), 5, 60);
    assert!(nd.is_some_and(|n| (wall as f64 - n).abs() < 1.0) || wall <= 0.0, "wall cell {wall} should have no data");
    assert_eq!(srs_code(out(&r)).as_deref(), Some("32614"));
    // A threshold nothing reaches is explained, not passed on as SAGA's own message.
    let none = try_tool(&e, "saga_channel_network", json!({"threshold": 60}), &[("in", &l)]);
    assert!(matches!(none.status, Status::Error));
    assert!(none.error.unwrap().contains("no channels"));
    // Works in degrees too (it counts cells only).
    let g = layer(&e, dir.path(), "chg", 4326, 0.001, &dem);
    tool(&e, "saga_channel_network", json!({"threshold": 2}), &[("in", &g)]);
}
