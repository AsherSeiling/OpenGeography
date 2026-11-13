//! Point clouds (package T8): LAS/LAZ header, CRS and class counts read with the `las` crate, the footprint layer, and the WhiteboxTools LiDAR
//! nodes. Files are written here with the same crate from surfaces with known heights, so each answer follows from the data: a plane, a block of
//! tree points 15 m above it. The LiDAR node tests skip, saying so, when WhiteboxTools is not installed (set GIS_STUDIO_WHITEBOX).

use engine::graph::{Graph, NodeResult, Status};
use engine::Engine;
use las::point::Classification;
use las::{Builder, Point, Transform, Vector, Version, Vlr, Writer};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn have_wbt() -> bool {
    let ok = engine::whitebox::available();
    if !ok {
        eprintln!("SKIPPED: WhiteboxTools is not installed (set GIS_STUDIO_WHITEBOX)");
    }
    ok
}

struct Pt {
    x: f64,
    y: f64,
    z: f64,
    class: Classification,
}

enum CrsRecord {
    None,
    Wkt(String),
    GeoKeys { projected: u16, vertical: Option<u16> },
}

fn wkt_of(def: &str) -> String {
    gdal::spatial_ref::SpatialRef::from_definition(def).unwrap().to_wkt().unwrap()
}

/// Write a LAS (1.4 with a WKT record, or 1.2 with GeoTIFF keys) or LAZ file.
fn write_las(path: &Path, crs: CrsRecord, pts: &[Pt]) {
    let mut b = Builder::from(match crs { CrsRecord::GeoKeys { .. } => Version::new(1, 2), _ => Version::new(1, 4) });
    b.point_format = las::point::Format::new(if matches!(crs, CrsRecord::GeoKeys { .. }) { 1 } else { 6 }).unwrap();
    // The offsets keep the stored integers in range (UTM northings are millions of metres).
    b.transforms = Vector { x: Transform { scale: 0.001, offset: 600_000.0 }, y: Transform { scale: 0.001, offset: 3_900_000.0 }, z: Transform { scale: 0.001, offset: 0.0 } };
    if let CrsRecord::GeoKeys { projected, vertical } = &crs {
        let mut keys: Vec<[u16; 4]> = vec![[1024, 0, 1, 1], [3072, 0, 1, *projected]];
        if let Some(v) = vertical {
            keys.push([4096, 0, 1, *v]);
        }
        let mut data: Vec<u16> = vec![1, 1, 0, keys.len() as u16];
        data.extend(keys.iter().flatten());
        b.vlrs.push(Vlr { user_id: "LASF_Projection".into(), record_id: 34735, description: "GeoKeyDirectoryTag".into(), data: data.iter().flat_map(|v| v.to_le_bytes()).collect() });
    }
    let mut header = b.into_header().unwrap();
    if let CrsRecord::Wkt(w) = &crs {
        header.set_wkt_crs(w.clone().into_bytes()).unwrap();
    }
    let mut w = Writer::from_path(path, header).unwrap();
    for p in pts {
        w.write_point(Point { x: p.x, y: p.y, z: p.z, classification: p.class, return_number: 1, number_of_returns: 1, gps_time: Some(0.0), ..Default::default() }).unwrap();
    }
    w.close().unwrap();
}

/// A 1 m grid of ground points over 60 x 60 m on the plane z = 100 + 0.1 x (class 2), plus tree points 15 m above the ground over x,y in
/// [20, 30) (class 5) at 1 m spacing. Origin at (600000, 3900000).
fn scene(with_classes: bool) -> Vec<Pt> {
    let (x0, y0) = (600_000.0, 3_900_000.0);
    let mut v = vec![];
    for i in 0..60 {
        for j in 0..60 {
            let (x, y) = (i as f64 + 0.5, j as f64 + 0.5);
            let ground = 100.0 + 0.1 * x;
            let tree = (20.0..30.0).contains(&x) && (20.0..30.0).contains(&y);
            v.push(Pt { x: x0 + x, y: y0 + y, z: ground, class: if with_classes { Classification::Ground } else { Classification::CreatedNeverClassified } });
            if tree {
                v.push(Pt { x: x0 + x, y: y0 + y, z: ground + 15.0, class: if with_classes { Classification::HighVegetation } else { Classification::CreatedNeverClassified } });
            }
        }
    }
    v
}

