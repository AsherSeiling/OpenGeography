//! Tissot indicatrix layer (R-H11): small geodesic circles on a lon/lat grid, stored in WGS 84 so the map draws them in
//! the map CRS and their shape shows that CRS's distortion. Each circle carries the engine's own factors for the CRS
//! (the same numbers as the status bar and the layout scale bar).

use crate::layers::LayerInfo;
use crate::layout::geo::{factors, Body, FrameCrs};
use crate::Engine;
use anyhow::{bail, Result};
use geographiclib_rs::DirectGeodesic;
use serde_json::{json, Map};

const VERTICES: usize = 48;

impl Engine {
    /// Circles of ground radius `radius_km` every `spacing_deg` (1–90) of lon/lat; returns the layer and the grid points the CRS cannot represent.
    pub fn tissot_layer(&self, crs: &str, spacing_deg: f64, radius_km: f64) -> Result<(LayerInfo, usize)> {
        if !(1.0..=90.0).contains(&spacing_deg) {
            bail!("The spacing must be between 1° and 90°");
        }
        if !(1.0..=3000.0).contains(&radius_km) {
            bail!("The circle radius must be between 1 km and 3000 km");
        }
        let body = Body::get("earth")?;
        let fc = FrameCrs::new(crs, &body)?;
        let geod = body.geodesic();
        let mut feats = Vec::new();
        let mut skipped = 0usize;
        let n_lat = (180.0 / spacing_deg).floor() as i32;
        let n_lon = (360.0 / spacing_deg).floor() as i32;
        for i in 0..=n_lat {
            let lat = -90.0 + i as f64 * spacing_deg;
            if lat.abs() > 85.0 {
                continue; // a circle over a pole is not a circle on the grid
            }
            for j in 0..n_lon {
                let lon = -180.0 + j as f64 * spacing_deg;
                let Ok((x, y)) = fc.from_lonlat(lon, lat) else { skipped += 1; continue };
                if !x.is_finite() || !y.is_finite() {
                    skipped += 1;
                    continue;
                }
                // A point the CRS cannot map back to the same place (the far side of a cone, beyond a zone) has no meaningful scale.
                match fc.to_lonlat(x, y) {
                    Ok((lo, la)) if (la - lat).abs() < 1e-6 && ((lo - lon + 540.0).rem_euclid(360.0) - 180.0).abs() < 1e-6 => {}
                    _ => {
                        skipped += 1;
                        continue;
                    }
                }
                let Ok(f) = factors(&body, &fc, x, y) else { skipped += 1; continue };
                if !f.k_east.is_finite() || !f.k_north.is_finite() {
                    skipped += 1;
                    continue;
                }
                let mut ring: Vec<(f64, f64)> = Vec::with_capacity(VERTICES + 1);
                for v in 0..=VERTICES {
                    let az = 360.0 * (v % VERTICES) as f64 / VERTICES as f64;
                    let (la, lo): (f64, f64) = geod.direct(lat, lon, az, radius_km * 1000.0);
                    ring.push((lo, la));
                }
                // Area ratio = area of the projected circle ÷ area of the geodesic circle on the ellipsoid (a cap of
                // radius r on a sphere of the local Gaussian radius). Scale along grid east/north is not the principal
                // scale away from the central meridian, so k_east × k_north is not the area ratio.
                let area_ratio = if fc.projected {
                    let proj: Option<Vec<(f64, f64)>> = ring.iter().map(|&(lo, la)| fc.from_lonlat(lo, la).ok().filter(|p| p.0.is_finite() && p.1.is_finite())).collect();
                    proj.map(|pts| {
                        let a2: f64 = pts.windows(2).map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1).sum::<f64>().abs() / 2.0 * fc.unit_m * fc.unit_m;
                        let (a, e2) = (6378137.0_f64, 0.00669437999014_f64);
                        let rg2 = a * a * (1.0 - e2) / (1.0 - e2 * lat.to_radians().sin().powi(2)).powi(2);
                        let rg = rg2.sqrt();
                        // The ring is a 48-gon inscribed in the circle: compare with the area of that polygon, not of the circle.
                        let step = 2.0 * std::f64::consts::PI / VERTICES as f64;
                        a2 / (2.0 * std::f64::consts::PI * rg2 * (1.0 - (radius_km * 1000.0 / rg).cos()) * (step.sin() / step))
                    })
                } else {
                    None
                };
                let ring: Vec<String> = ring.iter().map(|(lo, la)| format!("{lo:.6} {la:.6}")).collect();
                let mut p = Map::new();
                p.insert("lon".into(), json!(lon));
                p.insert("lat".into(), json!(lat));
                p.insert("k_east".into(), json!(f.k_east));
                p.insert("k_north".into(), json!(f.k_north));
                p.insert("area_ratio".into(), json!(area_ratio));
                p.insert("convergence".into(), json!(f.convergence_deg));
                feats.push((format!("POLYGON(({}))", ring.join(", ")), p));
            }
        }
        if feats.is_empty() {
            bail!("No circle could be drawn: this coordinate system cannot represent the grid points");
        }
        let layer = self.create_features(&format!("Tissot circles ({})", crate::crs::short(crs)), &feats)?;
        Ok((layer, skipped))
    }
}
