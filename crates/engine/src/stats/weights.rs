//! Spatial weights (package ST6): who counts as a neighbour. Contiguity (queen, rook) comes from polygon pairs found in SQL; k-nearest and
//! distance band come from point coordinates, with true geodesic distances (Karney 2013 through geographiclib-rs) when the coordinates are
//! degrees. Islands (features with no neighbour) are reported, never hidden. Also the small seeded random generator the permutation tests and
//! envelopes use, so a seed shown in a result reproduces it.

use geographiclib_rs::{Geodesic, InverseGeodesic};

pub type Res<T> = Result<T, String>;

/// Most features the dense and O(n^2) steps accept.
pub const MAX_FEATURES: usize = 20_000;

/// SplitMix64: tiny, fast, and the same on every platform.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Uniform integer in 0..n.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() as u128 * n as u128) >> 64) as usize
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

#[derive(Clone, Debug)]
pub struct Weights {
    pub n: usize,
    /// Neighbours of each feature (indices into the used rows), sorted.
    pub nbrs: Vec<Vec<usize>>,
    /// Human description for the result ("queen contiguity", "8 nearest neighbours", ...).
    pub kind: String,
}

impl Weights {
    pub fn islands(&self) -> Vec<usize> {
        (0..self.n).filter(|i| self.nbrs[*i].is_empty()).collect()
    }
    /// Number of neighbour links (each direction counts).
    pub fn links(&self) -> usize {
        self.nbrs.iter().map(|v| v.len()).sum()
    }
    /// Row-standardised weights w_ij = 1 / (number of neighbours of i); islands have an empty row.
    pub fn row_standardised(&self) -> Vec<Vec<(usize, f64)>> {
        self.nbrs.iter().map(|v| v.iter().map(|j| (*j, 1.0 / v.len() as f64)).collect()).collect()
    }
    pub fn binary(&self) -> Vec<Vec<(usize, f64)>> {
        self.nbrs.iter().map(|v| v.iter().map(|j| (*j, 1.0)).collect()).collect()
    }
    pub fn is_symmetric(&self) -> bool {
        self.nbrs.iter().enumerate().all(|(i, v)| v.iter().all(|j| self.nbrs[*j].binary_search(&i).is_ok()))
    }
}

/// Weights from a list of neighbour pairs (each pair is mutual).
pub fn from_pairs(n: usize, pairs: &[(usize, usize)], kind: &str) -> Weights {
    let mut nbrs = vec![vec![]; n];
    for &(a, b) in pairs {
        if a != b && a < n && b < n {
            nbrs[a].push(b);
            nbrs[b].push(a);
        }
    }
    for v in nbrs.iter_mut() {
        v.sort_unstable();
        v.dedup();
    }
    Weights { n, nbrs, kind: kind.into() }
}

fn ecef(lon: f64, lat: f64) -> [f64; 3] {
    let (a, e2) = (6_378_137.0f64, 0.006_694_379_990_141_316_5f64);
    let (l, p) = (lon.to_radians(), lat.to_radians());
    let nn = a / (1.0 - e2 * p.sin().powi(2)).sqrt();
    [nn * p.cos() * l.cos(), nn * p.cos() * l.sin(), nn * (1.0 - e2) * p.sin()]
}

/// Distance between two coordinate pairs: Euclidean for projected coordinates, WGS 84 geodesic metres for (lon, lat) degrees.
pub fn distance(a: (f64, f64), b: (f64, f64), geographic: bool) -> f64 {
    if geographic {
        let g = Geodesic::wgs84();
        let s12: f64 = g.inverse(a.1, a.0, b.1, b.0);
        s12
    } else {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
    }
}

/// The k nearest other points of every point (ties broken by index). Geographic coordinates are ranked by chord distance on the ellipsoid and the
/// 2k best re-ranked by exact geodesic distance.
pub fn knn(xy: &[(f64, f64)], k: usize, geographic: bool, cancel: &dyn Fn() -> bool) -> Res<Weights> {
    let n = xy.len();
    if k == 0 || k >= n {
        return Err(format!("The number of neighbours must be between 1 and {} (one less than the number of features).", n.saturating_sub(1)));
    }
    let nbrs: Vec<Vec<usize>> = if !geographic {
        crate::spatial_stats::knn(xy, k, cancel).map_err(|e| e.to_string())?
    } else {
        if n > MAX_FEATURES {
            return Err(format!("Geodesic neighbours are limited to {MAX_FEATURES} features ({n} given); project to a metre-based CRS or use a subset."));
        }
        let pts: Vec<[f64; 3]> = xy.iter().map(|p| ecef(p.0, p.1)).collect();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            if i % 256 == 0 && cancel() {
                return Err(crate::progress::CANCELLED.to_string());
            }
            let mut c: Vec<(f64, usize)> = (0..n).filter(|j| *j != i).map(|j| ((0..3).map(|d| (pts[i][d] - pts[j][d]).powi(2)).sum::<f64>(), j)).collect();
            c.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            c.truncate((2 * k).min(n - 1));
            let mut g: Vec<(f64, usize)> = c.iter().map(|(_, j)| (distance(xy[i], xy[*j], true), *j)).collect();
            g.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            out.push(g.iter().take(k).map(|x| x.1).collect());
        }
        out
    };
    let mut w = Weights { n, nbrs, kind: format!("{k} nearest neighbours{}", if geographic { " (geodesic distance)" } else { "" }) };
    for v in w.nbrs.iter_mut() {
        v.sort_unstable();
    }
    Ok(w)
}