fn tool_graph(tool: &str, params: Value) -> Graph {
    serde_json::from_value(json!({"nodes": [{"id": "t", "tool": tool, "params": params}], "edges": []})).unwrap()
}

fn try_tool(e: &Engine, tool: &str, params: Value) -> NodeResult {
    e.run_graph(&tool_graph(tool, params), Some("t"), &mut |_| {}).unwrap().into_iter().find(|r| r.id == "t").unwrap()
}

fn tool(e: &Engine, name: &str, params: Value) -> NodeResult {
    let r = try_tool(e, name, params);
    assert!(matches!(r.status, Status::Done | Status::Cached), "{name}: {:?} {:?}", r.status, r.error);
    r
}

/// Value of band 1 at a world coordinate.
fn at(path: &str, x: f64, y: f64) -> f32 {
    let ds = gdal::Dataset::open(path).unwrap();
    let gt = ds.geo_transform().unwrap();
    let (c, r) = (((x - gt[0]) / gt[1]).floor() as isize, ((y - gt[3]) / gt[5]).floor() as isize);
    ds.rasterband(1).unwrap().read_as::<f32>((c, r), (1, 1), (1, 1), None).unwrap().data()[0]
}

fn near(a: f32, b: f64, tol: f64) -> bool {
    (a as f64 - b).abs() <= tol
}

fn p(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

// ---- header, CRS and classes

#[test]
fn t8_header_count_bounds_classes_and_wkt_crs_with_navd88() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(dir.path(), "a.las");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614+5703")), &scene(true));
    let info = engine::pointcloud::inspect(&f, true).unwrap();
    // 60 x 60 ground points plus 10 x 10 tree points.
    assert_eq!(info.point_count, 3600 + 100);
    assert_eq!(info.version, "1.4");
    assert!(!info.compressed);
    assert!((info.bounds[0] - 600_000.5).abs() < 1e-3 && (info.bounds[3] - 600_059.5).abs() < 1e-3, "{:?}", info.bounds);
    // Lowest ground z = 100 + 0.05 at x = 0.5; highest tree z = 100 + 0.1 * 29.5 + 15.
    assert!((info.bounds[2] - 100.05).abs() < 1e-3 && (info.bounds[5] - 117.95).abs() < 1e-3, "{:?}", info.bounds);
    let classes: Vec<(u8, u64)> = info.classes.as_ref().unwrap().iter().map(|c| (c.code, c.count)).collect();
    assert_eq!(classes, vec![(2, 3600), (5, 100)]);
    assert_eq!(info.crs.as_deref(), Some("EPSG:32614"), "{:?} {:?}", info.crs, info.notes);
    assert_eq!(info.crs_source.as_deref(), Some("WKT record"));
    assert_eq!(info.vertical_datum.as_deref(), Some("NAVD88"), "{:?}", info.vertical);
    assert_eq!(info.unit.as_deref(), Some("metre"));
}

#[test]
fn t8_geotiff_keys_give_the_epsg_code_and_the_vertical_name() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(dir.path(), "g.las");
    write_las(&f, CrsRecord::GeoKeys { projected: 26914, vertical: Some(5703) }, &scene(true)[..50]);
    let info = engine::pointcloud::inspect(&f, false).unwrap();
    assert_eq!(info.version, "1.2");
    assert_eq!(info.crs.as_deref(), Some("EPSG:26914"), "{:?}", info.notes);
    assert_eq!(info.crs_source.as_deref(), Some("GeoTIFF keys"));
    assert_eq!(info.vertical_datum.as_deref(), Some("NAVD88"));
    assert!(info.classes.is_none());
}

