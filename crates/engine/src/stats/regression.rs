//! Linear and logistic regression (package ST5), solved by QR decomposition (nalgebra, Apache-2.0) rather than the normal equations, so
//! ill-conditioned designs keep their accuracy (checked on the NIST Wampler polynomial data). Pure functions: the tool in
//! `nodes/stat_regress.rs` builds the design matrix (numeric and dummy-coded categorical terms) and reports.
//!
//! * OLS: beta = R^-1 Q'y; covariance sigma^2 (X'X)^-1 = sigma^2 R^-1 R^-T; leverage = row sums of squares of Q.
//! * VIF_j = 1 / (1 - R_j^2) from regressing column j on the other columns (Belsley, Kuh and Welsch 1980).
//! * Breusch-Pagan in Koenker's studentised form: n R^2 of the regression of the squared residuals on the design, chi-square with k - 1 df
//!   (Breusch and Pagan 1979; Koenker 1981), the same statistic as R's lmtest::bptest(studentize = TRUE).
//! * Logistic regression: iteratively reweighted least squares (Nelder and Wedderburn 1972; McCullagh and Nelder 1989) with R's start,
//!   convergence rule and step halving.

use super::inference::{chisq_sf, f_sf};
use nalgebra::{DMatrix, DVector};
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

pub type Res<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct Ols {
    pub n: usize,
    /// Number of coefficients including the intercept.
    pub k: usize,
    pub beta: Vec<f64>,
    pub se: Vec<f64>,
    pub t: Vec<f64>,
    pub p: Vec<f64>,
    pub ci: Vec<(f64, f64)>,
    pub rss: f64,
    pub tss: f64,
    pub r2: f64,
    pub adj_r2: f64,
    /// Residual standard error.
    pub sigma: f64,
    pub df_resid: f64,
    pub f: f64,
    pub f_p: f64,
    pub fitted: Vec<f64>,
    pub resid: Vec<f64>,
    pub leverage: Vec<f64>,
    /// (X'X)^-1.
    pub xtx_inv: DMatrix<f64>,
}

fn mat_from_cols(cols: &[Vec<f64>]) -> DMatrix<f64> {
    let n = cols.first().map_or(0, |c| c.len());
    DMatrix::from_fn(n, cols.len(), |i, j| cols[j][i])
}

/// Least squares of y on the columns of `x` (which must contain the intercept column when one is wanted). `names` label the columns in errors.
pub fn lstsq(x: &DMatrix<f64>, y: &DVector<f64>, names: &[String]) -> Res<(DVector<f64>, DMatrix<f64>, DMatrix<f64>)> {
    let (n, k) = (x.nrows(), x.ncols());
    if n <= k {
        return Err(format!("There are {n} rows for {k} coefficients; a regression needs more rows than coefficients."));
    }
    let qr = x.clone().qr();
    let r = qr.r();
    let q = qr.q();
    let maxd = (0..k).map(|i| r[(i, i)].abs()).fold(0.0, f64::max);
    for i in 0..k {
        if r[(i, i)].abs() <= 1e-10 * maxd.max(1e-300) {
            let nm = names.get(i).cloned().unwrap_or_else(|| format!("column {}", i + 1));
            return Err(format!("'{nm}' is a linear combination of the terms before it (perfect collinearity), so its coefficient cannot be estimated. Remove it or one of the terms it duplicates."));
        }
    }
    let qty = q.transpose() * y;
    let beta = r.clone().solve_upper_triangular(&qty).ok_or("The design matrix is singular.")?;
    let rinv = r.solve_upper_triangular(&DMatrix::identity(k, k)).ok_or("The design matrix is singular.")?;
    Ok((beta, &rinv * rinv.transpose(), q))
}

/// Ordinary least squares with an intercept. `cols` are the predictor columns (no intercept), `names` their names.
pub fn ols(cols: &[Vec<f64>], y: &[f64], names: &[String], conf: f64) -> Res<Ols> {
    let n = y.len();
    let mut all = vec![vec![1.0; n]];
    all.extend(cols.iter().cloned());
    let mut nm = vec!["(Intercept)".to_string()];
    nm.extend(names.iter().cloned());
    let x = mat_from_cols(&all);
    let yv = DVector::from_column_slice(y);
    let (beta, xtx_inv, q) = lstsq(&x, &yv, &nm)?;
    let k = x.ncols();
    let fitted_v = &x * &beta;
    let resid_v = &yv - &fitted_v;
    let rss: f64 = resid_v.iter().map(|v| v * v).sum();
    let ybar = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - ybar).powi(2)).sum();
    let df = (n - k) as f64;
    let sigma2 = rss / df;
    let tdist = StudentsT::new(0.0, 1.0, df).unwrap();
    let tc = tdist.inverse_cdf(1.0 - (1.0 - conf) / 2.0);
    let (mut se, mut t, mut p, mut ci) = (vec![], vec![], vec![], vec![]);
    for j in 0..k {
        let s = (sigma2 * xtx_inv[(j, j)]).sqrt();
        let tv = beta[j] / s;
        se.push(s);
        t.push(tv);
        p.push((2.0 * tdist.cdf(-tv.abs())).min(1.0));
        ci.push((beta[j] - tc * s, beta[j] + tc * s));
    }
    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { f64::NAN };
    let nf = n as f64;
    let adj = 1.0 - (1.0 - r2) * (nf - 1.0) / df;
    let (f, f_p) = if k > 1 && tss > 0.0 && rss > 0.0 {
        let f = ((tss - rss) / (k as f64 - 1.0)) / sigma2;
        (f, f_sf(f, k as f64 - 1.0, df))
    } else {
        (f64::NAN, f64::NAN)
    };
    let leverage: Vec<f64> = (0..n).map(|i| (0..k).map(|j| q[(i, j)].powi(2)).sum()).collect();
    Ok(Ols { n, k, beta: beta.iter().copied().collect(), se, t, p, ci, rss, tss, r2, adj_r2: adj, sigma: sigma2.sqrt(), df_resid: df, f, f_p, fitted: fitted_v.iter().copied().collect(), resid: resid_v.iter().copied().collect(), leverage, xtx_inv })
}

