//! Body-aware geodesy for layouts (docs/LAYOUTS_PLAN.md L3, L4): point scale
//! factor, meridian convergence, frame extent. Every ellipsoid number in
//! `layout/` comes from a [`Body`]; this is the only file that names one.

use anyhow::{anyhow, bail, Result};
use gdal::spatial_ref::{AxisMappingStrategy, CoordTransform, SpatialRef};
use geographiclib_rs::{DirectGeodesic, Geodesic, InverseGeodesic};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LonConvention {
    /// −180…180, positive east (Earth).
    Signed180East,
}

#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub name: &'static str,
    /// Equatorial radius, metres.
    pub a: f64,
    /// Flattening.
    pub f: f64,
    pub lon_convention: LonConvention,
}

impl Body {
    /// F1 adds the Moon and Mars to this table.
    pub fn get(name: &str) -> Result<Body> {
        match name {
            "earth" => Ok(Body {
                name: "earth",
                a: 6_378_137.0,
                f: 1.0 / 298.257_223_563,
                lon_convention: LonConvention::Signed180East,
            }),
            other => bail!("Planetary bodies arrive in phase F (layout body '{other}')"),
        }
    }

    pub fn geodesic(&self) -> Geodesic {
        Geodesic::new(self.a, self.f)
    }

    pub fn format_lon(&self, lon: f64, decimals: usize) -> String {
        let LonConvention::Signed180East = self.lon_convention;
        let l = ((lon + 180.0).rem_euclid(360.0)) - 180.0;
        format!("{:.*}°{}", decimals, l.abs(), if l < 0.0 { "W" } else { "E" })
    }

    pub fn format_lat(&self, lat: f64, decimals: usize) -> String {
        format!("{:.*}°{}", decimals, lat.abs(), if lat < 0.0 { "S" } else { "N" })
    }
}

/// A frame CRS with transforms to and from its own lon/lat (geographic CS).
pub struct FrameCrs {
    srs: SpatialRef,
    to_geo: CoordTransform,
    from_geo: CoordTransform,
    pub projected: bool,
    /// Metres per CRS linear unit (1 for metres, 0.3048006… for US survey feet).
    pub unit_m: f64,
}

fn srs(def: &str) -> Result<SpatialRef> {
    let mut s = SpatialRef::from_definition(def).map_err(|e| anyhow!("Unknown or invalid coordinate system '{def}': {e}"))?;
    s.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
    Ok(s)
}

impl FrameCrs {
    pub fn new(crs: &str, _body: &Body) -> Result<FrameCrs> {
        let c = srs(&crate::crs::resolve(crs))?;
        // The CRS's own geographic CS (same datum), not WGS 84: a datum shift
        // would perturb the ground distances k is computed from (~2e-6 for
        // NAD83 → WGS 84). The body supplies only the ellipsoid.
        let mut g = c.geog_cs().map_err(|e| anyhow!("'{crs}' has no geographic base: {e}"))?;
        g.set_axis_mapping_strategy(AxisMappingStrategy::TraditionalGisOrder);
        let projected = !c.is_geographic();
        let unit_m = if projected { c.linear_units() } else { 1.0 };
        Ok(FrameCrs {
            to_geo: CoordTransform::new(&c, &g).map_err(|e| anyhow!("Cannot transform '{crs}' to lon/lat: {e}"))?,
            from_geo: CoordTransform::new(&g, &c).map_err(|e| anyhow!("Cannot transform lon/lat to '{crs}': {e}"))?,
            projected,
            unit_m,
            srs: c,
        })
    }

    /// Bounding box in `target` (a CRS string) of points given in this CRS.
    pub fn bounds_in(&self, target: &str, pts: &[(f64, f64)]) -> Result<[f64; 4]> {
        let t = CoordTransform::new(&self.srs, &srs(&crate::crs::resolve(target))?).map_err(|e| anyhow!("{e}"))?;
        let mut xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
        let mut ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
        t.transform_coords(&mut xs, &mut ys, &mut []).map_err(|e| anyhow!("{e}"))?;
        if !xs.iter().chain(ys.iter()).all(|v| v.is_finite()) {
            bail!("Frame outside the validity area of '{target}'");
        }
        let f = |v: &[f64], m: fn(f64, f64) -> f64, init: f64| v.iter().fold(init, |a, b| m(a, *b));
        Ok([f(&xs, f64::min, f64::INFINITY), f(&ys, f64::min, f64::INFINITY), f(&xs, f64::max, f64::NEG_INFINITY), f(&ys, f64::max, f64::NEG_INFINITY)])
    }