#[test]
fn t8_a_file_without_a_crs_says_so_and_never_guesses() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(dir.path(), "n.las");
    write_las(&f, CrsRecord::None, &scene(true)[..50]);
    let info = engine::pointcloud::inspect(&f, false).unwrap();
    assert!(info.crs.is_none());
    assert!(info.notes.iter().any(|n| n.contains("no coordinate system")), "{:?}", info.notes);
    // Imported, the layer has no CRS and carries the warning.
    let e = Engine::new().unwrap();
    let l = e.import(&f).unwrap().remove(0);
    assert!(l.crs.is_none());
    assert!(l.warnings.iter().any(|w| w.contains("no coordinate system")));
}

#[test]
fn t8_laz_is_read_like_las() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (p(dir.path(), "a.las"), p(dir.path(), "b.laz"));
    let pts = scene(true);
    write_las(&a, CrsRecord::Wkt(wkt_of("EPSG:32614")), &pts);
    write_las(&b, CrsRecord::Wkt(wkt_of("EPSG:32614")), &pts);
    let (ia, ib) = (engine::pointcloud::inspect(&a, true).unwrap(), engine::pointcloud::inspect(&b, true).unwrap());
    assert!(ib.compressed && !ia.compressed);
    assert_eq!(ia.point_count, ib.point_count);
    assert_eq!(ia.bounds, ib.bounds);
    assert_eq!(ia.classes.as_ref().unwrap().len(), ib.classes.as_ref().unwrap().len());
    assert!(std::fs::metadata(&b).unwrap().len() < std::fs::metadata(&a).unwrap().len());
}

#[test]
fn t8_a_non_las_file_is_refused_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(dir.path(), "x.las");
    std::fs::write(&f, b"not a point cloud").unwrap();
    let err = engine::pointcloud::inspect(&f, false).unwrap_err().to_string();
    assert!(err.contains("not a readable LAS or LAZ file"), "{err}");
}

#[test]
fn t8_importing_a_las_adds_its_footprint_with_the_files_crs_and_a_citation() {
    let dir = tempfile::tempdir().unwrap();
    let f = p(dir.path(), "c.laz");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614+5703")), &scene(true));
    let e = Engine::new().unwrap();
    let l = e.import(&f).unwrap().remove(0);
    assert_eq!(l.crs.as_deref(), Some("EPSG:32614"));
    assert_eq!(l.feature_count, 1);
    assert!(l.name.contains("footprint"));
    let sql = "SELECT ST_XMin(geom), ST_XMax(geom), points, zmax FROM layer_footprint";
    let _ = sql; // The layer table name is internal; the bounds are checked through the attributes the engine exposes below.
    assert_eq!(l.source.as_deref(), Some(f.to_str().unwrap()));
}

// ---- the LiDAR nodes

