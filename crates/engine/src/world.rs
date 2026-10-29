//! World view helpers (RD-10): the bundled world outline and the reference lines (graticule, outline, central meridian,
//! standard parallels) that make the shape of a projection readable. All are ordinary WGS 84 layers; the world-mode display
//! table (display.rs) cuts and projects them like any other layer.

use crate::display::{display_info, world_window, WorldInfo};
use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::sources::{self, Citation};
use crate::Engine;
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

const NE_110M_LAND: &str = include_str!("../assets/ne_110m_land.geojson");
/// Densify reference lines to this many degrees between vertices, so a parallel curves in a conic.
const STEP_DEG: f64 = 1.0;
const TROPIC: f64 = 23.4365;

pub fn natural_earth_citation() -> Citation {
    Citation {
        key: "natural-earth-110m-land".into(),
        author: "Natural Earth".into(),
        year: "2022".into(),
        title: "Natural Earth 1:110m Physical Vectors: Land".into(),
        version: "5.1.2".into(),
        publisher: "Natural Earth (naturalearthdata.com)".into(),
        url: "https://www.naturalearthdata.com/downloads/110m-physical-vectors/110m-land/".into(),
        kind: "Dataset".into(),
        license: "Public domain (Natural Earth terms of use: no permission needed, credit appreciated).".into(),
        annotation: "Generalised land outline bundled with the app so the world view is recognisable without a download.".into(),
        complete: true,
    }
}

fn line(pts: impl Iterator<Item = (f64, f64)>) -> String {
    format!("LINESTRING({})", pts.map(|(x, y)| format!("{x:.6} {y:.6}")).collect::<Vec<_>>().join(", "))
}

/// A meridian from `s` to `n` degrees of latitude.
fn meridian(lon: f64, s: f64, n: f64) -> String {
    let steps = ((n - s) / STEP_DEG).ceil().max(1.0) as usize;
    line((0..=steps).map(|i| (lon, s + (n - s) * i as f64 / steps as f64)))
}

/// A parallel across the whole range of longitudes (-180..180).
fn parallel(lat: f64) -> String {
    line((0..=360).map(|i| (-180.0 + i as f64, lat)))
}

fn norm_lon(lon: f64) -> f64 {
    (lon + 540.0).rem_euclid(360.0) - 180.0
}

fn props(kind: &str, value: f64, label: String) -> Map<String, Value> {
    let mut p = Map::new();
    p.insert("kind".into(), json!(kind));
    p.insert("value".into(), json!(value));
    p.insert("label".into(), json!(label));
    p
}

fn dms_label(v: f64, pos: &str, neg: &str) -> String {
    format!("{}°{}", v.abs(), if v >= 0.0 { pos } else { neg })
}

impl Engine {
    /// The bundled Natural Earth 1:110m land polygons as a new layer, cited in Sources. No network.
    pub fn world_outline(&self) -> Result<LayerInfo> {
        // Unique per call: two outlines added at once must not share (and delete) one file.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("gis-studio-ne110m-{}-{}.geojson", std::process::id(), SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::write(&path, NE_110M_LAND)?;
        let mut task = Task::start("Adding the world outline");
        let r = self.import_with_public(&path, &mut task);
        let _ = std::fs::remove_file(&path);
        let mut layers = r?;
        let l = layers.pop().ok_or_else(|| anyhow::anyhow!("The bundled world outline could not be read"))?;
        let l = self.rename_layer(&l.id, "World outline (Natural Earth 110m)")?;
        self.with_conn(|c| sources::record(c, &natural_earth_citation(), &l.id, &l.name))?;
        Ok(l)
    }

    /// Graticule every `spacing_deg` (5–90) and the CRS's own reference lines, as two layers. With a world-capable `crs` the lines
    /// stop at its latitude limits; otherwise they span the globe.
    pub fn world_reference(&self, crs: &str, spacing_deg: f64) -> Result<Vec<LayerInfo>> {
        if !(5.0..=90.0).contains(&spacing_deg) {
            bail!("The graticule spacing must be between 5° and 90°");
        }
        let base = crs.trim_start().strip_prefix(crate::display::WORLD_PREFIX).unwrap_or(crs);
        let di = display_info(&format!("{}{base}", crate::display::WORLD_PREFIX), None).or_else(|_| display_info(base, Some([-180.0, -90.0, 180.0, 90.0])))?;
        let w: WorldInfo = match world_window(&di.proj4) {
            Ok(w) => w,
            Err(_) => WorldInfo { lon0: 0.0, lat_south: -90.0, lat_north: 90.0, note: None },
        };
        let (s, n) = (w.lat_south, w.lat_north);

        let mut grat: Vec<(String, Map<String, Value>)> = Vec::new();
        let mut lon = -180.0;
        while lon < 180.0 - 1e-9 {
            grat.push((meridian(lon, s.max(-90.0), n.min(90.0)), props("meridian", lon, dms_label(lon, "E", "W"))));
            lon += spacing_deg;
        }
        let mut lat = -90.0 + spacing_deg;
        while lat < 90.0 - 1e-9 {
            if lat >= s && lat <= n && lat.abs() > 1e-9 {
                grat.push((parallel(lat), props("parallel", lat, dms_label(lat, "N", "S"))));
            }
            lat += spacing_deg;
        }
        for (v, kind, label) in [(0.0, "equator", "Equator"), (TROPIC, "tropic", "Tropic of Cancer"), (-TROPIC, "tropic", "Tropic of Capricorn")] {
            if v >= s && v <= n {
                grat.push((parallel(v), props(kind, v, label.into())));
            }
        }

        // Lines that describe this CRS: the central meridian, the cut, the latitude limits, the standard parallels.
        let mut refs: Vec<(String, Map<String, Value>)> = Vec::new();
        refs.push((meridian(norm_lon(w.lon0), s.max(-90.0), n.min(90.0)), props("central meridian", w.lon0, format!("Central meridian {}", dms_label(w.lon0, "E", "W")))));
        refs.push((meridian(norm_lon(w.lon0 + 180.0), s.max(-90.0), n.min(90.0)), props("outline", w.lon0 + 180.0, "Edge of the map".into())));
        for lim in [s, n] {
            if lim.abs() < 89.9 {
                refs.push((parallel(lim), props("outline", lim, "Edge of the map".into())));
            }
        }
        for name in ["lat_1", "lat_2", "lat_ts"] {
            if let Some(v) = di.proj4.split_whitespace().find_map(|t| t.strip_prefix(&format!("+{name}=")).and_then(|x| x.parse::<f64>().ok())) {
                if v.abs() < 89.9 && !refs.iter().any(|r| r.1.get("kind") == Some(&json!("standard parallel")) && r.1.get("value") == Some(&json!(v))) {
                    refs.push((parallel(v), props("standard parallel", v, format!("Standard parallel {}", dms_label(v, "N", "S")))));
                }
            }
        }
        let short = crate::crs::short(base);
        let a = self.create_features(&format!("Graticule {spacing_deg}°"), &grat)?;
        let b = self.create_features(&format!("Reference lines ({short})"), &refs)?;
        Ok(vec![a, b])
    }
}
