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
    /// "exact" (no datum change), "precise" (a datum grid or published operation, up to 2 m: the stated accuracy of NAD27 and WGS 84 ↔ NAD83 shifts), "approximate" (2–100 m, rougher than a grid shift),
    /// "ballpark" (no accurate operation at all: error unknown, can be large).
    pub level: String,
    /// Upper bound of the accuracy class reached, in metres (0 for "exact"); None for "ballpark".
    pub accuracy_m: Option<f64>,
    /// True when a datum shift of lower quality is being used because a better one needs files that are not installed or does not exist.
    pub warn: bool,
    /// True when the best operation for the area needs a datum grid that is not installed, so a rougher fallback is used.
    pub grid_missing: bool,
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

/// Can the best operation for the area really run? PROJ lists operations that need a grid file even when the file is not installed;
/// transforming one point of the area with "only the best operation" fails in that case.
fn best_is_usable(from: &gdal::spatial_ref::SpatialRef, to: &gdal::spatial_ref::SpatialRef, aoi: Option<[f64; 4]>, accuracy: f64) -> bool {
    let centre = match aoi {
        Some([w, s, e, n]) => Some(((w + e) / 2.0, (s + n) / 2.0)),
        None => None,
    };
    let Some((lon, lat)) = centre else { return true }; // no area given: nothing to test
    let Ok(mut wgs) = srs("EPSG:4326") else { return true };
    wgs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let Ok(t0) = gdal::spatial_ref::CoordTransform::new(&wgs, from) else { return true };
    let (mut x, mut y, mut z) = ([lon], [lat], [0.0]);
    if t0.transform_coords(&mut x, &mut y, &mut z).is_err() {
        return true;
    }
    // SAFETY: handles are created and destroyed here; the spatial references outlive the call.
    unsafe {
        let opts = gdal_sys::OCTNewCoordinateTransformationOptions();
        gdal_sys::OCTCoordinateTransformationOptionsSetAreaOfInterest(opts, aoi.unwrap()[0], aoi.unwrap()[1], aoi.unwrap()[2], aoi.unwrap()[3]);
        gdal_sys::OCTCoordinateTransformationOptionsSetBallparkAllowed(opts, 0);
        gdal_sys::OCTCoordinateTransformationOptionsSetDesiredAccuracy(opts, accuracy);
        gdal_sys::OCTCoordinateTransformationOptionsSetOnlyBest(opts, true);
        gdal_sys::CPLPushErrorHandler(Some(gdal_sys::CPLQuietErrorHandler));
        gdal_sys::CPLErrorReset();
        let t = gdal_sys::OCTNewCoordinateTransformationEx(from.to_c_hsrs(), to.to_c_hsrs(), opts);
        gdal_sys::OCTDestroyCoordinateTransformationOptions(opts);
        let ok = if t.is_null() {
            false
        } else {
            let (mut px, mut py, mut pz) = (x[0], y[0], 0.0f64);
            let done = gdal_sys::OCTTransform(t, 1, &mut px, &mut py, &mut pz) != 0 && px.is_finite() && py.is_finite();
            gdal_sys::OCTDestroyCoordinateTransformation(t);
            done
        };
        gdal_sys::CPLErrorReset();
        gdal_sys::CPLPopErrorHandler();
        ok
    }
}

/// The accuracy of moving coordinates from `from` to `to` over `aoi` (west, south, east, north in degrees; None: the whole CRS area).
pub fn transform_report(from: &str, to: &str, aoi: Option<[f64; 4]>) -> Result<TransformReport> {
    let (a, b) = (srs(&crate::crs::resolve(from))?, srs(&crate::crs::resolve(to))?);
    if crate::crs::same(from, to) {
        return Ok(TransformReport { level: "exact".into(), accuracy_m: Some(0.0), warn: false, grid_missing: false, text: "Same coordinate system: nothing is changed.".into() });
    }
    let reached = CLASSES.iter().copied().find(|&c| reaches(&a, &b, aoi, c));
    if let Some(c) = reached.filter(|&c| c > 0.01) {
        if !best_is_usable(&a, &b, aoi, c) {
            return Ok(TransformReport {
                level: "approximate".into(),
                accuracy_m: None,
                warn: true,
                grid_missing: true,
                text: format!("The precise datum shift for this area (error up to about {c} m) needs a datum grid that is not installed, so a rougher fallback is used: error typically several metres. Get the grid from the Datum grids menu."),
            });
        }
    }
    Ok(match reached {
        Some(c) if c <= 0.01 => TransformReport { level: "exact".into(), accuracy_m: Some(0.0), warn: false, grid_missing: false, text: "No datum change, only the map projection differs: exact.".into() },
        Some(c) if c <= 2.0 => TransformReport { level: "precise".into(), accuracy_m: Some(c), warn: false, grid_missing: false, text: format!("Datum shift with a grid or published operation: error up to about {c} m.") },
        Some(c) => TransformReport {
            level: "approximate".into(),
            accuracy_m: Some(c),
            warn: true,
            grid_missing: false,
            text: format!("Approximate datum shift: error up to about {c} m. A more precise datum grid is not installed for this area, or none exists."),
        },
        None => TransformReport {
            level: "ballpark".into(),
            accuracy_m: None,
            warn: true,
            grid_missing: false,
            text: "No accurate datum shift is available between these coordinate systems: coordinates are only approximate and can be off by metres or more.".into(),
        },
    })
}

// ---------------------------------------------------------------- policy (step A3)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

static STRICT: AtomicBool = AtomicBool::new(false);
static LOADED: Once = Once::new();

/// Where the choice is kept: next to the user's other app data (`GIS_STUDIO_DATA` overrides the base folder).
fn policy_file() -> Option<std::path::PathBuf> {
    crate::resources::templates_dir().and_then(|d| d.parent().map(|p| p.join("datum_policy.txt")))
}