    /// CRS coordinates → (lon, lat) degrees.
    pub fn to_lonlat(&self, x: f64, y: f64) -> Result<(f64, f64)> {
        let (mut xs, mut ys) = ([x], [y]);
        self.to_geo.transform_coords(&mut xs, &mut ys, &mut []).map_err(|e| anyhow!("{e}"))?;
        Ok((xs[0], ys[0]))
    }

    /// (lon, lat) degrees → CRS coordinates.
    pub fn from_lonlat(&self, lon: f64, lat: f64) -> Result<(f64, f64)> {
        let (mut xs, mut ys) = ([lon], [lat]);
        self.from_geo.transform_coords(&mut xs, &mut ys, &mut []).map_err(|e| anyhow!("{e}"))?;
        Ok((xs[0], ys[0]))
    }
}

/// Scale factor and orientation at one point.
#[derive(Clone, Copy, Debug)]
pub struct Factors {
    /// Grid distance ÷ ground distance (1 for a geographic frame, where the
    /// scale is defined along the parallel instead).
    pub k: f64,
    pub k_east: f64,
    pub k_north: f64,
    /// Clockwise bearing of true north measured from grid up, degrees.
    pub true_north_deg: f64,
    /// Meridian convergence γ = −true_north_deg (PROJ's sign: positive east
    /// of the central meridian in the northern hemisphere).
    pub convergence_deg: f64,
}

/// Half-length of the numerical differences, metres.
const STEP_M: f64 = 10.0;

/// Point scale factor and meridian convergence at (x, y) of the frame CRS
/// (L3): project ±10 m east and north, take the geodesic ground distance on
/// the body's ellipsoid, and divide.
pub fn factors(body: &Body, crs: &FrameCrs, x: f64, y: f64) -> Result<Factors> {
    if !crs.projected {
        return Ok(Factors { k: 1.0, k_east: 1.0, k_north: 1.0, true_north_deg: 0.0, convergence_deg: 0.0 });
    }
    let geod = body.geodesic();
    let d = STEP_M / crs.unit_m;
    let ratio = |p: (f64, f64), q: (f64, f64)| -> Result<f64> {
        let (lon1, lat1) = crs.to_lonlat(p.0, p.1)?;
        let (lon2, lat2) = crs.to_lonlat(q.0, q.1)?;
        let ground: f64 = geod.inverse(lat1, lon1, lat2, lon2);
        Ok(2.0 * STEP_M / ground)
    };
    let k_east = ratio((x - d, y), (x + d, y))?;
    let k_north = ratio((x, y - d), (x, y + d))?;
    // Conformal projections have the same scale in every direction.
    let k = if (k_east / k_north - 1.0).abs() < 1e-5 { (k_east + k_north) / 2.0 } else { k_east };

    let (lon, lat) = crs.to_lonlat(x, y)?;
    let (lat2, lon2): (f64, f64) = geod.direct(lat, lon, 0.0, STEP_M);
    let (x1, y1) = crs.from_lonlat(lon, lat)?;
    let (x2, y2) = crs.from_lonlat(lon2, lat2)?;
    let true_north_deg = (x2 - x1).atan2(y2 - y1).to_degrees();
    Ok(Factors { k, k_east, k_north, true_north_deg, convergence_deg: -true_north_deg })
}

/// Length in mm on the page of a bar showing `ground_m` metres of ground at
/// scale 1:`scale` where the grid scale factor is `k` (L3: scale bar =
/// ground truth at the frame centre).
pub fn bar_length_mm(ground_m: f64, k: f64, scale: f64) -> f64 {
    ground_m * k / scale * 1000.0
}

/// Frame extent `[xmin, ymin, xmax, ymax]` in CRS units. Projected: nominal
/// grid width = w_mm/1000 × scale. Geographic: the scale is that of the
/// ground at the centre, east along the parallel and north along the
/// meridian.
pub fn frame_extent(body: &Body, crs: &FrameCrs, center: [f64; 2], scale: f64, w_mm: f64, h_mm: f64) -> Result<[f64; 4]> {
    let (w_m, h_m) = (w_mm / 1000.0 * scale, h_mm / 1000.0 * scale);
    let (hw, hh) = if crs.projected {
        (w_m / crs.unit_m / 2.0, h_m / crs.unit_m / 2.0)
    } else {
        let geod = body.geodesic();
        let (lat, lon) = (center[1], center[0]);
        let (_, lon_e): (f64, f64) = geod.direct(lat, lon, 90.0, 1000.0);
        let (lat_n, _): (f64, f64) = geod.direct(lat, lon, 0.0, 1000.0);
        let (dlon, dlat) = ((lon_e - lon) / 1000.0, (lat_n - lat) / 1000.0);
        if !(dlon > 0.0 && dlat > 0.0) {
            bail!("Cannot size a geographic frame at latitude {lat}");
        }
        (w_m * dlon / 2.0, h_m * dlat / 2.0)
    };
    Ok([center[0] - hw, center[1] - hh, center[0] + hw, center[1] + hh])
}

