//! Generic ArcGIS REST catalog (phase D1): a local server stands in for a MapServer, so paging, errors, citations and the
//! catalog are tested without the network. Real services are in the ignored test at the bottom.

use engine::arcgis::{self, Spec};
use engine::{tnm_vector, Engine};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Empty,
    JsonError,
    ServerError,
}

struct Mock {
    base: String,
    queries: Arc<AtomicUsize>,
}

/// A MapServer with layer 0 "Parcels" (25 squares, pages of 10), layer 1 "Roads" (5 lines), a group layer.
fn start(mode: Mode) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let queries = Arc::new(AtomicUsize::new(0));
    let q2 = queries.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let q2 = q2.clone();
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 1024];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req).lines().next().unwrap_or("").to_string();
                let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
                let params: HashMap<String, String> = query.split('&').filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), v.replace("%20", " "))).collect();
                let reply = |s: &mut std::net::TcpStream, status: &str, body: Value| {
                    let b = body.to_string();
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                };
                if let Mode::ServerError = mode {
                    return reply(&mut s, "500 Internal Server Error", json!({"error": {"code": 500, "message": "boom"}}));
                }
                if let Mode::JsonError = mode {
                    if path.ends_with("/query") {
                        return reply(&mut s, "200 OK", json!({"error": {"code": 400, "message": "Invalid or missing input parameters."}}));
                    }
                }
                let layer_path = path.rsplit('/').next().unwrap_or("");
                match layer_path {
                    "MapServer" => reply(&mut s, "200 OK", json!({"layers": [{"id": 0, "name": "Parcels"}, {"id": 1, "name": "Roads"}, {"id": 2, "name": "Group", "subLayerIds": [3]}, {"id": 3, "name": "Hidden child"}]})),
                    "0" | "1" => {
                        let poly = layer_path == "0";
                        reply(&mut s, "200 OK", json!({"geometryType": if poly {"esriGeometryPolygon"} else {"esriGeometryPolyline"}, "fields": [
                            {"name": "FID", "type": "esriFieldTypeOID"}, {"name": "NAME", "type": "esriFieldTypeString"}, {"name": "UPDATED", "type": "esriFieldTypeDate"}]}));
                    }
                    "query" => {
                        q2.fetch_add(1, Ordering::SeqCst);
                        let layer: usize = path.trim_end_matches("/query").rsplit('/').next().unwrap().parse().unwrap();
                        let total: usize = if matches!(mode, Mode::Empty) { 0 } else if layer == 0 { 25 } else { 5 };
                        if params.get("returnCountOnly").map(|v| v == "true").unwrap_or(false) {
                            return reply(&mut s, "200 OK", json!({"count": total}));
                        }
                        let offset: usize = params.get("resultOffset").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let n = (total.saturating_sub(offset)).min(10);
                        let feats: Vec<Value> = (offset..offset + n)
                            .map(|i| {
                                let x = -97.0 + i as f64 * 0.01;
                                let geometry = if layer == 0 {
                                    json!({"rings": [[[x, 35.0], [x, 35.005], [x + 0.005, 35.005], [x + 0.005, 35.0], [x, 35.0]]]})
                                } else {
                                    json!({"paths": [[[x, 35.0], [x + 0.005, 35.01]]]})
                                };
                                json!({"attributes": {"FID": i, "NAME": format!("{}{i}", if layer == 0 { "parcel" } else { "road" }), "UPDATED": 1_704_067_200_000i64}, "geometry": geometry})
                            })
                            .collect();
                        reply(&mut s, "200 OK", json!({"features": feats, "exceededTransferLimit": offset + n < total}));
                    }
                    _ => reply(&mut s, "404 Not Found", json!({})),
                }
            });
        }
    });
    Mock { base: format!("http://127.0.0.1:{port}/arcgis/rest/services/Test/MapServer"), queries }
}

const BOX: [f64; 4] = [-97.1, 34.9, -96.5, 35.1];

fn fast_retries() {
    tnm_vector::RETRY_BACKOFF_MS.store(1, Ordering::Relaxed);
}

