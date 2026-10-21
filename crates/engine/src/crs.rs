//! Coordinate reference system helpers. CRSs are stored as strings PROJ
//! understands ("EPSG:26914", WKT, or a PROJ string). We never guess a CRS
//! silently: unknown CRSs stay `None` and the UI asks the user.

use crate::db::lit;
use anyhow::{anyhow, Result};
use duckdb::Connection;
use serde::Serialize;

pub const WGS84: &str = "EPSG:4326";
pub const WEB_MERCATOR: &str = "EPSG:3857";
const MERC_R: f64 = 6378137.0;
pub const MERC_MAX_LAT: f64 = 85.051129;

/// UTM zone EPSG code (WGS 84) for a lon/lat. Used for metric operations
/// such as Buffer when the data is in a geographic or non-metric CRS.
pub fn utm_epsg(lon: f64, lat: f64) -> String {
    let zone = (((lon + 180.0) / 6.0).floor() as i32).clamp(0, 59) + 1;
    let base = if lat >= 0.0 { 32600 } else { 32700 };
    format!("EPSG:{}", base + zone)
}

/// Check that PROJ can use this CRS.
pub fn validate(conn: &Connection, crs: &str) -> Result<()> {
    let sql = format!(
        "SELECT ST_AsText(ST_Transform(ST_Point(0, 0), {}, {}, true))",
        lit(WGS84),
        lit(crs)
    );
    conn.query_row(&sql, [], |r| r.get::<_, String>(0))
        .map(|_| ())
        .map_err(|e| anyhow!("Unknown or invalid coordinate system '{crs}': {e}"))
}

pub fn transform_point(conn: &Connection, x: f64, y: f64, from: &str, to: &str) -> Result<(f64, f64)> {
    let sql = format!(
        "SELECT ST_X(g), ST_Y(g) FROM (SELECT ST_Transform(ST_Point(?, ?), {}, {}, true) AS g)",
        lit(from),
        lit(to)
    );
    Ok(conn.query_row(&sql, duckdb::params![x, y], |r| Ok((r.get(0)?, r.get(1)?)))?)
}

pub fn lonlat_to_merc(lon: f64, lat: f64) -> (f64, f64) {
    let lat = lat.clamp(-MERC_MAX_LAT, MERC_MAX_LAT);
    let x = lon.to_radians() * MERC_R;
    let y = (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * MERC_R;
    (x, y)
}

pub fn merc_to_lonlat(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / MERC_R).to_degrees();
    let lat = (2.0 * (y / MERC_R).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
    (lon, lat)
}

/// Size of one 256px-tile pixel in Web Mercator meters at a zoom level.
pub fn pixel_size(z: u32) -> f64 {
    2.0 * std::f64::consts::PI * MERC_R / (256.0 * 2f64.powi(z as i32))
}

#[derive(Serialize, Clone, Debug)]
pub struct Suggestion {
    pub crs: String,
    pub reason: String,
}

/// Suggest likely CRSs for data with no CRS, based on its coordinate ranges.
/// These are only suggestions shown in the Define Projection dialog.
pub fn suggest(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if xmin >= -180.0 && xmax <= 180.0 && ymin >= -90.0 && ymax <= 90.0 {
        out.push(Suggestion { crs: "EPSG:4326".into(), reason: "Coordinates look like longitude/latitude degrees (WGS 84)".into() });
        out.push(Suggestion { crs: "EPSG:4269".into(), reason: "Longitude/latitude degrees on NAD83 (common for US federal data)".into() });
        out.push(Suggestion { crs: "EPSG:4267".into(), reason: "Longitude/latitude degrees on NAD27 (older USGS maps)".into() });
    } else if xmin >= 100_000.0 && xmax <= 900_000.0 && ymin >= 0.0 && ymax <= 10_000_000.0 {
        out.push(Suggestion { crs: "UTM".into(), reason: "Values look like UTM meters (easting 100k–900k). Pick the zone for your area.".into() });
    } else if xmax.abs() > 1_000_000.0 || ymax.abs() > 1_000_000.0 {
        out.push(Suggestion { crs: "STATEPLANE".into(), reason: "Large values may be State Plane (often US survey feet). Pick your state's zone.".into() });
        if xmin.abs() <= 20_037_508.35 && ymin.abs() <= 20_048_966.1 {
            out.push(Suggestion { crs: "EPSG:3857".into(), reason: "Could be Web Mercator meters (web map exports)".into() });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm_zones() {
        assert_eq!(utm_epsg(-97.5, 35.2), "EPSG:32614");
        assert_eq!(utm_epsg(151.2, -33.9), "EPSG:32756");
        assert_eq!(utm_epsg(180.0, 0.0), "EPSG:32660");
    }

    #[test]
    fn mercator_round_trip() {
        let (x, y) = lonlat_to_merc(-97.5, 35.2);
        let (lon, lat) = merc_to_lonlat(x, y);
        assert!((lon + 97.5).abs() < 1e-9 && (lat - 35.2).abs() < 1e-9);
    }
}
