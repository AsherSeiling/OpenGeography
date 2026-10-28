//! The one tolerance table of the CRS test suites (docs/CRS_PLAN.md RD-4). Loosen a value only here, with a written reason.

/// Projected coordinates against a reference, metres.
pub const PROJECTED_M: f64 = 1e-3;
/// Geographic coordinates against a reference, degrees (about 1 mm).
pub const GEOGRAPHIC_DEG: f64 = 1e-8;
/// Round trip A→B→A, projected (metres) and geographic (degrees).
pub const ROUND_TRIP_M: f64 = 1e-3;
pub const ROUND_TRIP_DEG: f64 = 1e-9;
/// Geodesic length against GeographicLib, relative.
pub const LENGTH_REL: f64 = 1e-5;
/// Geodesic area against GeographicLib, relative.
pub const AREA_REL: f64 = 1e-4;
/// Scale bar and 1:N against the engine's factors, relative.
pub const SCALE_REL: f64 = 1e-3;

/// Tolerance for a coordinate of the zoo CRS `key`: degrees for geographic CRSs, metres (or feet: the same number) otherwise.
pub fn coordinate(key: &str) -> f64 {
    if is_geographic(key) {
        GEOGRAPHIC_DEG
    } else {
        PROJECTED_M
    }
}

pub fn is_geographic(key: &str) -> bool {
    matches!(key, "4326" | "4269" | "4267" | "4807")
}
