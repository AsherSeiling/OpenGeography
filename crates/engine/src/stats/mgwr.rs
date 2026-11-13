//! Multiscale GWR (Fotheringham, Yang and Kang 2017, Annals of the American Association of Geographers 107(6), 1247-1265; Yang 2014 for the
//! backfitting form). Each covariate j has its own bandwidth b_j: y = sum_j diag(x_j) C_j y with the smoothers C_j found by backfitting.
//! Bandwidths are chosen covariate by covariate on the partial residuals with the corrected AIC of the one-covariate GWR (the way the `mgwr`
//! package does), repeated until they stop changing; inference uses the hat matrices of the converged backfit.

use super::spatial_reg::*;
use nalgebra::{DMatrix, DVector};

#[derive(Clone, Debug)]
pub struct Mgwr {
    pub n: usize,
    pub k: usize,
    pub bandwidths: Vec<f64>,
    pub adaptive: bool,
    pub beta: Vec<Vec<f64>>,
    pub se: Vec<Vec<f64>>,
    pub fitted: Vec<f64>,
    pub resid: Vec<f64>,
    pub rss: f64,
    pub tr_s: f64,
    pub enp: Vec<f64>,
    pub sigma2: f64,
    pub aicc: f64,
    pub rounds: usize,
}

/// Coefficient smoother of the one-covariate GWR: row i holds c_il = w_il x_lj / sum_m w_im x_mj^2, so that beta_ij = sum_l c_il eps_l.
fn smoother(xj: &[f64], dist: &Dist, bw: f64, adaptive: bool, kernel: Kernel) -> Result<DMatrix<f64>, String> {
    let n = xj.len();
    let mut c = DMatrix::zeros(n, n);
    for i in 0..n {
        let h = if adaptive { dist.nth(i, bw.round() as usize) } else { bw };
        if !(h > 0.0) {
            return Err("A bandwidth is zero because several locations share a position.".into());
        }
        let w: Vec<f64> = (0..n).map(|l| kernel_weight(kernel, dist.at(i, l), h)).collect();
        let denom: f64 = (0..n).map(|m| w[m] * xj[m] * xj[m]).sum();
        if !(denom > 0.0) {
            return Err("At some location the weighted sum of squares of a covariate is 0: the bandwidth is too small for it.".into());
        }
        for l in 0..n {
            c[(i, l)] = w[l] * xj[l] / denom;
        }
    }
    Ok(c)
}

fn kernel_weight(k: Kernel, d: f64, h: f64) -> f64 {
    match k {
        Kernel::Gaussian => (-0.5 * (d / h).powi(2)).exp(),
        Kernel::Exponential => (-d / h).exp(),
        Kernel::Bisquare => {
            if d < h {
                (1.0 - (d / h).powi(2)).powi(2)
            } else {
                0.0
            }
        }
    }
}

