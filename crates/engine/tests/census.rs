//! Census data join (phase D2): a local server stands in for api.census.gov. The key is passed in by the caller (the shell reads
//! it from the OS keychain) and must never appear in a message.

use engine::census::{self, Level, Spec};
use engine::Engine;
use serde_json::{json, Map, Value};
use std::io::{Read, Write};
use std::net::TcpListener;

/// A mock of the Census Data API. Returns (base URL, list of request targets).
fn start() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let seen = s2.clone();
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
                seen.lock().unwrap().push(target.clone());
                let send = |s: &mut std::net::TcpStream, status: &str, body: &str| {
                    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                };
                if target.contains("/1999/") {
                    return send(&mut s, "404 Not Found", "");
                }
                if target.contains("BAD_VAR") {
                    return send(&mut s, "400 Bad Request", "error: error: unknown variable 'BAD_VAR'");
                }
                if target.contains("key=BADKEY") {
                    return send(&mut s, "400 Bad Request", "error: Your key BADKEY is invalid");
                }
                let vars: Vec<String> = target.split("get=").nth(1).and_then(|x| x.split('&').next()).unwrap_or("").split(',').map(String::from).collect();
                let level_state = target.contains("for=state:*");
                let level_county = target.contains("for=county:*");
                let state = target.split("in=state:").nth(1).and_then(|x| x.get(0..2)).unwrap_or("").to_string();
                let mut header: Vec<Value> = vars.iter().map(|v| json!(v)).collect();
                let mut rows: Vec<Vec<Value>> = Vec::new();
                let val = |v: &str, i: usize| -> Value {
                    // B01003_001E = population; B19013_001E = median income, suppressed for tract 3.
                    match (v, i) {
                        ("B19013_001E", 2) => json!("-666666666"),
                        ("B01003_001E", _) => json!(format!("{}", 1000 * (i + 1))),
                        _ => json!(format!("{}", 50000 + i)),
                    }
                };
                if level_state {
                    header.push(json!("state"));
                    for (i, st) in ["01", "04", "06", "40"].iter().enumerate() {
                        let mut r: Vec<Value> = vars.iter().map(|v| val(v, i)).collect();
                        r.push(json!(st));
                        rows.push(r);
                    }
                } else if level_county {
                    header.extend([json!("state"), json!("county")]);
                    for (i, c) in ["001", "003"].iter().enumerate() {
                        let mut r: Vec<Value> = vars.iter().map(|v| val(v, i)).collect();
                        r.extend([json!(state), json!(c)]);
                        rows.push(r);
                    }
                } else {
                    header.extend([json!("state"), json!("county"), json!("tract")]);
                    let tracts: &[(&str, &str)] = if state == "40" { &[("109", "100100"), ("109", "100200"), ("109", "100300")] } else { &[("201", "000100")] };
                    for (i, (c, t)) in tracts.iter().enumerate() {
                        let mut r: Vec<Value> = vars.iter().map(|v| val(v, i)).collect();
                        r.extend([json!(state), json!(c), json!(t)]);
                        rows.push(r);
                    }
                }
                let mut table = vec![header];
                table.extend(rows.into_iter().map(|r| r));
                send(&mut s, "200 OK", &serde_json::to_string(&table).unwrap());
            });
        }
    });
    (format!("http://127.0.0.1:{port}/data"), seen)
}

fn layer(e: &Engine, name: &str, geoids: &[Value]) -> engine::layers::LayerInfo {
    let feats: Vec<(String, Map<String, Value>)> = geoids
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let mut p = Map::new();
            p.insert("GEOID".into(), g.clone());
            p.insert("name".into(), json!(format!("f{i}")));
            (format!("POINT({} 35)", -97.0 + i as f64 * 0.1), p)
        })
        .collect();
    e.create_features(name, &feats).unwrap()
}

fn spec(vars: &[&str]) -> Spec {
    Spec { dataset: "acs5".into(), vintage: 2023, level: None, variables: vars.iter().map(|s| s.to_string()).collect() }
}