/// True when approximate datum shifts are refused (Strict); the default is to warn and continue.
pub fn is_strict() -> bool {
    LOADED.call_once(|| {
        if let Some(t) = policy_file().and_then(|f| std::fs::read_to_string(f).ok()) {
            STRICT.store(t.trim() == "strict", Ordering::Relaxed);
        }
    });
    STRICT.load(Ordering::Relaxed)
}

/// Choose Strict (refuse approximate datum shifts) or Warn (the default); remembered between runs.
pub fn set_strict(strict: bool) {
    is_strict();
    STRICT.store(strict, Ordering::Relaxed);
    if let Some(f) = policy_file() {
        let _ = std::fs::write(f, if strict { "strict" } else { "warn" });
    }
}

/// `transform_report` for a layer whose extent is `extent` (minx, miny, maxx, maxy in `from`).
pub fn report_for_extent(from: &str, to: &str, extent: Option<[f64; 4]>) -> Result<TransformReport> {
    let aoi = extent.and_then(|e| {
        let a = srs(&crate::crs::resolve(from)).ok()?;
        let mut w = srs("EPSG:4326").ok()?;
        w.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
        let t = gdal::spatial_ref::CoordTransform::new(&a, &w).ok()?;
        let b = t.transform_bounds(&e, 21).ok()?;
        (b.iter().all(|v| v.is_finite()) && b[0] >= -180.0 && b[2] <= 180.0 && b[1] >= -90.0 && b[3] <= 90.0).then_some(b)
    });
    transform_report(from, to, aoi)
}

/// What a tool should do when it moves coordinates from `from` to `to`: None when the datum does not change, otherwise a note for the
/// result ("Datum change A → B: …"); an approximate shift is prefixed "Warning:" and refused in Strict mode.
pub fn datum_note(from: &str, to: &str, extent: Option<[f64; 4]>) -> Result<Option<String>> {
    let (Some(a), Some(b)) = (crate::crs::datum_of(from), crate::crs::datum_of(to)) else { return Ok(None) };
    if a == b {
        return Ok(None);
    }
    let r = report_for_extent(from, to, extent)?;
    if !r.warn {
        return Ok(Some(format!("Datum change {a} → {b}: {}", r.text)));
    }
    if is_strict() {
        anyhow::bail!("Stopped: this step would move coordinates from {a} to {b} with only an approximate datum shift. {} Strict mode is on; install the datum grid for this area (Datum grids) or switch to Warn to continue.", r.text);
    }
    Ok(Some(format!("Warning, datum change {a} → {b}: {}", r.text)))
}

// ---------------------------------------------------------------- coverage (step A5)

/// Regions checked at start-up and by `--self-test`: (region, from, to, west, south, east, north).
pub const COVERAGE: [(&str, &str, &str, f64, f64, f64, f64); 9] = [
    ("United States: NAD27 to WGS 84", "EPSG:4267", "EPSG:4326", -98.0, 35.0, -97.0, 36.0),
    ("United States: WGS 84 to NAD83", "EPSG:4326", "EPSG:4269", -98.0, 35.0, -97.0, 36.0),
    ("Canada: NAD27 to NAD83", "EPSG:4267", "EPSG:4269", -80.0, 44.0, -79.0, 45.0),
    ("Mexico: NAD27 to WGS 84", "EPSG:4267", "EPSG:4326", -100.0, 19.0, -99.0, 20.0),
    ("Europe: ED50 to ETRS89", "EPSG:4230", "EPSG:4258", 10.0, 50.0, 11.0, 51.0),
    ("United Kingdom: OSGB36 to ETRS89", "EPSG:4277", "EPSG:4258", -2.0, 52.0, -1.0, 53.0),
    ("Australia: GDA94 to GDA2020", "EPSG:4283", "EPSG:7844", 149.0, -36.0, 150.0, -35.0),
    ("New Zealand: NZGD49 to NZGD2000", "EPSG:4272", "EPSG:4167", 174.0, -37.0, 175.0, -36.0),
    ("Japan: Tokyo to JGD2000", "EPSG:4301", "EPSG:4612", 139.0, 35.0, 140.0, 36.0),
];

/// One line per region: how well this installation can move coordinates there, in plain words.
pub fn coverage() -> Vec<(String, TransformReport)> {
    crate::resources::ensure_proj_data();
    COVERAGE
        .iter()
        .map(|&(name, from, to, w, s, e, n)| {
            let r = transform_report(from, to, Some([w, s, e, n])).unwrap_or(TransformReport { level: "ballpark".into(), accuracy_m: None, warn: true, grid_missing: false, text: "This region could not be checked.".into() });
            (name.to_string(), r)
        })
        .collect()
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
    fn coverage_lists_every_region_and_the_us_is_not_a_ballpark() {
        let c = coverage();
        assert_eq!(c.len(), COVERAGE.len());
        let us = c.iter().find(|(n, _)| n.starts_with("United States: NAD27")).unwrap();
        assert_ne!(us.1.level, "ballpark", "{:?}", us.1);
        assert!(c.iter().all(|(_, r)| !r.text.is_empty()));
    }

    #[test]
    fn datum_note_is_silent_without_a_datum_change_and_warns_on_a_change() {
        assert_eq!(datum_note("EPSG:4326", "EPSG:32615", None).unwrap(), None);
        crate::resources::ensure_proj_data();
        let n = datum_note("EPSG:4267", "EPSG:4326", Some([-98.0, 35.0, -97.0, 36.0])).unwrap().unwrap();
        assert!(n.contains("Datum change North American Datum 1927"), "{n}");
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
