//! Generic ArcGIS REST catalog (phase D1): any MapServer/FeatureServer layer, from a bundled catalog of public datasets or a
//! URL the user pastes. Downloads for a lon/lat box through the same paging, Esri JSON conversion and retries as the National Map
//! datasets (`tnm_vector::fetch_sources`). Each entry carries its own citation.

use crate::sources::Citation;
use crate::tnm_vector::{get_json, host};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

/// One dataset of the bundled catalog (`assets/arcgis_catalog.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub group: String,
    pub label: String,
    pub service: String,
    /// Layer names, or "#number"; several are merged with a CATEGORY field.
    pub layers: Vec<String>,
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
    C.get_or_init(|| serde_json::from_str(include_str!("../assets/arcgis_catalog.json")).expect("assets/arcgis_catalog.json is valid"))
}

pub fn entry(id: &str) -> Result<&'static Entry> {
    catalog().iter().find(|e| e.id == id).ok_or_else(|| anyhow!("Unknown ArcGIS dataset '{id}'"))
}

/// What to download: a catalog entry, or one layer of a service URL.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub entry_id: Option<String>,
    pub url: Option<String>,
    /// Layer number or name for a custom URL.
    pub layer: Option<String>,
    pub label: Option<String>,
}

/// A layer of a service, for the "find layers" step of a custom URL.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLayer {
    pub id: i64,
    pub name: String,
    pub geometry_type: Option<String>,
}

/// Normalise a pasted service URL: trims, drops a trailing `/`, a `?f=…` query and a layer number or `/query`.
pub fn clean_service_url(url: &str) -> Result<String> {
    let u = url.trim().split('?').next().unwrap_or("").trim_end_matches('/').to_string();
    if !(u.starts_with("https://") || u.starts_with("http://")) {
        bail!("Enter the service address starting with https://");
    }
    let u = u.strip_suffix("/query").unwrap_or(&u).to_string();
    let lower = u.to_lowercase();
    let idx = lower.find("/mapserver").map(|i| (i, "/mapserver".len())).or_else(|| lower.find("/featureserver").map(|i| (i, "/featureserver".len())));
    match idx {
        Some((i, n)) => Ok(u[..i + n].to_string()),
        None => bail!("That doesn't look like an ArcGIS MapServer or FeatureServer address (it should contain /MapServer or /FeatureServer)."),
    }
}

/// The layers a service offers (group layers left out), with their geometry types.
pub fn describe_service(url: &str) -> Result<Vec<ServiceLayer>> {
    let svc = clean_service_url(url)?;
    let v = get_json(&format!("{svc}?f=json"))?;
    let layers = v["layers"].as_array().ok_or_else(|| anyhow!("{} didn't list any layers. Is this a MapServer or FeatureServer address?", host(&svc)))?;
    let mut out = Vec::new();
    for l in layers.iter().filter(|l| l["subLayerIds"].is_null()) {
        let (Some(id), Some(name)) = (l["id"].as_i64(), l["name"].as_str()) else { continue };
        let geometry_type = get_json(&format!("{svc}/{id}?f=json")).ok().and_then(|d| d["geometryType"].as_str().map(|g| g.trim_start_matches("esriGeometry").to_string()));
        out.push(ServiceLayer { id, name: name.to_string(), geometry_type });
    }
    if out.is_empty() {
        bail!("{} has no drawable layers", host(&svc));
    }
    Ok(out)
}

/// The citation for a spec: the catalog entry's own, or a stub for a custom service that the user completes.
pub fn citation(spec: &Spec) -> Result<(Citation, String)> {
    if let Some(id) = &spec.entry_id {
        let e = entry(id)?;
        let c = Citation {
            key: format!("arcgis:{}", e.id),
            author: e.author.clone(),
            year: e.year.clone(),
            title: e.title.clone(),
            version: e.version.clone(),
            publisher: e.publisher.clone(),
            url: e.url.clone(),
            kind: "Web service".into(),
            license: e.license.clone(),
            annotation: e.annotation.clone(),
            complete: true,
        };
        return Ok((c, e.label.clone()));
    }
    let svc = clean_service_url(spec.url.as_deref().ok_or_else(|| anyhow!("Choose a catalog dataset or enter a service address"))?)?;
    let layer = spec.layer.clone().ok_or_else(|| anyhow!("Choose a layer of the service"))?;
    let label = spec.label.clone().unwrap_or_else(|| format!("{} — {layer}", host(&svc)));
    let c = Citation {
        key: format!("arcgis:{svc}/{layer}"),
        author: String::new(),
        year: "n.d.".into(),
        title: label.clone(),
        version: String::new(),
        publisher: host(&svc).to_string(),
        url: svc.clone(),
        kind: "Web service".into(),
        license: String::new(),
        annotation: format!("ArcGIS REST layer {layer} of {svc}. Add the author, publisher and licence."),
        complete: false,
    };
    Ok((c, label))
}

/// The (service, layer) list and a cache id for a spec.
pub fn sources_of(spec: &Spec) -> Result<(Vec<(String, String)>, String)> {
    if let Some(id) = &spec.entry_id {
        let e = entry(id)?;
        return Ok((e.layers.iter().map(|l| (e.service.clone(), l.clone())).collect(), format!("arcgis_{}", e.id)));
    }
    let svc = clean_service_url(spec.url.as_deref().ok_or_else(|| anyhow!("Choose a catalog dataset or enter a service address"))?)?;
    let layer = spec.layer.clone().ok_or_else(|| anyhow!("Choose a layer of the service"))?;
    // A layer number stays a number; a name is looked up in the service.
    let l = if layer.chars().all(|c| c.is_ascii_digit()) && !layer.is_empty() { format!("#{layer}") } else { layer };
    let cache = format!("arcgis_{}", crate::crs::hash12(&format!("{svc}|{l}")));
    Ok((vec![(svc, l)], cache))
}