fn col(e: &Engine, l: &engine::layers::LayerInfo, field: &str, geoid: &str) -> Option<f64> {
    e.with_conn_for_tests(|c| Ok(c.query_row(&format!("SELECT \"{field}\" FROM \"{}\" WHERE CAST(\"GEOID\" AS VARCHAR) = '{geoid}'", l.table), [], |r| r.get::<_, Option<f64>>(0))?)).unwrap()
}

#[test]
fn d2_variable_codes_are_checked_before_any_request() {
    assert_eq!(census::check_variables(&["b01003_001e".into(), " B19013_001E ".into(), "B01003_001E".into()]).unwrap(), vec!["B01003_001E", "B19013_001E"]);
    assert!(census::check_variables(&[]).unwrap_err().to_string().contains("at least one"));
    assert!(census::check_variables(&["B01003 001E".into()]).unwrap_err().to_string().contains("not a Census variable code"));
    assert!(census::check_variables(&["B01003_001E;drop".into()]).is_err());
    assert!(census::check_variables(&(0..41).map(|i| format!("B{i:05}_001E")).collect::<Vec<_>>()).unwrap_err().to_string().contains("at most"));
    assert_eq!(Level::from_len(11), Some(Level::Tract));
    assert_eq!(Level::from_len(7), None);
    assert!(census::dataset("nope").is_err());
}

#[test]
fn d2_tracts_in_two_states_join_by_geoid_with_citation_and_no_overwrite() {
    let (base, seen) = start();
    let e = Engine::new().unwrap();
    // 4 tracts (3 in Oklahoma, 1 in Texas) and one that the table does not have.
    let src = layer(&e, "Tracts", &[json!("40109100100"), json!("40109100200"), json!("40109100300"), json!("48201000100"), json!("40109999999")]);
    let r = e.census_join_from(&base, &src.id, "GEOID", &spec(&["B01003_001E", "B19013_001E"]), None).unwrap();
    assert_eq!(r.layer.feature_count, 5, "no feature lost");
    assert_eq!((r.matched, r.unmatched), (4, 1));
    assert_eq!(r.suppressed, 1, "tract 3's median income is -666666666");
    assert_eq!(col(&e, &r.layer, "B01003_001E", "40109100200"), Some(2000.0));
    assert_eq!(col(&e, &r.layer, "B19013_001E", "40109100300"), None, "suppressed value is no value, not -666666666");
    assert_eq!(col(&e, &r.layer, "B01003_001E", "48201000100"), Some(1000.0));
    assert_eq!(col(&e, &r.layer, "B01003_001E", "40109999999"), None);
    assert!(r.layer.name.contains("Census ACS5"));
    assert_eq!(r.layer.geom_type, "Point");
    assert!(r.notes.iter().any(|n| n.contains("tract level, 2 states")));
    assert!(r.notes.iter().any(|n| n.contains("No API key was used")));
    // Two states = two requests; no key in the URL.
    let reqs: Vec<String> = seen.lock().unwrap().clone();
    assert_eq!(reqs.len(), 2, "{reqs:?}");
    assert!(reqs.iter().all(|u| u.contains("/2023/acs/acs5?get=B01003_001E,B19013_001E&for=tract:*&in=state:") && !u.contains("key=")));
    // The source layer is untouched, and the new layer is cited as the Census Data API and inherits the source's citations.
    assert_eq!(e.layer(&src.id).unwrap().fields.len(), src.fields.len());
    let s = e.sources().unwrap();
    let c = s.iter().find(|x| x.citation.key == "census-api:acs5:2023").expect("cited");
    assert!(c.citation.license.contains("not endorsed or certified by the Census Bureau") && c.citation.complete);
    assert_eq!(c.uses.len(), 1);
    // Joining the same variable again would overwrite a column: refused.
    let err = e.census_join_from(&base, &r.layer.id, "GEOID", &spec(&["B01003_001E"]), None).unwrap_err().to_string();
    assert!(err.contains("already has a field called B01003_001E"), "{err}");
}

