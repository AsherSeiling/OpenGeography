//! Spatial regression (package ST6): spatial lag and spatial error models by maximum likelihood, and geographically weighted regression (GWR).
//!
//! * Spatial lag (SAR), y = rho W y + X beta + e, and spatial error (SEM), y = X beta + u, u = lambda W u + e, are fitted by maximising the
//!   concentrated log-likelihood over rho (lambda) with the log-determinant computed from the eigenvalues of W (Ord 1975; Anselin 1988, ch. 6).
//!   Standard errors come from the asymptotic information matrix (Anselin 1988), which needs the dense inverse of I - rho W, so these models are
//!   limited to `MAX_ML` features.
//! * GWR (Brunsdon, Fotheringham and Charlton 1996; Fotheringham, Brunsdon and Charlton 2002): at every location a weighted least squares fit with
//!   kernel weights from distances; adaptive (number of neighbours) or fixed bandwidth chosen by the corrected AIC (Hurvich, Simonoff and Tsai 1998)
//!   or leave-one-out cross-validation; local standard errors from the hat matrix (sigma^2 = RSS / (n - 2 tr S + tr S'S)).
//! * MGWR (Fotheringham, Yang and Kang 2017): the model is the sum of one smooth term per covariate, each with its own bandwidth, fitted by
//!   backfitting; its bandwidths are chosen term by term on the partial residuals (as in the `mgwr` package), and inference uses the hat
//!   matrices of the final backfit.

use super::regression::lstsq;
use super::weights::{distance, lag};
use nalgebra::{Complex, DMatrix, DVector};

pub type Res<T> = Result<T, String>;

/// Most features the dense maximum likelihood models accept.
pub const MAX_ML: usize = 2500;
/// Most features GWR accepts (the distance matrix is n x n).
pub const MAX_GWR: usize = 4000;
/// Most features MGWR accepts (it also keeps one n x n hat matrix per covariate).
pub const MAX_MGWR: usize = 600;

fn dense(w: &[Vec<(usize, f64)>]) -> DMatrix<f64> {
    let n = w.len();
    let mut m = DMatrix::zeros(n, n);
    for (i, r) in w.iter().enumerate() {
        for (j, v) in r {
            m[(i, *j)] += v;
        }
    }
    m
}

/// Eigenvalues (real, imaginary) of the row-standardised weights W. For symmetric neighbour structures W is similar to the symmetric matrix
/// D^-1/2 A D^-1/2, whose eigenvalues are real and cheaper to find.
pub fn eigenvalues(w: &[Vec<(usize, f64)>], symmetric: bool) -> Vec<(f64, f64)> {
    let n = w.len();
    if symmetric {
        let deg: Vec<f64> = w.iter().map(|r| r.len() as f64).collect();
        let mut s = DMatrix::zeros(n, n);
        for (i, r) in w.iter().enumerate() {
            for (j, _) in r {
                s[(i, *j)] = 1.0 / (deg[i] * deg[*j]).sqrt();
            }
        }
        s.symmetric_eigen().eigenvalues.iter().map(|v| (*v, 0.0)).collect()
    } else {
        dense(w).complex_eigenvalues().iter().map(|c: &Complex<f64>| (c.re, c.im)).collect()
    }
}

/// ln|I - rho W| from the eigenvalues.
pub fn logdet(eigs: &[(f64, f64)], rho: f64) -> f64 {
    eigs.iter().map(|(re, im)| 0.5 * ((1.0 - rho * re).powi(2) + (rho * im).powi(2)).ln()).sum()
}

fn bounds(eigs: &[(f64, f64)]) -> (f64, f64) {
    let min_re = eigs.iter().map(|e| e.0).fold(f64::INFINITY, f64::min);
    let max_re = eigs.iter().map(|e| e.0).fold(f64::NEG_INFINITY, f64::max);
    let lo = if min_re < -1e-9 { 1.0 / min_re } else { -1.0 };
    let hi = if max_re > 1e-9 { 1.0 / max_re } else { 1.0 };
    (lo, hi)
}

/// Maximise a unimodal function on [lo, hi] by golden-section search.
pub fn golden_max(f: impl Fn(f64) -> f64, lo: f64, hi: f64, tol: f64) -> f64 {
    let phi = (5.0f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (lo, hi);
    let (mut c, mut d) = (b - phi * (b - a), a + phi * (b - a));
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..200 {
        if (b - a).abs() < tol {
            break;
        }
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - phi * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + phi * (b - a);
            fd = f(d);
        }
    }
    (a + b) / 2.0
}