/// R-squared of regressing `y` on `cols` (with an intercept).
fn r2_of(cols: &[Vec<f64>], y: &[f64]) -> Option<f64> {
    let n = y.len();
    let mut all = vec![vec![1.0; n]];
    all.extend(cols.iter().cloned());
    let x = mat_from_cols(&all);
    let yv = DVector::from_column_slice(y);
    let (beta, _, _) = lstsq(&x, &yv, &[]).ok()?;
    let res = &yv - &x * &beta;
    let rss: f64 = res.iter().map(|v| v * v).sum();
    let m = y.iter().sum::<f64>() / n as f64;
    let tss: f64 = y.iter().map(|v| (v - m).powi(2)).sum();
    if tss <= 0.0 { None } else { Some(1.0 - rss / tss) }
}

/// Variance inflation factor of each predictor column (None when a column is constant or the rest is collinear with it).
pub fn vif(cols: &[Vec<f64>]) -> Vec<Option<f64>> {
    (0..cols.len())
        .map(|j| {
            if cols.len() < 2 {
                return Some(1.0);
            }
            let others: Vec<Vec<f64>> = cols.iter().enumerate().filter(|(i, _)| *i != j).map(|(_, c)| c.clone()).collect();
            r2_of(&others, &cols[j]).map(|r2| if r2 >= 1.0 - 1e-12 { f64::INFINITY } else { 1.0 / (1.0 - r2) })
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct BreuschPagan {
    pub lm: f64,
    pub df: f64,
    pub p: f64,
}

/// Breusch-Pagan test of constant residual variance, Koenker's studentised form.
pub fn breusch_pagan(cols: &[Vec<f64>], resid: &[f64]) -> Option<BreuschPagan> {
    let e2: Vec<f64> = resid.iter().map(|e| e * e).collect();
    let r2 = r2_of(cols, &e2)?;
    let lm = resid.len() as f64 * r2;
    let df = cols.len() as f64;
    Some(BreuschPagan { lm, df, p: chisq_sf(lm, df) })
}

#[derive(Clone, Debug)]
pub struct Logit {
    pub n: usize,
    pub beta: Vec<f64>,
    pub se: Vec<f64>,
    pub z: Vec<f64>,
    pub p: Vec<f64>,
    pub ci: Vec<(f64, f64)>,
    pub deviance: f64,
    pub null_deviance: f64,
    pub df_resid: f64,
    pub aic: f64,
    pub fitted: Vec<f64>,
    pub iterations: usize,
    pub converged: bool,
    pub lr_chi2: f64,
    pub lr_p: f64,
    pub mcfadden: f64,
    /// True when some fitted probability is within 1e-8 of 0 or 1 (R's "fitted probabilities numerically 0 or 1 occurred").
    pub near_separation: bool,
}

fn deviance(y: &[f64], mu: &[f64]) -> f64 {
    y.iter()
        .zip(mu)
        .map(|(yi, m)| {
            let a = if *yi > 0.0 { yi * (yi / m).ln() } else { 0.0 };
            let b = if *yi < 1.0 { (1.0 - yi) * ((1.0 - yi) / (1.0 - m)).ln() } else { 0.0 };
            2.0 * (a + b)
        })
        .sum()
}

/// Logistic regression of a 0/1 outcome on `cols` (with an intercept), by IRLS.
pub fn logit(cols: &[Vec<f64>], y: &[f64], names: &[String], conf: f64) -> Res<Logit> {
    let n = y.len();
    let s1: f64 = y.iter().sum();
    if s1 == 0.0 || s1 == n as f64 {
        return Err("Every row has the same outcome, so a logistic regression cannot be fitted.".into());
    }
    let mut all = vec![vec![1.0; n]];
    all.extend(cols.iter().cloned());
    let mut nm = vec!["(Intercept)".to_string()];
    nm.extend(names.iter().cloned());
    let x = mat_from_cols(&all);
    let k = x.ncols();
    if n <= k {
        return Err(format!("There are {n} rows for {k} coefficients; a regression needs more rows than coefficients."));
    }
    let mut mu: Vec<f64> = y.iter().map(|v| (v + 0.5) / 2.0).collect();
    let mut eta: Vec<f64> = mu.iter().map(|m| (m / (1.0 - m)).ln()).collect();
    let mut dev = deviance(y, &mu);
    let mut beta = DVector::zeros(k);
    let mut cov = DMatrix::zeros(k, k);
    let mut converged = false;
    let mut it = 0;
    for iter in 1..=50 {
        it = iter;
        let w: Vec<f64> = mu.iter().map(|m| m * (1.0 - m)).collect();
        let z: Vec<f64> = (0..n).map(|i| eta[i] + (y[i] - mu[i]) / w[i]).collect();
        let sw: Vec<f64> = w.iter().map(|v| v.sqrt()).collect();
        let xw = DMatrix::from_fn(n, k, |i, j| x[(i, j)] * sw[i]);
        let zw = DVector::from_fn(n, |i, _| z[i] * sw[i]);
        let (b_new, c, _) = lstsq(&xw, &zw, &nm)?;
        cov = c;
        let mut step_beta = b_new.clone();
        let mut new_mu;
        let mut new_dev;
        let mut halvings = 0;
        loop {
            let e: Vec<f64> = (0..n).map(|i| (0..k).map(|j| x[(i, j)] * step_beta[j]).sum()).collect();
            new_mu = e.iter().map(|v| 1.0 / (1.0 + (-v).exp())).collect::<Vec<f64>>();
            new_mu.iter_mut().for_each(|m| *m = m.clamp(1e-15, 1.0 - 1e-15));
            new_dev = deviance(y, &new_mu);
            if new_dev.is_finite() && (new_dev <= dev + 1e-12 * dev.abs() || iter == 1 || halvings >= 10) {
                eta = e;
                break;
            }
            step_beta = (&step_beta + &beta) / 2.0;
            halvings += 1;
        }
        beta = step_beta;
        mu = new_mu;
        let conv = (new_dev - dev).abs() / (new_dev.abs() + 0.1) < 1e-8;
        dev = new_dev;
        if conv {
            converged = true;
            break;
        }
    }
    // Covariance at the final weights.
    {
        let w: Vec<f64> = mu.iter().map(|m| m * (1.0 - m)).collect();
        let sw: Vec<f64> = w.iter().map(|v| v.sqrt()).collect();
        let xw = DMatrix::from_fn(n, k, |i, j| x[(i, j)] * sw[i]);
        let zw = DVector::zeros(n);
        if let Ok((_, c, _)) = lstsq(&xw, &zw, &nm) {
            cov = c;
        }
    }
    let pbar = s1 / n as f64;
    let null_dev = deviance(y, &vec![pbar; n]);
    let norm = Normal::new(0.0, 1.0).unwrap();
    let zc = norm.inverse_cdf(1.0 - (1.0 - conf) / 2.0);
    let (mut se, mut zs, mut p, mut ci) = (vec![], vec![], vec![], vec![]);
    for j in 0..k {
        let s = cov[(j, j)].sqrt();
        let zv = beta[j] / s;
        se.push(s);
        zs.push(zv);
        p.push((2.0 * norm.cdf(-zv.abs())).min(1.0));
        ci.push((beta[j] - zc * s, beta[j] + zc * s));
    }
    let lr = null_dev - dev;
    let near = mu.iter().any(|m| *m < 1e-8 || *m > 1.0 - 1e-8);
    Ok(Logit {
        n,
        beta: beta.iter().copied().collect(),
        se,
        z: zs,
        p,
        ci,
        deviance: dev,
        null_deviance: null_dev,
        df_resid: (n - k) as f64,
        aic: dev + 2.0 * k as f64,
        fitted: mu,
        iterations: it,
        converged,
        lr_chi2: lr,
        lr_p: chisq_sf(lr, (k - 1) as f64),
        mcfadden: 1.0 - dev / null_dev,
        near_separation: near,
    })
}

/// Area under the ROC curve of fitted probabilities against 0/1 outcomes (Mann-Whitney form, ties count one half).
pub fn auc(y: &[f64], score: &[f64]) -> Option<f64> {
    let (r, _) = super::inference::ranks(score);
    let n1 = y.iter().filter(|v| **v > 0.5).count() as f64;
    let n0 = y.len() as f64 - n1;
    if n1 == 0.0 || n0 == 0.0 {
        return None;
    }
    let sum1: f64 = y.iter().zip(&r).filter(|(v, _)| **v > 0.5).map(|(_, r)| r).sum();
    Some((sum1 - n1 * (n1 + 1.0) / 2.0) / (n1 * n0))
}