#[test]
fn d1_service_addresses_are_cleaned_or_refused_in_words() {
    for (given, want) in [
        ("https://x.gov/arcgis/rest/services/A/MapServer", "https://x.gov/arcgis/rest/services/A/MapServer"),
        ("  https://x.gov/arcgis/rest/services/A/MapServer/  ", "https://x.gov/arcgis/rest/services/A/MapServer"),
        ("https://x.gov/arcgis/rest/services/A/MapServer/28?f=json", "https://x.gov/arcgis/rest/services/A/MapServer"),
        ("https://x.gov/arcgis/rest/services/A/FeatureServer/0/query", "https://x.gov/arcgis/rest/services/A/FeatureServer"),
    ] {
        assert_eq!(arcgis::clean_service_url(given).unwrap(), want, "{given}");
    }
    assert!(arcgis::clean_service_url("x.gov/A/MapServer").unwrap_err().to_string().contains("https://"));
    assert!(arcgis::clean_service_url("https://example.com/data.json").unwrap_err().to_string().contains("MapServer or FeatureServer"));
}

#[test]
fn d1_the_catalog_is_complete_and_every_entry_cites_itself() {
    let mut ids = std::collections::HashSet::new();
    for e in arcgis::catalog() {
        assert!(ids.insert(&e.id), "duplicate {}", e.id);
        assert!(e.service.starts_with("https://") && arcgis::clean_service_url(&e.service).unwrap() == e.service, "{}", e.id);
        assert!(!e.layers.is_empty() && !e.label.is_empty() && !e.group.is_empty(), "{}", e.id);
        for f in [&e.author, &e.title, &e.publisher, &e.url, &e.license, &e.annotation] {
            assert!(!f.trim().is_empty(), "{}: a citation field is empty", e.id);
        }
        let (c, label) = arcgis::citation(&Spec { entry_id: Some(e.id.clone()), url: None, layer: None, label: None }).unwrap();
        assert!(c.complete && c.key == format!("arcgis:{}", e.id) && label == e.label);
    }
    assert!(ids.len() >= 4);
    assert!(arcgis::entry("nope").unwrap_err().to_string().contains("Unknown ArcGIS dataset"));
}

#[test]
fn d1_layers_of_a_service_are_listed_without_group_layers() {
    let m = start(Mode::Normal);
    let l = arcgis::describe_service(&format!("{}/0", m.base)).unwrap();
    let got: Vec<(i64, &str, Option<&str>)> = l.iter().map(|x| (x.id, x.name.as_str(), x.geometry_type.as_deref())).collect();
    // The group layer itself is left out; its child is a layer like any other.
    assert_eq!(got, vec![(0, "Parcels", Some("Polygon")), (1, "Roads", Some("Polyline")), (3, "Hidden child", None)]);
}

#[test]
fn d1_a_custom_layer_is_paged_converted_cited_and_added() {
    let m = start(Mode::Normal);
    let e = Engine::new().unwrap();
    let spec = Spec { entry_id: None, url: Some(m.base.clone()), layer: Some("Parcels".into()), label: Some("Test parcels".into()) };
    let layers = e.arcgis_import(&spec, BOX, "Test area").unwrap();
    assert_eq!(layers.len(), 1);
    let l = &layers[0];
    assert_eq!(l.name, "Test parcels");
    assert_eq!(l.feature_count, 25, "all three pages arrived");
    assert!(m.queries.load(Ordering::SeqCst) >= 4, "a count and three pages");
    assert_eq!(l.crs.as_deref(), Some("EPSG:4326"));
    // The Esri date (ms since 1970) is an ISO date, the attributes are kept.
    let t = e.layer(&l.id).unwrap().table;
    let (name, upd): (String, String) = e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT \"NAME\", CAST(\"UPDATED\" AS VARCHAR) FROM \"{t}\" ORDER BY \"FID\" LIMIT 1"), [], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap();
    assert_eq!((name.as_str(), upd.as_str()), ("parcel0", "2024-01-01"));
    // Cited as a web service, with a stub the user completes.
    let s = e.sources().unwrap();
    let c = s.iter().find(|x| x.citation.key.starts_with("arcgis:http://127.0.0.1")).expect("cited");
    assert_eq!(c.citation.kind, "Web service");
    assert!(!c.citation.complete && c.citation.annotation.contains("Add the author"));
    assert_eq!(c.uses.len(), 1);
}

