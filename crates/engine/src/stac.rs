//! STAC search and cloud-optimised GeoTIFF clipping (phase D3). A STAC API lists scenes (items) for an area, date range and cloud
//! limit; one asset of one scene is clipped to the area straight from the cloud (GDAL `/vsicurl/` reads only the pixels needed)
//! and added as a raster layer, cited. Only public, anonymously readable data is offered: Landsat and NAIP on AWS are
//! requester-pays, so they are not in the catalog (see docs/DECISIONS.md).

use crate::sources::Citation;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;

/// The most cells one clip may have; a larger request is refused with the size it would have.
pub const MAX_CELLS: u64 = 60_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub label: String,
    /// Offers a cloud-cover limit.
    pub cloud: bool,
    /// (asset key, label)
    pub assets: Vec<(String, String)>,
    pub author: String,
    pub title: String,
    pub publisher: String,
    pub url: String,
    pub license: String,
    pub annotation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Api {
    pub id: String,
    pub label: String,
    pub url: String,
    pub collections: Vec<Collection>,
}

pub fn catalog() -> &'static [Api] {
    static C: std::sync::OnceLock<Vec<Api>> = std::sync::OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("../assets/stac_catalog.json")).expect("assets/stac_catalog.json is valid"))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSpec {
    /// STAC API address; a catalog API when absent.
    pub api: Option<String>,
    pub collection: String,
    pub bbox: [f64; 4],
    /// "2024-06-01/2024-09-30", "2024-06-01/..", or a single day.
    pub datetime: Option<String>,
    /// Keep scenes with at most this percentage of cloud.
    pub max_cloud: Option<f64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub key: String,
    pub title: String,
    /// A web address GDAL can read (an `s3://` address of a public bucket is rewritten).
    pub href: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub collection: String,
    pub datetime: Option<String>,
    pub cloud_cover: Option<f64>,
    pub assets: Vec<Asset>,
}

/// A web address for an asset href: `https://` and `http://` as they are, `s3://bucket/key` as the bucket's own https address.
pub fn https_href(href: &str) -> Result<String> {
    if href.starts_with("https://") || href.starts_with("http://") {
        return Ok(href.to_string());
    }
    if let Some(rest) = href.strip_prefix("s3://") {
        let (bucket, key) = rest.split_once('/').ok_or_else(|| anyhow!("'{href}' is not a valid S3 address"))?;
        return Ok(format!("https://{bucket}.s3.amazonaws.com/{key}"));
    }
    bail!("'{}' is not an address this app can read (only https and public S3 data)", href.chars().take(60).collect::<String>())
}

/// Check the dates before any request.
pub fn check_datetime(d: &str) -> Result<()> {
    let ok_day = |s: &str| s == ".." || (s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-' && s.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()));
    let parts: Vec<&str> = d.split('/').collect();
    if parts.len() > 2 || !parts.iter().all(|p| ok_day(p)) || (parts.len() == 2 && parts[0] == ".." && parts[1] == "..") {
        bail!("Dates look like 2024-06-01/2024-09-30 (from/to), or a single day 2024-06-01");
    }
    if let [a, b] = parts[..] {
        if a != ".." && b != ".." && a > b {
            bail!("The start date {a} is after the end date {b}");
        }
    }
    Ok(())
}

fn api_url(spec: &SearchSpec) -> Result<String> {
    match &spec.api {
        Some(u) => {
            let u = u.trim().trim_end_matches('/').to_string();
            if !(u.starts_with("https://") || u.starts_with("http://")) {
                bail!("Enter the STAC address starting with https://");
            }
            Ok(u)
        }
        None => Ok(catalog().first().map(|a| a.url.clone()).ok_or_else(|| anyhow!("No STAC catalog"))?),
    }
}

/// Search a STAC API; newest scenes first.
pub fn search(spec: &SearchSpec) -> Result<Vec<Item>> {
    let [w, s, e, n] = spec.bbox;
    if !(w < e && s < n) || spec.bbox.iter().any(|v| !v.is_finite()) {
        bail!("The area is empty");
    }
    if let Some(d) = &spec.datetime {
        check_datetime(d)?;
    }
    if spec.collection.trim().is_empty() {
        bail!("Choose a collection");
    }
    let limit = spec.limit.unwrap_or(20).clamp(1, 100);
    let base = api_url(spec)?;
    let mut body = json!({ "collections": [spec.collection], "bbox": spec.bbox, "limit": limit, "sortby": [{ "field": "properties.datetime", "direction": "desc" }] });
    if let Some(d) = &spec.datetime {
        body["datetime"] = json!(d);
    }
    if let Some(c) = spec.max_cloud {
        body["query"] = json!({ "eo:cloud_cover": { "lte": c } });
    }
    let host = crate::tnm_vector::host(&base).to_string();
    let resp = ureq::post(&format!("{base}/search"))
        .set("User-Agent", "GIS-Studio (open-source GIS)")
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(60))
        .send_string(&body.to_string());
    let text = match resp {
        Ok(r) => {
            let mut t = String::new();
            r.into_reader().take(50_000_000).read_to_string(&mut t)?;
            t
        }
        Err(ureq::Error::Status(code, r)) => {
            let mut t = String::new();
            r.into_reader().take(1000).read_to_string(&mut t).ok();
            let msg = serde_json::from_str::<Value>(&t).ok().and_then(|v| v["description"].as_str().or(v["message"].as_str()).map(String::from)).unwrap_or_else(|| t.chars().take(150).collect());
            bail!("The STAC service at {host} refused the search (HTTP {code}){}", if msg.is_empty() { String::new() } else { format!(": {msg}") });
        }
        Err(e) => bail!("Could not reach the STAC service at {host}: {e}"),
    };
    let v: Value = serde_json::from_str(&text).map_err(|_| anyhow!("The service at {host} answered with something that isn't a STAC search result"))?;
    let feats = v["features"].as_array().ok_or_else(|| anyhow!("The service at {host} answered with something that isn't a STAC search result (no features list). Is this a STAC API address?"))?;
    let mut out = Vec::new();
    for f in feats {
        let Some(id) = f["id"].as_str() else { continue };
        let assets: Vec<Asset> = f["assets"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(_, a)| a["type"].as_str().is_some_and(|t| t.starts_with("image/tiff")))
                    .filter_map(|(k, a)| Some(Asset { key: k.clone(), title: a["title"].as_str().unwrap_or(k).to_string(), href: https_href(a["href"].as_str()?).ok()? }))
                    .collect()
            })
            .unwrap_or_default();
        out.push(Item {
            id: id.to_string(),
            collection: f["collection"].as_str().unwrap_or(&spec.collection).to_string(),
            datetime: f["properties"]["datetime"].as_str().map(String::from),
            cloud_cover: f["properties"]["eo:cloud_cover"].as_f64(),
            assets,
        });
    }
    Ok(out)
}