/// Largest relative difference between the scale factor at the corners of
/// `extent` and at its centre (e.g. 0.023 = "scale varies 2.3%"). Corners
/// PROJ can't transform are ignored.
pub fn scale_variation(body: &Body, crs: &FrameCrs, extent: [f64; 4]) -> Result<f64> {
    if !crs.projected {
        return Ok(0.0);
    }
    let centre = factors(body, crs, (extent[0] + extent[2]) / 2.0, (extent[1] + extent[3]) / 2.0)?.k;
    let mut worst: f64 = 0.0;
    for (x, y) in [(extent[0], extent[1]), (extent[2], extent[1]), (extent[0], extent[3]), (extent[2], extent[3])] {
        if let Ok(f) = factors(body, crs, x, y) {
            worst = worst.max((f.k / centre - 1.0).abs());
        }
    }
    Ok(worst)
}

/// Length of one mm of paper in CRS units, for tolerances (projected: exact;
/// geographic: the larger of the two directions at `center`).
pub fn mm_in_crs_units(body: &Body, crs: &FrameCrs, center: [f64; 2], scale: f64) -> Result<f64> {
    let e = frame_extent(body, crs, center, scale, 1.0, 1.0)?;
    Ok((e[2] - e[0]).max(e[3] - e[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Coord {
        v: [f64; 4],
    }
    #[repr(C)]
    #[derive(Default)]
    struct ProjFactors {
        meridional_scale: f64,
        parallel_scale: f64,
        areal_scale: f64,
        angular_distortion: f64,
        meridian_parallel_angle: f64,
        meridian_convergence: f64,
        tissot_semimajor: f64,
        tissot_semiminor: f64,
        dx_dlam: f64,
        dx_dphi: f64,
        dy_dlam: f64,
        dy_dphi: f64,
    }
    #[link(name = "proj")]
    extern "C" {
        fn proj_create(ctx: *mut std::ffi::c_void, definition: *const std::ffi::c_char) -> *mut std::ffi::c_void;
        fn proj_factors(p: *mut std::ffi::c_void, lp: Coord) -> ProjFactors;
        fn proj_destroy(p: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    /// PROJ's own (meridional scale, parallel scale, convergence in degrees)
    /// for a projection definition at lon/lat degrees: the P4 reference.
    fn proj_reference(def: &str, lon: f64, lat: f64) -> (f64, f64, f64) {
        let f = proj_factors_for(def, lon, lat);
        (f.meridional_scale, f.parallel_scale, f.meridian_convergence.to_degrees())
    }

    fn proj_factors_for(def: &str, lon: f64, lat: f64) -> ProjFactors {
        let c = std::ffi::CString::new(def).unwrap();
        unsafe {
            let p = proj_create(std::ptr::null_mut(), c.as_ptr());
            assert!(!p.is_null(), "proj_create failed for {def}");
            let f = proj_factors(p, Coord { v: [lon.to_radians(), lat.to_radians(), 0.0, 0.0] });
            proj_destroy(p);
            f
        }
    }

    /// Scale along grid east from PROJ's partial derivatives and the GRS80
    /// ellipsoid: 1 / ground length of the geographic step that moves the
    /// grid point one unit east. (`parallel_scale` is along the parallel,
    /// which differs from grid east in a non-conformal CRS.)
    fn proj_grid_east_scale(def: &str, lon: f64, lat: f64) -> f64 {
        let f = proj_factors_for(def, lon, lat);
        let (a, e2) = (6_378_137.0f64, 0.006_694_380_022_900_787_8f64);
        let phi = lat.to_radians();
        let w = (1.0 - e2 * phi.sin().powi(2)).sqrt();
        let (m, n) = (a * (1.0 - e2) / w.powi(3), a / w);
        // PROJ's derivatives are of x/a, y/a (unit ellipsoid), so one metre
        // east is (1/a, 0) in their units: solve J · (dλ, dφ) = (1/a, 0).
        let det = f.dx_dlam * f.dy_dphi - f.dx_dphi * f.dy_dlam;
        let (dlam, dphi) = (f.dy_dphi / det / a, -f.dy_dlam / det / a);
        1.0 / ((m * dphi).powi(2) + (n * phi.cos() * dlam).powi(2)).sqrt()
    }

    #[test]
    fn p4_utm15_factors_match_proj() {
        let body = Body::get("earth").unwrap();
        let crs = FrameCrs::new("EPSG:26915", &body).unwrap();
        let (lon, lat) = (-96.0, 45.0);
        let (x, y) = crs.from_lonlat(lon, lat).unwrap();
        let f = factors(&body, &crs, x, y).unwrap();
        let (h, k, gamma) = proj_reference("+proj=utm +zone=15 +ellps=GRS80", lon, lat);
        assert!((f.k - k).abs() < 1e-6, "k {} vs PROJ {k}", f.k);
        assert!((h - k).abs() < 1e-9, "UTM is conformal");
        assert!((f.convergence_deg - gamma).abs() < 0.001, "convergence {} vs PROJ {gamma}", f.convergence_deg);
        // The numbers the plan quotes.
        assert!((f.k - 1.00029).abs() < 5e-5, "k = {}", f.k);
        assert!((f.convergence_deg.abs() - 2.12).abs() < 0.01, "γ = {}", f.convergence_deg);
    }

    #[test]
    fn factors_match_proj_for_albers_and_state_plane_feet() {
        let body = Body::get("earth").unwrap();
        // Albers is not conformal: east (parallel) scale is the reference direction.
        let crs = FrameCrs::new("EPSG:5070", &body).unwrap();
        let (x, y) = crs.from_lonlat(-110.0, 35.0).unwrap();
        let f = factors(&body, &crs, x, y).unwrap();
        let def = "+proj=aea +lat_0=23 +lon_0=-96 +lat_1=29.5 +lat_2=45.5 +ellps=GRS80";
        let (_, _, gamma) = proj_reference(def, -110.0, 35.0);
        let k = proj_grid_east_scale(def, -110.0, 35.0);
        assert!((f.k_east - k).abs() < 1e-6, "{} vs {k}", f.k_east);
        assert!((f.convergence_deg - gamma).abs() < 0.001, "{} vs {gamma}", f.convergence_deg);
        // US survey feet: ground metres, not feet, are what k compares.
        let sp = FrameCrs::new("EPSG:2276", &body).unwrap(); // Texas North Central, ftUS
        assert!((sp.unit_m - 0.3048006096).abs() < 1e-9);
        let (x, y) = sp.from_lonlat(-97.3, 32.8).unwrap();
        let f = factors(&body, &sp, x, y).unwrap();
        assert!((f.k - 1.0).abs() < 1e-3, "k = {}", f.k);
    }

    #[test]
    fn geographic_frame_extent_follows_the_ground() {
        let body = Body::get("earth").unwrap();
        let crs = FrameCrs::new("EPSG:4326", &body).unwrap();
        // 100 mm at 1:1,000,000 = 100 km, east at 60°N ≈ 1.8° of longitude.
        let e = frame_extent(&body, &crs, [10.0, 60.0], 1_000_000.0, 100.0, 100.0).unwrap();
        let (dlon, dlat) = (e[2] - e[0], e[3] - e[1]);
        assert!((dlon - 1.8).abs() < 0.02, "{dlon}");
        assert!((dlat - 0.9).abs() < 0.02, "{dlat}");
    }

    #[test]
    fn variation_is_large_for_web_mercator_at_60n() {
        let body = Body::get("earth").unwrap();
        let crs = FrameCrs::new("EPSG:3857", &body).unwrap();
        let (x, y) = crs.from_lonlat(10.0, 60.0).unwrap();
        let e = frame_extent(&body, &crs, [x, y], 5_000_000.0, 200.0, 150.0).unwrap();
        assert!(scale_variation(&body, &crs, e).unwrap() > 0.01);
        let utm = FrameCrs::new("EPSG:32632", &body).unwrap();
        let (x, y) = utm.from_lonlat(9.0, 60.0).unwrap();
        let e = frame_extent(&body, &utm, [x, y], 24_000.0, 200.0, 150.0).unwrap();
        assert!(scale_variation(&body, &utm, e).unwrap() < 1e-4);
    }

    #[test]
    fn only_earth_for_now() {
        assert!(Body::get("mars").is_err());
        assert_eq!(Body::get("earth").unwrap().format_lon(-96.0, 1), "96.0°W");
    }
}
