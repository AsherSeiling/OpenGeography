//! OGC API – Features client (phase D5): the standard way newer public services (USGS Water Data, many state and city portals) publish
//! vector data. A box and a collection give GeoJSON pages that are followed through their `next` links, then added as a layer, cited.

use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::sources::{self, Citation};
use crate::tnm_vector::host;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;

/// Features per page asked for, and the most one download may hold.
const PAGE: usize = 1000;
pub const MAX_FEATURES: usize = 200_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub group: String,
    pub label: String,
    pub api: String,
    pub collection: String,
    pub author: String,
    pub year: String,
    pub title: String,
    pub version: String,
    pub publisher: String,
    pub url: String,
    pub license: String,
    pub annotation: String,
}

pub fn catalog() -> &'static [Entry] {
    static C: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("../assets/ogcapi_catalog.json")).expect("assets/ogcapi_catalog.json is valid"))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub entry_id: Option<String>,
    pub api: Option<String>,
    pub collection: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub title: String,
}

/// A pasted API address, cleaned: no trailing `/`, no `/collections…` tail, no query.
pub fn clean_api_url(url: &str) -> Result<String> {
    let u = url.trim().split('?').next().unwrap_or("").trim_end_matches('/').to_string();
    if !(u.starts_with("https://") || u.starts_with("http://")) {
        bail!("Enter the API address starting with https://");
    }
    Ok(match u.find("/collections") {
        Some(i) => u[..i].to_string(),
        None => u,
    })
}

/// `https://host:port` of an address.
fn origin(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("https", url));
    format!("{scheme}://{}", rest.split('/').next().unwrap_or(rest))
}

fn get(url: &str, accept: &str) -> Result<Value> {
    let h = host(url).to_string();
    let mut last = String::new();
    for attempt in 0..3u64 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(crate::tnm_vector::RETRY_BACKOFF_MS.load(std::sync::atomic::Ordering::Relaxed) * attempt));
        }
        match ureq::get(url).set("User-Agent", "GIS-Studio (open-source GIS)").set("Accept", accept).timeout(std::time::Duration::from_secs(90)).call() {
            Ok(r) => {
                let mut t = String::new();
                r.into_reader().take(200_000_000).read_to_string(&mut t)?;
                return serde_json::from_str(&t).map_err(|_| anyhow!("The service at {h} answered with something that isn't JSON. Is this an OGC API address?"));
            }
            Err(ureq::Error::Status(code, r)) if code < 500 => {
                let mut t = String::new();
                r.into_reader().take(500).read_to_string(&mut t).ok();
                let msg = serde_json::from_str::<Value>(&t).ok().and_then(|v| v["description"].as_str().or(v["detail"].as_str()).or(v["message"].as_str()).map(String::from)).unwrap_or_default();
                bail!("The service at {h} refused the request (HTTP {code}){}", if msg.is_empty() { String::new() } else { format!(": {msg}") });
            }
            Err(e) => last = e.to_string(),
        }
    }
    bail!("The service at {h} isn't responding — try again later or use a smaller area. ({last})")
}

/// The collections of an API that offer features.
pub fn collections(api: &str) -> Result<Vec<Collection>> {
    let base = clean_api_url(api)?;
    let v = get(&format!("{base}/collections?f=json"), "application/json")?;
    let list = v["collections"].as_array().ok_or_else(|| anyhow!("The service at {} has no list of collections. Is this an OGC API – Features address?", host(&base)))?;
    let out: Vec<Collection> = list
        .iter()
        .filter(|c| c["links"].as_array().is_some_and(|l| l.iter().any(|x| x["rel"] == "items")) || c["itemType"] == "feature")
        .filter_map(|c| Some(Collection { id: c["id"].as_str()?.to_string(), title: c["title"].as_str().unwrap_or(c["id"].as_str()?).to_string() }))
        .collect();
    if out.is_empty() {
        bail!("The service at {} offers no feature collections", host(&base));
    }
    Ok(out)
}