fn design(cols: &[Vec<f64>], n: usize) -> DMatrix<f64> {
    DMatrix::from_fn(n, cols.len() + 1, |i, j| if j == 0 { 1.0 } else { cols[j - 1][i] })
}

fn full_names(names: &[String]) -> Vec<String> {
    let mut v = vec!["(Intercept)".to_string()];
    v.extend(names.iter().cloned());
    v
}

fn pvals_z(est: &[f64], se: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let z: Vec<f64> = est.iter().zip(se).map(|(e, s)| e / s).collect();
    let p = z.iter().map(|z| (2.0 * super::inference::pnorm(-z.abs())).min(1.0)).collect();
    (z, p)
}

#[derive(Clone, Debug)]
pub struct SpatialMl {
    pub n: usize,
    /// "lag" or "error".
    pub kind: &'static str,
    /// rho (lag) or lambda (error).
    pub rho: f64,
    pub rho_se: f64,
    pub rho_z: f64,
    pub rho_p: f64,
    pub beta: Vec<f64>,
    pub se: Vec<f64>,
    pub z: Vec<f64>,
    pub p: Vec<f64>,
    pub sigma2: f64,
    pub loglik: f64,
    pub aic: f64,
    pub loglik_ols: f64,
    pub lr: f64,
    pub lr_p: f64,
    /// Fitted values: rho W y + X beta (lag) or X beta (error). Residuals: y - fitted (lag: the model error; error: y - X beta, the raw residual).
    pub fitted: Vec<f64>,
    pub resid: Vec<f64>,
    /// Innovations e of the model (for the error model: (I - lambda W)(y - X beta)); what the likelihood treats as independent.
    pub innovations: Vec<f64>,
    pub bounds: (f64, f64),
}

fn ols_ll(n: usize, rss: f64) -> f64 {
    let nf = n as f64;
    -nf / 2.0 * ((2.0 * std::f64::consts::PI).ln() + 1.0 + (rss / nf).ln())
}

fn check_ml(n: usize, w: &[Vec<(usize, f64)>]) -> Res<()> {
    if n > MAX_ML {
        return Err(format!("Maximum likelihood spatial regression is limited to {MAX_ML} features ({n} given): it works with n x n matrices. Use a subset, or Geographically Weighted Regression."));
    }
    if w.iter().any(|r| r.is_empty()) {
        return Err("Some features have no neighbours (islands), which these models cannot handle. Use a distance band large enough to reach every feature, k nearest neighbours, or remove the islands.".into());
    }
    Ok(())
}

