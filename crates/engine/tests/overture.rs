//! Overture GeoParquet (phase D4): a local server stands in for Overture's S3 bucket (DuckDB httpfs needs HTTP Range). The real
//! release is in the ignored test at the bottom.

use engine::overture::{self, THEMES};
use engine::Engine;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

fn serve(file: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let data = Arc::new(file);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let data = data.clone();
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 2048];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&req).to_string();
                let method = head.split_whitespace().next().unwrap_or("").to_string();
                let total = data.len();
                let range = head.lines().find_map(|l| l.to_lowercase().strip_prefix("range: bytes=").map(String::from));
                let (status, a, b) = match range {
                    Some(r) => {
                        let (a, b) = r.split_once('-').unwrap();
                        let a: usize = a.trim().parse().unwrap_or(0);
                        let b: usize = b.trim().parse().unwrap_or(total.saturating_sub(1));
                        ("206 Partial Content", a.min(total), (b + 1).min(total))
                    }
                    None => ("200 OK", 0, total),
                };
                let extra = if status.starts_with("206") { format!("Content-Range: bytes {}-{}/{}\r\n", a, b.saturating_sub(1), total) } else { String::new() };
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", if method == "HEAD" { total } else { b - a });
                if method != "HEAD" {
                    let _ = s.write_all(&data[a..b]);
                }
            });
        }
    });
    format!("http://127.0.0.1:{port}/buildings.parquet")
}

/// A GeoParquet file like Overture's: 3 buildings near (-97.45, 35.2), 2 far away. `wkb`: geometry as plain WKB bytes.
fn parquet(e: &Engine, tag: &str, wkb: bool) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!("overture-test-{}-{tag}.parquet", std::process::id()));
    let geom = if wkb { "ST_AsWKB(g)" } else { "g" };
    e.with_conn_for_tests(|c| {
        c.execute_batch(&format!(
            "COPY (SELECT id, {geom} AS geometry, {{'xmin': ST_XMin(g), 'ymin': ST_YMin(g), 'xmax': ST_XMax(g), 'ymax': ST_YMax(g)}} AS bbox,
                          {{'primary': name}} AS names, class, height
                   FROM (VALUES ('b1', ST_GeomFromText('POLYGON((-97.451 35.200, -97.450 35.200, -97.450 35.201, -97.451 35.201, -97.451 35.200))'), 'Hall', 'civic', 12.5),
                                ('b2', ST_GeomFromText('POLYGON((-97.449 35.202, -97.448 35.202, -97.448 35.203, -97.449 35.203, -97.449 35.202))'), NULL, 'house', 6.0),
                                ('b3', ST_GeomFromText('POLYGON((-97.447 35.204, -97.446 35.204, -97.446 35.205, -97.447 35.205, -97.447 35.204))'), 'Barn', NULL, NULL),
                                ('far1', ST_GeomFromText('POLYGON((10.0 10.0, 10.001 10.0, 10.001 10.001, 10.0 10.001, 10.0 10.0))'), 'Far', 'house', 3.0),
                                ('far2', ST_GeomFromText('POLYGON((-120.0 40.0, -119.999 40.0, -119.999 40.001, -120.0 40.001, -120.0 40.0))'), 'Farther', 'house', 3.0)) t(id, g, name, class, height))
             TO '{}' (FORMAT PARQUET)",
            path.display()
        ))?;
        Ok(())
    })
    .unwrap();
    let b = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    b
}

const BOX: [f64; 4] = [-97.46, 35.19, -97.44, 35.21];

#[test]
fn d4_themes_and_files_are_well_formed() {
    let mut ids = std::collections::HashSet::new();
    for t in THEMES {
        assert!(ids.insert(t.id), "duplicate {}", t.id);
        let g = overture::glob(t);
        assert!(g.starts_with("s3://overturemaps-us-west-2/release/") && g.contains(&format!("theme={}/type={}/", t.theme, t.kind)), "{g}");
        let c = overture::citation(t);
        assert!(c.complete && c.license.contains("Overture") && c.version.contains(overture::RELEASE));
    }
    assert!(overture::theme("nope").unwrap_err().to_string().contains("Unknown Overture theme"));
    // The licence text warns about OpenStreetMap-derived data where it applies.
    assert!(overture::citation(overture::theme("buildings").unwrap()).license.contains("ODbL"));
    assert!(overture::citation(overture::theme("places").unwrap()).license.contains("CDLA"));
}

#[test]
fn d4_a_geoparquet_file_becomes_a_layer_with_flat_fields_for_the_area_only() {
    for wkb in [false, true] {
        let e = Engine::new().unwrap();
        let url = serve(parquet(&e, if wkb { "wkb" } else { "geom" }, wkb));
        let t = overture::theme("buildings").unwrap();
        let l = e.overture_import_from(t, &url, BOX, "Test area").unwrap();
        assert_eq!(l.feature_count, 3, "wkb={wkb}: only the buildings in the box");
        assert_eq!(l.crs.as_deref(), Some("EPSG:4326"));
        assert!(l.name.contains("Buildings") && l.name.contains("Test area"));
        assert_eq!(l.geom_type, "Polygon");
        let names: Vec<&str> = l.fields.iter().map(|f| f.name.as_str()).collect();
        for want in ["id", "class", "height", "name"] {
            assert!(names.contains(&want), "{names:?}");
        }
        assert!(!names.contains(&"num_floors") && !names.contains(&"subtype"), "fields the file lacks are left out: {names:?}");
        let (hall, nulls): (String, i64) = e
            .with_conn_for_tests(|c| {
                let h: String = c.query_row(&format!("SELECT name FROM \"{}\" WHERE id = 'b1'", l.table), [], |r| r.get(0))?;
                let n: i64 = c.query_row(&format!("SELECT count(*) FROM \"{}\" WHERE name IS NULL", l.table), [], |r| r.get(0))?;
                Ok((h, n))
            })
            .unwrap();
        assert_eq!((hall.as_str(), nulls), ("Hall", 1));
        let s = e.sources().unwrap();
        let c = s.iter().find(|x| x.citation.key == "overture:buildings").expect("cited");
        assert!(c.citation.license.contains("ODbL") && c.uses.len() == 1);
    }
}

#[test]
fn d4_empty_areas_and_bad_input_are_readable() {
    let e = Engine::new().unwrap();
    let url = serve(parquet(&e, "err", false));
    let t = overture::theme("buildings").unwrap();
    let err = e.overture_import_from(t, &url, [0.0, 0.0, 0.1, 0.1], "Nowhere").unwrap_err().to_string();
    assert!(err.contains("Overture has no buildings in this area"), "{err}");
    assert!(e.overture_import_from(t, &url, [1.0, 1.0, 1.0, 2.0], "x").unwrap_err().to_string().contains("area is empty"));
    let err = e.overture_import_from(t, "http://127.0.0.1:1/none.parquet", BOX, "x").unwrap_err().to_string();
    assert!(err.contains("Could not read the Overture data"), "{err}");
}

/// The real release: buildings in a few city blocks of Norman, Oklahoma.
#[test]
#[ignore = "network: Overture's public S3 bucket"]
fn d4_real_overture_buildings() {
    let e = Engine::new().unwrap();
    let l = e.overture_import("buildings", [-97.4465, 35.2050, -97.4425, 35.2085], "Norman").unwrap();
    assert!(l.feature_count > 20 && l.feature_count < 5000, "{} buildings", l.feature_count);
}
