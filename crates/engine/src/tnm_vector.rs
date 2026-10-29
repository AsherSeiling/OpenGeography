//! USGS The National Map vector data: hydrography (NHD, WBD), transportation,
//! structures, geographic names (GNIS) and federal lands, queried from the
//! TNM ArcGIS REST services for a lon/lat box.
//!
//! These services reject GeoJSON output when results are ordered (which
//! paging needs), so features come as Esri JSON and are converted here.
//! The servers are often slow, so requests are retried.

use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

const NHD: &str = "https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer";
const WBD: &str = "https://hydro.nationalmap.gov/arcgis/rest/services/wbd/MapServer";
const TRANS: &str = "https://carto.nationalmap.gov/arcgis/rest/services/transportation/MapServer";
const STRUCT: &str = "https://carto.nationalmap.gov/arcgis/rest/services/structures/MapServer";
const NAMES: &str = "https://carto.nationalmap.gov/arcgis/rest/services/geonames/MapServer";
const GOV: &str = "https://carto.nationalmap.gov/arcgis/rest/services/govunits/MapServer";

const PAGE: usize = 1000;
const MAX_FEATURES: usize = 300_000;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    /// (service, layer name). Several layers are merged into one layer,
    /// with a CATEGORY field naming the source layer.
    #[serde(skip)]
    sources: &'static [(&'static str, &'static str)],
}

pub const DATASETS: &[Dataset] = &[
    Dataset { id: "nhd_flowlines", group: "Hydrography", label: "Streams and rivers (NHD flowlines)", sources: &[(NHD, "Flowline - Large Scale")] },
    Dataset { id: "nhd_waterbodies", group: "Hydrography", label: "Lakes, ponds and reservoirs (NHD waterbodies)", sources: &[(NHD, "Waterbody - Large Scale")] },
    Dataset { id: "nhd_areas", group: "Hydrography", label: "Wide rivers and other water areas (NHD areas)", sources: &[(NHD, "Area - Large Scale")] },
    Dataset { id: "nhd_points", group: "Hydrography", label: "Springs, wells and other water points", sources: &[(NHD, "Point")] },
    Dataset { id: "wbd_huc8", group: "Hydrography", label: "Watersheds — HUC-8 subbasins", sources: &[(WBD, "8-digit HU  (Subbasin)")] },
    Dataset { id: "wbd_huc10", group: "Hydrography", label: "Watersheds — HUC-10", sources: &[(WBD, "10-digit HU (Watershed)")] },
    Dataset { id: "wbd_huc12", group: "Hydrography", label: "Watersheds — HUC-12 subwatersheds", sources: &[(WBD, "12-digit HU (Subwatershed)")] },
    Dataset { id: "roads_major", group: "Transportation", label: "Highways (interstate, US and state routes, ramps)", sources: &[(TRANS, "Controlled-access Highways"), (TRANS, "Secondary Highways"), (TRANS, "Ramps")] },
    Dataset { id: "roads_local", group: "Transportation", label: "Local roads and streets", sources: &[(TRANS, "Local Connecting Roads"), (TRANS, "Local Roads")] },
    Dataset { id: "roads_4wd", group: "Transportation", label: "4WD roads", sources: &[(TRANS, "4WD Roads")] },
    Dataset { id: "trails", group: "Transportation", label: "Trails", sources: &[(TRANS, "Trails")] },
    Dataset { id: "railroads", group: "Transportation", label: "Railroads", sources: &[(TRANS, "Railroads")] },
    Dataset { id: "airports", group: "Transportation", label: "Airports", sources: &[(TRANS, "Airports")] },
    Dataset { id: "st_schools", group: "Structures", label: "Schools, colleges and trade schools", sources: &[(STRUCT, "Schools"), (STRUCT, "Colleges/Universities"), (STRUCT, "Technical/Trade Schools")] },
    Dataset {
        id: "st_emergency",
        group: "Structures",
        label: "Hospitals, fire, EMS, police and prisons",
        sources: &[(STRUCT, "Hospitals/Medical Centers"), (STRUCT, "Ambulance Services"), (STRUCT, "Fire Stations/EMS Stations"), (STRUCT, "Police Stations"), (STRUCT, "Prisons/Correctional Facilities")],
    },
    Dataset {
        id: "st_government",
        group: "Structures",
        label: "Government buildings, cemeteries and landmarks",
        sources: &[(STRUCT, "Post Offices"), (STRUCT, "City/Town Halls"), (STRUCT, "Courthouses"), (STRUCT, "State Capitols"), (STRUCT, "Cemeteries"), (STRUCT, "Historic Sites/Points of Interest")],
    },
    Dataset {
        id: "st_recreation",
        group: "Structures",
        label: "Campgrounds, trailheads, cabins and shelters",
        sources: &[(STRUCT, "Campgrounds"), (STRUCT, "Trailheads"), (STRUCT, "Cabins"), (STRUCT, "Shelters"), (STRUCT, "Picnic Areas"), (STRUCT, "Visitor/Information Centers"), (STRUCT, "Ranger Stations")],
    },
    Dataset { id: "gn_places", group: "Geographic names (GNIS)", label: "Cities, towns and populated places", sources: &[(NAMES, "Incorporated Places (Civil)"), (NAMES, "Unincorporated Places (Census)"), (NAMES, "Populated Places")] },
    Dataset { id: "gn_landforms", group: "Geographic names (GNIS)", label: "Summits, ridges, valleys and other landforms", sources: &[(NAMES, "Landforms")] },
    Dataset { id: "gn_water", group: "Geographic names (GNIS)", label: "Stream, lake and spring names", sources: &[(NAMES, "Streams (Mouth)"), (NAMES, "Other Hydrographic Features")] },
    Dataset {
        id: "gu_federal",
        group: "Boundaries",
        label: "Federal lands (parks, forests, wilderness, BLM, military)",
        sources: &[(GOV, "National Park"), (GOV, "National Forest"), (GOV, "National Wilderness"), (GOV, "National Grassland"), (GOV, "US Fish & Wildlife Service"), (GOV, "Bureau of Land Management"), (GOV, "Military Reserve")],
    },
];

