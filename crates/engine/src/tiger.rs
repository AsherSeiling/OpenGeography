//! Census TIGER sample data from the TIGERweb REST services
//! (tigerweb.geo.census.gov), downloaded as GeoJSON and cached on disk.
//!
//! Layers are looked up by name, not number, because TIGERweb renumbers
//! layers between vintages.

use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const BASE: &str = "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb";
/// Features per page. Tract and block pages are large (~15–20 MB at 2000), and
/// TIGERweb sometimes cuts a big response short or answers with an HTML error
/// page, so pages start smaller and halve again on failure.
const PAGE: usize = 1000;
const MIN_PAGE: usize = 250;
const ATTEMPTS: u32 = 3;
const MAX_FEATURES: usize = 250_000;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: &'static str,
    pub label: &'static str,
    /// Comes from the chosen year's tigerWMS_<vintage> service (boundaries);
    /// false = only TIGERweb's current Transportation/Hydro services exist.
    pub by_year: bool,
    /// Service for datasets that aren't by year.
    #[serde(skip)]
    service: &'static str,
    /// Layer names to try, in order (names differ between vintages).
    #[serde(skip)]
    layers: &'static [&'static str],
    /// Filtered by STATE/COUNTY attributes (true) or by the area's bounding box (false).
    #[serde(skip)]
    by_attribute: bool,
    /// Too large for a whole state: a county must be chosen.
    pub needs_county: bool,
}

const ZCTA: &[&str] = &["2020 Census ZIP Code Tabulation Areas", "2010 Census ZIP Code Tabulation Areas", "Zip Code Tabulation Areas", "ZIP Code Tabulation Areas"];

pub const DATASETS: &[Dataset] = &[
    Dataset { id: "states", label: "State boundary", by_year: true, service: "", layers: &["States"], by_attribute: true, needs_county: false },
    Dataset { id: "counties", label: "Counties", by_year: true, service: "", layers: &["Counties"], by_attribute: true, needs_county: false },
    // Decennial Census vintages carry POP100 (population) and HU100 (housing units).
    Dataset { id: "tracts", label: "Census tracts", by_year: true, service: "", layers: &["Census Tracts"], by_attribute: true, needs_county: false },
    Dataset { id: "block_groups", label: "Block groups", by_year: true, service: "", layers: &["Census Block Groups"], by_attribute: true, needs_county: false },
    Dataset { id: "blocks", label: "Census blocks", by_year: true, service: "", layers: &["Census Blocks", "2020 Census Blocks"], by_attribute: true, needs_county: true },
    Dataset { id: "places", label: "Cities and towns (incorporated places)", by_year: true, service: "", layers: &["Incorporated Places"], by_attribute: true, needs_county: false },
    Dataset { id: "zcta", label: "ZIP code areas (ZCTAs)", by_year: true, service: "", layers: ZCTA, by_attribute: false, needs_county: true },
    Dataset { id: "primary_roads", label: "Primary roads (interstates)", by_year: false, service: "Transportation", layers: &["Primary Roads"], by_attribute: false, needs_county: false },
    Dataset { id: "secondary_roads", label: "Secondary roads (US and state highways)", by_year: false, service: "Transportation", layers: &["Secondary Roads 72_1k scale"], by_attribute: false, needs_county: false },
    Dataset { id: "local_roads", label: "Local roads (all streets)", by_year: false, service: "Transportation", layers: &["Local Roads"], by_attribute: false, needs_county: true },
    Dataset { id: "railroads", label: "Railroads", by_year: false, service: "Transportation", layers: &["Railroads"], by_attribute: false, needs_county: false },
    Dataset { id: "water_areas", label: "Lakes and rivers (areas)", by_year: false, service: "Hydro", layers: &["Areal Hydrography"], by_attribute: false, needs_county: true },
    Dataset { id: "water_lines", label: "Streams (lines)", by_year: false, service: "Hydro", layers: &["Linear Hydrography"], by_attribute: false, needs_county: true },
];

pub fn dataset(id: &str) -> Result<&'static Dataset> {
    DATASETS.iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown TIGER dataset '{id}'"))
}

/// A TIGERweb vintage (year of the geography).
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Vintage {
    /// "Current", "ACS2023", "Census2020"…
    pub id: String,
    /// Year of the geography (the current year for "Current").
    pub year: i32,
    pub label: String,
    /// Decennial Census geography (has blocks and population counts).
    pub decennial: bool,
}