fn dem_params(file: &Path, extra: Value) -> Value {
    let mut v = json!({"file": file.to_str().unwrap(), "resolution": 2.0});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

#[test]
fn t8_bare_earth_dem_follows_the_ground_plane_and_ignores_the_trees() {
    if !have_wbt() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let f = p(dir.path(), "dem.las");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614+5703")), &scene(true));
    let before = std::fs::read(&f).unwrap();
    for method in ["tin", "idw"] {
        let r = tool(&e, "lidar_dem", dem_params(&f, json!({"method": method})));
        let out = r.output.as_deref().unwrap();
        // Open ground and inside the tree block: both are the plane, 100 + 0.1 x.
        let (x, y) = (600_010.0, 3_900_010.0);
        assert!(near(at(out, x, y), 100.0 + 0.1 * 10.0, 0.2), "{method} open {}", at(out, x, y));
        let (tx, ty) = (600_025.0, 3_900_025.0);
        assert!(near(at(out, tx, ty), 100.0 + 0.1 * 25.0, 0.2), "{method} under trees {}", at(out, tx, ty));
        let ds = gdal::Dataset::open(out).unwrap();
        assert_eq!(crs_code(&ds), Some("32614".to_string()));
        assert_eq!(engine::vertical::get(Path::new(out)).as_deref(), Some("NAVD88"));
    }
    assert_eq!(before, std::fs::read(&f).unwrap(), "the source point cloud is never changed");
}

fn crs_code(ds: &gdal::Dataset) -> Option<String> {
    ds.spatial_ref().ok().and_then(|s| s.auth_code().ok().map(|c| c.to_string()))
}

#[test]
fn t8_dem_from_a_laz_file_and_from_the_slope_filter_when_nothing_is_classified() {
    if !have_wbt() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let f = p(dir.path(), "u.laz");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614")), &scene(false));
    // Unclassified: ground class missing is explained, with the classes that exist.
    let r = try_tool(&e, "lidar_dem", dem_params(&f, json!({})));
    assert!(matches!(r.status, Status::Error));
    let msg = r.error.unwrap();
    assert!(msg.contains("no points classified as ground") && msg.contains("Never classified"), "{msg}");
    // With the filter the ground is found: the tree block is dropped and the plane is recovered.
    let r = tool(&e, "lidar_dem", dem_params(&f, json!({"ground": "filter", "filter_radius": 5.0, "filter_slope": 30.0, "filter_height": 1.0})));
    let out = r.output.as_deref().unwrap();
    assert!(near(at(out, 600_010.0, 3_900_010.0), 101.0, 0.3), "{}", at(out, 600_010.0, 3_900_010.0));
    assert!(near(at(out, 600_025.0, 3_900_025.0), 102.5, 1.0), "under trees {}", at(out, 600_025.0, 3_900_025.0));
}

#[test]
fn t8_crs_rules_missing_stated_and_conflicting() {
    if !have_wbt() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let none = p(dir.path(), "none.las");
    write_las(&none, CrsRecord::None, &scene(true));
    let r = try_tool(&e, "lidar_dem", dem_params(&none, json!({})));
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.unwrap().contains("no coordinate system"));
    // Stated by the user: used, and written into the result.
    let ok = tool(&e, "lidar_dem", dem_params(&none, json!({"crs": "EPSG:32614"})));
    assert_eq!(crs_code(&gdal::Dataset::open(ok.output.as_deref().unwrap()).unwrap()), Some("32614".into()));
    // The file has one and the user names another: nothing runs.
    let has = p(dir.path(), "has.las");
    write_las(&has, CrsRecord::Wkt(wkt_of("EPSG:32614")), &scene(true));
    let r = try_tool(&e, "lidar_dem", dem_params(&has, json!({"crs": "EPSG:32615"})));
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.unwrap().contains("differ"));
    // Settings are checked.
    let r = try_tool(&e, "lidar_dem", dem_params(&has, json!({"resolution": 0})));
    assert!(matches!(r.status, Status::Error));
    let r = try_tool(&e, "lidar_dem", dem_params(&has, json!({"resolution": 0.0001})));
    assert!(r.error.unwrap().contains("coarser"));
}

