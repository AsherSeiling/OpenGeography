//! Export Table (docs/TEST_PLAN.md §Export Table): attributes plus coordinates in a chosen CRS, never over a source file.

use engine::export_table::{GridRefSpec, TableExportSpec};
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
    TableExportSpec { path: path.to_str().unwrap().into(), format: format.into(), crs: crs.into(), grid_ref: None, fids: None }
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

#[test]
fn usng_and_mgrs_columns_alone_or_beside_coordinates() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("p.gpkg");
    write_utm(&src);
    let e = Engine::new().unwrap();
    let l = e.import(&src).unwrap().remove(0);
    let grid = |style: &str, precision: u8| Some(GridRefSpec { style: style.into(), precision, datum: "auto".into() });

    // USNG only: no CRS needed, no x/y columns, the reference with spaces (zone 15, band S, square WE on NAD 83).
    let out = dir.path().join("usng.csv");
    let mut s = spec(&out, "csv", "");
    s.grid_ref = grid("usng", 5);
    let r = e.export_table(&l.id, &s).unwrap();
    assert_eq!((r.rows, r.x_column.as_str(), r.grid_column.as_deref()), (2, "", Some("USNG")));
    let text = std::fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "fid,id,x,USNG");
    assert!(lines[1].ends_with(",15S WE 00000 00000") || lines[1].ends_with(",15S VE 99999 99999"), "{}", lines[1]);
    assert!(lines[2].ends_with(",15S WE 00100 00200"), "{}", lines[2]);
    let side = std::fs::read_to_string(&r.sidecar).unwrap();
    assert!(side.contains("no X/Y columns") && side.contains("USNG reference") && side.contains("NAD 83 in UTM zones 1–23"), "{side}");

    // MGRS beside lon/lat, at 1 km: both written, the reference without spaces.
    let out2 = dir.path().join("both.csv");
    let mut s = spec(&out2, "csv", "EPSG:4326");
    s.grid_ref = grid("mgrs", 2);
    let r = e.export_table(&l.id, &s).unwrap();
    assert_eq!((r.x_column.as_str(), r.grid_column.as_deref()), ("lon", Some("MGRS")));
    let text = std::fs::read_to_string(&out2).unwrap();
    assert_eq!(text.lines().next().unwrap(), "fid,id,x,lon,lat,MGRS");
    assert!(text.lines().nth(2).unwrap().ends_with(",15SWE0000"), "{}", text.lines().nth(2).unwrap());
    let side = std::fs::read_to_string(&r.sidecar).unwrap();
    assert!(side.contains("MGRS reference") && side.contains("a square 1000 m wide") && side.contains("Units: degree"), "{side}");

    // Neither a CRS nor a grid reference is refused in words.
    let err = e.export_table(&l.id, &spec(&dir.path().join("none.csv"), "csv", "")).unwrap_err().to_string();
    assert!(err.contains("or a USNG/MGRS reference"), "{err}");
}

#[test]
fn usng_export_of_a_generated_test_layer_writes_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new().unwrap();
    let l = e.generate_test_layer("points", 12, [-100.0, 30.0, -95.0, 35.0]).unwrap();
    let out = dir.path().join("g.csv");
    let mut s = spec(&out, "csv", "");
    s.grid_ref = Some(GridRefSpec { style: "usng".into(), precision: 2, datum: "auto".into() });
    let r = e.export_table(&l.id, &s).unwrap();
    assert_eq!(r.rows, 12);
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(text.lines().count(), 13, "{text}");
}
