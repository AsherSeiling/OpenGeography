//! Export Table (docs/TEST_PLAN.md §Export Table): attributes plus coordinates in a chosen CRS, never over a source file.

use engine::export_table::TableExportSpec;
use engine::Engine;
use std::path::Path;

fn write_utm(path: &Path) {
    let c = engine::duckdb::Connection::open_in_memory().unwrap();
    engine::db::load_spatial(&c).unwrap();
    c.execute_batch(&format!(
        "COPY (SELECT * FROM (VALUES (1, 'a', ST_Point(500000.0, 4400000.0)), (2, 'b', ST_Point(500100.0, 4400200.0))) t(id, x, geom)) TO '{}' (FORMAT GDAL, DRIVER 'GPKG', SRS 'EPSG:26915', USE_TMP_FILE false)",
        path.display()
    ))
    .unwrap();
}

fn spec(path: &Path, format: &str, crs: &str) -> TableExportSpec {
    TableExportSpec { path: path.to_str().unwrap().into(), format: format.into(), crs: crs.into(), fids: None }
}

#[test]
fn csv_in_lonlat_and_projected_with_sidecar() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("p.gpkg");
    write_utm(&src);
    let e = Engine::new().unwrap();
    let l = e.import(&src).unwrap().remove(0);

    let out = dir.path().join("ll.csv");
    let r = e.export_table(&l.id, &spec(&out, "csv", "EPSG:4326")).unwrap();
    assert_eq!((r.rows, r.x_column.as_str(), r.y_column.as_str()), (2, "lon", "lat"));
    let text = std::fs::read_to_string(&out).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap(), "fid,id,x,lon,lat");
    let cols: Vec<&str> = lines.next().unwrap().split(',').collect();
    assert!((cols[3].parse::<f64>().unwrap() + 93.0).abs() < 1e-3 && (cols[4].parse::<f64>().unwrap() - 39.75).abs() < 0.01, "{cols:?}");
    let side = std::fs::read_to_string(&r.sidecar).unwrap();
    assert!(side.contains("Units: degree") && side.contains("EPSG:4326"), "{side}");
    // The datum shift is described too: NAD83 to WGS 84 is a datum change with a stated accuracy.
    assert!(side.contains("Transformation: Datum change North American Datum 1983 → World Geodetic System 1984"), "{side}");

    // A projected target keeps metres and says so; only the selected feature is written.
    let out2 = dir.path().join("utm.csv");
    let mut s = spec(&out2, "csv", "EPSG:26915");
    s.fids = Some(vec![]);
    assert_eq!(e.export_table(&l.id, &s).unwrap().rows, 0);
    let r = e.export_table(&l.id, &spec(&out2, "csv", "EPSG:26915")).unwrap();
    assert_eq!((r.x_column.as_str(), r.unit.as_str()), ("x_", "metre"));
    let text = std::fs::read_to_string(&out2).unwrap();
    assert!(text.lines().nth(1).unwrap().starts_with("1,1,a,500000"), "{text}");
}

#[test]
fn parquet_and_refusals() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("p.gpkg");
    write_utm(&src);
    let e = Engine::new().unwrap();
    let l = e.import(&src).unwrap().remove(0);
    let out = dir.path().join("t.parquet");
    assert_eq!(e.export_table(&l.id, &spec(&out, "parquet", "EPSG:4326")).unwrap().rows, 2);
    assert!(out.exists());

    let err = e.export_table(&l.id, &spec(&src, "csv", "EPSG:4326")).unwrap_err().to_string();
    assert!(err.contains("source file"), "{err}");
    let err = e.export_table(&l.id, &spec(&out, "csv", "")).unwrap_err().to_string();
    assert!(err.contains("coordinate system"), "{err}");
    let err = e.export_table(&l.id, &spec(&out, "csv", "EPSG:99999999")).unwrap_err().to_string();
    assert!(err.contains("Can't use"), "{err}");
}
