//! How accurate is moving coordinates from one CRS to another (CRS accuracy plan, step A1)? PROJ picks the best operation it can
//! with the datum files installed; this asks GDAL's transformation API for the best accuracy class it can reach for an area, with
//! rough "ballpark" fallbacks refused, so the user is told before a result built on a rough shift is used.

use crate::crs::srs;
use anyhow::Result;
use serde::Serialize;

/// Accuracy classes tried, in metres, tightest first.
const CLASSES: [f64; 6] = [0.01, 0.1, 0.5, 2.0, 10.0, 100.0];

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransformReport {
    /// "exact" (no datum change), "precise" (a datum grid or exact operation, under 0.5 m), "approximate" (0.5–100 m),
    /// "ballpark" (no accurate operation at all: error unknown, can be large).
    pub level: String,
    /// Upper bound of the accuracy class reached, in metres (0 for "exact"); None for "ballpark".
    pub accuracy_m: Option<f64>,
    /// True when a datum shift of lower quality is being used because a better one needs files that are not installed or does not exist.
    pub warn: bool,
    pub text: String,
}

fn reaches(from: &gdal::spatial_ref::SpatialRef, to: &gdal::spatial_ref::SpatialRef, aoi: Option<[f64; 4]>, accuracy: f64) -> bool {
    // SAFETY: the option and transformation handles are created and destroyed here; the spatial references outlive the call.
    unsafe {
        let opts = gdal_sys::OCTNewCoordinateTransformationOptions();
        if let Some([w, s, e, n]) = aoi {
            gdal_sys::OCTCoordinateTransformationOptionsSetAreaOfInterest(opts, w, s, e, n);
        }
        gdal_sys::OCTCoordinateTransformationOptionsSetBallparkAllowed(opts, 0);
        gdal_sys::OCTCoordinateTransformationOptionsSetDesiredAccuracy(opts, accuracy);
        // A refused probe is an answer, not an error: keep GDAL from printing it.
        gdal_sys::CPLPushErrorHandler(Some(gdal_sys::CPLQuietErrorHandler));
        gdal_sys::CPLErrorReset();
        let t = gdal_sys::OCTNewCoordinateTransformationEx(from.to_c_hsrs(), to.to_c_hsrs(), opts);
        gdal_sys::OCTDestroyCoordinateTransformationOptions(opts);
        gdal_sys::CPLErrorReset();
        gdal_sys::CPLPopErrorHandler();
        if t.is_null() {
            false
        } else {
            gdal_sys::OCTDestroyCoordinateTransformation(t);
            true
        }
    }
}

/// The accuracy of moving coordinates from `from` to `to` over `aoi` (west, south, east, north in degrees; None: the whole CRS area).
pub fn transform_report(from: &str, to: &str, aoi: Option<[f64; 4]>) -> Result<TransformReport> {
    let (a, b) = (srs(&crate::crs::resolve(from))?, srs(&crate::crs::resolve(to))?);
    if crate::crs::same(from, to) {
        return Ok(TransformReport { level: "exact".into(), accuracy_m: Some(0.0), warn: false, text: "Same coordinate system: nothing is changed.".into() });
    }
    let reached = CLASSES.iter().copied().find(|&c| reaches(&a, &b, aoi, c));
    Ok(match reached {
        Some(c) if c <= 0.01 => TransformReport { level: "exact".into(), accuracy_m: Some(0.0), warn: false, text: "No datum change, only the map projection differs: exact.".into() },
        Some(c) if c <= 0.5 => TransformReport { level: "precise".into(), accuracy_m: Some(c), warn: false, text: format!("Datum shift with a precise operation or grid: error up to about {c} m.") },
        Some(c) => TransformReport {
            level: "approximate".into(),
            accuracy_m: Some(c),
            warn: true,
            text: format!("Approximate datum shift: error up to about {c} m. A more precise datum grid is not installed for this area, or none exists."),
        },
        None => TransformReport {
            level: "ballpark".into(),
            accuracy_m: None,
            warn: true,
            text: "No accurate datum shift is available between these coordinate systems: coordinates are only approximate and can be off by metres or more.".into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_only_is_exact_and_same_crs_is_unchanged() {
        let r = transform_report("EPSG:4326", "EPSG:32615", Some([-94.0, 40.0, -93.0, 41.0])).unwrap();
        assert_eq!((r.level.as_str(), r.warn), ("exact", false), "{r:?}");
        let s = transform_report("EPSG:32615", "EPSG:32615", None).unwrap();
        assert_eq!(s.level, "exact");
    }

    #[test]
    fn a_us_datum_shift_with_grids_is_not_a_ballpark() {
        // Needs the NOAA grids (ensure_proj_data points PROJ at a folder that has them: Homebrew locally, proj-data in CI).
        crate::resources::ensure_proj_data();
        let r = transform_report("EPSG:4267", "EPSG:4326", Some([-98.0, 35.0, -97.0, 36.0])).unwrap();
        assert!(r.accuracy_m.is_some() && r.accuracy_m.unwrap() <= 10.0, "{r:?}");
        assert_ne!(r.level, "ballpark");
    }
}