#[test]
fn d1_a_layer_can_be_named_by_number_and_an_unknown_name_says_so() {
    let m = start(Mode::Normal);
    let e = Engine::new().unwrap();
    let by_number = Spec { entry_id: None, url: Some(m.base.clone()), layer: Some("1".into()), label: Some("Test roads".into()) };
    let l = e.arcgis_import(&by_number, BOX, "Test area").unwrap();
    assert_eq!((l[0].feature_count, l[0].geom_type.as_str()), (5, "Line"));
    let bad = Spec { entry_id: None, url: Some(m.base.clone()), layer: Some("Nope".into()), label: None };
    let err = e.arcgis_import(&bad, BOX, "Test area").unwrap_err().to_string();
    assert!(err.contains("has no layer named 'Nope'"), "{err}");
    let none = Spec { entry_id: None, url: Some(m.base.clone()), layer: None, label: None };
    assert!(e.arcgis_import(&none, BOX, "x").unwrap_err().to_string().contains("Choose a layer"));
}

#[test]
fn d1_several_layers_merge_with_a_category_field() {
    let m = start(Mode::Normal);
    let dir = tempfile::tempdir().unwrap();
    let p = tnm_vector::fetch_sources("Both", "d1_both", &[(m.base.as_str(), "Parcels"), (m.base.as_str(), "Roads")], BOX, dir.path(), &|_, _| Ok(())).unwrap().unwrap();
    let v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let f = v["features"].as_array().unwrap();
    assert_eq!(f.len(), 30);
    let cats: std::collections::BTreeSet<&str> = f.iter().filter_map(|x| x["properties"]["CATEGORY"].as_str()).collect();
    assert_eq!(cats.into_iter().collect::<Vec<_>>(), vec!["Parcels", "Roads"]);
}

#[test]
fn d1_an_empty_area_adds_nothing_and_service_errors_are_readable() {
    fast_retries();
    let e = Engine::new().unwrap();
    let spec = |base: &str| Spec { entry_id: None, url: Some(base.to_string()), layer: Some("#0".into()), label: Some("T".into()) };
    let m = start(Mode::Empty);
    assert!(e.arcgis_import(&spec(&m.base), BOX, "Empty area").unwrap().is_empty());
    let m = start(Mode::JsonError);
    let err = format!("{:#}", e.arcgis_import(&spec(&m.base), BOX, "A").unwrap_err());
    assert!(err.contains("The service at 127.0.0.1") && err.contains("Invalid or missing input parameters"), "{err}");
    let m = start(Mode::ServerError);
    let err = format!("{:#}", e.arcgis_import(&spec(&m.base), BOX, "A").unwrap_err());
    assert!(err.contains("isn't responding") && err.contains("status code 500"), "{err}");
    // Not reachable at all.
    let dead = spec("http://127.0.0.1:1/arcgis/rest/services/X/MapServer");
    assert!(e.arcgis_import(&dead, BOX, "A").is_err());
    assert!(e.arcgis_import(&spec("https://example.com/nothing"), BOX, "A").unwrap_err().to_string().contains("MapServer or FeatureServer"));
}

/// Real services: every catalog entry answers a small query (may be empty).
#[test]
#[ignore = "network: queries each catalog service"]
fn d1_real_catalog_entries_answer() {
    let e = Engine::new().unwrap();
    for en in arcgis::catalog() {
        let spec = Spec { entry_id: Some(en.id.clone()), url: None, layer: None, label: None };
        e.arcgis_import(&spec, [-97.5, 35.1, -97.4, 35.2], "Norman").unwrap_or_else(|x| panic!("{}: {x:#}", en.id));
    }
}
