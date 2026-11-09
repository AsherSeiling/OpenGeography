//! Point pattern analysis (Wave 1, package T4): Average Nearest Neighbour and Ripley's K. Both need the study area as a polygon layer:
//! a pattern is clustered or dispersed only relative to the region the points could have fallen in, so the tools never assume a bounding box.
//!
//! Average nearest neighbour (Clark and Evans 1954, Ecology 35:445): with n points in area A, the expected mean nearest-neighbour distance of
//! a random pattern is E = 0.5 / sqrt(n / A) and its standard error is SE = 0.26136 / sqrt(n^2 / A); the nearest-neighbour ratio is
//! R = observed mean distance / E and z = (observed - E) / SE (the constants are those used by Clark and Evans and by ArcGIS Pro). Edge effects
//! are NOT corrected: points near the border of the study area have nearest neighbours that may lie outside it, which biases R upward.
//!
//! Ripley's K (Ripley 1976, J. Applied Probability 13:255; 1977, J. Royal Statistical Society B 39:172): K(d) = A / (n (n - 1)) * the number of
//! ordered pairs at distance <= d. The optional border correction is the reduced-sample estimator (Ripley 1977; Diggle 2003): for each d only
//! points at least d from the border of the study area act as centres, K(d) = A / ((n - 1) m_d) * sum over those m_d centres of the number
//! of points within d. L(d) = sqrt(K(d) / pi) (Besag 1977), so L(d) - d is 0 for complete spatial randomness (CSR). The Monte Carlo envelope
//! is the pointwise minimum and maximum of K over the chosen number of simulated CSR patterns of the same number of points in the study area
//! (Diggle 2003; Besag and Diggle 1977), a 2 / (permutations + 1) level at each distance. The seed is an input and is written in the result.
//! Ripley's K works in a projected coordinate system only; a geographic layer is refused with a request to project it first.

use super::centrography::{read_pts, sqlnum};
use super::*;
use crate::geomkit::{self, Metric, Rng, Space};

/// Study area rings (outer ring first, then holes, per part) in the points' CRS, and its area in square metres.
struct Area {
    parts: Vec<Vec<Vec<(f64, f64)>>>,
    area_m2: f64,
}

impl Area {
    fn rings(&self) -> Vec<&Vec<(f64, f64)>> {
        self.parts.iter().flatten().collect()
    }
    fn bbox(&self) -> [f64; 4] {
        let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for r in self.parts.iter().flatten() {
            for (x, y) in r {
                b = [b[0].min(*x), b[1].min(*y), b[2].max(*x), b[3].max(*y)];
            }
        }
        b
    }
}

fn shoelace(r: &[(f64, f64)]) -> f64 {
    r.windows(2).map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1).sum::<f64>().abs() / 2.0
}

/// The union of the study-area layer in the points' CRS (an explicit transform when the layers differ) and its area. In a geographic CRS the
/// area is measured after transforming the outline to a Lambert azimuthal equal-area projection centred on the area, which is exact for areas on
/// the ellipsoid up to the straight-edge approximation of the outline.
fn read_area(ctx: &Ctx, area: &Rel, pts_crs: &str, m: &Metric) -> Result<Area> {
    if !area.has_geom {
        bail!("The study area must be a polygon layer");
    }
    if ctx.dimension(area)? != 2 {
        bail!("The study area must be a polygon layer, not points or lines");
    }
    let g = ctx.geom_in(area, "a", pts_crs)?;
    let wkt: Option<String> = ctx.conn.query_row(
        &format!("SELECT ST_AsText(ST_Union_Agg(g)) FROM (SELECT {g} AS g FROM {} a WHERE a.geom IS NOT NULL AND NOT ST_IsEmpty(a.geom)) WHERE g IS NOT NULL", ident(&area.table)),
        [],
        |r| r.get(0),
    )?;
    let parts = geomkit::parse_wkt(&wkt.ok_or_else(|| anyhow!("The study area layer has no polygon that can be placed in the points' coordinate system"))?)?;
    if parts.is_empty() {
        bail!("The study area layer has no polygons");
    }
    let area_m2 = if m.geographic {
        let (lon, lat) = ctx.center_lonlat(area)?;
        let def = format!("+proj=laea +lat_0={lat:?} +lon_0={lon:?} +a={:?} +b={:?} +units=m +no_defs", m.a, m.a * (1.0 - m.f));
        let t = geomkit::tx(pts_crs, &def)?;
        let mut total = 0.0;
        for part in &parts {
            for (k, ring) in part.iter().enumerate() {
                let r: Option<Vec<(f64, f64)>> = ring.iter().map(|(x, y)| t.pt(*x, *y)).collect();
                let a = shoelace(&r.ok_or_else(|| anyhow!("The study area could not be projected to measure its area"))?);
                total += if k == 0 { a } else { -a };
            }
        }
        total
    } else {
        parts.iter().map(|p| p.iter().enumerate().map(|(k, r)| if k == 0 { shoelace(r) } else { -shoelace(r) }).sum::<f64>()).sum::<f64>() * m.unit_m * m.unit_m
    };
    if !(area_m2 > 0.0) {
        bail!("The study area has no area");
    }
    Ok(Area { parts, area_m2 })
}

