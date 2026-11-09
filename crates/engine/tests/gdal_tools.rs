//! GDAL utility nodes of Wave 1 package T3 (Proximity, Sieve), each checked against an answer worked out by hand.

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use serde_json::{json, Value};
use std::path::Path;

fn write(path: &Path, epsg: u32, cell: f64, ty: &str, rows: &[Vec<f32>]) {
    let (h, w) = (rows.len(), rows[0].len());
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let flat = rows.concat();
    let (x0, y0) = if epsg == 4326 { (-100.0, 40.0) } else { (600_000.0, 3_900_000.0) };
    macro_rules! mk {
        ($t:ty) => {{
            let mut ds = drv.create_with_band_type::<$t, _>(path, w, h, 1).unwrap();
            ds.set_geo_transform(&[x0, cell, 0.0, y0, 0.0, -cell]).unwrap();
            ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(epsg).unwrap()).unwrap();
            let mut b = ds.rasterband(1).unwrap();
            b.write((0, 0), (w, h), &mut gdal::raster::Buffer::new((w, h), flat.iter().map(|v| *v as $t).collect())).unwrap();
        }};
    }
    if ty == "u8" { mk!(u8) } else { mk!(f32) }
}

fn grid(rows: usize, cols: usize, f: impl Fn(usize, usize) -> f32) -> Vec<Vec<f32>> {
    (0..rows).map(|r| (0..cols).map(|c| f(r, c)).collect()).collect()
}

fn px(path: &str, col: isize, row: isize) -> f32 {
    gdal::Dataset::open(path).unwrap().rasterband(1).unwrap().read_as::<f32>((col, row), (1, 1), (1, 1), None).unwrap().data()[0]
}

fn layer(e: &Engine, dir: &Path, name: &str, epsg: u32, cell: f64, ty: &str, rows: &[Vec<f32>]) -> String {
    let p = dir.join(format!("{name}.tif"));
    write(&p, epsg, cell, ty, rows);
    e.import(&p).unwrap().remove(0).id
}

fn try_tool(e: &Engine, tool: &str, params: Value, layer: &str) -> NodeResult {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "i", "tool": "input_layer", "params": {"layer": layer}}, {"id": "t", "tool": tool, "params": params}],
        "edges": [{"source": "i", "target": "t", "targetHandle": "in"}]
    }))
    .unwrap();
    e.run_graph(&g, Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn tool(e: &Engine, name: &str, params: Value, layer: &str) -> NodeResult {
    let r = try_tool(e, name, params, layer);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{name}: {:?} {:?}", r.status, r.error);
    r
}

#[test]
fn t3_proximity_is_the_straight_line_distance_in_map_units() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Target at column 10, row 10 of 10 m cells: 3 columns east and 4 rows south is 5 cells = 50 m.
    let l = layer(&e, dir.path(), "p", 32614, 10.0, "f32", &grid(21, 21, |r, c| if (r, c) == (10, 10) { 1.0 } else { 0.0 }));
    let r = tool(&e, "proximity", json!({}), &l);
    let o = r.output.as_deref().unwrap();
    assert!((px(o, 13, 14) - 50.0).abs() < 1e-3, "{}", px(o, 13, 14));
    assert!((px(o, 20, 10) - 100.0).abs() < 1e-3);
    assert_eq!(px(o, 10, 10), 0.0);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // A 60 m limit leaves the far corner without data and says how many cells.
    let r = tool(&e, "proximity", json!({"max_distance": 60}), &l);
    assert_eq!(px(r.output.as_deref().unwrap(), 0, 0), -9999.0);
    assert!(r.message.as_deref().unwrap_or("").contains("no data"), "{:?}", r.message);
    // Target values: only cells equal to 2 count, so the nearest 2 is at (10, 14) from (10, 10): 40 m.
    let l2 = layer(&e, dir.path(), "p2", 32614, 10.0, "f32", &grid(21, 21, |r, c| match (r, c) { (10, 14) => 2.0, (10, 10) => 1.0, _ => 0.0 }));
    let r = tool(&e, "proximity", json!({"values": "2"}), &l2);
    assert!((px(r.output.as_deref().unwrap(), 10, 10) - 40.0).abs() < 1e-3);
    // A raster in degrees is refused with the project-first sentence; a bad list is refused too.
    let g = layer(&e, dir.path(), "g", 4326, 0.001, "f32", &grid(5, 5, |r, c| if (r, c) == (2, 2) { 1.0 } else { 0.0 }));
    let bad = try_tool(&e, "proximity", json!({}), &g);
    assert!(matches!(bad.status, Status::Error) && bad.error.as_deref().unwrap().contains("Project"), "{:?}", bad.error);
    assert!(matches!(try_tool(&e, "proximity", json!({"values": "a,b"}), &l).status, Status::Error));
}

#[test]
fn t3_sieve_removes_patches_smaller_than_the_minimum() {
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    // Class 1 everywhere; a 2-cell patch of class 2 at (2,2)-(2,3), a 6-cell patch of class 3 at rows 6-7, columns 6-8.
    let rows = grid(10, 10, |r, c| match (r, c) { (2, 2) | (2, 3) => 2.0, (6..=7, 6..=8) => 3.0, _ => 1.0 });
    let l = layer(&e, dir.path(), "s", 32614, 30.0, "u8", &rows);
    let r = tool(&e, "sieve", json!({"min_cells": 4}), &l);
    let o = r.output.as_deref().unwrap();
    assert_eq!(px(o, 2, 2), 1.0, "the 2-cell patch is merged");
    assert_eq!(px(o, 3, 2), 1.0);
    assert_eq!(px(o, 7, 6), 3.0, "the 6-cell patch stays");
    assert!(r.message.as_deref().unwrap_or("").contains("2 of 100 cells changed"), "{:?}", r.message);
    assert_eq!(r.crs.as_deref(), Some("EPSG:32614"));
    // Connectedness: two class-2 cells that touch only at a corner are two patches of 1 with 4 neighbours, one patch of 2 with 8.
    let diag = grid(10, 10, |r, c| if (r, c) == (4, 4) || (r, c) == (5, 5) { 2.0 } else { 1.0 });
    let l = layer(&e, dir.path(), "d", 32614, 30.0, "u8", &diag);
    let four = tool(&e, "sieve", json!({"min_cells": 2, "connectedness": "4"}), &l);
    assert_eq!(px(four.output.as_deref().unwrap(), 4, 4), 1.0);
    let eight = tool(&e, "sieve", json!({"min_cells": 2, "connectedness": "8"}), &l);
    assert_eq!(px(eight.output.as_deref().unwrap(), 4, 4), 2.0);
    // A floating-point raster and a zero minimum are refused in words.
    let f = layer(&e, dir.path(), "f", 32614, 30.0, "f32", &rows);
    assert!(matches!(try_tool(&e, "sieve", json!({}), &f).status, Status::Error));
    assert!(matches!(try_tool(&e, "sieve", json!({"min_cells": 0}), &l).status, Status::Error));
}