/// Vintage ids are spliced into service URLs: only accept TIGERweb's forms.
fn check_vintage(v: &str) -> Result<()> {
    let ok = v == "Current" || ["ACS", "Census"].iter().any(|p| v.strip_prefix(p).is_some_and(|y| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit())));
    if !ok {
        bail!("Unknown TIGER year '{v}'");
    }
    Ok(())
}

/// The vintages TIGERweb offers, newest first ("Current" on top).
pub fn vintages() -> Result<Vec<Vintage>> {
    static CACHE: OnceLock<Vec<Vintage>> = OnceLock::new();
    if let Some(v) = CACHE.get() {
        return Ok(v.clone());
    }
    let v = get_json(&format!("{BASE}?f=json"))?;
    let mut out: Vec<Vintage> = v["services"]
        .as_array()
        .map(|ss| {
            ss.iter()
                .filter_map(|s| s["name"].as_str()?.strip_prefix("TIGERweb/tigerWMS_").map(String::from))
                .filter(|id| check_vintage(id).is_ok())
                .map(|id| {
                    let year = id.trim_start_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(9999);
                    let decennial = id.starts_with("Census");
                    let label = if id == "Current" { "Current (latest)".to_string() } else if decennial { format!("{year} Census (decennial)") } else { format!("{year} (ACS geography)") };
                    Vintage { id, year, label, decennial }
                })
                .collect()
        })
        .unwrap_or_default();
    if out.is_empty() {
        bail!("TIGERweb didn't list any years");
    }
    out.sort_by(|a, b| b.year.cmp(&a.year).then(b.decennial.cmp(&a.decennial)));
    let _ = CACHE.set(out.clone());
    Ok(out)
}

/// The service a dataset comes from for a vintage.
fn service_for(d: &Dataset, vintage: &str) -> String {
    if d.by_year {
        format!("tigerWMS_{vintage}")
    } else {
        d.service.to_string()
    }
}

/// Which datasets a vintage has (e.g. blocks only exist for decennial Census
/// years and Current): (dataset id, available).
pub fn availability(vintage: &str) -> Result<Vec<(String, bool)>> {
    check_vintage(vintage)?;
    Ok(DATASETS.iter().map(|d| (d.id.to_string(), find_layer(&service_for(d, vintage), d.layers).is_ok())).collect())
}

fn get_json(url: &str) -> Result<Value> {
    let mut last = None;
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2 * attempt as u64));
        }
        match get_json_once(url) {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("TIGERweb request failed")))
}

fn get_json_once(url: &str) -> Result<Value> {
    let resp = ureq::get(url)
        .set("User-Agent", "GIS-Studio (open-source GIS)")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| anyhow!("Could not reach the Census TIGERweb service: {e}"))?;
    let status = resp.status();
    let content_type = resp.content_type().to_string();
    // Read the whole body (pages can exceed ureq's 10 MB string limit) so a bad
    // response can be described instead of only failing to parse.
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), 512 * 1024 * 1024), &mut body)
        .map_err(|e| anyhow!("The TIGERweb download was cut off ({e}). The service may be busy; try again."))?;
    let v = parse_body(&body, status, &content_type)?;
    if let Some(err) = v.get("error") {
        bail!("TIGERweb error: {}", err.get("message").and_then(|m| m.as_str()).unwrap_or("unknown"));
    }
    Ok(v)
}

/// Parse a TIGERweb response, or say in plain words what came back instead.
fn parse_body(body: &[u8], status: u16, content_type: &str) -> Result<Value> {
    match serde_json::from_slice::<Value>(body) {
        Ok(v) => Ok(v),
        Err(e) => {
            let text = String::from_utf8_lossy(&body[..body.len().min(400)]);
            let snippet: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect();
            if text.trim_start().starts_with('<') {
                let title = text.split("<title>").nth(1).and_then(|t| t.split("</title>").next()).map(|t| format!(" (\"{}\")", t.trim())).unwrap_or_default();
                bail!("TIGERweb sent a web page instead of data{title}, HTTP {status}. The Census service is probably busy or down; try again in a few minutes.");
            }
            if e.is_eof() {
                bail!("The TIGERweb response ended early ({} bytes, HTTP {status}). The connection or the service dropped; try again, or choose a county for a smaller download.", body.len());
            }
            bail!("TIGERweb returned something that isn't JSON (HTTP {status}, {content_type}): {snippet}")
        }
    }
}

