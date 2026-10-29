//! `gis-studio --self-test`: exercise the engine without the UI, to check an
//! installed app is self-contained (bundled DuckDB spatial, GDAL, PROJ).
//! Prints a report and exits 0 on success.

use engine::Engine;

pub fn run() -> i32 {
    match check() {
        Ok(report) => {
            println!("SELF-TEST OK\n{report}");
            0
        }
        Err(e) => {
            eprintln!("SELF-TEST FAILED: {e:#}");
            1
        }
    }
}

fn check() -> anyhow::Result<String> {
    let mut out = Vec::new();
    out.push(format!("resources: {:?}", engine::resources::dir()));
    out.push(format!("spatial extension: {:?}", engine::resources::spatial_extension()));
    out.push(format!("GDAL: {}", gdal::version::VersionInfo::version_summary()));
    out.push(format!("GDAL_DATA={:?} PROJ_DATA={:?}", std::env::var("GDAL_DATA").ok(), std::env::var("PROJ_DATA").ok()));

    let e = Engine::new()?;
    // DuckDB spatial + PROJ: State Plane transform.
    let (x, y) = e.transform_point(-97.5, 35.47, "EPSG:4326", "EPSG:2267")?;
    anyhow::ensure!(x > 2_000_000.0 && y > 100_000.0, "unexpected State Plane coordinates {x} {y}");
    out.push(format!("EPSG:4326 → EPSG:2267 (OK North ftUS): {x:.1}, {y:.1}"));

    // Datum shifts: PROJ's datum files must be present (NAD27/NAD83/WGS 84 would be off by metres without them).
    let datum = e.datum_data_status();
    anyhow::ensure!(datum.ok, "datum data: {}", datum.problems.join("; "));
    out.push(format!("datum shifts match PROJ's best operation (PROJ_DATA = {:?})", datum.proj_data));

    // Datum coverage by region (CRS accuracy plan A5): informational for the world, required for the United States, whose grids ship with the app.
    for (name, r) in e.datum_coverage() {
        out.push(format!("datum coverage, {name}: {}{}", r.level, r.accuracy_m.map(|a| format!(" (up to {a} m)")).unwrap_or_default()));
        anyhow::ensure!(!(name.starts_with("United States") && r.warn), "datum coverage, {name}: {}", r.text);
    }

    // Vector layer + tile.
    let l = e.generate_test_layer("polygons", 2000, [-98.0, 35.0, -97.0, 36.0])?;
    let tile = e.tile(&l.id, 6, 14, 25, &[])?;
    out.push(format!("vector tile: {} bytes", tile.len()));

    // GDAL: write a small DEM, import it, draw a tile, make contours.
    let dir = std::env::temp_dir().join(format!("gis-studio-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let dem = dir.join("dem.tif");
    {
        let drv = gdal::DriverManager::get_driver_by_name("GTiff")?;
        let mut ds = drv.create_with_band_type::<f32, _>(&dem, 100, 100, 1)?;
        ds.set_geo_transform(&[600_000.0, 30.0, 0.0, 3_900_000.0, 0.0, -30.0])?;
        ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_epsg(32614)?)?;
        let data: Vec<f32> = (0..100 * 100).map(|i| ((i % 100) + (i / 100)) as f32).collect();
        let mut buf = gdal::raster::Buffer::new((100, 100), data);
        ds.rasterband(1)?.write((0, 0), (100, 100), &mut buf)?;
    }
    let r = e.import(&dem)?.remove(0);
    let ext = r.extent.unwrap_or_default();
    let (lon, lat) = ((ext[0] + ext[2]) / 2.0, (ext[1] + ext[3]) / 2.0);
    let z = 11u32;
    let n = 2f64.powi(z as i32);
    let tx = ((lon + 180.0) / 360.0 * n) as u32;
    let ty = ((1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI) / 2.0 * n) as u32;
    let png = e.tile(&r.id, z, tx, ty, &[])?;
    anyhow::ensure!(png.starts_with(b"\x89PNG"), "raster tile is not a PNG");
    out.push(format!("raster tile: {} bytes PNG", png.len()));
    let g: engine::graph::Graph = serde_json::from_value(serde_json::json!({
        "nodes": [
            {"id": "in", "tool": "input_layer", "params": {"layer": r.id}},
            {"id": "c", "tool": "contours", "params": {"interval": 20}}
        ],
        "edges": [{"source": "in", "target": "c", "targetHandle": "in"}]
    }))?;
    let res = e.run_graph(&g, None, &mut |_| {})?;
    let c = res.iter().find(|x| x.id == "c").ok_or_else(|| anyhow::anyhow!("no contour result"))?;
    anyhow::ensure!(c.error.is_none(), "contours: {:?}", c.error);
    out.push(format!("contours: {} lines", c.rows.unwrap_or(0)));
    let _ = std::fs::remove_dir_all(&dir);
    Ok(out.join("\n"))
}