/// Spatial lag model by maximum likelihood. `w` must be row-standardised.
pub fn sar(cols: &[Vec<f64>], y: &[f64], names: &[String], w: &[Vec<(usize, f64)>], eigs: &[(f64, f64)]) -> Res<SpatialMl> {
    let n = y.len();
    check_ml(n, w)?;
    let x = design(cols, n);
    let k = x.ncols();
    let nm = full_names(names);
    let yv = DVector::from_column_slice(y);
    let wy = DVector::from_vec(lag(w, y));
    let (b0, xtx_inv, _) = lstsq(&x, &yv, &nm)?;
    let (bl, _, _) = lstsq(&x, &wy, &nm)?;
    let e0 = &yv - &x * &b0;
    let el = &wy - &x * &bl;
    let rss0 = e0.dot(&e0);
    let nf = n as f64;
    let ll = |rho: f64| -> f64 {
        let e = &e0 - &el * rho;
        let s2 = e.dot(&e) / nf;
        -nf / 2.0 * (2.0 * std::f64::consts::PI).ln() - nf / 2.0 * s2.ln() + logdet(eigs, rho) - nf / 2.0
    };
    let (lo, hi) = bounds(eigs);
    let rho = golden_max(&ll, lo + 1e-6 * (hi - lo), hi - 1e-6 * (hi - lo), 1e-10);
    let beta = &b0 - &bl * rho;
    let innov = &e0 - &el * rho;
    let sigma2 = innov.dot(&innov) / nf;
    let loglik = ll(rho);
    // Asymptotic covariance (Anselin 1988): information matrix for (beta, rho, sigma^2).
    let a = DMatrix::identity(n, n) - dense(w) * rho;
    let ainv = a.try_inverse().ok_or("I - rho W is singular at the estimate.")?;
    let g = dense(w) * &ainv;
    let xb = &x * &beta;
    let gxb = &g * &xb;
    let tr_g: f64 = (0..n).map(|i| g[(i, i)]).sum();
    let tr_gg: f64 = g.iter().map(|v| v * v).sum::<f64>(); // tr(G'G)
    let tr_g2: f64 = (0..n).map(|i| (0..n).map(|j| g[(i, j)] * g[(j, i)]).sum::<f64>()).sum(); // tr(G G)
    let d = k + 2;
    let mut info = DMatrix::zeros(d, d);
    let xtx = x.transpose() * &x;
    for i in 0..k {
        for j in 0..k {
            info[(i, j)] = xtx[(i, j)] / sigma2;
        }
    }
    let xtg = x.transpose() * &gxb;
    for i in 0..k {
        info[(i, k)] = xtg[i] / sigma2;
        info[(k, i)] = xtg[i] / sigma2;
    }
    info[(k, k)] = tr_gg + tr_g2 + gxb.dot(&gxb) / sigma2;
    info[(k, k + 1)] = tr_g / sigma2;
    info[(k + 1, k)] = tr_g / sigma2;
    info[(k + 1, k + 1)] = nf / (2.0 * sigma2 * sigma2);
    let cov = info.try_inverse().ok_or("The information matrix is singular.")?;
    let se: Vec<f64> = (0..k).map(|i| cov[(i, i)].max(0.0).sqrt()).collect();
    let rho_se = cov[(k, k)].max(0.0).sqrt();
    let bvec: Vec<f64> = beta.iter().copied().collect();
    let (z, p) = pvals_z(&bvec, &se);
    let lo_ll = ols_ll(n, rss0);
    let lr = (2.0 * (loglik - lo_ll)).max(0.0);
    let fitted: Vec<f64> = (&wy * rho + &xb).iter().copied().collect();
    let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
    let _ = xtx_inv;
    Ok(SpatialMl {
        n,
        kind: "lag",
        rho,
        rho_se,
        rho_z: rho / rho_se,
        rho_p: (2.0 * super::inference::pnorm(-(rho / rho_se).abs())).min(1.0),
        beta: bvec,
        se,
        z,
        p,
        sigma2,
        loglik,
        aic: -2.0 * loglik + 2.0 * (k as f64 + 2.0),
        loglik_ols: lo_ll,
        lr,
        lr_p: super::inference::chisq_sf(lr, 1.0),
        fitted,
        resid,
        innovations: innov.iter().copied().collect(),
        bounds: (lo, hi),
    })
}