/// Citation for a catalog collection, with the scene named.
pub fn citation(collection: &str, item_id: &str, asset: &str) -> Option<Citation> {
    let c = catalog().iter().flat_map(|a| a.collections.iter()).find(|c| c.id == collection)?;
    Some(Citation {
        key: format!("stac:{}", c.id),
        author: c.author.clone(),
        year: "n.d.".into(),
        title: c.title.clone(),
        version: format!("STAC collection {}", c.id),
        publisher: c.publisher.clone(),
        url: c.url.clone(),
        kind: "Web service".into(),
        license: c.license.clone(),
        annotation: format!("{} Scene {item_id}, asset {asset}.", c.annotation),
        complete: true,
    })
}

/// Clip one cloud-optimised GeoTIFF to a lon/lat box. Refuses a clip of more than `MAX_CELLS` cells, saying how big it would be.
pub fn clip(href: &str, bbox: [f64; 4], max_cells: u64) -> Result<PathBuf> {
    use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
    let url = https_href(href)?;
    let host = crate::tnm_vector::host(&url).to_string();
    let vsi = format!("/vsicurl/{url}");
    let ds = gdal::Dataset::open(&vsi).map_err(|e| anyhow!("Could not open the image at {host}: {e}"))?;
    let gt = ds.geo_transform().map_err(|_| anyhow!("The image at {host} has no georeferencing"))?;
    let srs = ds.spatial_ref().map_err(|_| anyhow!("The image at {host} has no coordinate system"))?;
    let mut ll = SpatialRef::from_epsg(4326)?;
    ll.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let mut tgt = srs.clone();
    tgt.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    let t = CoordTransform::new(&ll, &tgt)?;
    let [w, s, e, n] = bbox;
    let (mut xs, mut ys) = (vec![w, e, w, e, (w + e) / 2.0, (w + e) / 2.0], vec![s, s, n, n, s, n]);
    t.transform_coords(&mut xs, &mut ys, &mut [])?;
    let (xmin, xmax) = (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let (ymin, ymax) = (ys.iter().cloned().fold(f64::INFINITY, f64::min), ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    if !(xmin.is_finite() && ymin.is_finite()) {
        bail!("The area cannot be shown in the image's coordinate system");
    }
    // Does the box meet the image at all?
    let (cols, rows) = ds.raster_size();
    let (ix0, iy1) = (gt[0], gt[3]);
    let (ix1, iy0) = (gt[0] + gt[1] * cols as f64, gt[3] + gt[5] * rows as f64);
    if xmax < ix0.min(ix1) || xmin > ix0.max(ix1) || ymax < iy0.min(iy1) || ymin > iy0.max(iy1) {
        bail!("This image does not cover the area. Pick a scene whose footprint contains it.");
    }
    let cells = ((xmax - xmin) / gt[1].abs()).ceil().max(1.0) as u64 * ((ymax - ymin) / gt[5].abs()).ceil().max(1.0) as u64;
    let bands = ds.raster_count() as u64;
    if cells > max_cells {
        bail!("Clipping this area would give {} million cells ({} columns × {} rows). The limit is {} million; choose a smaller area.", cells / 1_000_000, ((xmax - xmin) / gt[1].abs()).ceil(), ((ymax - ymin) / gt[5].abs()).ceil(), max_cells / 1_000_000);
    }
    let _ = bands;
    let out = crate::raster::new_store_path("stac_clip", "tif");
    let args: Vec<String> = ["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-projwin_srs", "EPSG:4326", "-projwin"].map(String::from).into_iter().chain([bbox[0].to_string(), bbox[3].to_string(), bbox[2].to_string(), bbox[1].to_string()]).collect();
    crate::raster::translate(&ds, &out, &args).map_err(|e| anyhow!("Could not read the image from {host}: {e}"))?;
    Ok(out)
}

/// One asset of one scene to clip.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSpec {
    pub href: String,
    pub bbox: [f64; 4],
    pub label: String,
    pub collection: String,
    pub item_id: String,
    pub asset: String,
    pub area_label: String,
}