fn area_port() -> Port {
    Port { id: "area", label: "Study area (polygons)", kind: PortKind::Vector }
}
fn pts_port() -> Port {
    Port { id: "in", label: "Points", kind: PortKind::Vector }
}

pub struct AverageNearestNeighbour;
impl Tool for AverageNearestNeighbour {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "average_nearest_neighbour", name: "Average Nearest Neighbour", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "Are the points clustered, random or evenly spread? Compares the mean distance from each point to its nearest neighbour with the mean expected for a random pattern in the same study area (Clark and Evans 1954): expected = 0.5 / sqrt(n / A), standard error = 0.26136 / sqrt(n squared / A). A ratio below 1 means clustered, above 1 dispersed; the z-score and p-value test it. The study area is a required polygon layer (the result depends on it, so no bounding box is assumed); its area is measured on the ellipsoid on a geographic coordinate system. Distances are in metres, geodesic on a geographic coordinate system. Edge effects are not corrected. Lines and polygons count by their centroid. One-row table.",
            inputs: vec![pts_port(), area_port()], output: Some(PortKind::Table), params: vec![] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (i, a) = (&inputs["in"], &inputs["area"]);
        let crs = need_crs(i)?;
        let m = geomkit::metric(crs)?;
        let pts = read_pts(ctx, i, None, "Average Nearest Neighbour")?;
        let n = pts.xy.len();
        if n < 3 {
            bail!("Average Nearest Neighbour needs at least 3 points");
        }
        let area = read_area(ctx, a, crs, &m)?;
        let rings = area.rings();
        let outside = pts.xy.iter().filter(|(x, y)| !geomkit::inside(&rings, *x, *y)).count();
        if outside > 0 {
            ctx.add_note(format!("Average Nearest Neighbour: {outside} of {n} points lie outside the study area. They are still used; the expected distance assumes all points are inside, so check the study area."));
        }
        let space = Space::new(m.clone(), pts.xy.clone());
        let cell = (area.area_m2 / n as f64).sqrt().max(1e-3);
        let grid = space.grid(cell);
        let (mut sum, mut zero) = (0.0, 0usize);
        for k in 0..n {
            if k % 1024 == 0 && crate::progress::run_cancelled() {
                bail!("{}", crate::progress::CANCELLED);
            }
            let (_, d) = space.nearest(&grid, k).ok_or_else(|| anyhow!("A point has no neighbour"))?;
            if d == 0.0 {
                zero += 1;
            }
            sum += d;
        }
        if zero > 0 {
            ctx.add_note(format!("Average Nearest Neighbour: {zero} point(s) have another point at exactly the same place (distance 0)."));
        }
        let (nf, aa) = (n as f64, area.area_m2);
        let obs = sum / nf;
        let exp = 0.5 / (nf / aa).sqrt();
        let se = 0.26136 / (nf * nf / aa).sqrt();
        let z = (obs - exp) / se;
        let p = crate::spatial_stats::p_two_sided(z);
        let pattern = if p >= 0.05 { "random" } else if obs < exp { "clustered" } else { "dispersed" };
        Ok(Output::Select {
            sql: format!(
                "SELECT {n} AS points, {} AS study_area_m2, {} AS observed_mean_distance_m, {} AS expected_mean_distance_m, {} AS nn_ratio, {} AS z_score, {} AS p_value, '{pattern}' AS pattern, {outside} AS points_outside_area, 'not corrected' AS edge_effects",
                sqlnum(aa), sqlnum(obs), sqlnum(exp), sqlnum(obs / exp), sqlnum(z), sqlnum(p)
            ),
            crs: None,
            view: false,
        })
    }
}

