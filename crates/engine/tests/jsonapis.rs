//! USGS earthquakes and NRCS SSURGO (phase D6): local servers stand in for the services; the real ones are in the ignored tests.

use engine::jsonapis;
use engine::Engine;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Answers every request with `reply(path_and_query, body)`; records them.
fn serve(reply: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(vec![]));
    let (s2, reply) = (seen.clone(), Arc::new(reply));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let (seen, reply) = (s2.clone(), reply.clone());
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                    if let Some(h) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&req[..h]).to_lowercase();
                        let want: usize = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
                        if req.len() >= h + 4 + want {
                            break;
                        }
                    }
                }
                let h = req.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(req.len());
                let head = String::from_utf8_lossy(&req[..h]).to_string();
                let body = String::from_utf8_lossy(&req[(h + 4).min(req.len())..]).to_string();
                let target = head.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("").to_string();
                seen.lock().unwrap().push((target.clone(), body.clone()));
                let (status, out) = reply(&target, &body);
                let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
            });
        }
    });
    (format!("http://127.0.0.1:{port}/q"), seen)
}

const BOX: [f64; 4] = [-98.0, 35.0, -96.0, 37.0];

fn quake_json(n: usize) -> String {
    let feats: Vec<Value> = (0..n).map(|i| json!({"type": "Feature", "id": format!("ok{i}"), "properties": {"mag": 3.0 + i as f64 * 0.1, "place": "5 km N of Somewhere, OK", "time": 1_704_067_200_000i64 + i as i64 * 86_400_000, "type": "earthquake"}, "geometry": {"type": "Point", "coordinates": [-97.0 + i as f64 * 0.1, 36.0, 5.5]}})).collect();
    json!({"type": "FeatureCollection", "features": feats}).to_string()
}

#[test]
fn d6_earthquakes_become_points_with_readable_fields_and_a_citation() {
    let (url, seen) = serve(|_, _| (200, quake_json(4)));
    let e = Engine::new().unwrap();
    let l = e.earthquakes_import_from(&url, BOX, Some("2024-01-01"), Some("2024-12-31"), 3.0, "Oklahoma").unwrap().expect("events");
    assert_eq!((l.feature_count, l.geom_type.as_str()), (4, "Point"));
    let (target, _) = seen.lock().unwrap()[0].clone();
    for part in ["minlatitude=35", "maxlatitude=37", "minlongitude=-98", "maxlongitude=-96", "minmagnitude=3", "starttime=2024-01-01", "endtime=2024-12-31"] {
        assert!(target.contains(part), "{target}");
    }
    let (time, mag, depth): (String, f64, f64) = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT CAST(time AS VARCHAR), mag, depth_km FROM \"{}\" ORDER BY mag LIMIT 1", l.table), [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap();
    assert_eq!((time.as_str(), mag, depth), ("2024-01-01", 3.0, 5.5));
    let s = e.sources().unwrap();
    assert!(s.iter().any(|x| x.citation.key == "usgs-earthquakes" && x.uses.len() == 1));
}

#[test]
fn d6_earthquake_inputs_and_failures_are_checked_in_words() {
    let e = Engine::new().unwrap();
    let (url, seen) = serve(|_, _| (200, quake_json(0)));
    assert!(e.earthquakes_import_from(&url, BOX, None, None, 2.5, "x").unwrap().is_none(), "no events: no layer");
    for (start, mag, text) in [(Some("June 2024"), 3.0, "Dates look like"), (None, 12.0, "magnitude")] {
        assert!(e.earthquakes_import_from(&url, BOX, start, None, mag, "x").unwrap_err().to_string().contains(text));
    }
    assert!(e.earthquakes_import_from(&url, [1.0, 1.0, 1.0, 2.0], None, None, 3.0, "x").unwrap_err().to_string().contains("area is empty"));
    assert_eq!(seen.lock().unwrap().len(), 1, "only the valid call reached the service");
    let (busy, _) = serve(|_, _| (400, "Error 400: Bad Request\n\nBad minmagnitude".into()));
    let err = e.earthquakes_import_from(&busy, BOX, None, None, 3.0, "x").unwrap_err().to_string();
    assert!(err.contains("refused the request (HTTP 400)") && err.contains("Bad minmagnitude"), "{err}");
    let (many, _) = serve(|_, _| (200, quake_json(jsonapis::QUAKE_MAX)));
    assert!(e.earthquakes_import_from(&many, BOX, None, None, 0.0, "x").unwrap_err().to_string().contains("Choose a smaller area"));
    let (junk, _) = serve(|_, _| (200, "<html>".into()));
    assert!(e.earthquakes_import_from(&junk, BOX, None, None, 3.0, "x").unwrap_err().to_string().contains("isn't GeoJSON"));
    assert!(e.earthquakes_import_from("http://127.0.0.1:1/q", BOX, None, None, 3.0, "x").unwrap_err().to_string().contains("Could not reach"));
}

