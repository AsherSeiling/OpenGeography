//! CRS and rasters (docs/CRS_PLAN.md R5): sampling across CRSs, warping an analytic plane, terrain on a geographic DEM, edge cases.
//! Cases R-G2, R-G4, R-G5, R-G10. Expected values come from the plane's own formula (RD-1).

use engine::graph::{Graph, Status};
use engine::Engine;
use gdal::spatial_ref::SpatialRef;
use serde_json::json;
use std::path::Path;

/// A Float32 GeoTIFF whose cell (col, row) holds `f(x, y)` at the cell centre; `crs` is an EPSG code (0 = none).
fn plane(path: &Path, epsg: u32, gt: [f64; 6], cols: usize, rows: usize, f: &dyn Fn(f64, f64) -> f32) {
    let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
    let mut ds = drv.create_with_band_type::<f32, _>(path, cols, rows, 1).unwrap();
    ds.set_geo_transform(&gt).unwrap();
    if epsg != 0 {
        ds.set_spatial_ref(&SpatialRef::from_epsg(epsg).unwrap()).unwrap();
    }
    let data: Vec<f32> = (0..cols * rows)
        .map(|i| {
            let (c, r) = ((i % cols) as f64, (i / cols) as f64);
            f(gt[0] + (c + 0.5) * gt[1], gt[3] + (r + 0.5) * gt[5])
        })
        .collect();
    let mut buf = gdal::raster::Buffer::new((cols, rows), data);
    ds.rasterband(1).unwrap().write((0, 0), (cols, rows), &mut buf).unwrap();
}

fn run(e: &Engine, tool: &str, layer: &str, params: serde_json::Value) -> Result<engine::layers::LayerInfo, String> {
    let g: Graph = serde_json::from_value(json!({
        "nodes": [{"id": "in", "tool": "input_layer", "params": {"layer": layer}}, {"id": "t", "tool": tool, "params": params}, {"id": "o", "tool": "layer_output", "params": {"layer": layer}}],
        "edges": [{"source": "in", "target": "t", "targetHandle": "in"}, {"source": "t", "target": "o", "targetHandle": "in"}]
    }))
    .unwrap();
    let res = e.run_graph(&g, None, &mut |_| {}).map_err(|e| format!("{e:#}"))?;
    let t = res.iter().find(|r| r.id == "t").unwrap();
    if t.status != Status::Done && t.status != Status::Cached {
        return Err(t.error.clone().unwrap_or_else(|| format!("{:?}", t.status)));
    }
    res.iter().find(|r| r.id == "o").and_then(|o| o.layer.clone()).ok_or_else(|| "no result layer".to_string())
}

const UTM15_GT: [f64; 6] = [400_000.0, 30.0, 0.0, 4_003_000.0, 0.0, -30.0];
fn utm_plane(x: f64, y: f64) -> f32 {
    (0.0001 * (x - 400_000.0) + 0.0002 * (y - 4_000_000.0)) as f32
}

#[test]
fn r_g4_sampling_a_utm_raster_from_lon_lat_equals_the_native_cell() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("utm.tif");
    plane(&p, 26915, UTM15_GT, 100, 100, &utm_plane);
    let e = Engine::new().unwrap();
    let l = e.import(&p).unwrap().remove(0);
    let mut checked = 0;
    for (c, r) in [(5, 5), (50, 50), (94, 10), (10, 90), (99, 99)] {
        let (x, y) = (UTM15_GT[0] + (c as f64 + 0.5) * 30.0, UTM15_GT[3] - (r as f64 + 0.5) * 30.0);
        let (lon, lat) = e.transform_point(x, y, "EPSG:26915", "EPSG:4326").unwrap();
        let v = e.raster_value(&l.id, lon, lat).unwrap().unwrap_or_else(|| panic!("no value at cell ({c}, {r})"));
        assert!((v - utm_plane(x, y) as f64).abs() < 0.05, "cell ({c}, {r}): {v} vs {}", utm_plane(x, y));
        checked += 1;
    }
    assert_eq!(checked, 5);
    // Outside the raster: no value, not the nearest edge cell (B-T6).
    let (lon, lat) = e.transform_point(399_000.0, 4_001_000.0, "EPSG:26915", "EPSG:4326").unwrap();
    assert_eq!(e.raster_value(&l.id, lon, lat).unwrap(), None);
    let (lon, lat) = e.transform_point(500_000.0, 4_001_000.0, "EPSG:26915", "EPSG:4326").unwrap();
    assert_eq!(e.raster_value(&l.id, lon, lat).unwrap(), None);
}