pub fn dataset(id: &str) -> Result<&'static Dataset> {
    DATASETS.iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown National Map dataset '{id}'"))
}

pub(crate) fn host(url: &str) -> &str {
    url.trim_start_matches("https://").trim_start_matches("http://").split('/').next().unwrap_or(url)
}

/// Wait before the first retry of a slow service, in milliseconds (doubles each time). Tests set it low.
pub static RETRY_BACKOFF_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(2000);

/// GET JSON, retrying slow or failing servers (timeouts, 5xx).
pub(crate) fn get_json(url: &str) -> Result<Value> {
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(RETRY_BACKOFF_MS.load(std::sync::atomic::Ordering::Relaxed) * attempt));
        }
        let resp = ureq::get(url).set("User-Agent", "GIS-Studio (open-source GIS)").timeout(Duration::from_secs(150)).call();
        match resp {
            Ok(r) => {
                let v: Value = match serde_json::from_reader(std::io::BufReader::new(r.into_reader())) {
                    Ok(v) => v,
                    Err(e) => {
                        last = format!("unreadable reply ({e})");
                        continue;
                    }
                };
                if let Some(err) = v.get("error") {
                    let code = err["code"].as_i64().unwrap_or(0);
                    last = err["message"].as_str().unwrap_or("unknown error").to_string();
                    if code >= 500 {
                        continue;
                    }
                    bail!("The service at {} said: {last}", host(url));
                }
                return Ok(v);
            }
            Err(ureq::Error::Status(code, _)) if code < 500 => bail!("The service at {} refused the request (HTTP {code})", host(url)),
            Err(e) => last = e.to_string(),
        }
    }
    bail!("The service at {} isn't responding — try again later or use a smaller area. ({last})", host(url))
}

#[derive(Clone)]
pub(crate) struct LayerMeta {
    id: i64,
    oid: String,
    dates: Vec<String>,
}

