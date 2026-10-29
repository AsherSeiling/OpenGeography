//! Small public JSON services (phase D6): USGS earthquakes (FDSN event service, GeoJSON) and NRCS SSURGO soil map units (Soil Data
//! Access). Each is one request for a lon/lat box, turned into a layer and cited.

use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::sources::{self, Citation};
use crate::tnm_vector::host;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map, Value};
use std::io::Read;

pub const QUAKE_URL: &str = "https://earthquake.usgs.gov/fdsnws/event/1/query";
pub const SDA_URL: &str = "https://sdmdataaccess.nrcs.usda.gov/Tabular/post.rest";
/// Soil map units are many and detailed: the area is limited to this many degrees on a side (about 25 km).
pub const SSURGO_MAX_DEG: f64 = 0.25;
/// The earthquake service refuses more than 20 000 events per request.
pub const QUAKE_MAX: usize = 20_000;

fn read(r: Result<ureq::Response, ureq::Error>, url: &str) -> Result<String> {
    let h = host(url).to_string();
    match r {
        Ok(r) => {
            let mut t = String::new();
            r.into_reader().take(300_000_000).read_to_string(&mut t)?;
            Ok(t)
        }
        Err(ureq::Error::Status(code, r)) => {
            let mut t = String::new();
            r.into_reader().take(400).read_to_string(&mut t).ok();
            bail!("The service at {h} refused the request (HTTP {code}){}", if t.trim().is_empty() { String::new() } else { format!(": {}", t.trim().chars().take(200).collect::<String>()) })
        }
        Err(e) => bail!("Could not reach the service at {h}: {e}"),
    }
}

pub fn quake_citation() -> Citation {
    Citation {
        key: "usgs-earthquakes".into(),
        author: "U.S. Geological Survey".into(),
        year: "n.d.".into(),
        title: "ANSS Comprehensive Earthquake Catalog (ComCat)".into(),
        version: "FDSN Event Web Service".into(),
        publisher: "U.S. Geological Survey, Earthquake Hazards Program".into(),
        url: "https://earthquake.usgs.gov/fdsnws/event/1/".into(),
        kind: "Web service".into(),
        license: "Public domain (U.S. Government work); USGS asks that it be credited.".into(),
        annotation: "Earthquake locations, magnitudes and times from the USGS catalog. Used for earthquake points.".into(),
        complete: true,
    }
}

pub fn ssurgo_citation() -> Citation {
    Citation {
        key: "nrcs-ssurgo".into(),
        author: "USDA Natural Resources Conservation Service".into(),
        year: "n.d.".into(),
        title: "Soil Survey Geographic Database (SSURGO)".into(),
        version: "Soil Data Access web service".into(),
        publisher: "USDA NRCS Soil Survey Staff".into(),
        url: "https://sdmdataaccess.nrcs.usda.gov/".into(),
        kind: "Web service".into(),
        license: "Public domain (U.S. Government work). Soil map units are generalised: use them for planning, not for a single lot.".into(),
        annotation: "Detailed soil map unit polygons with map unit key, symbol and name. Used for soil map units.".into(),
        complete: true,
    }
}

/// Date as `YYYY-MM-DD`.
fn check_day(d: &str) -> Result<()> {
    let ok = d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !ok {
        bail!("Dates look like 2024-06-01");
    }
    Ok(())
}

/// Earthquakes in a box as GeoJSON features.
pub fn earthquakes(base: &str, bbox: [f64; 4], start: Option<&str>, end: Option<&str>, min_mag: f64) -> Result<Vec<Value>> {
    let [w, s, e, n] = bbox;
    if !(w < e && s < n) {
        bail!("The area is empty");
    }
    for d in [start, end].into_iter().flatten().filter(|d| !d.is_empty()) {
        check_day(d)?;
    }
    if !(-2.0..=10.0).contains(&min_mag) {
        bail!("The smallest magnitude must be between -2 and 10");
    }
    let mut url = format!("{base}?format=geojson&minlatitude={s}&maxlatitude={n}&minlongitude={w}&maxlongitude={e}&minmagnitude={min_mag}&orderby=time&limit={QUAKE_MAX}");
    if let Some(d) = start.filter(|d| !d.is_empty()) {
        url.push_str(&format!("&starttime={d}"));
    }
    if let Some(d) = end.filter(|d| !d.is_empty()) {
        url.push_str(&format!("&endtime={d}"));
    }
    let text = read(ureq::get(&url).set("User-Agent", "GIS-Studio (open-source GIS)").timeout(std::time::Duration::from_secs(90)).call(), &url)?;
    let v: Value = serde_json::from_str(&text).map_err(|_| anyhow!("The earthquake service answered with something that isn't GeoJSON"))?;
    let feats = v["features"].as_array().ok_or_else(|| anyhow!("The earthquake service answered without a list of events"))?;
    if feats.len() >= QUAKE_MAX {
        bail!("There are {QUAKE_MAX} or more earthquakes in this area and period. Choose a smaller area, a shorter period or a larger smallest magnitude.");
    }
    Ok(feats.clone())
}