/// Every pair of points closer than `d` (metres for geographic coordinates, map units otherwise) is a neighbour pair.
pub fn distance_band(xy: &[(f64, f64)], d: f64, geographic: bool, cancel: &dyn Fn() -> bool) -> Res<Weights> {
    let n = xy.len();
    if !(d > 0.0 && d.is_finite()) {
        return Err("The distance band must be a positive number.".into());
    }
    if n > MAX_FEATURES * 5 || (geographic && n > MAX_FEATURES) {
        return Err(format!("{n} features is more than the distance band supports; use a subset or k nearest neighbours."));
    }
    let mut pairs = vec![];
    if geographic {
        let pts: Vec<[f64; 3]> = xy.iter().map(|p| ecef(p.0, p.1)).collect();
        for i in 0..n {
            if i % 256 == 0 && cancel() {
                return Err(crate::progress::CANCELLED.to_string());
            }
            for j in i + 1..n {
                let c2: f64 = (0..3).map(|k| (pts[i][k] - pts[j][k]).powi(2)).sum();
                // The chord is never longer than the geodesic, so this only discards pairs that cannot qualify.
                if c2 <= d * d && distance(xy[i], xy[j], true) <= d {
                    pairs.push((i, j));
                }
            }
        }
    } else {
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|a, b| xy[*a].0.partial_cmp(&xy[*b].0).unwrap());
        for (a, &i) in idx.iter().enumerate() {
            if a % 1024 == 0 && cancel() {
                return Err(crate::progress::CANCELLED.to_string());
            }
            for &j in &idx[a + 1..] {
                if xy[j].0 - xy[i].0 > d {
                    break;
                }
                if distance(xy[i], xy[j], false) <= d {
                    pairs.push((i, j));
                }
            }
        }
    }
    Ok(from_pairs(n, &pairs, &format!("distance band {d}{}", if geographic { " m (geodesic)" } else { "" })))
}

/// Moran's I of `x` under weights `w` (any non-negative weights, usually row-standardised), with the analytic variance under randomisation
/// (Cliff and Ord 1981) and under normality.
#[derive(Clone, Debug)]
pub struct MoranResult {
    pub n: usize,
    pub i: f64,
    pub expected: f64,
    pub var_randomisation: f64,
    pub z: f64,
    pub p: f64,
    pub var_normality: f64,
    pub z_normality: f64,
    pub p_normality: f64,
}

pub fn moran(x: &[f64], w: &[Vec<(usize, f64)>]) -> Res<MoranResult> {
    let n = x.len();
    if n < 4 {
        return Err(format!("Moran's I needs at least 4 features ({n} given)."));
    }
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let z: Vec<f64> = x.iter().map(|v| v - mean).collect();
    let m2: f64 = z.iter().map(|v| v * v).sum();
    if m2 <= 0.0 {
        return Err("All values are the same, so there is no pattern to measure.".into());
    }
    let (mut s0, mut num) = (0.0, 0.0);
    let (mut row, mut col) = (vec![0.0; n], vec![0.0; n]);
    let mut pair: std::collections::HashMap<(usize, usize), f64> = std::collections::HashMap::new();
    for (i, r) in w.iter().enumerate() {
        for &(j, wij) in r {
            s0 += wij;
            num += wij * z[i] * z[j];
            row[i] += wij;
            col[j] += wij;
            *pair.entry((i, j)).or_insert(0.0) += wij;
        }
    }
    if s0 <= 0.0 {
        return Err("Every feature is an island (no neighbours), so there is nothing to measure.".into());
    }
    let mut s1 = 0.0;
    for (&(i, j), &wij) in &pair {
        let wji = pair.get(&(j, i)).copied().unwrap_or(0.0);
        s1 += (wij + wji).powi(2);
        if !pair.contains_key(&(j, i)) {
            s1 += wij * wij;
        }
    }
    // Pairs (i, j) are visited once per ordered pair, so every unordered pair was counted twice: S1 = 1/2 sum_i sum_j (w_ij + w_ji)^2.
    let s1 = 0.5 * s1;
    let s2: f64 = (0..n).map(|i| (row[i] + col[i]).powi(2)).sum();
    let i_stat = (nf / s0) * num / m2;
    let e = -1.0 / (nf - 1.0);
    let kurt = (z.iter().map(|v| v.powi(4)).sum::<f64>() / nf) / (m2 / nf).powi(2);
    let var_r = (nf * ((nf * nf - 3.0 * nf + 3.0) * s1 - nf * s2 + 3.0 * s0 * s0) - kurt * ((nf * nf - nf) * s1 - 2.0 * nf * s2 + 6.0 * s0 * s0)) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0) * s0 * s0) - e * e;
    let var_n = (nf * nf * s1 - nf * s2 + 3.0 * s0 * s0) / ((nf * nf - 1.0) * s0 * s0) - e * e;
    let p_of = |v: f64| {
        let zs = (i_stat - e) / v.sqrt();
        (zs, super::inference::pnorm(-zs.abs()) * 2.0)
    };
    if !(var_r.is_finite() && var_r > 0.0) {
        return Err("The variance of Moran's I could not be computed for this layout.".into());
    }
    let (zr, pr) = p_of(var_r);
    let (zn, pn) = p_of(var_n);
    Ok(MoranResult { n, i: i_stat, expected: e, var_randomisation: var_r, z: zr, p: pr.min(1.0), var_normality: var_n, z_normality: zn, p_normality: pn.min(1.0) })
}

