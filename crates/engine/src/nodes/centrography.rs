//! Centrography (Wave 1, package T4): Mean Centre, Standard Distance and Directional Distribution (standard deviational ellipse).
//! Textbook methods written directly from the published formulas, no library code:
//! - Mean centre: the (weighted) arithmetic mean of the positions (Neft 1966, Mitchell 2005 "The ESRI Guide to GIS Analysis vol. 2").
//!   On a geographic CRS the mean is taken on the ellipsoid: the mean of Earth-centred Cartesian positions, brought back to the surface
//!   along the ellipsoid normal, so the mean of two points at the same latitude on opposite meridians sits at the pole, as it should.
//! - Standard distance: sqrt( sum w_i d_i^2 / sum w_i ) with d_i the distance to the mean centre (Bachi 1963; Mitchell 2005); d_i is the
//!   geodesic distance (Karney 2013) on a geographic CRS and the plane distance in a projected one.
//! - Directional distribution: the standard deviational ellipse of Lefever (1926) in Yuill's (1971) form. With deviations x, y from the weighted
//!   mean and second moments Sxx, Syy, Sxy (weighted means of the products), the axes are the eigenvectors of [[Sxx, Sxy], [Sxy, Syy]]
//!   and the semi-axes are sqrt(2 * lambda) times the chosen number of standard deviations (Yuill 1971; the same definition as ArcGIS Pro's
//!   Directional Distribution). On a geographic CRS the ellipse is computed in an azimuthal equidistant projection centred on the mean
//!   centre (distances and azimuths from the centre are true there) and its outline is transformed back.

use super::*;
use crate::geomkit::{self, Metric};

pub(super) struct PtSet {
    pub xy: Vec<(f64, f64)>,
    pub w: Vec<f64>,
}

pub(super) fn sqlnum(v: f64) -> String {
    if v.is_finite() { format!("{v:?}") } else { "NULL".into() }
}

/// Positions of a layer's features (the centroid of a line or polygon), with an optional weight field. Skipped features are counted in a note.
pub(super) fn read_pts(ctx: &Ctx, rel: &Rel, weight: Option<&str>, tool: &str) -> Result<PtSet> {
    if !rel.has_geom {
        bail!("{tool} needs a layer with geometry");
    }
    let crs = need_crs(rel)?;
    let w = match weight {
        Some(f) => format!("TRY_CAST({}::VARCHAR AS DOUBLE)", ident(f)),
        None => "1.0".to_string(),
    };
    let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&rel.table)), [], |r| r.get(0))?;
    let mut stmt = ctx.conn.prepare(&format!(
        "SELECT ST_X(c), ST_Y(c), w, nonpt FROM (SELECT ST_Centroid(geom) AS c, {w} AS w, ST_GeometryType(geom)::VARCHAR <> 'POINT' AS nonpt FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom))",
        ident(&rel.table)
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<f64>>(1)?, r.get::<_, Option<f64>>(2)?, r.get::<_, bool>(3)?)))?;
    let (mut xy, mut ws, mut bad_w, mut non_point) = (vec![], vec![], 0i64, 0i64);
    for r in rows {
        let (x, y, w, np) = r?;
        let (Some(x), Some(y)) = (x, y) else { continue };
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        let Some(w) = w.filter(|v| v.is_finite()) else {
            bad_w += 1;
            continue;
        };
        if w < 0.0 {
            bail!("{tool}: the weight field has a negative value ({w}). Weights must be zero or more.");
        }
        non_point += np as i64;
        xy.push((x, y));
        ws.push(w);
    }
    let no_geom = total - xy.len() as i64 - bad_w;
    if no_geom > 0 {
        ctx.add_note(format!("{tool}: {no_geom} feature(s) without a usable geometry were left out."));
    }
    if bad_w > 0 {
        ctx.add_note(format!("{tool}: {bad_w} feature(s) without a number in the weight field were left out."));
    }
    if non_point > 0 {
        ctx.add_note(format!("{tool}: {non_point} feature(s) are not points; their centroids were used."));
    }
    if xy.is_empty() {
        bail!("{tool}: the layer has no features with a position{}", if weight.is_some() { " and a weight" } else { "" });
    }
    if ws.iter().sum::<f64>() <= 0.0 {
        bail!("{tool}: the weights add up to zero");
    }
    if let Some(e) = xy.iter().fold(None, |e: Option<[f64; 4]>, p| Some(e.map_or([p.0, p.1, p.0, p.1], |e| [e[0].min(p.0), e[1].min(p.1), e[2].max(p.0), e[3].max(p.1)]))) {
        if let Some(n) = crate::crs_warnings::planar_note(tool, crs, e) {
            if !crate::crs::is_geographic(crs) {
                ctx.add_note(n);
            }
        }
    }
    Ok(PtSet { xy, w: ws })
}