pub(crate) fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Layer number for a layer name within a service (cached).
fn layer_id(service: &str, name: &str) -> Result<i64> {
    static CACHE: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();
    let key = format!("{service}/{name}");
    if let Some(id) = CACHE.get_or_init(Default::default).lock().get(&key) {
        return Ok(*id);
    }
    let v = get_json(&format!("{BASE}/{service}/MapServer?f=json"))?;
    let id = v["layers"]
        .as_array()
        .and_then(|ls| ls.iter().find(|l| l["name"].as_str() == Some(name)))
        .and_then(|l| l["id"].as_i64())
        .ok_or_else(|| anyhow!("TIGERweb no longer has a layer named '{name}'"))?;
    CACHE.get_or_init(Default::default).lock().insert(key, id);
    Ok(id)
}

/// The first of `names` that the service has.
fn find_layer(service: &str, names: &[&str]) -> Result<i64> {
    for n in names {
        if let Ok(id) = layer_id(service, n) {
            return Ok(id);
        }
    }
    bail!("This year's TIGER data has no '{}' layer", names[0])
}

fn query_url(service: &str, layer: i64, params: &[(&str, String)]) -> String {
    let q: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect();
    format!("{BASE}/{service}/MapServer/{layer}/query?{}", q.join("&"))
}

fn area_where(state: &str, county: Option<&str>) -> String {
    match county {
        Some(c) => format!("STATE='{state}' AND COUNTY='{c}'"),
        None => format!("STATE='{state}'"),
    }
}

fn check_code(code: &str, len: usize) -> Result<()> {
    if code.len() != len || !code.chars().all(|c| c.is_ascii_digit()) {
        bail!("Invalid FIPS code '{code}'");
    }
    Ok(())
}