#[test]
fn d2_numbers_that_lost_their_leading_zero_still_join_and_the_level_is_guessed() {
    let (base, _) = start();
    let e = Engine::new().unwrap();
    // State FIPS as numbers: Alabama is 1, Arizona 4, California 6.
    let src = layer(&e, "States", &[json!(1), json!(4), json!(6), json!(40)]);
    let r = e.census_join_from(&base, &src.id, "GEOID", &spec(&["B01003_001E"]), None).unwrap();
    assert_eq!((r.matched, r.unmatched), (4, 0), "{:?}", r.notes);
    assert_eq!(col(&e, &r.layer, "B01003_001E", "1.0"), Some(1000.0), "Alabama (1 → 01)");
    assert!(r.notes.iter().any(|n| n.contains("state level")));
    // County level by length: 5-digit GEOIDs, asked for state by state.
    let counties = layer(&e, "Counties", &[json!("40001"), json!("40003"), json!("48001")]);
    let c = e.census_join_from(&base, &counties.id, "GEOID", &spec(&["B01003_001E"]), None).unwrap();
    assert_eq!(c.matched, 3, "the mock serves counties 001 and 003 for each state asked for: Oklahoma 001 and 003, Texas 001");
    // A length no geography has needs the level chosen.
    let odd = layer(&e, "Odd", &[json!("12345678")]);
    assert!(e.census_join_from(&base, &odd.id, "GEOID", &spec(&["B01003_001E"]), None).unwrap_err().to_string().contains("choose the level"));
    let forced = Spec { level: Some(Level::County), ..spec(&["B01003_001E"]) };
    assert!(e.census_join_from(&base, &odd.id, "GEOID", &forced, None).is_ok());
}

#[test]
fn d2_failures_are_readable_and_the_key_never_appears() {
    let (base, _) = start();
    let e = Engine::new().unwrap();
    let src = layer(&e, "Tracts", &[json!("40109100100")]);
    let go = |vars: &[&str], key: Option<&str>, vintage: u32| {
        let s = Spec { vintage, ..spec(vars) };
        e.census_join_from(&base, &src.id, "GEOID", &s, key).unwrap_err().to_string()
    };
    let bad_var = go(&["BAD_VAR"], None, 2023);
    assert!(bad_var.contains("no variable 'BAD_VAR'") && bad_var.contains("variables.html"), "{bad_var}");
    let bad_key = go(&["B01003_001E"], Some("BADKEY"), 2023);
    assert!(bad_key.contains("did not accept the API key") && !bad_key.contains("BADKEY"), "{bad_key}");
    assert!(go(&["B01003_001E"], None, 1999).contains("2005") || go(&["B01003_001E"], None, 1999).contains("between"));
    let old = go(&["B01003_001E"], None, 2004);
    assert!(old.contains("between 2005"), "{old}");
    let nofield = e.census_join_from(&base, &src.id, "NOPE", &spec(&["B01003_001E"]), None).unwrap_err().to_string();
    assert!(nofield.contains("no field called 'NOPE'"));
    let dead = e.census_join_from("http://127.0.0.1:1/data", &src.id, "GEOID", &spec(&["B01003_001E"]), Some("SECRETKEY123")).unwrap_err().to_string();
    assert!(dead.contains("Could not reach the Census Bureau") && !dead.contains("SECRETKEY123"), "{dead}");
    let bad_dataset = Spec { dataset: "nope".into(), ..spec(&["B01003_001E"]) };
    assert!(e.census_join_from(&base, &src.id, "GEOID", &bad_dataset, None).unwrap_err().to_string().contains("Unknown Census dataset"));
}

/// The real API (ignored): Oklahoma counties, total population, no key needed for one request.
#[test]
#[ignore = "network: api.census.gov"]
fn d2_real_api_oklahoma_counties() {
    let e = Engine::new().unwrap();
    let src = layer(&e, "OK counties", &[json!("40109"), json!("40143")]);
    let r = e.census_join(&src.id, "GEOID", &spec(&["B01003_001E"]), std::env::var("CENSUS_API_KEY").ok().as_deref()).unwrap();
    let oklahoma_county = col(&e, &r.layer, "B01003_001E", "40109").unwrap();
    assert!(oklahoma_county > 600_000.0, "Oklahoma County population {oklahoma_county}");
}
