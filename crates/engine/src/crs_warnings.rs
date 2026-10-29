//! Plain-words warnings about measuring in a map projection (CRS accuracy plan C3). Tools that work in map units (distance bands,
//! neighbours, interpolation and kriging cell sizes) are only as true as the projection is over the data's extent.

use crate::layout::geo::{factors, Body, FrameCrs};

/// Length scale factors (min, max) over the extent's corners and centre; None for a geographic CRS or when PROJ cannot say.
pub fn scale_range(crs: &str, extent: [f64; 4]) -> Option<(f64, f64)> {
    let body = Body::get("earth").ok()?;
    let fc = FrameCrs::new(crs, &body).ok()?;
    let [x0, y0, x1, y1] = extent;
    let pts = [(x0, y0), (x1, y0), (x0, y1), (x1, y1), ((x0 + x1) / 2.0, (y0 + y1) / 2.0)];
    let ks: Vec<f64> = pts.iter().filter_map(|&(x, y)| factors(&body, &fc, x, y).ok()).flat_map(|f| [f.k_east, f.k_north]).filter(|k| k.is_finite() && *k > 0.0).collect();
    if ks.is_empty() {
        return None;
    }
    Some((ks.iter().cloned().fold(f64::MAX, f64::min), ks.iter().cloned().fold(f64::MIN, f64::max)))
}

/// A note for a tool that measures in map units: None when lengths are within 0.1 % of true over the extent, otherwise how far off they are.
pub fn planar_note(tool: &str, crs: &str, extent: [f64; 4]) -> Option<String> {
    let (lo, hi) = scale_range(crs, extent)?;
    let worst = (lo - 1.0).abs().max((hi - 1.0).abs());
    if worst < 0.001 {
        return None;
    }
    let name = crate::crs::info(crs).map(|i| i.name).unwrap_or_else(|_| crate::crs::resolve(crs));
    Some(format!(
        "Warning, {tool} measures in the map units of {name}, where lengths over this area are off by up to {:.1} % and areas by up to {:.1} %. Results are only approximate. Project the layer to a local projected coordinate system (a UTM zone or State Plane for the area; the coordinate system picker suggests one) first.",
        worst * 100.0,
        ((1.0 + worst).powi(2) - 1.0).abs().max((1.0 - worst).powi(2) - 1.0).abs() * 100.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm_inside_its_zone_is_fine_and_web_mercator_at_45_degrees_is_not() {
        // UTM 15N near its central meridian (-93°), 4 km wide: scale 0.9996 at most.
        assert!(planar_note("Hot Spot Analysis", "EPSG:32615", [500_000.0, 4_400_000.0, 504_000.0, 4_404_000.0]).is_none());
        // Web Mercator near 45°N: lengths stretched by 1 / cos(45°) = 1.41.
        let m = planar_note("Kriging", "EPSG:3857", [-10_400_000.0, 5_600_000.0, -10_300_000.0, 5_700_000.0]).unwrap();
        assert!(m.starts_with("Warning, Kriging measures in the map units of") && m.contains("off by up to"), "{m}");
        let (lo, hi) = scale_range("EPSG:3857", [-10_400_000.0, 5_600_000.0, -10_300_000.0, 5_700_000.0]).unwrap();
        assert!(lo > 1.3 && hi < 1.5, "{lo} {hi}");
    }

    #[test]
    fn the_edge_of_a_utm_zone_is_flagged_and_geographic_crs_has_no_scale() {
        // 3° from the central meridian of zone 15, mid-latitudes: k ≈ 1.0003 - 1.0012 — near the line; far east it grows.
        let far = planar_note("Moran's I", "EPSG:32615", [900_000.0, 4_400_000.0, 910_000.0, 4_410_000.0]);
        assert!(far.is_some(), "{far:?}");
        assert!(scale_range("EPSG:4326", [-1.0, 40.0, 1.0, 41.0]).map(|(lo, hi)| lo == 1.0 && hi == 1.0).unwrap_or(true));
    }

    #[test]
    fn feet_are_told_apart() {
        let us = crate::crs::info("EPSG:2267").unwrap(); // NAD83 / Oklahoma North (ftUS)
        assert!(us.unit_note.as_deref().unwrap_or("").starts_with("US survey foot"), "{us:?}");
        let m = crate::crs::info("EPSG:32615").unwrap();
        assert!(m.unit_note.is_none());
        assert!(crate::crs::unit_note("foot").unwrap().starts_with("International foot"));
    }
}