/// Counties of a state: (3-digit county code, name), sorted by name.
/// Counties of a state in a vintage (they change: e.g. Connecticut's 2022 planning regions).
pub fn counties(state: &str, vintage: &str) -> Result<Vec<(String, String)>> {
    check_code(state, 2)?;
    check_vintage(vintage)?;
    let svc = format!("tigerWMS_{vintage}");
    let id = layer_id(&svc, "Counties")?;
    let v = get_json(&query_url(
        &svc,
        id,
        &[("where", area_where(state, None)), ("outFields", "COUNTY,NAME".into()), ("returnGeometry", "false".into()), ("f", "json".into())],
    ))?;
    let mut out: Vec<(String, String)> = v["features"]
        .as_array()
        .map(|fs| {
            fs.iter()
                .filter_map(|f| Some((f["attributes"]["COUNTY"].as_str()?.to_string(), f["attributes"]["NAME"].as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// Bounding box (lon/lat) of a state or county, for datasets without STATE/COUNTY fields.
fn area_extent(state: &str, county: Option<&str>, vintage: &str) -> Result<String> {
    let (layer, name) = if county.is_some() { ("Counties", "county") } else { ("States", "state") };
    let svc = format!("tigerWMS_{vintage}");
    let id = layer_id(&svc, layer)?;
    let v = get_json(&query_url(
        &svc,
        id,
        &[("where", area_where(state, county)), ("returnExtentOnly", "true".into()), ("outSR", "4326".into()), ("f", "json".into())],
    ))?;
    let e = &v["extent"];
    match (e["xmin"].as_f64(), e["ymin"].as_f64(), e["xmax"].as_f64(), e["ymax"].as_f64()) {
        (Some(a), Some(b), Some(c), Some(d)) => Ok(format!("{a},{b},{c},{d}")),
        _ => bail!("Could not find that {name} in TIGERweb"),
    }
}

/// Download a dataset for a state (and optionally one county) as a GeoJSON
/// file in `cache_dir`, reusing an earlier download when present.
pub fn fetch(id: &str, vintage: &str, state: &str, county: Option<&str>, cache_dir: &Path, report: &dyn Fn(f64, &str) -> Result<()>) -> Result<PathBuf> {
    let d = dataset(id)?;
    check_code(state, 2)?;
    if let Some(c) = county {
        check_code(c, 3)?;
    }
    if d.needs_county && county.is_none() {
        bail!("{} is too large for a whole state: choose a county.", d.label);
    }
    std::fs::create_dir_all(cache_dir)?;
    check_vintage(vintage)?;
    let svc = service_for(d, vintage);
    let path = cache_dir.join(format!("tiger3_{}_{}_{}{}.geojson", svc, d.id, state, county.map(|c| format!("_{c}")).unwrap_or_default()));
    if path.exists() {
        report(1.0, &format!("{} (already downloaded)", d.label))?;
        return Ok(path);
    }
    report(0.0, &format!("{}: counting features…", d.label))?;
    let layer = find_layer(&svc, d.layers).map_err(|_| anyhow!("{} isn't available for this year — choose a decennial Census year or Current.", d.label))?;
    let mut filter: Vec<(&str, String)> = if d.by_attribute {
        vec![("where", area_where(state, if d.id == "states" { None } else { county }))]
    } else {
        vec![
            ("where", "1=1".into()),
            ("geometry", area_extent(state, county, if d.by_year { vintage } else { "Current" })?),
            ("geometryType", "esriGeometryEnvelope".into()),
            ("inSR", "4326".into()),
            ("spatialRel", "esriSpatialRelIntersects".into()),
        ]
    };
    let mut count_q = filter.clone();
    count_q.push(("returnCountOnly", "true".into()));
    count_q.push(("f", "json".into()));
    let total = get_json(&query_url(&svc, layer, &count_q)).ok().and_then(|v| v["count"].as_u64()).unwrap_or(0) as usize;
    filter.extend([
        ("outFields", "*".into()),
        ("returnGeometry", "true".into()),
        ("outSR", "4326".into()),
        ("geometryPrecision", "6".into()),
        ("orderByFields", "OBJECTID".into()),
        ("f", "geojson".into()),
    ]);
    let mut features: Vec<Value> = Vec::new();
    let mut page_size = PAGE;
    loop {
        let mut params = filter.clone();
        params.push(("resultOffset", features.len().to_string()));
        params.push(("resultRecordCount", page_size.to_string()));
        let page = match get_json(&query_url(&svc, layer, &params)) {
            Ok(p) => p,
            // A big page that keeps failing: ask for smaller ones from here on.
            Err(_) if page_size > MIN_PAGE => {
                page_size /= 2;
                report(if total > 0 { features.len() as f64 / total as f64 } else { 0.0 }, &format!("{}: the service is slow, retrying with smaller pages…", d.label))?;
                continue;
            }
            Err(e) => return Err(e),
        };
        let got = page["features"].as_array().cloned().unwrap_or_default();
        let n = got.len();
        features.extend(got);
        if total > 0 {
            report(features.len() as f64 / total as f64, &format!("{}: {} of {total} features", d.label, features.len()))?;
        }
        let more = page["exceededTransferLimit"].as_bool().unwrap_or(false)
            || page["properties"]["exceededTransferLimit"].as_bool().unwrap_or(false);
        if n == 0 || (!more && n < page_size) {
            break;
        }
        if features.len() >= MAX_FEATURES {
            bail!("More than {MAX_FEATURES} features: choose a county to make the download smaller.");
        }
    }
    if features.is_empty() {
        bail!("TIGERweb returned no {} for this area", d.label.to_lowercase());
    }
    let fc = json!({ "type": "FeatureCollection", "features": features });
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, serde_json::to_vec(&fc)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::parse_body;

    #[test]
    fn bad_responses_are_described_not_just_not_json() {
        assert!(parse_body(br#"{"type":"FeatureCollection","features":[]}"#, 200, "application/geo+json").is_ok());
        let html = parse_body(b"<html><head><title>503 Service Unavailable</title></head></html>", 200, "text/html").unwrap_err().to_string();
        assert!(html.contains("web page instead of data") && html.contains("503 Service Unavailable"), "{html}");
        let cut = parse_body(br#"{"type":"FeatureCollection","features":[{"type":"Feat"#, 200, "application/geo+json").unwrap_err().to_string();
        assert!(cut.contains("ended early"), "{cut}");
        let other = parse_body(b"Service temporarily overloaded", 200, "text/plain").unwrap_err().to_string();
        assert!(other.contains("isn't JSON") && other.contains("overloaded"), "{other}");
    }
}
