//! Small geometry helpers shared by the Wave 1 analysis tools (point-pattern statistics, tabulate area, profiles, network analysis,
//! clustering): an ellipsoid and unit description of a CRS, PROJ point transforms, a WKT reader, scan-line polygon spans, authalic
//! cell areas on the ellipsoid, a seeded random generator and a spatial index whose distances are geodesic on a geographic CRS.
//! Written from the published formulas (Snyder 1987 for authalic areas; Karney 2013 through `geographiclib-rs` for geodesics).

use anyhow::{anyhow, bail, Result};
use gdal::spatial_ref::CoordTransform;
use geographiclib_rs::{Geodesic, InverseGeodesic};
use std::collections::HashMap;

/// What a tool needs to know about a CRS to measure in metres.
#[derive(Clone, Debug)]
pub struct Metric {
    pub geographic: bool,
    /// Ellipsoid semi-major axis (m) and flattening.
    pub a: f64,
    pub f: f64,
    /// Metres per CRS linear unit (1 for degrees: they are measured geodesically instead).
    pub unit_m: f64,
}

pub fn metric(crs: &str) -> Result<Metric> {
    let s = crate::crs::srs(&crate::crs::resolve(crs))?;
    let a = s.semi_major().map_err(|e| anyhow!("The ellipsoid of this coordinate system is unknown ({e})"))?;
    let b = s.semi_minor().map_err(|e| anyhow!("The ellipsoid of this coordinate system is unknown ({e})"))?;
    let geographic = s.is_geographic();
    Ok(Metric { geographic, a, f: (a - b) / a, unit_m: if geographic { 1.0 } else { s.linear_units() } })
}

impl Metric {
    pub fn geodesic(&self) -> Geodesic {
        Geodesic::new(self.a, self.f)
    }
    pub fn e2(&self) -> f64 {
        self.f * (2.0 - self.f)
    }
    /// Lon/lat (degrees) to Earth-centred Cartesian metres on the surface of the ellipsoid.
    pub fn to_ecef(&self, lon: f64, lat: f64) -> [f64; 3] {
        let (l, p) = (lon.to_radians(), lat.to_radians());
        let n = self.a / (1.0 - self.e2() * p.sin().powi(2)).sqrt();
        [n * p.cos() * l.cos(), n * p.cos() * l.sin(), n * (1.0 - self.e2()) * p.sin()]
    }
    /// Geodetic lon/lat (degrees) of the point on the surface along the ellipsoid normal through an Earth-centred position.
    pub fn from_ecef(&self, v: [f64; 3]) -> (f64, f64) {
        let e2 = self.e2();
        let p = v[0].hypot(v[1]);
        let mut lat = v[2].atan2(p * (1.0 - e2));
        for _ in 0..10 {
            let n = self.a / (1.0 - e2 * lat.sin().powi(2)).sqrt();
            lat = (v[2] + e2 * n * lat.sin()).atan2(p);
        }
        (v[1].atan2(v[0]).to_degrees(), lat.to_degrees())
    }
    /// Area (m²) of a lon/lat cell between two latitudes and `dlon` degrees wide on the ellipsoid (authalic formula, Snyder 1987 eq. 3-12).
    pub fn band_area(&self, lat1: f64, lat2: f64, dlon: f64) -> f64 {
        let (e2, e) = (self.e2(), self.e2().sqrt());
        let q = |phi: f64| {
            let s = phi.to_radians().sin();
            if e < 1e-12 { 2.0 * s } else { s / (1.0 - e2 * s * s) + (1.0 / (2.0 * e)) * ((1.0 + e * s) / (1.0 - e * s)).ln() }
        };
        (self.a * self.a * (1.0 - e2) / 2.0 * dlon.to_radians() * (q(lat2) - q(lat1))).abs()
    }
}

/// A PROJ transform between two CRSs (x east, y north).
pub struct Tx(CoordTransform);

pub fn tx(from: &str, to: &str) -> Result<Tx> {
    let (a, b) = (crate::crs::srs(&crate::crs::resolve(from))?, crate::crs::srs(&crate::crs::resolve(to))?);
    Ok(Tx(CoordTransform::new(&a, &b).map_err(|e| anyhow!("Cannot transform coordinates from {from} to {to}: {e}"))?))
}

