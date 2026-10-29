//! Spatial statistics on point sets (phase H3): k-nearest-neighbour weights, global Moran's I (Moran 1950; variance under
//! randomisation after Cliff & Ord 1981) and the Getis-Ord Gi* hot-spot statistic (Ord & Getis 1995). Textbook formulas, no library code.
//! Coordinates are planar, so callers must refuse geographic CRSs.

use anyhow::{bail, Result};

/// Most features the statistics accept; the neighbour search is O(n·k) with a grid, the sums O(n·k).
pub const MAX_FEATURES: usize = 200_000;

/// For every point, the indices of its `k` nearest other points (ties broken by index, so results are repeatable).
pub fn knn(xy: &[(f64, f64)], k: usize, cancel: &dyn Fn() -> bool) -> Result<Vec<Vec<usize>>> {
    let n = xy.len();
    if k == 0 || k >= n {
        bail!("The number of neighbours must be between 1 and {} (one less than the number of features)", n.saturating_sub(1));
    }
    // Uniform grid sized for ~2 points per cell; search rings of cells until k are found and the ring bound exceeds the k-th distance.
    let (minx, maxx) = xy.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (miny, maxy) = xy.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let side = ((maxx - minx).max(maxy - miny)).max(1e-9);
    let cells = ((n as f64 / 2.0).sqrt().ceil() as usize).max(1);
    let cs = side / cells as f64;
    let cell_of = |v: f64, lo: f64| (((v - lo) / cs).floor() as isize).clamp(0, cells as isize - 1) as usize;
    let mut grid: Vec<Vec<usize>> = vec![vec![]; cells * cells];
    for (i, p) in xy.iter().enumerate() {
        grid[cell_of(p.1, miny) * cells + cell_of(p.0, minx)].push(i);
    }
    let mut out = Vec::with_capacity(n);
    for (i, p) in xy.iter().enumerate() {
        if i % 1024 == 0 && cancel() {
            bail!("{}", crate::progress::CANCELLED);
        }
        let (cx, cy) = (cell_of(p.0, minx) as isize, cell_of(p.1, miny) as isize);
        let mut cand: Vec<(f64, usize)> = vec![];
        let mut r = 0isize;
        loop {
            for gy in (cy - r).max(0)..=(cy + r).min(cells as isize - 1) {
                for gx in (cx - r).max(0)..=(cx + r).min(cells as isize - 1) {
                    if (gy - cy).abs().max((gx - cx).abs()) != r {
                        continue; // only the new ring
                    }
                    for &j in &grid[gy as usize * cells + gx as usize] {
                        if j != i {
                            let (dx, dy) = (xy[j].0 - p.0, xy[j].1 - p.1);
                            cand.push((dx * dx + dy * dy, j));
                        }
                    }
                }
            }
            cand.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            // Everything within r·cs of the point is inside the searched rings.
            let safe = r as f64 * cs;
            if cand.len() >= k && cand[k - 1].0.sqrt() <= safe || r as usize >= cells {
                break;
            }
            r += 1;
        }
        out.push(cand.iter().take(k).map(|c| c.1).collect());
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct Moran {
    pub n: usize,
    pub i: f64,
    pub expected: f64,
    pub variance: f64,
    pub z: f64,
    pub p: f64,
}

/// Standard normal CDF (Abramowitz & Stegun 7.1.26 through erf).
pub fn normal_cdf(z: f64) -> f64 {
    let x = z / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    0.5 * (1.0 + if x >= 0.0 { y } else { -y })
}

/// Two-sided p-value for a z-score.
pub fn p_two_sided(z: f64) -> f64 {
    (2.0 * (1.0 - normal_cdf(z.abs()))).clamp(0.0, 1.0)
}

/// Global Moran's I with row-standardised k-nearest-neighbour weights. Needs at least 4 features and some variation in the values.
pub fn morans_i(values: &[f64], neighbours: &[Vec<usize>]) -> Result<Moran> {
    let n = values.len();
    if n < 4 {
        bail!("Moran's I needs at least 4 features with a number ({n} given)");
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let z: Vec<f64> = values.iter().map(|v| v - mean).collect();
    let m2: f64 = z.iter().map(|v| v * v).sum();
    if m2 <= 0.0 {
        bail!("All values are the same, so there is no pattern to measure");
    }
    // w_ij = 1/k_i for j in neighbours(i).
    let mut s0 = 0.0;
    let mut num = 0.0;
    let mut col = vec![0.0; n];
    let mut row = vec![0.0; n];
    let mut pair: std::collections::HashMap<(usize, usize), f64> = std::collections::HashMap::new();
    for (i, nb) in neighbours.iter().enumerate() {
        let w = 1.0 / nb.len() as f64;
        for &j in nb {
            s0 += w;
            num += w * z[i] * z[j];
            row[i] += w;
            col[j] += w;
            *pair.entry((i, j)).or_insert(0.0) += w;
        }
    }
    // S1 = 1/2 ΣΣ (w_ij + w_ji)²; S2 = Σ (row_i + col_i)².
    let mut s1 = 0.0;
    for (&(i, j), &wij) in &pair {
        let wji = pair.get(&(j, i)).copied().unwrap_or(0.0);
        s1 += (wij + wji).powi(2);
        if !pair.contains_key(&(j, i)) {
            s1 += wij * wij; // the (j,i) term with w_ij = 0
        }
    }
    let s1 = 0.5 * s1;
    let s2: f64 = (0..n).map(|i| (row[i] + col[i]).powi(2)).sum();
    let nf = n as f64;
    let i_stat = (nf / s0) * num / m2;
    let e = -1.0 / (nf - 1.0);
    let kurt = (z.iter().map(|v| v.powi(4)).sum::<f64>() / nf) / (m2 / nf).powi(2);
    let var = if n >= 4 {
        (nf * ((nf * nf - 3.0 * nf + 3.0) * s1 - nf * s2 + 3.0 * s0 * s0) - kurt * ((nf * nf - nf) * s1 - 2.0 * nf * s2 + 6.0 * s0 * s0)) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0) * s0 * s0) - e * e
    } else {
        f64::NAN
    };
    if !(var.is_finite() && var > 0.0) {
        bail!("The variance of Moran's I could not be computed for this layout of features");
    }
    let zs = (i_stat - e) / var.sqrt();
    Ok(Moran { n, i: i_stat, expected: e, variance: var, z: zs, p: p_two_sided(zs) })
}

#[derive(Debug, Clone, Copy)]
pub struct Gi {
    pub z: f64,
    pub p: f64,
    /// -3..=3: sign = hot (+) or cold (-); 3 = 99 %, 2 = 95 %, 1 = 90 % confidence; 0 = not significant.
    pub class: i8,
}

/// Getis-Ord Gi* for every feature: binary weights over the feature itself plus its `k` nearest neighbours.
pub fn getis_ord_gi_star(values: &[f64], neighbours: &[Vec<usize>]) -> Result<Vec<Gi>> {
    let n = values.len();
    if n < 4 {
        bail!("Hot spot analysis needs at least 4 features with a number ({n} given)");
    }
    let nf = n as f64;
    let mean = values.iter().sum::<f64>() / nf;
    let s = (values.iter().map(|v| v * v).sum::<f64>() / nf - mean * mean).max(0.0).sqrt();
    if s <= 0.0 {
        bail!("All values are the same, so there is no pattern to measure");
    }
    Ok(neighbours
        .iter()
        .enumerate()
        .map(|(i, nb)| {
            let w = (nb.len() + 1) as f64; // Σ w_ij with binary weights, self included
            let sum = values[i] + nb.iter().map(|&j| values[j]).sum::<f64>();
            let denom = s * ((nf * w - w * w) / (nf - 1.0)).sqrt();
            let z = (sum - mean * w) / denom;
            let p = p_two_sided(z);
            let level = if p < 0.01 { 3 } else if p < 0.05 { 2 } else if p < 0.10 { 1 } else { 0 };
            Gi { z, p, class: if z >= 0.0 { level } else { -level } }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lattice(n: usize) -> Vec<(f64, f64)> {
        (0..n * n).map(|i| ((i % n) as f64 * 10.0, (i / n) as f64 * 10.0)).collect()
    }

    #[test]
    fn knn_matches_brute_force() {
        // Irrational-ish spacing so there are no distance ties.
        let xy: Vec<(f64, f64)> = (0..300).map(|i| { let t = i as f64; ((t * 7.31).sin() * 500.0 + t, (t * 3.77).cos() * 400.0 - t * 0.3) }).collect();
        let nb = knn(&xy, 6, &|| false).unwrap();
        for i in 0..xy.len() {
            let mut all: Vec<(f64, usize)> = (0..xy.len()).filter(|&j| j != i).map(|j| ((xy[j].0 - xy[i].0).powi(2) + (xy[j].1 - xy[i].1).powi(2), j)).collect();
            all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            assert_eq!(nb[i], all.iter().take(6).map(|a| a.1).collect::<Vec<_>>(), "point {i}");
        }
    }

    #[test]
    fn knn_refuses_bad_k_and_stops_on_cancel() {
        let xy = lattice(3);
        assert!(knn(&xy, 0, &|| false).unwrap_err().to_string().contains("neighbours"));
        assert!(knn(&xy, 9, &|| false).is_err());
        assert!(knn(&xy, 2, &|| true).unwrap_err().to_string().contains("Cancelled"));
    }

    #[test]
    fn morans_i_hand_worked_example() {
        // Points on a line at 0, 1, 3, 6 with values 1..4; k = 1 gives neighbours 0→1, 1→0, 2→1, 3→2.
        let xy = [(0.0, 0.0), (1.0, 0.0), (3.0, 0.0), (6.0, 0.0)];
        let nb = knn(&xy, 1, &|| false).unwrap();
        assert_eq!(nb, vec![vec![1], vec![0], vec![1], vec![2]]);
        let m = morans_i(&[1.0, 2.0, 3.0, 4.0], &nb).unwrap();
        // z = -1.5 -0.5 0.5 1.5; Σz² = 5; Σ w z z = 0.75 + 0.75 - 0.25 + 0.75 = 2; S0 = 4 → I = (4/4)·2/5 = 0.4; E = -1/3.
        assert!((m.i - 0.4).abs() < 1e-12, "{}", m.i);
        assert!((m.expected + 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn morans_i_separates_clustered_random_and_checkerboard() {
        let xy = lattice(12);
        let nb = knn(&xy, 8, &|| false).unwrap();
        let clustered: Vec<f64> = xy.iter().map(|p| if p.0 < 60.0 { 10.0 } else { 0.0 }).collect();
        let m = morans_i(&clustered, &nb).unwrap();
        assert!(m.i > 0.6 && m.p < 0.001, "{m:?}");
        let checker: Vec<f64> = xy.iter().map(|p| (((p.0 / 10.0) as i64 + (p.1 / 10.0) as i64) % 2) as f64).collect();
        // With k = 8 the four diagonal neighbours share the colour and cancel the four edge neighbours; k = 4 sees only the opposite colour.
        let nb4 = knn(&xy, 4, &|| false).unwrap();
        let c = morans_i(&checker, &nb4).unwrap();
        assert!(c.i < 0.0 && c.z < -2.0, "{c:?}");
        // A fixed pseudo-random pattern is not significant.
        let mut s = 12345u64;
        let random: Vec<f64> = (0..144).map(|_| { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (s >> 33) as f64 }).collect();
        let r = morans_i(&random, &nb).unwrap();
        assert!(r.p > 0.01 && r.i.abs() < 0.3, "{r:?}");
    }

    #[test]
    fn morans_i_refuses_constant_values_and_tiny_sets() {
        let xy = lattice(3);
        let nb = knn(&xy, 3, &|| false).unwrap();
        assert!(morans_i(&[5.0; 9], &nb).unwrap_err().to_string().contains("same"));
        assert!(morans_i(&[1.0, 2.0, 3.0], &[vec![1], vec![0], vec![1]]).unwrap_err().to_string().contains("at least 4"));
    }

    #[test]
    fn gi_star_finds_the_hot_and_cold_corners() {
        let xy = lattice(12);
        let nb = knn(&xy, 8, &|| false).unwrap();
        // High values in the south-west block, low in the north-east, medium elsewhere.
        let v: Vec<f64> = xy.iter().map(|p| if p.0 < 30.0 && p.1 < 30.0 { 100.0 } else if p.0 > 80.0 && p.1 > 80.0 { 0.0 } else { 50.0 }).collect();
        let g = getis_ord_gi_star(&v, &nb).unwrap();
        assert_eq!(g[0].class, 3, "{:?}", g[0]);
        assert_eq!(g[143].class, -3, "{:?}", g[143]);
        assert_eq!(g[6 * 12 + 6].class, 0);
        assert!(g[0].z > 3.0 && g[0].p < 0.01);
    }

    #[test]
    fn normal_cdf_known_values() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-7);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((p_two_sided(2.5758) - 0.01).abs() < 1e-4);
    }
}