/// Weighted mean centre in CRS coordinates: on the ellipsoid for a geographic CRS, in the plane otherwise.
pub(super) fn mean_centre(m: &Metric, p: &PtSet) -> (f64, f64) {
    let sw: f64 = p.w.iter().sum();
    if m.geographic {
        let mut v = [0.0; 3];
        for ((x, y), w) in p.xy.iter().zip(&p.w) {
            let e = m.to_ecef(*x, *y);
            for k in 0..3 {
                v[k] += w * e[k];
            }
        }
        m.from_ecef([v[0] / sw, v[1] / sw, v[2] / sw])
    } else {
        (p.xy.iter().zip(&p.w).map(|(q, w)| q.0 * w).sum::<f64>() / sw, p.xy.iter().zip(&p.w).map(|(q, w)| q.1 * w).sum::<f64>() / sw)
    }
}

fn point_wkt(x: f64, y: f64) -> String {
    format!("ST_GeomFromText('POINT({x:?} {y:?})')")
}

fn weight_param() -> Param {
    Param::new("weight", "Weight field (empty: every feature counts once)", ParamKind::Field).field_of("in").optional()
}

fn weight_of(p: &Params) -> Option<String> {
    p.str("weight")
}

pub struct MeanCentre;
impl Tool for MeanCentre {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "mean_centre", name: "Mean Centre", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "The average position of the features, optionally weighted by a number field (for example population), as one point with the number of features and the total weight. On a geographic coordinate system the mean is taken on the ellipsoid (the mean of Earth-centred positions, brought back to the surface), not the average of the degrees, so it is right across large areas and the date line; in a projected coordinate system it is the plane average in that projection. Lines and polygons count by their centroid. Features without a position or weight are left out and counted.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Vector), params: vec![weight_param()] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("n", "BIGINT")).add(NewField::new("weight_sum", "DOUBLE")).add(NewField::new("method", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let m = geomkit::metric(crs)?;
        let pts = read_pts(ctx, i, weight_of(p).as_deref(), "Mean Centre")?;
        let (x, y) = mean_centre(&m, &pts);
        let method = if m.geographic { "mean of Earth-centred positions on the ellipsoid" } else { "weighted mean in the plane of the layer's projection" };
        Ok(Output::Select {
            sql: format!("SELECT {} AS n, {} AS weight_sum, {} AS method, {} AS geom", pts.xy.len(), sqlnum(pts.w.iter().sum()), lit(method), point_wkt(x, y)),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

/// A ring of `n` points around (x, y): geodesic circle on a geographic CRS, plane circle otherwise (radius in metres).
fn circle(m: &Metric, x: f64, y: f64, radius_m: f64) -> Vec<(f64, f64)> {
    use geographiclib_rs::DirectGeodesic;
    let n = 180;
    let g = m.geodesic();
    (0..=n)
        .map(|k| {
            let az = 360.0 * (k % n) as f64 / n as f64;
            if m.geographic {
                let (la, lo): (f64, f64) = g.direct(y, x, az, radius_m);
                (lo, la)
            } else {
                let r = radius_m / m.unit_m;
                (x + r * az.to_radians().sin(), y + r * az.to_radians().cos())
            }
        })
        .collect()
}

fn ring_wkt(r: &[(f64, f64)]) -> String {
    format!("ST_GeomFromText('POLYGON(({}))')", r.iter().map(|(x, y)| format!("{x:?} {y:?}")).collect::<Vec<_>>().join(", "))
}

pub struct StandardDistance;
impl Tool for StandardDistance {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "standard_distance", name: "Standard Distance", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "How spread out the features are around their mean centre: the square root of the (weighted) mean of the squared distances to the mean centre. Output is a circle of that radius around the mean centre, with the radius in metres, the number of features and the total weight. On a geographic coordinate system the distances are geodesic (on the ellipsoid) and the mean centre is the ellipsoidal mean; in a projected one they are plane distances in that projection, converted to metres. Lines and polygons count by their centroid; features without a position or weight are left out and counted.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Vector), params: vec![weight_param()] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[]).add(NewField::new("n", "BIGINT")).add(NewField::new("weight_sum", "DOUBLE")).add(NewField::new("std_distance_m", "DOUBLE")).add(NewField::new("centre_x", "DOUBLE")).add(NewField::new("centre_y", "DOUBLE"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let m = geomkit::metric(need_crs(i)?)?;
        let pts = read_pts(ctx, i, weight_of(p).as_deref(), "Standard Distance")?;
        let (cx, cy) = mean_centre(&m, &pts);
        let sw: f64 = pts.w.iter().sum();
        let mut acc = 0.0;
        if m.geographic {
            use geographiclib_rs::InverseGeodesic;
            let g = m.geodesic();
            for ((x, y), w) in pts.xy.iter().zip(&pts.w) {
                let d: f64 = g.inverse(cy, cx, *y, *x);
                acc += w * d * d;
            }
        } else {
            for ((x, y), w) in pts.xy.iter().zip(&pts.w) {
                let d = ((x - cx).hypot(y - cy)) * m.unit_m;
                acc += w * d * d;
            }
        }
        let sd = (acc / sw).sqrt();
        let ring = circle(&m, cx, cy, sd);
        if m.geographic && ring.windows(2).any(|w| (w[0].0 - w[1].0).abs() > 180.0) {
            ctx.add_note("Standard Distance: the circle crosses the date line or a pole and is drawn as one outline that wraps; its radius is still right.".into());
        }
        if sd == 0.0 {
            ctx.add_note("Standard Distance: all features are at the same place, so the distance is 0.".into());
        }
        Ok(Output::Select {
            sql: format!("SELECT {} AS n, {} AS weight_sum, {} AS std_distance_m, {} AS centre_x, {} AS centre_y, {} AS geom", pts.xy.len(), sqlnum(sw), sqlnum(sd), sqlnum(cx), sqlnum(cy), ring_wkt(&ring)),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

/// Standard deviational ellipse of plane deviations: (centre, semi-major, semi-minor, direction of the major axis in radians from +x), unscaled
/// by the number of standard deviations: semi-axes are sqrt(2 * lambda).
pub(super) fn ellipse_moments(xy: &[(f64, f64)], w: &[f64]) -> Result<((f64, f64), f64, f64, f64)> {
    let sw: f64 = w.iter().sum();
    let (mx, my) = (xy.iter().zip(w).map(|(p, w)| p.0 * w).sum::<f64>() / sw, xy.iter().zip(w).map(|(p, w)| p.1 * w).sum::<f64>() / sw);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (p, w) in xy.iter().zip(w) {
        let (dx, dy) = (p.0 - mx, p.1 - my);
        sxx += w * dx * dx;
        syy += w * dy * dy;
        sxy += w * dx * dy;
    }
    let (sxx, syy, sxy) = (sxx / sw, syy / sw, sxy / sw);
    let half = (sxx + syy) / 2.0;
    let rad = (((sxx - syy) / 2.0).powi(2) + sxy * sxy).sqrt();
    let (l1, l2) = (half + rad, half - rad);
    if !(l2 > 1e-12 * l1.max(1e-300)) || l1 <= 0.0 {
        bail!("Directional Distribution needs features that are spread in two directions; these lie on a line or at one place");
    }
    let phi = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    Ok(((mx, my), (2.0 * l1).sqrt(), (2.0 * l2).sqrt(), phi))
}

pub struct DirectionalDistribution;
impl Tool for DirectionalDistribution {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "directional_distribution", name: "Directional Distribution (Standard Deviational Ellipse)", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "The ellipse that shows the direction and spread of the features (Lefever 1926; Yuill 1971): its long axis follows the direction of greatest spread and each semi-axis is the square root of 2 times the standard deviation along that axis, times the number of standard deviations you choose (1 by default, as in ArcGIS Pro's Directional Distribution). Output is the ellipse as a polygon with the semi-axes in metres, the azimuth of the long axis (degrees clockwise from north, 0 to 180) and the number of features. On a geographic coordinate system the ellipse is computed in an azimuthal equidistant projection centred on the mean centre, where distances and directions from the centre are true, and its outline is transformed back to longitude and latitude; in a projected coordinate system it is computed in that projection (azimuth from grid north). Needs at least 3 features spread in two directions. Lines and polygons count by their centroid.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Vector),
            params: vec![weight_param(), Param::new("sd", "Standard deviations (1, 2 or 3)", ParamKind::Enum).default("1").options(&[("1", "1 standard deviation"), ("2", "2 standard deviations"), ("3", "3 standard deviations")])] }
    }
    fn output_fields(&self) -> OutFields {
        OutFields::from(&[])
            .add(NewField::new("n", "BIGINT"))
            .add(NewField::new("weight_sum", "DOUBLE"))
            .add(NewField::new("semi_major_m", "DOUBLE"))
            .add(NewField::new("semi_minor_m", "DOUBLE"))
            .add(NewField::new("azimuth_deg", "DOUBLE"))
            .add(NewField::new("centre_x", "DOUBLE"))
            .add(NewField::new("centre_y", "DOUBLE"))
            .add(NewField::new("sd", "INTEGER"))
            .add(NewField::new("plane", "VARCHAR"))
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = need_crs(i)?;
        let m = geomkit::metric(crs)?;
        let pts = read_pts(ctx, i, weight_of(p).as_deref(), "Directional Distribution")?;
        if pts.xy.len() < 3 {
            bail!("Directional Distribution needs at least 3 features");
        }
        let k: f64 = match p.str("sd").as_deref() { Some("2") => 2.0, Some("3") => 3.0, _ => 1.0 };
        // The plane the moments are taken in, and the way back to the layer's coordinates.
        let (plane, back, projection): (Vec<(f64, f64)>, Option<geomkit::Tx>, String) = if m.geographic {
            let (lon0, lat0) = mean_centre(&m, &pts);
            let def = format!("+proj=aeqd +lat_0={lat0:?} +lon_0={lon0:?} +a={:?} +b={:?} +units=m +no_defs", m.a, m.a * (1.0 - m.f));
            let fwd = geomkit::tx(crs, &def)?;
            let mut v = Vec::with_capacity(pts.xy.len());
            for (x, y) in &pts.xy {
                v.push(fwd.pt(*x, *y).ok_or_else(|| anyhow!("A feature could not be placed in the azimuthal equidistant projection used for the ellipse"))?);
            }
            (v, Some(geomkit::tx(&def, crs)?), format!("azimuthal equidistant centred on {lon0:.6}, {lat0:.6}"))
        } else {
            (pts.xy.clone(), None, "plane of the layer's projection".to_string())
        };
        let unit = if m.geographic { 1.0 } else { m.unit_m };
        let ((cx, cy), a, b, phi) = ellipse_moments(&plane, &pts.w)?;
        let (a, b) = (a * k, b * k);
        let ring: Vec<(f64, f64)> = (0..=180)
            .map(|t| {
                let th = 2.0 * std::f64::consts::PI * (t % 180) as f64 / 180.0;
                let (u, v) = (a * th.cos(), b * th.sin());
                (cx + u * phi.cos() - v * phi.sin(), cy + u * phi.sin() + v * phi.cos())
            })
            .collect();
        let (ring, centre) = match &back {
            Some(t) => {
                let r: Option<Vec<(f64, f64)>> = ring.iter().map(|(x, y)| t.pt(*x, *y)).collect();
                (r.ok_or_else(|| anyhow!("The ellipse reaches a place that cannot be turned back into longitude and latitude; use fewer standard deviations"))?, t.pt(cx, cy).unwrap_or((f64::NAN, f64::NAN)))
            }
            None => (ring, (cx, cy)),
        };
        // Azimuth clockwise from north of the major axis, folded into [0, 180).
        let az = (90.0 - phi.to_degrees()).rem_euclid(180.0);
        Ok(Output::Select {
            sql: format!(
                "SELECT {} AS n, {} AS weight_sum, {} AS semi_major_m, {} AS semi_minor_m, {} AS azimuth_deg, {} AS centre_x, {} AS centre_y, {} AS sd, {} AS plane, {} AS geom",
                pts.xy.len(), sqlnum(pts.w.iter().sum()), sqlnum(a * unit), sqlnum(b * unit), sqlnum(az), sqlnum(centre.0), sqlnum(centre.1), k as i32, lit(&projection), ring_wkt(&ring)
            ),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "mean_centre" => &["centroid of points", "average location", "centre of gravity", "weighted centre", "center of population"],
        "standard_distance" => &["spread", "dispersion", "circle of spread", "how spread out"],
        "directional_distribution" => &["standard deviational ellipse", "trend", "orientation", "direction of spread", "ellipse"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(MeanCentre), Box::new(StandardDistance), Box::new(DirectionalDistribution)]
}