/// Soil map units intersecting a box: `(WKT, mukey, symbol, name)`.
pub fn ssurgo(base: &str, bbox: [f64; 4]) -> Result<Vec<(String, String, String, String)>> {
    let [w, s, e, n] = bbox;
    if !(w < e && s < n) {
        bail!("The area is empty");
    }
    if e - w > SSURGO_MAX_DEG || n - s > SSURGO_MAX_DEG {
        bail!("Soil map units are very detailed. Choose an area at most {SSURGO_MAX_DEG}° (about 25 km) on a side; this one is {:.2}° × {:.2}°.", e - w, n - s);
    }
    let q = format!(
        "SELECT mu.mukey, mu.musym, mu.muname, mp.mupolygongeo.STAsText() AS wkt \
         FROM SDA_Get_Mupolygonkey_from_intersection_with_WktWgs84('polygon(({w} {s}, {e} {s}, {e} {n}, {w} {n}, {w} {s}))') k \
         INNER JOIN mupolygon mp ON mp.mupolygonkey = k.mupolygonkey INNER JOIN mapunit mu ON mu.mukey = mp.mukey"
    );
    let body = json!({ "format": "JSON+COLUMNNAME", "query": q }).to_string();
    let text = read(ureq::post(base).set("User-Agent", "GIS-Studio (open-source GIS)").set("Content-Type", "application/json").timeout(std::time::Duration::from_secs(120)).send_string(&body), base)?;
    if text.trim().is_empty() {
        return Ok(vec![]); // the service answers an empty body when nothing matches
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| anyhow!("The soil service answered with something that isn't a table: {}", text.chars().take(150).collect::<String>()))?;
    let rows = v["Table"].as_array().ok_or_else(|| anyhow!("The soil service answered without a table"))?;
    let header: Vec<&str> = rows.first().and_then(|h| h.as_array()).map(|h| h.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
    let col = |n: &str| header.iter().position(|h| h.eq_ignore_ascii_case(n)).ok_or_else(|| anyhow!("The soil table has no '{n}' column"));
    let (ik, is, im, iw) = (col("mukey")?, col("musym")?, col("muname")?, col("wkt")?);
    let cell = |r: &Value, i: usize| r[i].as_str().unwrap_or("").to_string();
    Ok(rows.iter().skip(1).map(|r| (cell(r, iw), cell(r, ik), cell(r, is), cell(r, im))).filter(|r| !r.0.is_empty()).collect())
}

impl Engine {
    pub fn earthquakes_import(&self, bbox: [f64; 4], start: Option<&str>, end: Option<&str>, min_mag: f64, area_label: &str) -> Result<Option<LayerInfo>> {
        self.earthquakes_import_from(QUAKE_URL, bbox, start, end, min_mag, area_label)
    }

    pub fn earthquakes_import_from(&self, base: &str, bbox: [f64; 4], start: Option<&str>, end: Option<&str>, min_mag: f64, area_label: &str) -> Result<Option<LayerInfo>> {
        let task = Task::start(format!("Downloading earthquakes for {area_label}")).cancellable();
        task.set(0.2, Some("Asking the USGS earthquake catalog…".to_string()));
        let feats = earthquakes(base, bbox, start, end, min_mag)?;
        task.check()?;
        let rows: Vec<(String, Map<String, Value>)> = feats
            .iter()
            .filter_map(|f| {
                let c = f["geometry"]["coordinates"].as_array()?;
                let (x, y) = (c.first()?.as_f64()?, c.get(1)?.as_f64()?);
                let p = &f["properties"];
                let mut m = Map::new();
                m.insert("mag".into(), p["mag"].clone());
                m.insert("place".into(), p["place"].clone());
                // The service gives milliseconds since 1970; a date is easier to read and sort.
                m.insert("time".into(), p["time"].as_i64().map(|ms| json!(crate::tnm_vector::date_of_days(ms.div_euclid(86_400_000)))).unwrap_or(Value::Null));
                m.insert("depth_km".into(), c.get(2).cloned().unwrap_or(Value::Null));
                m.insert("event".into(), f["id"].clone());
                m.insert("type".into(), p["type"].clone());
                Some((format!("POINT({x} {y})"), m))
            })
            .collect();
        if rows.is_empty() {
            return Ok(None);
        }
        let info = self.create_features(&format!("Earthquakes — {area_label}"), &rows)?;
        self.with_conn(|c| sources::record(c, &quake_citation(), &info.id, &info.name))?;
        Ok(Some(info))
    }

    pub fn ssurgo_import(&self, bbox: [f64; 4], area_label: &str) -> Result<Option<LayerInfo>> {
        self.ssurgo_import_from(SDA_URL, bbox, area_label)
    }

    pub fn ssurgo_import_from(&self, base: &str, bbox: [f64; 4], area_label: &str) -> Result<Option<LayerInfo>> {
        let task = Task::start(format!("Downloading soil map units for {area_label}")).cancellable();
        task.set(0.2, Some("Asking NRCS Soil Data Access…".to_string()));
        let rows = ssurgo(base, bbox)?;
        task.check()?;
        if rows.is_empty() {
            return Ok(None);
        }
        let feats: Vec<(String, Map<String, Value>)> = rows
            .into_iter()
            .map(|(wkt, key, sym, name)| {
                let mut m = Map::new();
                m.insert("mukey".into(), json!(key));
                m.insert("musym".into(), json!(sym));
                m.insert("muname".into(), json!(name));
                (wkt, m)
            })
            .collect();
        let info = self.create_features(&format!("Soil map units (SSURGO) — {area_label}"), &feats)?;
        self.with_conn(|c| sources::record(c, &ssurgo_citation(), &info.id, &info.name))?;
        Ok(Some(info))
    }
}