/// Layer number, object-id field and date fields for a layer name. Names can
/// appear twice (a label layer and a feature layer); the feature layer comes last.
pub(crate) fn layer_meta(service: &str, name: &str) -> Result<LayerMeta> {
    static CACHE: OnceLock<Mutex<HashMap<String, LayerMeta>>> = OnceLock::new();
    let key = format!("{service}/{name}");
    if let Some(m) = CACHE.get_or_init(Default::default).lock().get(&key) {
        return Ok(m.clone());
    }
    // "#28" names a layer by number; anything else by its name.
    let id = match name.strip_prefix('#').and_then(|n| n.parse::<i64>().ok()) {
        Some(id) => id,
        None => {
            let v = get_json(&format!("{service}?f=json"))?;
            v["layers"]
                .as_array()
                .and_then(|ls| ls.iter().filter(|l| l["name"].as_str() == Some(name) && l["subLayerIds"].is_null()).last())
                .and_then(|l| l["id"].as_i64())
                .ok_or_else(|| anyhow!("The service at {} has no layer named '{name}'", host(service)))?
        }
    };
    let lv = get_json(&format!("{service}/{id}?f=json"))?;
    let fields = lv["fields"].as_array().cloned().unwrap_or_default();
    let oid = fields
        .iter()
        .find(|f| f["type"] == "esriFieldTypeOID")
        .and_then(|f| f["name"].as_str())
        .unwrap_or("OBJECTID")
        .to_string();
    let dates = fields.iter().filter(|f| f["type"] == "esriFieldTypeDate").filter_map(|f| f["name"].as_str().map(String::from)).collect();
    let m = LayerMeta { id, oid, dates };
    CACHE.get_or_init(Default::default).lock().insert(key, m.clone());
    Ok(m)
}

fn query_url(service: &str, layer: i64, bbox: [f64; 4], extra: &[(&str, String)]) -> String {
    let [w, s, e, n] = bbox;
    let mut params: Vec<(&str, String)> = vec![
        ("where", "1=1".into()),
        ("geometry", format!("{w},{s},{e},{n}")),
        ("geometryType", "esriGeometryEnvelope".into()),
        ("inSR", "4326".into()),
        ("spatialRel", "esriSpatialRelIntersects".into()),
    ];
    params.extend(extra.iter().cloned());
    let q: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", crate::tiger::enc(v))).collect();
    format!("{service}/{layer}/query?{}", q.join("&"))
}

fn count(service: &str, m: &LayerMeta, bbox: [f64; 4]) -> Option<usize> {
    let v = get_json(&query_url(service, m.id, bbox, &[("returnCountOnly", "true".into()), ("f", "json".into())])).ok()?;
    v["count"].as_u64().map(|c| c as usize)
}

/// Signed area of a ring (positive = counter-clockwise).
fn ring_area(r: &[Value]) -> f64 {
    let pts: Vec<(f64, f64)> = r.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect();
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % pts.len()];
        a += x0 * y1 - x1 * y0;
    }
    a / 2.0
}