impl Tx {
    /// None when PROJ cannot place the point (outside the projection's domain).
    pub fn pt(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let (mut xs, mut ys) = ([x], [y]);
        self.0.transform_coords(&mut xs, &mut ys, &mut []).ok()?;
        (xs[0].is_finite() && ys[0].is_finite()).then_some((xs[0], ys[0]))
    }
}

/// Parts of a geometry read from WKT: parts, each a list of rings (polygon) or one line, each a list of (x, y). Z and M values are dropped.
pub fn parse_wkt(s: &str) -> Result<Vec<Vec<Vec<(f64, f64)>>>> {
    let s = s.trim();
    if s.to_ascii_uppercase().contains("EMPTY") {
        return Ok(vec![]);
    }
    let open = s.find('(').ok_or_else(|| anyhow!("Unreadable geometry"))?;
    let kind = s[..open].trim().to_ascii_uppercase();
    // Nested lists: depth 1 holds coordinates for LINESTRING; POLYGON and MULTILINESTRING depth 2; MULTIPOLYGON depth 3.
    fn coords(t: &str) -> Result<Vec<(f64, f64)>> {
        t.split(',')
            .map(|p| {
                let mut it = p.split_whitespace();
                let x = it.next().and_then(|v| v.parse::<f64>().ok());
                let y = it.next().and_then(|v| v.parse::<f64>().ok());
                x.zip(y).ok_or_else(|| anyhow!("Unreadable coordinate '{}'", p.trim()))
            })
            .collect()
    }
    let body = &s[open..];
    let mut lists: Vec<(usize, Vec<(f64, f64)>)> = vec![]; // (depth, coordinates)
    let (mut depth, mut start) = (0usize, 0usize);
    for (i, ch) in body.char_indices() {
        match ch {
            '(' => {
                depth += 1;
                start = i + 1;
            }
            ')' => {
                let t = &body[start..i];
                if !t.trim().is_empty() {
                    lists.push((depth, coords(t)?));
                }
                depth -= 1;
                start = i + 1;
            }
            _ => {}
        }
    }
    let k = kind.as_str();
    if k.starts_with("MULTIPOLYGON") {
        // A polygon is the group of rings between the parentheses at depth 2.
        let mut parts: Vec<Vec<Vec<(f64, f64)>>> = vec![];
        let (mut d, mut st) = (0usize, 0usize);
        let mut cur: Vec<Vec<(f64, f64)>> = vec![];
        for (i, ch) in body.char_indices() {
            match ch {
                '(' => {
                    d += 1;
                    st = i + 1;
                    if d == 2 {
                        cur = vec![];
                    }
                }
                ')' => {
                    let t = &body[st..i];
                    if d == 3 && !t.trim().is_empty() {
                        cur.push(coords(t)?);
                    }
                    if d == 2 {
                        parts.push(std::mem::take(&mut cur));
                    }
                    d -= 1;
                    st = i + 1;
                }
                _ => {}
            }
        }
        return Ok(parts);
    }
    if k.starts_with("MULTILINESTRING") {
        return Ok(lists.into_iter().filter(|(d, _)| *d == 2).map(|(_, c)| vec![c]).collect());
    }
    if k.starts_with("POLYGON") {
        return Ok(vec![lists.into_iter().filter(|(d, _)| *d == 2).map(|(_, c)| c).collect()]);
    }
    if k.starts_with("LINESTRING") {
        return Ok(lists.into_iter().filter(|(d, _)| *d == 1).map(|(_, c)| vec![c]).collect());
    }
    bail!("Unsupported geometry type {kind}")
}