/// Spatial lag of x: sum_j w_ij x_j.
pub fn lag(w: &[Vec<(usize, f64)>], x: &[f64]) -> Vec<f64> {
    w.iter().map(|r| r.iter().map(|(j, v)| v * x[*j]).sum()).collect()
}

#[derive(Clone, Debug)]
pub struct Lisa {
    pub i: f64,
    pub z: f64,
    pub lag_z: f64,
    /// Pseudo p-value by conditional permutation (two-sided).
    pub p: f64,
    /// 1 = high-high, 2 = low-high, 3 = low-low, 4 = high-low (GeoDa's numbering); 0 when the feature has no neighbours.
    pub quadrant: u8,
}

/// Local Moran's I (Anselin 1995) with row-standardised weights: I_i = z_i * sum_j w_ij z_j / m2, m2 = sum z^2 / n, and a conditional permutation
/// p-value: x_i is held fixed and its neighbours' values are drawn at random from the other features, `perms` times.
pub fn local_moran(x: &[f64], w: &[Vec<(usize, f64)>], perms: usize, seed: u64, cancel: &dyn Fn() -> bool) -> Res<Vec<Lisa>> {
    let n = x.len();
    if n < 4 {
        return Err("Local Moran needs at least 4 features.".into());
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    let z: Vec<f64> = x.iter().map(|v| v - mean).collect();
    let m2 = z.iter().map(|v| v * v).sum::<f64>() / n as f64;
    if m2 <= 0.0 {
        return Err("All values are the same, so there is no pattern to measure.".into());
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if i % 64 == 0 && cancel() {
            return Err(crate::progress::CANCELLED.to_string());
        }
        let lz: f64 = w[i].iter().map(|(j, v)| v * z[*j]).sum();
        let ii = z[i] * lz / m2;
        let k = w[i].len();
        let quadrant = if k == 0 {
            0
        } else if z[i] >= 0.0 && lz >= 0.0 {
            1
        } else if z[i] < 0.0 && lz >= 0.0 {
            2
        } else if z[i] < 0.0 {
            3
        } else {
            4
        };
        let mut p = f64::NAN;
        if k > 0 && perms > 0 && n > k {
            let wi = 1.0 / k as f64;
            let (mut hi, mut lo) = (0usize, 0usize);
            // Candidate values: every feature except i.
            let others: Vec<usize> = (0..n).filter(|j| *j != i).collect();
            for _ in 0..perms {
                // Draw k distinct others by partial Fisher-Yates over an index scratch.
                let mut s = 0.0;
                let mut picked: Vec<usize> = Vec::with_capacity(k);
                while picked.len() < k {
                    let c = others[rng.below(others.len())];
                    if !picked.contains(&c) {
                        picked.push(c);
                        s += wi * z[c];
                    }
                }
                let ip = z[i] * s / m2;
                if ip >= ii {
                    hi += 1;
                }
                if ip <= ii {
                    lo += 1;
                }
            }
            let r = perms as f64 + 1.0;
            p = (2.0 * ((hi + 1) as f64).min((lo + 1) as f64) / r).min(1.0);
        }
        out.push(Lisa { i: ii, z: z[i], lag_z: lz, p, quadrant });
    }
    Ok(out)
}