/// K(d) for each distance (metres) of one pattern.
fn k_values(space: &Space, ds: &[f64], area_m2: f64, border: Option<&[f64]>, cancel: bool) -> Result<Vec<f64>> {
    let n = space.len();
    let dmax = *ds.last().unwrap();
    let grid = space.grid(dmax.max(1e-6));
    let mut acc = vec![0.0f64; ds.len()];
    let mut cnt = vec![0.0f64; ds.len()];
    for i in 0..n {
        if cancel && i % 512 == 0 && crate::progress::run_cancelled() {
            bail!("{}", crate::progress::CANCELLED);
        }
        let mut dist: Vec<f64> = space.within(&grid, i, dmax).into_iter().map(|c| c.1).collect();
        dist.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (k, d) in ds.iter().enumerate() {
            let c = dist.partition_point(|x| x <= d) as f64;
            match border {
                None => acc[k] += c,
                Some(b) => {
                    if b[i] >= *d {
                        acc[k] += c;
                        cnt[k] += 1.0;
                    }
                }
            }
        }
    }
    let nf = n as f64;
    Ok((0..ds.len()).map(|k| match border { None => area_m2 / (nf * (nf - 1.0)) * acc[k], Some(_) => if cnt[k] > 0.0 { area_m2 / ((nf - 1.0) * cnt[k]) * acc[k] } else { f64::NAN } }).collect())
}

fn border_distances(rings: &[&Vec<(f64, f64)>], xy: &[(f64, f64)], unit_m: f64) -> Vec<f64> {
    xy.iter().map(|(x, y)| if geomkit::inside(rings, *x, *y) { geomkit::boundary_distance(rings, *x, *y) * unit_m } else { 0.0 }).collect()
}

fn parse_distances(text: &str) -> Result<Vec<f64>> {
    let mut v: Vec<f64> = text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|t| !t.is_empty()).map(|t| t.parse::<f64>().map_err(|_| anyhow!("'{t}' in the distances is not a number"))).collect::<Result<_>>()?;
    if v.is_empty() {
        bail!("Give at least one distance, in metres, separated by commas (for example 100, 200, 500)");
    }
    if v.iter().any(|d| !(d.is_finite() && *d > 0.0)) {
        bail!("Distances must be numbers greater than 0");
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v.dedup();
    if v.len() > 200 {
        bail!("At most 200 distances");
    }
    Ok(v)
}

