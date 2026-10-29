//! Ordinary kriging (phase H2): empirical variogram, model fit and prediction from the nearest points.
//! Matheron (1963); the estimator and its system follow Isaaks & Srivastava, "An Introduction to Applied Geostatistics" (1989), ch. 12.
//! Written from the textbook equations; no library code. Coordinates are planar, so callers must refuse geographic CRSs.

use crate::raster_ops::{check_cells, cancelled, create_like, Grid, OUT_NODATA};
use anyhow::{anyhow, bail, Result};
use gdal::raster::Buffer;
use gdal::Metadata;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Model {
    Spherical,
    Exponential,
    Gaussian,
}

impl Model {
    pub fn parse(s: &str) -> Result<Option<Model>> {
        Ok(match s {
            "auto" => None,
            "spherical" => Some(Model::Spherical),
            "exponential" => Some(Model::Exponential),
            "gaussian" => Some(Model::Gaussian),
            o => bail!("Unknown variogram model \"{o}\""),
        })
    }
    /// 0 at lag 0, 1 at (and for the sphere beyond) the range.
    fn shape(self, r: f64) -> f64 {
        match self {
            Model::Spherical => if r >= 1.0 { 1.0 } else { 1.5 * r - 0.5 * r * r * r },
            Model::Exponential => 1.0 - (-3.0 * r).exp(),
            Model::Gaussian => 1.0 - (-3.0 * r * r).exp(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Variogram {
    pub model: Model,
    pub nugget: f64,
    /// Partial sill: the sill is `nugget + sill`.
    pub sill: f64,
    pub range: f64,
}

impl Variogram {
    pub fn gamma(&self, h: f64) -> f64 {
        if h <= 0.0 { 0.0 } else { self.nugget + self.sill * self.model.shape(h / self.range) }
    }
}

/// Points with equal coordinates are merged (mean value): they would make the kriging system singular.
pub fn merge_duplicates(pts: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64)> {
    let mut map: std::collections::BTreeMap<(u64, u64), (f64, usize, f64, f64)> = std::collections::BTreeMap::new();
    for &(x, y, v) in pts {
        let e = map.entry((x.to_bits(), y.to_bits())).or_insert((0.0, 0, x, y));
        e.0 += v;
        e.1 += 1;
    }
    map.into_values().map(|(s, n, x, y)| (x, y, s / n as f64)).collect()
}

/// Binned semivariances: (mean lag, semivariance, pair count) for up to 15 bins out to half the longest extent.
pub fn empirical(pts: &[(f64, f64, f64)]) -> Vec<(f64, f64, u64)> {
    let step = (pts.len() / 2000).max(1); // thin very large sets evenly so the pair loop stays bounded
    let s: Vec<&(f64, f64, f64)> = pts.iter().step_by(step).collect();
    let (minx, maxx) = s.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (miny, maxy) = s.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let maxlag = 0.5 * ((maxx - minx).powi(2) + (maxy - miny).powi(2)).sqrt();
    const BINS: usize = 15;
    let w = maxlag / BINS as f64;
    let mut acc = [(0.0f64, 0.0f64, 0u64); BINS];
    for i in 0..s.len() {
        for j in i + 1..s.len() {
            let h = ((s[i].0 - s[j].0).powi(2) + (s[i].1 - s[j].1).powi(2)).sqrt();
            if h > 0.0 && h < maxlag {
                let b = ((h / w) as usize).min(BINS - 1);
                acc[b].0 += h;
                acc[b].1 += 0.5 * (s[i].2 - s[j].2).powi(2);
                acc[b].2 += 1;
            }
        }
    }
    acc.iter().filter(|a| a.2 > 0).map(|a| (a.0 / a.2 as f64, a.1 / a.2 as f64, a.2)).collect()
}

/// Fit nugget, partial sill and range by weighted least squares over a grid of candidates (weights: pair counts).
/// `model` None tries all three and keeps the best.
pub fn fit(pts: &[(f64, f64, f64)], model: Option<Model>) -> Result<Variogram> {
    if pts.len() < 6 {
        bail!("Kriging needs at least 6 points with a number ({} given)", pts.len());
    }
    let emp = empirical(pts);
    if emp.len() < 3 {
        bail!("The points are too few or too close together to estimate a variogram");
    }
    let n = pts.len() as f64;
    let mean = pts.iter().map(|p| p.2).sum::<f64>() / n;
    let var = pts.iter().map(|p| (p.2 - mean).powi(2)).sum::<f64>() / (n - 1.0);
    if var <= 0.0 {
        bail!("All values are the same, so there is nothing to interpolate");
    }
    let maxlag = emp.last().unwrap().0 * 1.2;
    let models = match model { Some(m) => vec![m], None => vec![Model::Spherical, Model::Exponential, Model::Gaussian] };
    let mut best: Option<(f64, Variogram)> = None;
    for &m in &models {
        for ri in 1..=24 {
            let range = maxlag * ri as f64 / 20.0;
            for &nf in &[0.0, 0.05, 0.1, 0.2, 0.3, 0.5] {
                for si in 4..=16 {
                    let v = Variogram { model: m, nugget: nf * var, sill: var * si as f64 / 10.0, range };
                    let sse: f64 = emp.iter().map(|&(h, g, c)| c as f64 * (g - v.gamma(h)).powi(2)).sum();
                    if best.as_ref().is_none_or(|b| sse < b.0) {
                        best = Some((sse, v));
                    }
                }
            }
        }
    }
    Ok(best.unwrap().1)
}

/// Nearest-point lookup for arbitrary query positions (uniform grid, ~2 points per cell).
pub struct Index<'a> {
    pts: &'a [(f64, f64, f64)],
    minx: f64,
    miny: f64,
    cs: f64,
    cells: usize,
    grid: Vec<Vec<u32>>,
}

impl<'a> Index<'a> {
    pub fn new(pts: &'a [(f64, f64, f64)]) -> Index<'a> {
        let (minx, maxx) = pts.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        let (miny, maxy) = pts.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        let side = (maxx - minx).max(maxy - miny).max(1e-9);
        let cells = ((pts.len() as f64 / 2.0).sqrt().ceil() as usize).max(1);
        let cs = side / cells as f64;
        let mut grid = vec![vec![]; cells * cells];
        for (i, p) in pts.iter().enumerate() {
            let cx = (((p.0 - minx) / cs) as usize).min(cells - 1);
            let cy = (((p.1 - miny) / cs) as usize).min(cells - 1);
            grid[cy * cells + cx].push(i as u32);
        }
        Index { pts, minx, miny, cs, cells, grid }
    }

    /// Indices of the `k` points nearest (x, y), nearest first.
    pub fn nearest(&self, x: f64, y: f64, k: usize) -> Vec<usize> {
        let c = self.cells as isize;
        let cx = (((x - self.minx) / self.cs).floor() as isize).clamp(0, c - 1);
        let cy = (((y - self.miny) / self.cs).floor() as isize).clamp(0, c - 1);
        // How far the query lies outside the grid: rings are measured from the clamped cell, so add it to the safe distance test.
        let outside = ((self.minx - x).max(x - (self.minx + self.cs * self.cells as f64)).max(0.0)).hypot((self.miny - y).max(y - (self.miny + self.cs * self.cells as f64)).max(0.0));
        let mut cand: Vec<(f64, usize)> = vec![];
        let mut r = 0isize;
        loop {
            for gy in (cy - r).max(0)..=(cy + r).min(c - 1) {
                for gx in (cx - r).max(0)..=(cx + r).min(c - 1) {
                    if (gy - cy).abs().max((gx - cx).abs()) == r {
                        for &j in &self.grid[(gy * c + gx) as usize] {
                            let p = self.pts[j as usize];
                            cand.push(((p.0 - x).powi(2) + (p.1 - y).powi(2), j as usize));
                        }
                    }
                }
            }
            cand.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            let safe = (r as f64 * self.cs - outside).max(0.0);
            if (cand.len() >= k && cand[k - 1].0.sqrt() <= safe) || r >= c {
                break;
            }
            r += 1;
        }
        cand.into_iter().take(k).map(|c| c.1).collect()
    }
}

/// Solve A x = b in place (Gaussian elimination with partial pivoting); None when the matrix is singular.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f != 0.0 {
                for k in c..n {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    Some(x)
}

/// Ordinary kriging prediction and variance at (x, y) from the points `near` (indices into `pts`).
pub fn predict(pts: &[(f64, f64, f64)], near: &[usize], v: &Variogram, x: f64, y: f64) -> Option<(f64, f64)> {
    let m = near.len();
    let mut a = vec![vec![0.0; m + 1]; m + 1];
    let mut b = vec![0.0; m + 1];
    for i in 0..m {
        for j in 0..m {
            let (p, q) = (pts[near[i]], pts[near[j]]);
            a[i][j] = v.gamma(((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt());
        }
        a[i][m] = 1.0;
        a[m][i] = 1.0;
        let p = pts[near[i]];
        b[i] = v.gamma(((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt());
    }
    b[m] = 1.0;
    let sol = solve(a, b.clone())?;
    let pred: f64 = (0..m).map(|i| sol[i] * pts[near[i]].2).sum();
    let var: f64 = (0..m).map(|i| sol[i] * b[i]).sum::<f64>() + sol[m];
    (pred.is_finite() && var.is_finite()).then_some((pred, var.max(0.0)))
}

/// Krige onto a grid covering the points (snapped to multiples of `cell`): band 1 prediction, band 2 kriging variance.
/// `neighbours` nearest points are used per cell (8–32). `crs` is the layer's CRS (projected).
pub fn krige_raster(pts: &[(f64, f64, f64)], v: &Variogram, cell: f64, neighbours: usize, crs: &str) -> Result<PathBuf> {
    if !(cell.is_finite() && cell > 0.0) {
        bail!("The cell size must be a number greater than 0");
    }
    if !(8..=32).contains(&neighbours) {
        bail!("The number of neighbours must be between 8 and 32 (12 is usual)");
    }
    let (minx, maxx) = pts.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
    let (miny, maxy) = pts.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let (x0, y0) = ((minx / cell).floor() * cell, (miny / cell).floor() * cell);
    let cols = (((maxx - x0) / cell).ceil()).max(1.0);
    let rows = (((maxy - y0) / cell).ceil()).max(1.0);
    check_cells(cols, rows)?;
    let (cols, rows) = (cols as usize, rows as usize);
    let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt().map_err(|e| anyhow!("Could not describe the coordinate system: {e}"))?;
    let g = Grid { cols, rows, gt: [x0, cell, 0.0, y0 + rows as f64 * cell, 0.0, -cell], wkt, bands: 2 };
    let out = crate::raster::new_store_path("kriging", "tif");
    let mut ds = create_like(&out, &g, 2, Some(OUT_NODATA))?;
    // The fitted model travels with the raster: band names plus the parameters (units of the layer's CRS and of the value field squared).
    ds.rasterband(1)?.set_description("kriging prediction")?;
    ds.rasterband(2)?.set_description("kriging variance")?;
    ds.set_metadata_item("VARIOGRAM", &format!("{:?} nugget={} sill={} range={}", v.model, v.nugget, v.sill, v.range), "")?;
    let idx = Index::new(pts);
    let k = neighbours.min(pts.len());
    let run = (|| -> Result<()> {
        for r in 0..rows {
            cancelled()?;
            let y = g.gt[3] - (r as f64 + 0.5) * cell;
            let mut pred = vec![OUT_NODATA as f32; cols];
            let mut var = vec![OUT_NODATA as f32; cols];
            for c in 0..cols {
                let x = x0 + (c as f64 + 0.5) * cell;
                if let Some((p, s)) = predict(pts, &idx.nearest(x, y, k), v, x, y) {
                    pred[c] = p as f32;
                    var[c] = s as f32;
                }
            }
            ds.rasterband(1)?.write((0, r as isize), (cols, 1), &mut Buffer::new((cols, 1), pred))?;
            ds.rasterband(2)?.write((0, r as isize), (cols, 1), &mut Buffer::new((cols, 1), var))?;
        }
        Ok(())
    })();
    drop(ds);
    if let Err(e) = run {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    crate::raster::build_overviews(&out)?;
    Ok(out)
}

/// Fit (or use the given parameters) and krige. Convenience for the node: `fixed` = (nugget, partial sill, range) when the user supplies them.
pub fn run(gpkg_points: &[(f64, f64, f64)], model: Option<Model>, fixed: Option<(f64, f64, f64)>, cell: f64, neighbours: usize, crs: &str) -> Result<(PathBuf, Variogram)> {
    let pts = merge_duplicates(gpkg_points);
    let v = match (fixed, model) {
        (Some((nugget, sill, range)), m) => {
            if !(nugget >= 0.0 && sill > 0.0 && range > 0.0) {
                bail!("Nugget must be 0 or more; sill and range must be greater than 0");
            }
            Variogram { model: m.unwrap_or(Model::Spherical), nugget, sill, range }
        }
        (None, m) => fit(&pts, m)?,
    };
    if pts.len() < 6 {
        bail!("Kriging needs at least 6 points with a number ({} given)", pts.len());
    }
    Ok((krige_raster(&pts, &v, cell, neighbours, crs)?, v))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn pts_from(f: impl Fn(f64, f64) -> f64, n: usize, step: f64) -> Vec<(f64, f64, f64)> {
        let mut v = vec![];
        for i in 0..n {
            for j in 0..n {
                // A small deterministic jitter keeps the lattice from being perfectly regular.
                let (x, y) = (i as f64 * step + ((i * 7 + j * 3) % 5) as f64, j as f64 * step + ((i * 2 + j * 11) % 7) as f64);
                v.push((x, y, f(x, y)));
            }
        }
        v
    }

    #[test]
    fn solve_matches_a_hand_solved_system() {
        // 2x + y = 5, x + 3y = 10 → x = 1, y = 3.
        let x = solve(vec![vec![2.0, 1.0], vec![1.0, 3.0]], vec![5.0, 10.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12 && (x[1] - 3.0).abs() < 1e-12);
        assert!(solve(vec![vec![1.0, 2.0], vec![2.0, 4.0]], vec![1.0, 2.0]).is_none());
    }

    #[test]
    fn model_shapes_hit_their_known_values() {
        let v = Variogram { model: Model::Spherical, nugget: 0.0, sill: 10.0, range: 100.0 };
        assert_eq!(v.gamma(0.0), 0.0);
        assert!((v.gamma(50.0) - 10.0 * (1.5 * 0.5 - 0.5 * 0.125)).abs() < 1e-12); // 6.875
        assert_eq!(v.gamma(100.0), 10.0);
        assert_eq!(v.gamma(500.0), 10.0);
        let e = Variogram { model: Model::Exponential, nugget: 2.0, sill: 8.0, range: 100.0 };
        assert!((e.gamma(100.0) - (2.0 + 8.0 * (1.0 - (-3.0f64).exp()))).abs() < 1e-12);
    }

    #[test]
    fn kriging_is_exact_at_a_data_point_and_has_zero_variance_there() {
        let pts = pts_from(|x, y| 0.01 * x + 0.02 * y, 6, 100.0);
        let v = Variogram { model: Model::Spherical, nugget: 0.0, sill: 5.0, range: 400.0 };
        let idx = Index::new(&pts);
        let p = pts[14];
        let (z, var) = predict(&pts, &idx.nearest(p.0, p.1, 12), &v, p.0, p.1).unwrap();
        assert!((z - p.2).abs() < 1e-6, "{z} vs {}", p.2);
        assert!(var < 1e-6, "{var}");
    }

    #[test]
    fn kriging_a_smooth_surface_recovers_it_between_points_with_small_variance_near_data() {
        let f = |x: f64, y: f64| 10.0 + 0.01 * x + 0.02 * y;
        let pts = pts_from(f, 8, 100.0);
        let v = fit(&pts, None).unwrap();
        let idx = Index::new(&pts);
        let (qx, qy) = (333.0, 421.0);
        let (z, _) = predict(&pts, &idx.nearest(qx, qy, 12), &v, qx, qy).unwrap();
        assert!((z - f(qx, qy)).abs() < 0.25, "{z} vs {} with {v:?}", f(qx, qy));
        // Far from every point the variance is larger than between neighbours.
        let (_, near) = predict(&pts, &idx.nearest(250.0, 250.0, 12), &v, 250.0, 250.0).unwrap();
        let (_, far) = predict(&pts, &idx.nearest(5000.0, 5000.0, 12), &v, 5000.0, 5000.0).unwrap();
        assert!(far > near, "{far} !> {near}");
    }

    #[test]
    fn fit_finds_the_range_of_a_known_spatial_process() {
        // Values from a smooth field with a correlation length of a few hundred metres: the fitted range is of that order.
        let pts = pts_from(|x, y| (x / 150.0).sin() * 3.0 + (y / 200.0).cos() * 2.0, 14, 60.0);
        let v = fit(&pts, Some(Model::Spherical)).unwrap();
        assert!(v.range > 100.0 && v.range < 1500.0 && v.sill > 0.0, "{v:?}");
        assert_eq!(v.model, Model::Spherical);
    }

    #[test]
    fn merge_duplicates_averages_and_refusals_are_readable() {
        let m = merge_duplicates(&[(1.0, 1.0, 2.0), (1.0, 1.0, 4.0), (2.0, 2.0, 7.0)]);
        assert_eq!(m.len(), 2);
        assert!(m.iter().any(|p| p.0 == 1.0 && p.2 == 3.0));
        assert!(fit(&[(0.0, 0.0, 1.0); 3], None).unwrap_err().to_string().contains("at least 6"));
        let flat: Vec<_> = (0..10).map(|i| (i as f64 * 10.0, (i * i) as f64, 5.0)).collect();
        assert!(fit(&flat, None).unwrap_err().to_string().contains("same"));
        assert!(Model::parse("cubic").is_err());
    }

    #[test]
    fn nearest_matches_brute_force_inside_and_outside_the_data() {
        let pts: Vec<(f64, f64, f64)> = (0..200).map(|i| { let t = i as f64; ((t * 5.3).sin() * 300.0 + t, (t * 2.9).cos() * 250.0 - t, 0.0) }).collect();
        let idx = Index::new(&pts);
        for &(qx, qy) in &[(0.0, 0.0), (100.0, -50.0), (2000.0, 2000.0), (-900.0, 300.0)] {
            let mut all: Vec<(f64, usize)> = pts.iter().enumerate().map(|(j, p)| ((p.0 - qx).powi(2) + (p.1 - qy).powi(2), j)).collect();
            all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)));
            assert_eq!(idx.nearest(qx, qy, 9), all.iter().take(9).map(|a| a.1).collect::<Vec<_>>(), "query ({qx},{qy})");
        }
    }
}