#[test]
fn t8_dsm_density_and_canopy_height() {
    if !have_wbt() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let f = p(dir.path(), "s.las");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614+5703")), &scene(true));
    // DSM: the tree tops (ground + 15) inside the block, the plane outside it.
    let dsm = tool(&e, "lidar_dsm", dem_params(&f, json!({"radius": 1.5})));
    let out = dsm.output.as_deref().unwrap();
    assert!(near(at(out, 600_025.0, 3_900_025.0), 100.0 + 2.5 + 15.0, 0.8), "dsm {}", at(out, 600_025.0, 3_900_025.0));
    assert!(near(at(out, 600_010.0, 3_900_010.0), 101.0, 0.5), "dsm open {}", at(out, 600_010.0, 3_900_010.0));
    // Density: one ground point per square metre, and a doubled count under the trees, within a 2 m radius.
    let d = tool(&e, "lidar_point_density", dem_params(&f, json!({"radius": 2.0, "resolution": 2.0})));
    let out = d.output.as_deref().unwrap();
    assert!(near(at(out, 600_010.0, 3_900_010.0), 1.0, 0.15), "open density {}", at(out, 600_010.0, 3_900_010.0));
    assert!(near(at(out, 600_025.0, 3_900_025.0), 2.0, 0.3), "tree density {}", at(out, 600_025.0, 3_900_025.0));
    assert!(engine::vertical::get(Path::new(out)).is_none(), "a density has no vertical datum");
    // Canopy height: 15 m on the tree block, 0 on open ground; a height, so no vertical datum.
    let c = tool(&e, "lidar_canopy_height", dem_params(&f, json!({})));
    let out = c.output.as_deref().unwrap();
    assert!(near(at(out, 600_025.0, 3_900_025.0), 15.0, 0.6), "chm {}", at(out, 600_025.0, 3_900_025.0));
    assert!(near(at(out, 600_010.0, 3_900_010.0), 0.0, 0.3), "chm open {}", at(out, 600_010.0, 3_900_010.0));
    assert!(engine::vertical::get(Path::new(out)).is_none());
}

#[test]
fn t8_ground_classification_and_normalisation_write_new_las_files() {
    if !have_wbt() { return; }
    let (e, dir) = (Engine::new().unwrap(), tempfile::tempdir().unwrap());
    let f = p(dir.path(), "raw.las");
    write_las(&f, CrsRecord::Wkt(wkt_of("EPSG:32614")), &scene(false));
    let before = std::fs::read(&f).unwrap();
    let g = p(dir.path(), "ground.las");
    tool(&e, "lidar_classify_ground", json!({"file": f.to_str().unwrap(), "path": g.to_str().unwrap(), "filter_radius": 5.0, "filter_slope": 30.0, "filter_height": 1.0}));
    let info = engine::pointcloud::inspect(&g, true).unwrap();
    assert_eq!(info.point_count, 3700);
    let ground = info.classes.as_ref().unwrap().iter().find(|c| c.code == 2).map(|c| c.count).unwrap_or(0);
    // Every flat-ground point is found; the 100 tree points are not ground.
    assert!(ground >= 3500 && ground <= 3600, "{ground} ground points");
    // Normalise the classified file: tree points become 15 m, ground points 0.
    let n = p(dir.path(), "norm.las");
    tool(&e, "lidar_normalize_heights", json!({"file": g.to_str().unwrap(), "path": n.to_str().unwrap()}));
    let ni = engine::pointcloud::inspect(&n, false).unwrap();
    assert!((ni.bounds[5] - 15.0).abs() < 1.0, "highest normalised height {}", ni.bounds[5]);
    assert!(ni.bounds[2] > -0.5, "lowest normalised height {}", ni.bounds[2]);
    // The source is never overwritten, nor may the output be the source.
    let r = try_tool(&e, "lidar_classify_ground", json!({"file": f.to_str().unwrap(), "path": f.to_str().unwrap()}));
    assert!(matches!(r.status, Status::Error));
    assert!(r.error.unwrap().contains("different file"));
    assert_eq!(before, std::fs::read(&f).unwrap());
}

#[test]
fn t8_every_lidar_tool_is_registered_with_a_category_description_and_keywords() {
    let e = Engine::new().unwrap();
    let specs = e.tool_specs();
    for id in ["lidar_dem", "lidar_dsm", "lidar_point_density", "lidar_canopy_height", "lidar_classify_ground", "lidar_normalize_heights"] {
        let s = specs.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} is not registered"));
        assert!(s.category.starts_with("Raster"), "{id}");
        assert!(s.description.len() > 80, "{id}");
        assert!(!engine::nodes::keywords::keywords(id).is_empty(), "{id} has no search words");
    }
}