/// Fetch the features of `collection` in a lon/lat box, following `next` links. `report(fraction or NaN, message)`.
pub fn fetch(api: &str, collection: &str, bbox: [f64; 4], report: &dyn Fn(f64, &str) -> Result<()>) -> Result<Vec<Value>> {
    let base = clean_api_url(api)?;
    let [w, s, e, n] = bbox;
    if !(w < e && s < n) {
        bail!("The area is empty");
    }
    if collection.is_empty() || !collection.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:/".contains(c)) {
        bail!("'{collection}' is not a collection name");
    }
    let mut url = format!("{base}/collections/{collection}/items?f=json&bbox={w},{s},{e},{n}&limit={PAGE}");
    let mut feats: Vec<Value> = Vec::new();
    let mut matched: Option<u64> = None;
    loop {
        report(matched.map_or(f64::NAN, |m| (feats.len() as f64 / m.max(1) as f64).min(1.0)), &format!("{} features so far", feats.len()))?;
        let page = get(&url, "application/geo+json, application/json")?;
        let got = page["features"].as_array().ok_or_else(|| anyhow!("The service at {} answered without a list of features. Check the collection name '{collection}'.", host(&base)))?;
        matched = matched.or(page["numberMatched"].as_u64());
        feats.extend(got.iter().cloned());
        if feats.len() > MAX_FEATURES {
            bail!("More than {MAX_FEATURES} features in this area. Choose a smaller area.");
        }
        let next = page["links"].as_array().and_then(|l| l.iter().find(|x| x["rel"] == "next")).and_then(|x| x["href"].as_str());
        match next {
            Some(h) if !got.is_empty() => url = if h.starts_with("http") { h.to_string() } else if h.starts_with('/') { format!("{}{h}", origin(&base)) } else { format!("{base}/{h}") },
            _ => break,
        }
    }
    Ok(feats)
}

pub fn citation(spec: &Spec) -> Result<(Citation, String)> {
    if let Some(id) = &spec.entry_id {
        let e = catalog().iter().find(|e| &e.id == id).ok_or_else(|| anyhow!("Unknown OGC API dataset '{id}'"))?;
        return Ok((
            Citation { key: format!("ogcapi:{}", e.id), author: e.author.clone(), year: e.year.clone(), title: e.title.clone(), version: e.version.clone(), publisher: e.publisher.clone(), url: e.url.clone(), kind: "Web service".into(), license: e.license.clone(), annotation: e.annotation.clone(), complete: true },
            e.label.clone(),
        ));
    }
    let api = clean_api_url(spec.api.as_deref().ok_or_else(|| anyhow!("Choose a dataset or enter an API address"))?)?;
    let coll = spec.collection.clone().ok_or_else(|| anyhow!("Choose a collection"))?;
    let label = spec.label.clone().unwrap_or_else(|| format!("{} — {coll}", host(&api)));
    Ok((
        Citation { key: format!("ogcapi:{api}/{coll}"), author: String::new(), year: "n.d.".into(), title: label.clone(), version: "OGC API – Features".into(), publisher: host(&api).to_string(), url: api.clone(), kind: "Web service".into(), license: String::new(), annotation: format!("OGC API – Features collection {coll} of {api}. Add the author, publisher and licence."), complete: false },
        label,
    ))
}

impl Engine {
    /// Download a collection for a box and add it as a layer, cited. An area with no features adds nothing and returns None.
    pub fn ogc_import(&self, spec: &Spec, bbox: [f64; 4], area_label: &str) -> Result<Option<LayerInfo>> {
        let (cite, label) = citation(spec)?;
        let (api, coll) = match &spec.entry_id {
            Some(id) => {
                let e = catalog().iter().find(|e| &e.id == id).ok_or_else(|| anyhow!("Unknown OGC API dataset '{id}'"))?;
                (e.api.clone(), e.collection.clone())
            }
            None => (spec.api.clone().unwrap_or_default(), spec.collection.clone().unwrap_or_default()),
        };
        let task = Task::start(format!("Downloading {label} for {area_label}")).cancellable();
        let feats = fetch(&api, &coll, bbox, &|f, msg| {
            if f.is_nan() {
                task.detail(msg.to_string());
            } else {
                task.set(f * 0.85, Some(msg.to_string()));
            }
            task.check()
        })?;
        let feats: Vec<Value> = feats.into_iter().filter(|f| !f["geometry"].is_null()).collect();
        if feats.is_empty() {
            return Ok(None);
        }
        let dir = std::env::temp_dir().join("gis-studio-cache");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("ogc_{}.geojson", crate::crs::hash12(&format!("{api}|{coll}|{bbox:?}|{}", std::process::id()))));
        std::fs::write(&path, serde_json::to_vec(&json!({ "type": "FeatureCollection", "features": feats }))?)?;
        task.set(0.9, Some(format!("{label}: building the layer…")));
        let r = self.import_with(&path, &mut Task::start(format!("Adding {label}")));
        let _ = std::fs::remove_file(&path);
        let mut out = r?;
        let info = out.pop().ok_or_else(|| anyhow!("The data could not be read"))?;
        let info = self.rename_layer(&info.id, &label)?;
        self.with_conn(|c| sources::record(c, &cite, &info.id, &info.name))?;
        Ok(Some(info))
    }
}
