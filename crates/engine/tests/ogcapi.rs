//! OGC API – Features client (phase D5): a local server stands in for a service. Real service in the ignored test at the bottom.

use engine::ogcapi::{self, Spec};
use engine::{tnm_vector, Engine};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::Ordering;

/// 25 point features in pages of 10, linked by `next`. `/collections` lists "sites" (features) and "codes" (a table, no items link).
fn start(mode: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut buf = [0u8; 2048];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&buf[..n]),
                    }
                }
                let line = String::from_utf8_lossy(&req).lines().next().unwrap_or("").to_string();
                let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
                let q: std::collections::HashMap<&str, &str> = query.split('&').filter_map(|p| p.split_once('=')).collect();
                let send = |s: &mut std::net::TcpStream, status: &str, body: Value| {
                    let b = body.to_string();
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/geo+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len());
                };
                if mode == "down" {
                    return send(&mut s, "503 Service Unavailable", json!({}));
                }
                if mode == "refuse" {
                    return send(&mut s, "400 Bad Request", json!({"description": "Unknown query parameter 'bbox'"}));
                }
                if path == "/api/collections" {
                    return send(&mut s, "200 OK", json!({"collections": [
                        {"id": "sites", "title": "Monitoring sites", "links": [{"rel": "items", "href": "x"}]},
                        {"id": "codes", "title": "Code table", "links": [{"rel": "self", "href": "x"}]}]}));
                }
                if path == "/api/collections/sites/items" {
                    let offset: usize = q.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let total: usize = if mode == "empty" { 0 } else { 25 };
                    let n = total.saturating_sub(offset).min(10);
                    let feats: Vec<Value> = (offset..offset + n)
                        .map(|i| if i == 3 { json!({"type": "Feature", "id": i, "properties": {"name": "no location"}, "geometry": null}) } else { json!({"type": "Feature", "id": i, "properties": {"name": format!("site {i}"), "n": i}, "geometry": {"type": "Point", "coordinates": [-97.0 + i as f64 * 0.01, 35.0]}}) })
                        .collect();
                    let mut links = vec![json!({"rel": "self", "href": "x"})];
                    if offset + n < total {
                        links.push(json!({"rel": "next", "type": "application/geo+json", "href": format!("/api/collections/sites/items?f=json&bbox={}&limit=10&offset={}", q.get("bbox").unwrap_or(&""), offset + n)}));
                    }
                    return send(&mut s, "200 OK", json!({"type": "FeatureCollection", "numberMatched": total, "numberReturned": n, "features": feats, "links": links}));
                }
                send(&mut s, "404 Not Found", json!({"description": "nope"}));
            });
        }
    });
    format!("http://127.0.0.1:{port}/api")
}

const BOX: [f64; 4] = [-97.1, 34.9, -96.5, 35.1];

#[test]
fn d5_addresses_and_the_catalog_are_checked_without_a_request() {
    assert_eq!(ogcapi::clean_api_url("https://x.gov/ogcapi/v0/").unwrap(), "https://x.gov/ogcapi/v0");
    assert_eq!(ogcapi::clean_api_url("https://x.gov/ogcapi/v0/collections/sites/items?f=json").unwrap(), "https://x.gov/ogcapi/v0");
    assert!(ogcapi::clean_api_url("x.gov/api").unwrap_err().to_string().contains("https://"));
    let mut ids = std::collections::HashSet::new();
    for e in ogcapi::catalog() {
        assert!(ids.insert(&e.id) && e.api.starts_with("https://") && !e.collection.is_empty() && !e.license.is_empty());
        let (c, label) = ogcapi::citation(&Spec { entry_id: Some(e.id.clone()), api: None, collection: None, label: None }).unwrap();
        assert!(c.complete && label == e.label);
    }
}

#[test]
fn d5_collections_with_features_are_listed() {
    let api = start("ok");
    let c = ogcapi::collections(&api).unwrap();
    assert_eq!(c, vec![ogcapi::Collection { id: "sites".into(), title: "Monitoring sites".into() }]);
    assert!(ogcapi::collections("http://127.0.0.1:1/api").is_err());
}

#[test]
fn d5_pages_are_followed_features_without_geometry_are_skipped_and_the_layer_is_cited() {
    let api = start("ok");
    let e = Engine::new().unwrap();
    let spec = Spec { entry_id: None, api: Some(api.clone()), collection: Some("sites".into()), label: Some("Test sites".into()) };
    let l = e.ogc_import(&spec, BOX, "Test area").unwrap().expect("features");
    assert_eq!(l.name, "Test sites");
    assert_eq!(l.feature_count, 24, "25 listed over three pages, one without a location skipped");
    assert_eq!(l.geom_type, "Point");
    let s = e.sources().unwrap();
    let c = s.iter().find(|x| x.citation.key.starts_with("ogcapi:http://127.0.0.1")).expect("cited");
    assert!(!c.citation.complete && c.citation.annotation.contains("Add the author") && c.uses.len() == 1);
}

#[test]
fn d5_empty_areas_and_failures_are_readable() {
    tnm_vector::RETRY_BACKOFF_MS.store(1, Ordering::Relaxed);
    let e = Engine::new().unwrap();
    let sp = |api: &str| Spec { entry_id: None, api: Some(api.to_string()), collection: Some("sites".into()), label: Some("T".into()) };
    assert!(e.ogc_import(&sp(&start("empty")), BOX, "A").unwrap().is_none());
    let err = e.ogc_import(&sp(&start("refuse")), BOX, "A").unwrap_err().to_string();
    assert!(err.contains("refused the request (HTTP 400)") && err.contains("Unknown query parameter"), "{err}");
    let err = e.ogc_import(&sp(&start("down")), BOX, "A").unwrap_err().to_string();
    assert!(err.contains("isn't responding"), "{err}");
    let bad_coll = Spec { collection: Some("nope".into()), ..sp(&start("ok")) };
    assert!(e.ogc_import(&bad_coll, BOX, "A").unwrap_err().to_string().contains("refused the request (HTTP 404)"));
    let odd = Spec { collection: Some("a b;drop".into()), ..sp(&start("ok")) };
    assert!(e.ogc_import(&odd, BOX, "A").unwrap_err().to_string().contains("not a collection name"));
    assert!(e.ogc_import(&sp("https://example.com/none"), BOX, "A").is_err());
    let none = Spec { entry_id: None, api: None, collection: None, label: None };
    assert!(e.ogc_import(&none, BOX, "A").unwrap_err().to_string().contains("Choose a dataset"));
}

/// The real USGS Water Data service: monitoring locations around Norman, Oklahoma.
#[test]
#[ignore = "network: api.waterdata.usgs.gov"]
fn d5_real_usgs_water_sites() {
    let e = Engine::new().unwrap();
    let spec = Spec { entry_id: Some("usgs-waterdata-sites".into()), api: None, collection: None, label: None };
    let l = e.ogc_import(&spec, [-97.6, 35.1, -97.3, 35.3], "Norman").unwrap().expect("sites near Norman");
    assert!(l.feature_count > 3, "{} sites", l.feature_count);
}