/// Spatial error model by maximum likelihood. `w` must be row-standardised.
pub fn sem(cols: &[Vec<f64>], y: &[f64], names: &[String], w: &[Vec<(usize, f64)>], eigs: &[(f64, f64)]) -> Res<SpatialMl> {
    let n = y.len();
    check_ml(n, w)?;
    let x = design(cols, n);
    let k = x.ncols();
    let nm = full_names(names);
    let yv = DVector::from_column_slice(y);
    let wm = dense(w);
    let wy = &wm * &yv;
    let wx = &wm * &x;
    let nf = n as f64;
    let (b_ols, _, _) = lstsq(&x, &yv, &nm)?;
    let rss0 = (&yv - &x * &b_ols).norm_squared();
    let fit = |lam: f64| -> Res<(DVector<f64>, f64)> {
        let ys = &yv - &wy * lam;
        let xs = &x - &wx * lam;
        let (b, _, _) = lstsq(&xs, &ys, &nm)?;
        let e = &ys - &xs * &b;
        Ok((b, e.norm_squared() / nf))
    };
    let ll = |lam: f64| -> f64 {
        match fit(lam) {
            Ok((_, s2)) => -nf / 2.0 * (2.0 * std::f64::consts::PI).ln() - nf / 2.0 * s2.ln() + logdet(eigs, lam) - nf / 2.0,
            Err(_) => f64::NEG_INFINITY,
        }
    };
    let (lo, hi) = bounds(eigs);
    let lam = golden_max(&ll, lo + 1e-6 * (hi - lo), hi - 1e-6 * (hi - lo), 1e-10);
    let (beta, sigma2) = fit(lam)?;
    let loglik = ll(lam);
    let a = DMatrix::identity(n, n) - &wm * lam;
    let ainv = a.clone().try_inverse().ok_or("I - lambda W is singular at the estimate.")?;
    let wt = &wm * &ainv;
    let tr_w: f64 = (0..n).map(|i| wt[(i, i)]).sum();
    let tr_ww: f64 = wt.iter().map(|v| v * v).sum::<f64>();
    let tr_w2: f64 = (0..n).map(|i| (0..n).map(|j| wt[(i, j)] * wt[(j, i)]).sum::<f64>()).sum();
    // beta block: sigma^2 (X' A'A X)^-1; (lambda, sigma^2) block from the information matrix.
    let xs = &x - &wx * lam;
    let xtx = xs.transpose() * &xs;
    let xtx_inv = xtx.try_inverse().ok_or("The transformed design is singular.")?;
    let se: Vec<f64> = (0..k).map(|i| (sigma2 * xtx_inv[(i, i)]).max(0.0).sqrt()).collect();
    let mut info = DMatrix::zeros(2, 2);
    info[(0, 0)] = tr_ww + tr_w2;
    info[(0, 1)] = tr_w / sigma2;
    info[(1, 0)] = tr_w / sigma2;
    info[(1, 1)] = nf / (2.0 * sigma2 * sigma2);
    let cov = info.try_inverse().ok_or("The information matrix is singular.")?;
    let lam_se = cov[(0, 0)].max(0.0).sqrt();
    let bvec: Vec<f64> = beta.iter().copied().collect();
    let (z, p) = pvals_z(&bvec, &se);
    let lo_ll = ols_ll(n, rss0);
    let lr = (2.0 * (loglik - lo_ll)).max(0.0);
    let fitted: Vec<f64> = (&x * &beta).iter().copied().collect();
    let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
    let u = DVector::from_column_slice(&resid);
    let innov = &a * u;
    Ok(SpatialMl {
        n,
        kind: "error",
        rho: lam,
        rho_se: lam_se,
        rho_z: lam / lam_se,
        rho_p: (2.0 * super::inference::pnorm(-(lam / lam_se).abs())).min(1.0),
        beta: bvec,
        se,
        z,
        p,
        sigma2,
        loglik,
        aic: -2.0 * loglik + 2.0 * (k as f64 + 2.0),
        loglik_ols: lo_ll,
        lr,
        lr_p: super::inference::chisq_sf(lr, 1.0),
        fitted,
        resid,
        innovations: innov.iter().copied().collect(),
        bounds: (lo, hi),
    })
}

// ---------------------------------------------------------------------- GWR

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    Gaussian,
    Bisquare,
    Exponential,
}