#[test]
fn d6_soil_map_units_are_requested_by_polygon_and_become_a_layer() {
    let (url, seen) = serve(|_, _| {
        (200, json!({"Table": [["mukey", "musym", "muname", "wkt"],
            ["382123", "60", "Bethany silt loam, 0 to 1 percent slopes", "POLYGON ((-97.455 35.200, -97.450 35.200, -97.450 35.205, -97.455 35.205, -97.455 35.200))"],
            ["382124", "61", "Port silt loam", "POLYGON ((-97.450 35.200, -97.445 35.200, -97.445 35.205, -97.450 35.205, -97.450 35.200))"]]}).to_string())
    });
    let e = Engine::new().unwrap();
    let l = e.ssurgo_import_from(&url, [-97.46, 35.19, -97.44, 35.21], "Norman").unwrap().expect("units");
    assert_eq!((l.feature_count, l.geom_type.as_str()), (2, "Polygon"));
    let (_, body) = seen.lock().unwrap()[0].clone();
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["format"], "JSON+COLUMNNAME");
    let q = v["query"].as_str().unwrap();
    assert!(q.contains("SDA_Get_Mupolygonkey_from_intersection_with_WktWgs84") && q.contains("polygon((-97.46 35.19, -97.44 35.19, -97.44 35.21, -97.46 35.21, -97.46 35.19))"), "{q}");
    let name: String = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT muname FROM \"{}\" WHERE mukey = '382123'", l.table), [], |r| r.get(0))?)).unwrap();
    assert!(name.starts_with("Bethany silt loam"));
    assert!(e.sources().unwrap().iter().any(|x| x.citation.key == "nrcs-ssurgo"));
}

#[test]
fn d6_soil_refusals_empty_bodies_and_errors_are_readable() {
    let e = Engine::new().unwrap();
    let (empty, _) = serve(|_, _| (200, String::new()));
    assert!(e.ssurgo_import_from(&empty, [-97.46, 35.19, -97.44, 35.21], "x").unwrap().is_none(), "an empty body means nothing there");
    let (never, seen) = serve(|_, _| (200, String::new()));
    let err = e.ssurgo_import_from(&never, [-98.0, 35.0, -97.0, 36.0], "x").unwrap_err().to_string();
    assert!(err.contains("Choose an area at most 0.25°") && err.contains("1.00° × 1.00°"), "{err}");
    assert!(seen.lock().unwrap().is_empty(), "an oversized area is refused before any request");
    let (bad, _) = serve(|_, _| (200, json!({"Table": [["a", "b"]]}).to_string()));
    assert!(e.ssurgo_import_from(&bad, [-97.46, 35.19, -97.44, 35.21], "x").unwrap_err().to_string().contains("no 'mukey' column"));
    let (down, _) = serve(|_, _| (500, "oops".into()));
    assert!(e.ssurgo_import_from(&down, [-97.46, 35.19, -97.44, 35.21], "x").unwrap_err().to_string().contains("HTTP 500"));
}

#[test]
#[ignore = "network: USGS earthquakes and NRCS Soil Data Access"]
fn d6_real_services() {
    let e = Engine::new().unwrap();
    let q = e.earthquakes_import([-98.0, 35.0, -96.0, 37.0], Some("2024-01-01"), None, 3.0, "Oklahoma").unwrap().expect("Oklahoma has had magnitude 3 earthquakes");
    assert!(q.feature_count > 5);
    let s = e.ssurgo_import([-97.46, 35.19, -97.44, 35.21], "Norman").unwrap().expect("soil units");
    assert!(s.feature_count > 3, "{}", s.feature_count);
}