#[test]
fn r_g2_warping_a_plane_keeps_its_values_in_every_target_crs() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("utm.tif");
    plane(&p, 26915, UTM15_GT, 100, 100, &utm_plane);
    let e = Engine::new().unwrap();
    let src = e.import(&p).unwrap().remove(0);
    let mut bad = Vec::new();
    for target in ["EPSG:4326", "EPSG:3857", "EPSG:5070", "EPSG:32615"] {
        let out = run(&e, "warp", &src.id, json!({ "crs": target, "resampling": "bilinear" })).unwrap_or_else(|err| panic!("warp to {target}: {err}"));
        assert!(engine::crs::same(out.crs.as_deref().expect("a CRS"), target), "warp to {target} came out as {:?}", out.crs);
        let mut n = 0;
        // 25 interior points of the source, as lon/lat; the warped raster must give the plane's value there (within a warped cell).
        for i in 1..6 {
            for j in 1..6 {
                let (x, y) = (400_000.0 + 3000.0 * i as f64 / 6.0, 4_000_000.0 + 3000.0 * j as f64 / 6.0);
                let (lon, lat) = e.transform_point(x, y, "EPSG:26915", "EPSG:4326").unwrap();
                match e.raster_value(&out.id, lon, lat).unwrap() {
                    Some(v) if (v - utm_plane(x, y) as f64).abs() < 0.06 => n += 1,
                    other => bad.push(format!("{target} at ({x}, {y}): got {other:?}, plane {}", utm_plane(x, y))),
                }
            }
        }
        assert_eq!(n, 25, "{target}");
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Geographic DEM, 0.0003° cells at latitude 45: a plane tilted 30° towards `towards` ("north" or "east").
fn tilted_dem(path: &Path, towards: &str) {
    let lat0 = 45.0f64;
    let per_lat = 111_132.954 - 559.822 * (2.0 * lat0.to_radians()).cos();
    let per_lon = 111_412.84 * lat0.to_radians().cos() - 93.5 * (3.0 * lat0.to_radians()).cos();
    let t = 30.0f64.to_radians().tan();
    let towards = towards.to_string();
    plane(path, 4326, [-100.0, 0.0003, 0.0, 45.015, 0.0, -0.0003], 100, 100, &move |lon, lat| {
        let z = if towards == "north" { t * (lat - 45.0) * per_lat } else { t * (lon + 100.0) * per_lon };
        z as f32
    });
}

#[test]
fn r_g5_slope_on_a_geographic_dem_is_30_degrees_whichever_way_the_plane_faces() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    for towards in ["north", "east"] {
        let p = dir.path().join(format!("{towards}.tif"));
        tilted_dem(&p, towards);
        let dem = e.import(&p).unwrap().remove(0);
        let slope = run(&e, "slope", &dem.id, json!({ "units": "degrees" })).unwrap();
        // The result keeps the DEM's degrees georeferencing.
        assert!(engine::crs::same(slope.crs.as_deref().unwrap(), "EPSG:4326"));
        let v = e.raster_value(&slope.id, -99.985, 45.0).unwrap().expect("a slope value at the centre");
        assert!((v - 30.0).abs() < 0.5, "a plane tilted 30° towards the {towards} reads as {v}°");
        // Aspect: north-facing slope descends... the value is a bearing; it must be defined and finite.
        let aspect = run(&e, "aspect", &dem.id, json!({})).unwrap();
        let a = e.raster_value(&aspect.id, -99.985, 45.0).unwrap().expect("an aspect");
        assert!(a.is_finite() && (0.0..=360.0).contains(&a), "{a}");
        // North-facing plane rises towards the north, so it faces south (180°); east-facing rises east, faces west (270°).
        let want = if towards == "north" { 180.0 } else { 270.0 };
        assert!((a - want).abs() < 2.0, "aspect of a plane that rises towards the {towards} is {a}°, expected {want}°");
    }
}

#[test]
fn r_g5_a_projected_dem_gives_the_same_slope_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("p.tif");
    // 30 m cells; rises 17.32 m per 30 m east = 30°.
    plane(&p, 26915, UTM15_GT, 100, 100, &|x, _| ((x - 400_000.0) * 30.0f64.to_radians().tan()) as f32);
    let e = Engine::new().unwrap();
    let dem = e.import(&p).unwrap().remove(0);
    let slope = run(&e, "slope", &dem.id, json!({ "units": "degrees" })).unwrap();
    let (lon, lat) = e.transform_point(401_500.0, 4_001_500.0, "EPSG:26915", "EPSG:4326").unwrap();
    let v = e.raster_value(&slope.id, lon, lat).unwrap().expect("a value");
    assert!((v - 30.0).abs() < 0.05, "{v}");
}

#[test]
fn r_g10_a_raster_without_a_crs_is_refused_in_words_and_all_nodata_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let p = dir.path().join("nocrs.tif");
    plane(&p, 0, UTM15_GT, 50, 50, &utm_plane);
    let r = e.import(&p).unwrap().remove(0);
    assert!(r.crs.is_none());
    let err = run(&e, "warp", &r.id, json!({ "crs": "EPSG:4326" })).unwrap_err();
    assert!(err.to_lowercase().contains("coordinate system"), "{err}");

    // A raster that is all NoData: terrain runs or says why; never a crash.
    let p2 = dir.path().join("nodata.tif");
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff").unwrap();
        let mut ds = drv.create_with_band_type::<f32, _>(&p2, 20, 20, 1).unwrap();
        ds.set_geo_transform(&UTM15_GT).unwrap();
        ds.set_spatial_ref(&SpatialRef::from_epsg(26915).unwrap()).unwrap();
        let mut band = ds.rasterband(1).unwrap();
        band.set_no_data_value(Some(-9999.0)).unwrap();
        let mut buf = gdal::raster::Buffer::new((20, 20), vec![-9999.0f32; 400]);
        band.write((0, 0), (20, 20), &mut buf).unwrap();
    }
    let n = e.import(&p2).unwrap().remove(0);
    match run(&e, "slope", &n.id, json!({ "units": "degrees" })) {
        Ok(s) => {
            let (lon, lat) = e.transform_point(400_300.0, 4_002_700.0, "EPSG:26915", "EPSG:4326").unwrap();
            assert_eq!(e.raster_value(&s.id, lon, lat).unwrap(), None, "no data stays no data");
        }
        Err(msg) => assert!(!msg.is_empty()),
    }
}