impl Kernel {
    pub fn parse(s: Option<&str>) -> Kernel {
        match s {
            Some("gaussian") => Kernel::Gaussian,
            Some("exponential") => Kernel::Exponential,
            _ => Kernel::Bisquare,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Kernel::Gaussian => "Gaussian",
            Kernel::Bisquare => "bisquare",
            Kernel::Exponential => "exponential",
        }
    }
    fn weight(self, d: f64, h: f64) -> f64 {
        match self {
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
}

/// Dense matrix of distances between all pairs of locations (geodesic metres for degrees, map units otherwise).
pub struct Dist {
    pub n: usize,
    d: Vec<f64>,
    sorted_cache: Vec<Vec<f64>>,
}

impl Dist {
    pub fn new(xy: &[(f64, f64)], geographic: bool, max_n: usize, cancel: &dyn Fn() -> bool) -> Res<Dist> {
        let n = xy.len();
        if n > max_n {
            return Err(format!("This model works with an n x n distance matrix and is limited to {max_n} features ({n} given). Use a subset."));
        }
        let mut d = vec![0.0; n * n];
        for i in 0..n {
            if i % 64 == 0 && cancel() {
                return Err(crate::progress::CANCELLED.to_string());
            }
            for j in i + 1..n {
                let v = distance(xy[i], xy[j], geographic);
                d[i * n + j] = v;
                d[j * n + i] = v;
            }
        }
        let sorted_cache = (0..n)
            .map(|i| {
                let mut r = d[i * n..(i + 1) * n].to_vec();
                r.sort_by(|a, b| a.partial_cmp(b).unwrap());
                r
            })
            .collect();
        Ok(Dist { n, d, sorted_cache })
    }
    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.d[i * self.n + j]
    }
    /// Distance to the m-th nearest location counting the location itself as the first.
    pub fn nth(&self, i: usize, m: usize) -> f64 {
        self.sorted_cache[i][m.clamp(1, self.n) - 1]
    }
    pub fn max(&self) -> f64 {
        self.sorted_cache.iter().map(|r| *r.last().unwrap()).fold(0.0, f64::max)
    }
    pub fn min_positive(&self) -> f64 {
        self.sorted_cache.iter().filter_map(|r| r.iter().find(|v| **v > 0.0).copied()).fold(f64::INFINITY, f64::min)
    }
}

#[derive(Clone, Debug)]
pub struct Gwr {
    pub n: usize,
    pub k: usize,
    pub bandwidth: f64,
    pub adaptive: bool,
    pub beta: Vec<Vec<f64>>,
    pub se: Vec<Vec<f64>>,
    pub fitted: Vec<f64>,
    pub resid: Vec<f64>,
    pub local_r2: Vec<f64>,
    pub rss: f64,
    pub tr_s: f64,
    pub tr_sts: f64,
    /// Residual degrees of freedom n - 2 tr S + tr S'S.
    pub delta1: f64,
    pub sigma2: f64,
    pub aicc: f64,
    /// Effective number of parameters, tr S.
    pub enp: f64,
    pub cv: f64,
}

fn bw_h(dist: &Dist, i: usize, bw: f64, adaptive: bool) -> f64 {
    if adaptive {
        dist.nth(i, bw.round() as usize)
    } else {
        bw
    }
}

/// One GWR fit at a given bandwidth. `se` = also compute local standard errors and local R^2 (a little slower).
pub fn gwr_fit(x: &DMatrix<f64>, y: &[f64], dist: &Dist, bw: f64, adaptive: bool, kernel: Kernel, full: bool) -> Res<Gwr> {
    let (n, k) = (x.nrows(), x.ncols());
    let mut beta = vec![vec![0.0; k]; n];
    let mut se = vec![vec![f64::NAN; k]; n];
    let mut fitted = vec![0.0; n];
    let mut local_r2 = vec![f64::NAN; n];
    let (mut tr_s, mut tr_sts, mut rss, mut cv) = (0.0, 0.0, 0.0, 0.0);
    let mut minv_all: Vec<DMatrix<f64>> = Vec::with_capacity(if full { n } else { 0 });
    let mut wcache: Vec<Vec<f64>> = Vec::with_capacity(if full { n } else { 0 });
    for i in 0..n {
        let h = bw_h(dist, i, bw, adaptive);
        if !(h > 0.0) {
            return Err("The bandwidth is zero: several locations share the same position, so the neighbour distance is 0. Use a larger number of neighbours or a fixed distance.".into());
        }
        let w: Vec<f64> = (0..n).map(|j| kernel.weight(dist.at(i, j), h)).collect();
        let mut m = DMatrix::zeros(k, k);
        let mut xty = DVector::zeros(k);
        for j in 0..n {
            if w[j] > 0.0 {
                for a in 0..k {
                    xty[a] += w[j] * x[(j, a)] * y[j];
                    for b in a..k {
                        m[(a, b)] += w[j] * x[(j, a)] * x[(j, b)];
                    }
                }
            }
        }
        for a in 0..k {
            for b in 0..a {
                m[(a, b)] = m[(b, a)];
            }
        }
        let minv = m.clone().try_inverse().ok_or_else(|| "At some location the local design is singular: too few neighbours get weight for the number of predictors (or a predictor does not vary locally). Use a larger bandwidth.".to_string())?;
        let b = &minv * &xty;
        let xi = DVector::from_fn(k, |a, _| x[(i, a)]);
        let yhat = xi.dot(&b);
        for a in 0..k {
            beta[i][a] = b[a];
        }
        fitted[i] = yhat;
        rss += (y[i] - yhat).powi(2);
        let z = &minv * &xi;
        // S_ij = (z . x_j) w_j. Leverage S_ii and the sum of squares of the row for tr(S'S).
        let mut s_ii = 0.0;
        for j in 0..n {
            if w[j] > 0.0 {
                let sij = (0..k).map(|a| z[a] * x[(j, a)]).sum::<f64>() * w[j];
                tr_sts += sij * sij;
                if j == i {
                    s_ii = sij;
                }
            }
        }
        tr_s += s_ii;
        // Cross-validation score (Cleveland 1979): predict y_i from a calibration that gives location i itself no weight.
        {
            let mut m2 = m.clone();
            let mut r2 = xty.clone();
            for a in 0..k {
                r2[a] -= w[i] * x[(i, a)] * y[i];
                for b in 0..k {
                    m2[(a, b)] -= w[i] * x[(i, a)] * x[(i, b)];
                }
            }
            match m2.try_inverse() {
                Some(inv2) => cv += (y[i] - xi.dot(&(inv2 * r2))).powi(2),
                None => cv += 1e300 / n as f64,
            }
        }
        if full {
            // Weighted local R^2.
            let sw: f64 = w.iter().sum();
            let ybar: f64 = (0..n).map(|j| w[j] * y[j]).sum::<f64>() / sw;
            let (mut ssr, mut sst) = (0.0, 0.0);
            for j in 0..n {
                if w[j] > 0.0 {
                    let yh: f64 = (0..k).map(|a| x[(j, a)] * b[a]).sum();
                    ssr += w[j] * (y[j] - yh).powi(2);
                    sst += w[j] * (y[j] - ybar).powi(2);
                }
            }
            local_r2[i] = if sst > 0.0 { 1.0 - ssr / sst } else { f64::NAN };
            minv_all.push(minv);
            wcache.push(w);
        }
    }
    let nf = n as f64;
    let delta1 = nf - 2.0 * tr_s + tr_sts;
    let sigma2 = rss / delta1;
    if full {
        for i in 0..n {
            let (minv, w) = (&minv_all[i], &wcache[i]);
            let mut var = vec![0.0; k];
            for j in 0..n {
                if w[j] > 0.0 {
                    let xj = DVector::from_fn(k, |a, _| x[(j, a)]);
                    let c = minv * xj;
                    for a in 0..k {
                        var[a] += (c[a] * w[j]).powi(2);
                    }
                }
            }
            for a in 0..k {
                se[i][a] = (sigma2 * var[a]).sqrt();
            }
        }
    }
    let aicc = nf * (rss / nf).ln() + nf * (2.0 * std::f64::consts::PI).ln() + nf * (nf + tr_s) / (nf - 2.0 - tr_s);
    let resid: Vec<f64> = y.iter().zip(&fitted).map(|(a, b)| a - b).collect();
    Ok(Gwr { n, k, bandwidth: bw, adaptive, beta, se, fitted, resid, local_r2, rss, tr_s, tr_sts, delta1, sigma2, aicc, enp: tr_s, cv })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Criterion {
    Aicc,
    Cv,
}

/// Bandwidth that minimises the criterion by golden-section search (adaptive: the number of neighbours, searched over whole numbers).
pub fn gwr_select(x: &DMatrix<f64>, y: &[f64], dist: &Dist, adaptive: bool, kernel: Kernel, crit: Criterion) -> Res<f64> {
    let n = x.nrows();
    let score = |bw: f64| -> f64 {
        match gwr_fit(x, y, dist, bw, adaptive, kernel, false) {
            Ok(f) => {
                if crit == Criterion::Aicc {
                    if f.aicc.is_finite() { f.aicc } else { f64::INFINITY }
                } else {
                    f.cv
                }
            }
            Err(_) => f64::INFINITY,
        }
    };
    let (lo, hi) = if adaptive { ((x.ncols() + 2).max(5) as f64, n as f64) } else { (dist.min_positive(), dist.max()) };
    if !(hi > lo) {
        return Err("There are too few locations to select a bandwidth.".into());
    }
    let sc = |b: f64| if adaptive { score(b.round()) } else { score(b) };
    let phi = (5.0f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (lo, hi);
    let (mut c, mut d) = (b - phi * (b - a), a + phi * (b - a));
    let (mut fc, mut fd) = (sc(c), sc(d));
    for _ in 0..60 {
        if (b - a).abs() < if adaptive { 1.5 } else { (hi - lo) * 1e-4 } {
            break;
        }
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - phi * (b - a);
            fc = sc(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + phi * (b - a);
            fd = sc(d);
        }
    }
    let best = (a + b) / 2.0;
    let best = if adaptive { best.round() } else { best };
    // Never worse than the widest bandwidth (the global model).
    let (sb, sh) = (sc(best), sc(hi));
    if !sb.is_finite() && !sh.is_finite() {
        return Err("No bandwidth gave a usable fit: the local designs are singular everywhere. Check for predictors that are constant or collinear.".into());
    }
    Ok(if sb <= sh { best } else { hi })
}