/// Intervals of x inside the rings (even-odd rule, so holes work) along the horizontal line at `y`.
pub fn spans(rings: &[&Vec<(f64, f64)>], y: f64) -> Vec<(f64, f64)> {
    let mut xs: Vec<f64> = vec![];
    for r in rings {
        for w in r.windows(2) {
            let ((x1, y1), (x2, y2)) = (w[0], w[1]);
            if (y1 <= y) != (y2 <= y) {
                xs.push(x1 + (y - y1) * (x2 - x1) / (y2 - y1));
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

/// True when (x, y) is inside the rings (even-odd rule).
pub fn inside(rings: &[&Vec<(f64, f64)>], x: f64, y: f64) -> bool {
    let mut c = false;
    for r in rings {
        for w in r.windows(2) {
            let ((x1, y1), (x2, y2)) = (w[0], w[1]);
            if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
                c = !c;
            }
        }
    }
    c
}

/// Distance from (x, y) to the nearest edge of the rings.
pub fn boundary_distance(rings: &[&Vec<(f64, f64)>], x: f64, y: f64) -> f64 {
    let mut best = f64::INFINITY;
    for r in rings {
        for w in r.windows(2) {
            let ((x1, y1), (x2, y2)) = (w[0], w[1]);
            let (dx, dy) = (x2 - x1, y2 - y1);
            let l2 = dx * dx + dy * dy;
            let t = if l2 == 0.0 { 0.0 } else { (((x - x1) * dx + (y - y1) * dy) / l2).clamp(0.0, 1.0) };
            best = best.min((x - (x1 + t * dx)).hypot(y - (y1 + t * dy)));
        }
    }
    best
}

/// A small seeded generator (xoshiro256**, seeded through SplitMix64; Blackman and Vigna 2018) so a run can be repeated from its seed.
pub struct Rng([u64; 4]);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E3779B97F4A7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
            x ^ (x >> 31)
        };
        Rng([next(), next(), next(), next()])
    }
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.0;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }
    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Points with distances in metres: planar in a projected CRS, geodesic (WGS 84 or the CRS's ellipsoid) in a geographic one. Neighbours
/// are found through a hash grid of Earth-centred positions; a chord is never longer than the geodesic, so a chord search with the same
/// radius cannot miss a point, and geodesic distances are then checked exactly.
pub struct Space {
    pub metric: Metric,
    pub xy: Vec<(f64, f64)>,
    keys: Vec<[f64; 3]>,
    geod: Option<Geodesic>,
}

pub struct Grid3 {
    cell: f64,
    map: HashMap<(i64, i64, i64), Vec<u32>>,
}

impl Space {
    pub fn new(metric: Metric, xy: Vec<(f64, f64)>) -> Space {
        let keys = xy.iter().map(|&(x, y)| if metric.geographic { metric.to_ecef(x, y) } else { [x * metric.unit_m, y * metric.unit_m, 0.0] }).collect();
        let geod = metric.geographic.then(|| metric.geodesic());
        Space { metric, xy, keys, geod }
    }
    pub fn len(&self) -> usize {
        self.xy.len()
    }
    /// Distance in metres between two points.
    pub fn dist(&self, i: usize, j: usize) -> f64 {
        match &self.geod {
            Some(g) => g.inverse(self.xy[i].1, self.xy[i].0, self.xy[j].1, self.xy[j].0),
            None => {
                let (a, b) = (self.keys[i], self.keys[j]);
                (a[0] - b[0]).hypot(a[1] - b[1])
            }
        }
    }
    pub fn grid(&self, cell: f64) -> Grid3 {
        let cell = cell.max(1e-6);
        let mut map: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
        for (i, k) in self.keys.iter().enumerate() {
            map.entry(Self::cell_of(k, cell)).or_default().push(i as u32);
        }
        Grid3 { cell, map }
    }
    fn cell_of(k: &[f64; 3], cell: f64) -> (i64, i64, i64) {
        ((k[0] / cell).floor() as i64, (k[1] / cell).floor() as i64, (k[2] / cell).floor() as i64)
    }
    /// Every point other than `i` within `r` metres of point `i`, with its distance.
    pub fn within(&self, g: &Grid3, i: usize, r: f64) -> Vec<(usize, f64)> {
        let k = self.keys[i];
        let (c0, n) = (Self::cell_of(&k, g.cell), (r / g.cell).ceil() as i64);
        let planar = self.geod.is_none();
        let mut out = vec![];
        for dx in -n..=n {
            for dy in -n..=n {
                for dz in if planar { 0..=0 } else { -n..=n } {
                    if let Some(v) = g.map.get(&(c0.0 + dx, c0.1 + dy, c0.2 + dz)) {
                        for &j in v {
                            let j = j as usize;
                            if j == i {
                                continue;
                            }
                            let q = self.keys[j];
                            let chord = ((k[0] - q[0]).powi(2) + (k[1] - q[1]).powi(2) + (k[2] - q[2]).powi(2)).sqrt();
                            if chord <= r {
                                let d = if planar { chord } else { self.dist(i, j) };
                                if d <= r {
                                    out.push((j, d));
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }
    /// The nearest other point to `i` and its distance in metres (None when there is no other point).
    pub fn nearest(&self, g: &Grid3, i: usize) -> Option<(usize, f64)> {
        if self.len() < 2 {
            return None;
        }
        let mut r = g.cell;
        loop {
            let c = self.within(g, i, r);
            if let Some(best) = c.into_iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
                return Some(best);
            }
            r *= 2.0;
            if r > 4.0e7 {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wkt_reader_handles_holes_multi_parts_and_z() {
        let p = parse_wkt("MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0),(2 2, 4 2, 4 4, 2 4, 2 2)),((20 20, 30 20, 30 30, 20 20)))").unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].len(), 2);
        assert_eq!(p[1][0].len(), 4);
        let rings: Vec<&Vec<(f64, f64)>> = p.iter().flatten().collect();
        assert!(inside(&rings, 1.0, 1.0) && !inside(&rings, 3.0, 3.0) && inside(&rings, 25.0, 22.0) && !inside(&rings, 15.0, 15.0));
        assert_eq!(spans(&rings, 3.0), vec![(0.0, 2.0), (4.0, 10.0)]);
        assert!((boundary_distance(&rings, 1.0, 5.0) - 1.0).abs() < 1e-12);
        let l = parse_wkt("LINESTRING Z (1 2 3, 4 5 6)").unwrap();
        assert_eq!(l, vec![vec![vec![(1.0, 2.0), (4.0, 5.0)]]]);
        assert!(parse_wkt("POLYGON EMPTY").unwrap().is_empty());
    }

    #[test]
    fn authalic_cell_area_matches_the_sphere_and_known_wgs84_values() {
        let sphere = Metric { geographic: true, a: 6_371_000.0, f: 0.0, unit_m: 1.0 };
        let want = 6_371_000.0f64.powi(2) * 1.0f64.to_radians() * (1.0f64.to_radians().sin());
        assert!((sphere.band_area(0.0, 1.0, 1.0) - want).abs() / want < 1e-12);
        let wgs = metric("EPSG:4326").unwrap();
        // The whole ellipsoid: 510 065 621.7 km2 (WGS 84 authalic radius 6 371 007.18 m).
        let total = wgs.band_area(-90.0, 90.0, 360.0);
        assert!((total / 1e6 - 510_065_621.7).abs() < 1.0, "{total}");
    }

    #[test]
    fn generator_repeats_from_its_seed() {
        let (mut a, mut b, mut c) = (Rng::new(5), Rng::new(5), Rng::new(6));
        let (x, y, z) = (a.f64(), b.f64(), c.f64());
        assert_eq!(x, y);
        assert!(x != z && (0.0..1.0).contains(&x));
    }

    #[test]
    fn nearest_and_within_use_geodesic_distance_on_degrees() {
        let m = metric("EPSG:4326").unwrap();
        // Two points 1 degree of longitude apart at 60 N (about 55.8 km) and one far away.
        let s = Space::new(m, vec![(0.0, 60.0), (1.0, 60.0), (40.0, 10.0)]);
        let g = s.grid(50_000.0);
        let (j, d) = s.nearest(&g, 0).unwrap();
        assert_eq!(j, 1);
        assert!((d - 55_800.0).abs() < 300.0, "{d}");
        assert_eq!(s.within(&g, 0, 50_000.0).len(), 0);
        assert_eq!(s.within(&g, 0, 60_000.0).len(), 1);
    }
}