// ---------------------------------------------------------------- P18: identify and select at the poles (bug G4)

#[test]
fn p18_identify_and_rectangle_select_work_at_the_south_pole() {
    let e = std::sync::Arc::new(Engine::new().unwrap());
    let mut props = serde_json::Map::new();
    props.insert("k".into(), json!(1));
    let layer = e
        .create_features("pole points", &[("POINT(0 -90)".to_string(), props.clone()), ("POINT(45 -89.5)".to_string(), props.clone()), ("POINT(10 -60)".to_string(), props)])
        .unwrap();
    let crs = "EPSG:3031";
    e.crs_display_info(crs, None).unwrap();
    e.build_display_blocking(&layer.id, crs).unwrap();
    // (45°, −89.5°) in EPSG:3031 is about 54 km from the pole (0.5° of latitude, 55.7 km, times the projection's scale at the pole), at 45°.
    let (nx, ny) = e.transform_point(45.0, -89.5, "EPSG:4326", crs).unwrap();
    assert!((nx - ny).abs() < 1e-6 && nx > 0.0 && (50_000.0..56_000.0).contains(&nx.hypot(ny)), "{nx} {ny}");
    let at = |x: f64, y: f64, tol: f64| e.identify(&layer.id, x, y, crs, tol).unwrap();
    let pole = at(0.0, 0.0, 500.0);
    assert_eq!(pole.len(), 1, "the point at the pole: {pole:?}");
    let near = at(nx, ny, 500.0);
    assert_eq!(near.len(), 1, "the point 55 km from the pole: {near:?}");
    assert_ne!(pole[0], near[0]);
    assert!(at(1_000_000.0, 1_000_000.0, 500.0).is_empty(), "nothing out there");
    // Rectangle around the pole: both polar points, not the one at 60°S.
    let boxed = e.select_rect(&layer.id, [-100_000.0, -100_000.0, 100_000.0, 100_000.0], crs).unwrap();
    assert_eq!(boxed.len(), 2, "{boxed:?}");
    // A definition query still limits what is found.
    e.set_definition_query(&layer.id, Some("k = 2")).unwrap();
    assert!(at(0.0, 0.0, 500.0).is_empty());
    assert!(e.select_rect(&layer.id, [-100_000.0, -100_000.0, 100_000.0, 100_000.0], crs).unwrap().is_empty());
}

#[test]
fn r_g1_a_raster_keeps_its_modified_definition_instead_of_the_authority_code() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    // Plain EPSG:26915 is recognised by its code.
    let a = dir.path().join("plain.tif");
    plane(&a, 26915, UTM15_GT, 20, 20, &utm_plane);
    let plain = e.import(&a).unwrap().remove(0);
    assert_eq!(plain.crs.as_deref(), Some("EPSG:26915"));
    // The same CRS with a false easting changed but the EPSG authority node left in place. A GeoTIFF would keep only the code, so the
    // definition is carried by a VRT, which stores the WKT as written.
    let b = dir.path().join("modified.vrt");
    let wkt = SpatialRef::from_epsg(26915).unwrap().to_wkt().unwrap().replace("\"false_easting\",500000", "\"false_easting\",600000");
    assert!(wkt.contains("600000") && wkt.contains("AUTHORITY[\"EPSG\",\"26915\"]"), "{wkt}");
    let xml_wkt = wkt.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
    std::fs::write(
        &b,
        format!(
            "<VRTDataset rasterXSize=\"20\" rasterYSize=\"20\"><SRS>{xml_wkt}</SRS><GeoTransform>{}, 30, 0, {}, 0, -30</GeoTransform><VRTRasterBand dataType=\"Float32\" band=\"1\"><SimpleSource><SourceFilename relativeToVRT=\"0\">{}</SourceFilename><SourceBand>1</SourceBand></SimpleSource></VRTRasterBand></VRTDataset>",
            UTM15_GT[0], UTM15_GT[3], a.display()
        ),
    )
    .unwrap();
    let modified = e.import(&b).unwrap().remove(0);
    let c = modified.crs.expect("a CRS");
    assert_ne!(c, "EPSG:26915", "a modified definition must not be reported as plain EPSG:26915");
    assert!(!engine::crs::same(&c, "EPSG:26915"));
    assert!(c.contains("600000"), "the file's own definition is kept: {c:.200}");
}