pub fn mgwr(x: &DMatrix<f64>, y: &[f64], dist: &Dist, adaptive: bool, kernel: Kernel, crit: Criterion, cancel: &dyn Fn() -> bool) -> Result<Mgwr, String> {
    let (n, k) = (x.nrows(), x.ncols());
    if n > MAX_MGWR {
        return Err(format!("MGWR is limited to {MAX_MGWR} features ({n} given) because it keeps an n x n matrix for every covariate. Use a subset, or GWR."));
    }
    let col = |j: usize| -> Vec<f64> { (0..n).map(|i| x[(i, j)]).collect() };
    let col_mat = |j: usize| DMatrix::from_fn(n, 1, |i, _| x[(i, j)]);
    // Start from an ordinary GWR with one shared bandwidth.
    let bw0 = gwr_select(x, y, dist, adaptive, kernel, crit)?;
    let init = gwr_fit(x, y, dist, bw0, adaptive, kernel, false)?;
    let mut f: Vec<Vec<f64>> = (0..k).map(|j| (0..n).map(|i| init.beta[i][j] * x[(i, j)]).collect()).collect();
    let mut bws = vec![bw0; k];
    let mut rounds = 0;
    let mut betas: Vec<Vec<f64>> = (0..k).map(|j| (0..n).map(|i| init.beta[i][j]).collect()).collect();
    // Phase A: re-select every covariate's bandwidth on its partial residuals until none changes.
    for round in 0..10 {
        rounds = round + 1;
        if cancel() {
            return Err(crate::progress::CANCELLED.to_string());
        }
        let mut changed = false;
        for j in 0..k {
            let eps: Vec<f64> = (0..n).map(|i| y[i] - (0..k).filter(|m| *m != j).map(|m| f[m][i]).sum::<f64>()).collect();
            let xm = col_mat(j);
            let b = gwr_select(&xm, &eps, dist, adaptive, kernel, crit)?;
            if (b - bws[j]).abs() > if adaptive { 0.5 } else { 1e-6 * dist.max() } {
                changed = true;
            }
            bws[j] = b;
            let g = gwr_fit(&xm, &eps, dist, b, adaptive, kernel, false)?;
            for i in 0..n {
                betas[j][i] = g.beta[i][0];
                f[j][i] = g.beta[i][0] * x[(i, j)];
            }
        }
        if !changed && round > 0 {
            break;
        }
    }
    // Phase B: backfit with the bandwidths fixed until the fitted terms stop moving.
    for _ in 0..200 {
        if cancel() {
            return Err(crate::progress::CANCELLED.to_string());
        }
        let mut change = 0.0;
        let mut scale = 0.0;
        for j in 0..k {
            let eps: Vec<f64> = (0..n).map(|i| y[i] - (0..k).filter(|m| *m != j).map(|m| f[m][i]).sum::<f64>()).collect();
            let xm = col_mat(j);
            let g = gwr_fit(&xm, &eps, dist, bws[j], adaptive, kernel, false)?;
            for i in 0..n {
                let nf = g.beta[i][0] * x[(i, j)];
                change += (nf - f[j][i]).powi(2);
                scale += nf * nf;
                betas[j][i] = g.beta[i][0];
                f[j][i] = nf;
            }
        }
        if change <= 1e-10 * scale.max(1e-300) {
            break;
        }
    }
    // Hat matrices: C_j <- Cu_j (I - sum_{m != j} R_m), R_j = diag(x_j) C_j, swept to convergence.
    let cu: Vec<DMatrix<f64>> = (0..k).map(|j| smoother(&col(j), dist, bws[j], adaptive, kernel)).collect::<Result<_, _>>()?;
    let mut cj: Vec<DMatrix<f64>> = cu.clone();
    let mut rj: Vec<DMatrix<f64>> = (0..k).map(|j| DMatrix::from_fn(n, n, |i, l| x[(i, j)] * cj[j][(i, l)])).collect();
    for _ in 0..60 {
        if cancel() {
            return Err(crate::progress::CANCELLED.to_string());
        }
        let mut change = 0.0f64;
        for j in 0..k {
            let mut others = DMatrix::zeros(n, n);
            for m in 0..k {
                if m != j {
                    others += &rj[m];
                }
            }
            let a = DMatrix::identity(n, n) - others;
            let new_c = &cu[j] * a;
            let new_r = DMatrix::from_fn(n, n, |i, l| x[(i, j)] * new_c[(i, l)]);
            change = change.max((&new_r - &rj[j]).amax());
            cj[j] = new_c;
            rj[j] = new_r;
        }
        if change < 1e-8 {
            break;
        }
    }
    let mut s = DMatrix::zeros(n, n);
    for r in &rj {
        s += r;
    }
    let yv = DVector::from_column_slice(y);
    let fitted_v = &s * &yv;
    let fitted: Vec<f64> = fitted_v.iter().copied().collect();
    let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
    let rss: f64 = resid.iter().map(|r| r * r).sum();
    let enp: Vec<f64> = rj.iter().map(|r| (0..n).map(|i| r[(i, i)]).sum()).collect();
    let tr_s: f64 = enp.iter().sum();
    let tr_sts: f64 = s.iter().map(|v| v * v).sum();
    let nf = n as f64;
    let delta1 = nf - 2.0 * tr_s + tr_sts;
    let sigma2 = rss / delta1;
    let beta: Vec<Vec<f64>> = (0..n).map(|i| (0..k).map(|j| (0..n).map(|l| cj[j][(i, l)] * y[l]).sum()).collect()).collect();
    let se: Vec<Vec<f64>> = (0..n).map(|i| (0..k).map(|j| (sigma2 * (0..n).map(|l| cj[j][(i, l)].powi(2)).sum::<f64>()).sqrt()).collect()).collect();
    let aicc = nf * (rss / nf).ln() + nf * (2.0 * std::f64::consts::PI).ln() + nf * (nf + tr_s) / (nf - 2.0 - tr_s);
    Ok(Mgwr { n, k, bandwidths: bws, adaptive, beta, se, fitted, resid, rss, tr_s, enp, sigma2, aicc, rounds })
}
