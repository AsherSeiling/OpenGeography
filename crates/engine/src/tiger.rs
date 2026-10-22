//! Census TIGER sample data from the TIGERweb REST services
//! (tigerweb.geo.census.gov), downloaded as GeoJSON and cached on disk.
//!
//! Layers are looked up by name, not number, because TIGERweb renumbers
//! layers between vintages.

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const BASE: &str = "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb";
const PAGE: usize = 2000;
const MAX_FEATURES: usize = 250_000;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: &'static str,
    pub label: &'static str,
    #[serde(skip)]
    service: &'static str,
    #[serde(skip)]
    layer: &'static str,
    /// Filtered by STATE/COUNTY attributes (true) or by the area's bounding box (false).
    #[serde(skip)]
    by_attribute: bool,
    /// Too large for a whole state: a county must be chosen.
    pub needs_county: bool,
}

pub const DATASETS: &[Dataset] = &[
    Dataset { id: "states", label: "State boundary", service: "tigerWMS_Current", layer: "States", by_attribute: true, needs_county: false },
    Dataset { id: "counties", label: "Counties", service: "tigerWMS_Current", layer: "Counties", by_attribute: true, needs_county: false },
    Dataset { id: "tracts", label: "Census tracts", service: "tigerWMS_Current", layer: "Census Tracts", by_attribute: true, needs_county: false },
    Dataset { id: "block_groups", label: "Block groups", service: "tigerWMS_Current", layer: "Census Block Groups", by_attribute: true, needs_county: false },
    Dataset { id: "blocks", label: "Census blocks (2020)", service: "tigerWMS_Current", layer: "2020 Census Blocks", by_attribute: true, needs_county: true },
    Dataset { id: "places", label: "Cities and towns (incorporated places)", service: "tigerWMS_Current", layer: "Incorporated Places", by_attribute: true, needs_county: false },
    Dataset { id: "zcta", label: "ZIP code areas (ZCTAs)", service: "tigerWMS_Current", layer: "2020 Census ZIP Code Tabulation Areas", by_attribute: false, needs_county: true },
    Dataset { id: "primary_roads", label: "Primary roads (interstates)", service: "Transportation", layer: "Primary Roads", by_attribute: false, needs_county: false },
    Dataset { id: "secondary_roads", label: "Secondary roads (US and state highways)", service: "Transportation", layer: "Secondary Roads 72_1k scale", by_attribute: false, needs_county: false },
    Dataset { id: "local_roads", label: "Local roads (all streets)", service: "Transportation", layer: "Local Roads", by_attribute: false, needs_county: true },
    Dataset { id: "railroads", label: "Railroads", service: "Transportation", layer: "Railroads", by_attribute: false, needs_county: false },
    Dataset { id: "water_areas", label: "Lakes and rivers (areas)", service: "Hydro", layer: "Areal Hydrography", by_attribute: false, needs_county: true },
    Dataset { id: "water_lines", label: "Streams (lines)", service: "Hydro", layer: "Linear Hydrography", by_attribute: false, needs_county: true },
];

pub fn dataset(id: &str) -> Result<&'static Dataset> {
    DATASETS.iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown TIGER dataset '{id}'"))
}

fn get_json(url: &str) -> Result<Value> {
    let resp = ureq::get(url)
        .set("User-Agent", "GIS-Studio (open-source GIS)")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| anyhow!("Could not reach the Census TIGERweb service: {e}"))?;
    // Stream-parse: pages of blocks or roads can exceed ureq's 10 MB string limit.
    let v: Value = serde_json::from_reader(std::io::BufReader::new(resp.into_reader())).context("TIGERweb returned something that isn't JSON")?;
    if let Some(err) = v.get("error") {
        bail!("TIGERweb error: {}", err.get("message").and_then(|m| m.as_str()).unwrap_or("unknown"));
    }
    Ok(v)
}

fn enc(s: &str) -> String {
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
pub fn counties(state: &str) -> Result<Vec<(String, String)>> {
    check_code(state, 2)?;
    let id = layer_id("tigerWMS_Current", "Counties")?;
    let v = get_json(&query_url(
        "tigerWMS_Current",
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
fn area_extent(state: &str, county: Option<&str>) -> Result<String> {
    let (layer, name) = if county.is_some() { ("Counties", "county") } else { ("States", "state") };
    let id = layer_id("tigerWMS_Current", layer)?;
    let v = get_json(&query_url(
        "tigerWMS_Current",
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
pub fn fetch(id: &str, state: &str, county: Option<&str>, cache_dir: &Path) -> Result<PathBuf> {
    let d = dataset(id)?;
    check_code(state, 2)?;
    if let Some(c) = county {
        check_code(c, 3)?;
    }
    if d.needs_county && county.is_none() {
        bail!("{} is too large for a whole state: choose a county.", d.label);
    }
    std::fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(format!("tiger_{}_{}{}.geojson", d.id, state, county.map(|c| format!("_{c}")).unwrap_or_default()));
    if path.exists() {
        return Ok(path);
    }
    let layer = layer_id(d.service, d.layer)?;
    let mut filter: Vec<(&str, String)> = if d.by_attribute {
        vec![("where", area_where(state, if d.id == "states" { None } else { county }))]
    } else {
        vec![
            ("where", "1=1".into()),
            ("geometry", area_extent(state, county)?),
            ("geometryType", "esriGeometryEnvelope".into()),
            ("inSR", "4326".into()),
            ("spatialRel", "esriSpatialRelIntersects".into()),
        ]
    };
    filter.extend([
        ("outFields", "*".into()),
        ("returnGeometry", "true".into()),
        ("outSR", "4326".into()),
        ("geometryPrecision", "6".into()),
        ("orderByFields", "OBJECTID".into()),
        ("f", "geojson".into()),
    ]);
    let mut features: Vec<Value> = Vec::new();
    loop {
        let mut params = filter.clone();
        params.push(("resultOffset", features.len().to_string()));
        params.push(("resultRecordCount", PAGE.to_string()));
        let page = get_json(&query_url(d.service, layer, &params))?;
        let got = page["features"].as_array().cloned().unwrap_or_default();
        let n = got.len();
        features.extend(got);
        let more = page["exceededTransferLimit"].as_bool().unwrap_or(false)
            || page["properties"]["exceededTransferLimit"].as_bool().unwrap_or(false);
        if n == 0 || (!more && n < PAGE) {
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