fn point_in_ring(x: f64, y: f64, r: &[Value]) -> bool {
    let pts: Vec<(f64, f64)> = r.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect();
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let ((xi, yi), (xj, yj)) = (pts[i], pts[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Esri JSON geometry → GeoJSON geometry. Esri polygons list rings flat:
/// outer rings run clockwise, holes counter-clockwise.
pub fn esri_to_geojson(g: &Value) -> Option<Value> {
    if let (Some(x), Some(y)) = (g["x"].as_f64(), g["y"].as_f64()) {
        return Some(json!({ "type": "Point", "coordinates": [x, y] }));
    }
    if let Some(pts) = g["points"].as_array() {
        return (!pts.is_empty()).then(|| json!({ "type": "MultiPoint", "coordinates": pts }));
    }
    if let Some(paths) = g["paths"].as_array() {
        return match paths.len() {
            0 => None,
            1 => Some(json!({ "type": "LineString", "coordinates": paths[0] })),
            _ => Some(json!({ "type": "MultiLineString", "coordinates": paths })),
        };
    }
    let rings = g["rings"].as_array()?;
    let rings: Vec<&Vec<Value>> = rings.iter().filter_map(|r| r.as_array()).filter(|r| r.len() >= 4).collect();
    let (mut outers, mut holes): (Vec<Vec<Value>>, Vec<&Vec<Value>>) = (Vec::new(), Vec::new());
    for r in &rings {
        if ring_area(r) <= 0.0 {
            // GeoJSON wants outer rings counter-clockwise.
            outers.push(vec![Value::Array(r.iter().rev().cloned().collect())]);
        } else {
            holes.push(r);
        }
    }
    if outers.is_empty() {
        // Unexpected orientation: treat every ring as its own polygon.
        outers = rings.iter().map(|r| vec![Value::Array((*r).clone())]).collect();
        holes.clear();
    }
    for h in holes {
        let (x, y) = (h[0][0].as_f64().unwrap_or(0.0), h[0][1].as_f64().unwrap_or(0.0));
        let host = outers.iter_mut().find(|o| o[0].as_array().is_some_and(|r| point_in_ring(x, y, r)));
        match host {
            Some(o) => o.push(Value::Array(h.iter().rev().cloned().collect())),
            None => outers.push(vec![Value::Array(h.clone())]),
        }
    }
    match outers.len() {
        0 => None,
        1 => Some(json!({ "type": "Polygon", "coordinates": outers.remove(0) })),
        _ => Some(json!({ "type": "MultiPolygon", "coordinates": outers })),
    }
}

/// Esri epoch-millisecond dates → ISO dates.
fn fix_dates(props: &mut Map<String, Value>, dates: &[String]) {
    for d in dates {
        if let Some(ms) = props.get(d).and_then(|v| v.as_i64()) {
            let days = ms.div_euclid(86_400_000);
            let date = chrono_date(days);
            props.insert(d.clone(), Value::String(date));
        }
    }
}

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn chrono_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn cache_path(cache_dir: &Path, id: &str, bbox: [f64; 4]) -> PathBuf {
    let [w, s, e, n] = bbox;
    cache_dir.join(format!("tnm_{id}_{w:.4}_{s:.4}_{e:.4}_{n:.4}.geojson").replace('-', "m"))
}

/// Download a dataset for a lon/lat box as GeoJSON in `cache_dir` (reused if
/// already downloaded), reporting progress (0..1, or NaN while the total is
/// still unknown, plus a message) to `report`. Returns None when the area has
/// no such features.
pub fn fetch(id: &str, bbox: [f64; 4], cache_dir: &Path, report: &(dyn Fn(f64, &str) -> Result<()> + Sync)) -> Result<Option<PathBuf>> {
    let d = dataset(id)?;
    fetch_sources(d.label, id, d.sources, bbox, cache_dir, report)
}

/// `fetch` for any list of (service URL, layer name or "#number") on ArcGIS REST servers; several layers are merged with a
/// CATEGORY field naming the source layer. `cache_id` names the cached file. Used by the National Map datasets and the generic catalog (D1).
pub fn fetch_sources(label: &str, cache_id: &str, sources: &[(&str, &str)], bbox: [f64; 4], cache_dir: &Path, report: &(dyn Fn(f64, &str) -> Result<()> + Sync)) -> Result<Option<PathBuf>> {
    let id = cache_id;
    let [w, s, e, n] = bbox;
    if !(w < e && s < n) {
        bail!("The area is empty");
    }
    std::fs::create_dir_all(cache_dir)?;
    let path = cache_path(cache_dir, id, bbox);
    if path.exists() {
        report(1.0, &format!("{label} (already downloaded)"))?;
        return Ok(Some(path));
    }

    report(0.0, &format!("{label}: downloading from {}…", host(sources[0].0)))?;
    let metas: Vec<(String, String, LayerMeta)> = sources.iter().map(|(svc, name)| Ok((svc.to_string(), name.to_string(), layer_meta(svc, name)?))).collect::<Result<_>>()?;

    // Counting costs as much as a page on slow servers, so count in the
    // background while downloading; the bar becomes exact once counts arrive.
    // A detached thread, so a slow count never delays a finished download.
    let total: std::sync::Arc<Mutex<Option<usize>>> = Default::default();
    {
        let (total, jobs): (_, Vec<(String, LayerMeta)>) = (total.clone(), metas.iter().map(|(svc, _, m)| (svc.clone(), m.clone())).collect());
        std::thread::spawn(move || {
            let counts: Vec<Option<usize>> = std::thread::scope(|cs| {
                let hs: Vec<_> = jobs.iter().map(|(svc, m)| cs.spawn(move || count(svc, m, bbox))).collect();
                hs.into_iter().map(|h| h.join().ok().flatten()).collect()
            });
            if counts.iter().all(Option::is_some) {
                *total.lock() = Some(counts.iter().flatten().sum());
            }
        });
    }
    let features = (|| -> Result<Vec<Value>> {
        let mut features: Vec<Value> = Vec::new();
        for (svc, name, m) in &metas {
            let mut offset = 0;
            loop {
                let page = get_json(&query_url(
                    svc,
                    m.id,
                    bbox,
                    &[
                        ("outFields", "*".into()),
                        ("returnGeometry", "true".into()),
                        ("outSR", "4326".into()),
                        ("geometryPrecision", "6".into()),
                        ("orderByFields", m.oid.clone()),
                        ("resultOffset", offset.to_string()),
                        ("resultRecordCount", PAGE.to_string()),
                        ("f", "json".into()),
                    ],
                ))
                .with_context(|| format!("Downloading {label}"))?;
                let got = page["features"].as_array().cloned().unwrap_or_default();
                let n = got.len();
                for f in got {
                    let Some(geom) = esri_to_geojson(&f["geometry"]) else { continue };
                    let mut props = f["attributes"].as_object().cloned().unwrap_or_default();
                    fix_dates(&mut props, &m.dates);
                    if sources.len() > 1 {
                        props.insert("CATEGORY".into(), Value::String(name.to_string()));
                    }
                    features.push(json!({ "type": "Feature", "properties": props, "geometry": geom }));
                }
                offset += n;
                match *total.lock() {
                    Some(t) if t > MAX_FEATURES => bail!("{label} has about {t} features in this area (limit {MAX_FEATURES}). Zoom in or pick a smaller area."),
                    Some(t) => report((features.len() as f64 / t.max(1) as f64).min(1.0), &format!("{label}: {} of {t} features", features.len()))?,
                    None => report(f64::NAN, &format!("{label}: {} features so far", features.len()))?,
                }
                let more = page["exceededTransferLimit"].as_bool().unwrap_or(false);
                if n == 0 || (!more && n < PAGE) {
                    break;
                }
                if features.len() >= MAX_FEATURES {
                    bail!("More than {MAX_FEATURES} features: pick a smaller area.");
                }
            }
        }
        Ok(features)
    })()?;
    report(1.0, &format!("{label}: {} features", features.len()))?;
    if features.is_empty() {
        return Ok(None);
    }
    let fc = json!({ "type": "FeatureCollection", "features": features });
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, serde_json::to_vec(&fc)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_esri_polygons_with_holes() {
        // Clockwise outer square with a counter-clockwise hole, plus a second polygon.
        let g = json!({ "rings": [
            [[0,0],[0,10],[10,10],[10,0],[0,0]],
            [[2,2],[4,2],[4,4],[2,4],[2,2]],
            [[20,0],[20,5],[25,5],[25,0],[20,0]]
        ]});
        let out = esri_to_geojson(&g).unwrap();
        assert_eq!(out["type"], "MultiPolygon");
        let polys = out["coordinates"].as_array().unwrap();
        assert_eq!(polys.len(), 2);
        assert_eq!(polys[0].as_array().unwrap().len(), 2, "hole attached to the first polygon");
        assert!(ring_area(polys[0][0].as_array().unwrap()) > 0.0, "outer ring is counter-clockwise");
        assert!(ring_area(polys[0][1].as_array().unwrap()) < 0.0, "hole is clockwise");
    }

    #[test]
    fn converts_points_lines_and_dates() {
        assert_eq!(esri_to_geojson(&json!({"x": 1.5, "y": 2.0})).unwrap()["type"], "Point");
        assert_eq!(esri_to_geojson(&json!({"paths": [[[0,0],[1,1]]]})).unwrap()["type"], "LineString");
        assert_eq!(esri_to_geojson(&json!({"paths": [[[0,0],[1,1]],[[2,2],[3,3]]]})).unwrap()["type"], "MultiLineString");
        assert!(esri_to_geojson(&json!({})).is_none());
        assert_eq!(chrono_date(0), "1970-01-01");
        assert_eq!(chrono_date(19_723), "2024-01-01");
    }
}