pub struct RipleysK;
impl Tool for RipleysK {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "ripleys_k", name: "Ripley's K Function", category: "Analysis › Spatial Statistics", role: Role::Tool,
            description: "Is the pattern clustered or dispersed, and at what scale? For each distance, K(d) is the study area divided by n(n-1), times the number of ordered pairs of points within d (Ripley 1976); L(d) = sqrt(K/pi) and L(d) - d is 0 for a random pattern, above 0 for clustering, below 0 for dispersion. The study area is a required polygon layer. Edge correction: none, or the border (reduced-sample) method, where only points at least d from the study-area edge count as centres (Ripley 1977). A Monte Carlo envelope is made from simulated random patterns of the same number of points in the study area: the pointwise minimum and maximum of K, a 2/(permutations + 1) level at each distance; the seed is shown in the result so a run can be repeated. Distances are in metres. Needs a projected layer (project it first); distances much larger than a quarter of the study area's shorter side are unreliable. Output is a table with one row per distance.",
            inputs: vec![pts_port(), area_port()], output: Some(PortKind::Table),
            params: vec![
                Param::new("distances", "Distances (metres, comma-separated)", ParamKind::Text),
                Param::new("edge", "Edge correction", ParamKind::Enum).default("border").options(&[("none", "None"), ("border", "Border (reduced sample)")]),
                Param::new("permutations", "Random patterns for the envelope (0 = none)", ParamKind::Number).default(99),
                Param::new("seed", "Random seed", ParamKind::Number).default(12345),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (i, a) = (&inputs["in"], &inputs["area"]);
        let crs = need_crs(i)?;
        let m = geomkit::metric(crs)?;
        if m.geographic {
            bail!("Ripley's K measures plane distances, and this layer is in degrees. Project the points and the study area to a metre-based coordinate system first (Project).");
        }
        let ds = parse_distances(&p.req_str("distances", "Distances")?)?;
        let perms = p.f64("permutations").unwrap_or(99.0);
        if !(perms.is_finite() && perms >= 0.0 && perms.fract() == 0.0 && perms <= 9999.0) {
            bail!("'Random patterns for the envelope' must be a whole number from 0 to 9999");
        }
        let perms = perms as usize;
        let seed = p.f64("seed").unwrap_or(12345.0);
        if !(seed.is_finite() && seed >= 0.0 && seed.fract() == 0.0) {
            bail!("'Random seed' must be a whole number, 0 or more");
        }
        let border_on = p.str("edge").as_deref() != Some("none");
        let pts = read_pts(ctx, i, None, "Ripley's K")?;
        let n = pts.xy.len();
        if n < 3 {
            bail!("Ripley's K needs at least 3 points");
        }
        let area = read_area(ctx, a, crs, &m)?;
        let rings = area.rings();
        let bb = area.bbox();
        let short = ((bb[2] - bb[0]).min(bb[3] - bb[1])) * m.unit_m;
        if *ds.last().unwrap() > short / 4.0 {
            ctx.add_note(format!("Ripley's K: the largest distance ({} m) is more than a quarter of the study area's shorter side ({:.0} m); estimates at that range are unreliable.", ds.last().unwrap(), short));
        }
        let outside = pts.xy.iter().filter(|(x, y)| !geomkit::inside(&rings, *x, *y)).count();
        if outside > 0 {
            ctx.add_note(format!("Ripley's K: {outside} of {n} points lie outside the study area; with the border correction they are not used as centres."));
        }
        let space = Space::new(m.clone(), pts.xy.clone());
        let bd = border_distances(&rings, &pts.xy, m.unit_m);
        let obs = k_values(&space, &ds, area.area_m2, border_on.then_some(&bd[..]), true)?;
        // Monte Carlo: n uniform points in the study area per pattern.
        let mut rng = Rng::new(seed as u64);
        let mut sims: Vec<Vec<f64>> = vec![];
        let task = crate::progress::Task::start("Ripley's K: simulating random patterns").cancellable();
        for s in 0..perms {
            task.check()?;
            task.set(s as f64 / perms.max(1) as f64, Some(format!("pattern {} of {perms}", s + 1)));
            let mut xy = Vec::with_capacity(n);
            let mut tries = 0u64;
            while xy.len() < n {
                let (x, y) = (bb[0] + rng.f64() * (bb[2] - bb[0]), bb[1] + rng.f64() * (bb[3] - bb[1]));
                if geomkit::inside(&rings, x, y) {
                    xy.push((x, y));
                }
                tries += 1;
                if tries > 50_000_000 {
                    bail!("The study area is too thin or small for random points to be placed in it");
                }
            }
            let sp = Space::new(m.clone(), xy);
            let b = border_on.then(|| border_distances(&rings, &sp.xy, m.unit_m));
            sims.push(k_values(&sp, &ds, area.area_m2, b.as_deref(), false)?);
        }
        let mut rows = vec![];
        for (k, d) in ds.iter().enumerate() {
            let col: Vec<f64> = sims.iter().map(|s| s[k]).filter(|v| v.is_finite()).collect();
            let (lo, hi, mean) = if col.is_empty() { (f64::NAN, f64::NAN, f64::NAN) } else { (col.iter().cloned().fold(f64::MAX, f64::min), col.iter().cloned().fold(f64::MIN, f64::max), col.iter().sum::<f64>() / col.len() as f64) };
            let l = |kv: f64| if kv.is_finite() && kv >= 0.0 { (kv / std::f64::consts::PI).sqrt() } else { f64::NAN };
            let verdict = if !obs[k].is_finite() { "no centres far enough from the edge" } else if !hi.is_finite() { "no envelope" } else if obs[k] > hi { "clustered" } else if obs[k] < lo { "dispersed" } else { "consistent with random" };
            let reduced = if border_on { bd.iter().filter(|b| **b >= *d).count() as i64 } else { n as i64 };
            rows.push(format!(
                "({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '{verdict}')",
                sqlnum(*d), sqlnum(obs[k]), sqlnum(std::f64::consts::PI * d * d), sqlnum(l(obs[k])), sqlnum(l(obs[k]) - d), sqlnum(mean), sqlnum(lo), sqlnum(hi), sqlnum(l(lo) - d), sqlnum(l(hi) - d), reduced
            ));
        }
        Ok(Output::Select {
            sql: format!(
                "SELECT t.*, {n} AS points, {} AS study_area_m2, '{}' AS edge_correction, {perms} AS permutations, {} AS seed FROM (VALUES {}) t(distance_m, k, k_csr_expected, l, l_minus_d, k_sim_mean, k_sim_min, k_sim_max, l_minus_d_sim_min, l_minus_d_sim_max, centres_used, verdict)",
                sqlnum(area.area_m2), if border_on { "border (reduced sample)" } else { "none" }, seed as u64, rows.join(", ")
            ),
            crs: None,
            view: false,
        })
    }
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "average_nearest_neighbour" => &["clark evans", "nearest neighbour index", "clustered or random", "point pattern", "nni"],
        "ripleys_k" => &["l function", "clustering at scale", "point pattern", "csr envelope", "monte carlo", "multi-distance"],
        _ => &[],
    }
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(AverageNearestNeighbour), Box::new(RipleysK)]
}
