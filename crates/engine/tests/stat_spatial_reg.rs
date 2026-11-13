//! ST6 part 2: spatial lag and error models by maximum likelihood, and GWR. No R spatialreg/GWmodel output was available offline, so each
//! check is against an independent computation of the same published definition inside the test: the full log-likelihood with a dense
//! determinant (Ord 1975), a numerical second derivative for the standard error, GWR with a very wide kernel reproducing OLS exactly, and
//! simulated data with known parameters.

use engine::stats::regression::ols;
use engine::stats::spatial_reg::*;
use engine::stats::weights::*;
use nalgebra::{DMatrix, DVector};

const SIDE: usize = 15;

fn grid() -> (Vec<(f64, f64)>, Weights) {
    let xy: Vec<(f64, f64)> = (0..SIDE * SIDE).map(|i| ((i % SIDE) as f64, (i / SIDE) as f64)).collect();
    let w = distance_band(&xy, 1.01, false, &|| false).unwrap();
    (xy, w)
}

fn normal(rng: &mut Rng) -> f64 {
    let (u, v) = (rng.f64().max(1e-12), rng.f64());
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

fn dense(w: &[Vec<(usize, f64)>]) -> DMatrix<f64> {
    let n = w.len();
    let mut m = DMatrix::zeros(n, n);
    for (i, r) in w.iter().enumerate() {
        for (j, v) in r {
            m[(i, *j)] = *v;
        }
    }
    m
}

/// y = (I - rho W)^-1 (X beta + e)  (lag)  or  y = X beta + (I - lam W)^-1 e  (error).
fn simulate(kind: &str, par: f64, seed: u64) -> (Vec<Vec<f64>>, Vec<f64>, Weights) {
    let (_, w) = grid();
    let ws = w.row_standardised();
    let n = SIDE * SIDE;
    let mut rng = Rng::new(seed);
    let x1: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    let x2: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    let e = DVector::from_fn(n, |_, _| normal(&mut rng) * 0.5);
    let xb = DVector::from_fn(n, |i, _| 1.0 + 2.0 * x1[i] - 1.0 * x2[i]);
    let a = DMatrix::identity(n, n) - dense(&ws) * par;
    let ainv = a.try_inverse().unwrap();
    let y = if kind == "lag" { &ainv * (&xb + &e) } else { &xb + &ainv * &e };
    (vec![x1, x2], y.iter().copied().collect(), w)
}

fn names() -> Vec<String> {
    vec!["x1".into(), "x2".into()]
}

#[test]
fn spatial_lag_matches_a_brute_force_likelihood_and_recovers_the_parameters() {
    let (cols, y, w) = simulate("lag", 0.5, 11);
    let ws = w.row_standardised();
    let eigs = eigenvalues(&ws, w.is_symmetric());
    let m = sar(&cols, &y, &names(), &ws, &eigs).unwrap();
    assert!((m.rho - 0.5).abs() < 0.12, "rho {}", m.rho);
    assert!((m.beta[1] - 2.0).abs() < 0.2 && (m.beta[2] + 1.0).abs() < 0.2, "{:?}", m.beta);
    // The full log-likelihood with the determinant taken from a dense LU, maximised on a fine grid, agrees with the eigenvalue version.
    let n = y.len();
    let nf = n as f64;
    let wd = dense(&ws);
    let yv = DVector::from_column_slice(&y);
    let x = DMatrix::from_fn(n, 3, |i, j| if j == 0 { 1.0 } else { cols[j - 1][i] });
    let full_ll = |rho: f64| -> f64 {
        let a = DMatrix::identity(n, n) - &wd * rho;
        let ay = &a * &yv;
        let b = (x.transpose() * &x).try_inverse().unwrap() * x.transpose() * &ay;
        let e = &ay - &x * b;
        let s2 = e.dot(&e) / nf;
        let logdet = a.lu().determinant().abs().ln();
        -nf / 2.0 * (2.0 * std::f64::consts::PI * s2).ln() + logdet - e.dot(&e) / (2.0 * s2)
    };
    assert!((full_ll(m.rho) - m.loglik).abs() < 1e-6, "{} vs {}", full_ll(m.rho), m.loglik);
    let best = (0..=200).map(|i| -0.99 + i as f64 * 0.0099).max_by(|a, b| full_ll(*a).partial_cmp(&full_ll(*b)).unwrap()).unwrap();
    assert!((best - m.rho).abs() < 0.011, "grid max {best} vs {}", m.rho);
    // Standard error of rho: the numerical curvature of the profile likelihood is close to the asymptotic value.
    let h = 1e-3;
    let curv = -(full_ll(m.rho + h) - 2.0 * full_ll(m.rho) + full_ll(m.rho - h)) / (h * h);
    let se_num = 1.0 / curv.sqrt();
    assert!((m.rho_se / se_num - 1.0).abs() < 0.35, "se {} vs numerical {}", m.rho_se, se_num);
    assert!(m.rho_p < 0.001 && m.lr_p < 0.001, "strong dependence is detected");
    assert!(m.loglik > m.loglik_ols);
    // The innovations are no longer spatially autocorrelated.
    let mi = moran(&m.innovations, &ws).unwrap();
    assert!(mi.p > 0.01, "innovations Moran p {}", mi.p);
}

#[test]
fn spatial_error_matches_a_brute_force_likelihood_and_recovers_lambda() {
    let (cols, y, w) = simulate("error", 0.6, 5);
    let ws = w.row_standardised();
    let eigs = eigenvalues(&ws, w.is_symmetric());
    let m = sem(&cols, &y, &names(), &ws, &eigs).unwrap();
    assert!((m.rho - 0.6).abs() < 0.15, "lambda {}", m.rho);
    assert!((m.beta[1] - 2.0).abs() < 0.2, "{:?}", m.beta);
    let n = y.len();
    let nf = n as f64;
    let wd = dense(&ws);
    let yv = DVector::from_column_slice(&y);
    let x = DMatrix::from_fn(n, 3, |i, j| if j == 0 { 1.0 } else { cols[j - 1][i] });
    let full_ll = |lam: f64| -> f64 {
        let a = DMatrix::identity(n, n) - &wd * lam;
        let (ys, xs) = (&a * &yv, &a * &x);
        let b = (xs.transpose() * &xs).try_inverse().unwrap() * xs.transpose() * &ys;
        let e = &ys - &xs * b;
        let s2 = e.dot(&e) / nf;
        -nf / 2.0 * (2.0 * std::f64::consts::PI * s2).ln() + a.lu().determinant().abs().ln() - e.dot(&e) / (2.0 * s2)
    };
    assert!((full_ll(m.rho) - m.loglik).abs() < 1e-6);
    let best = (0..=200).map(|i| -0.99 + i as f64 * 0.0099).max_by(|a, b| full_ll(*a).partial_cmp(&full_ll(*b)).unwrap()).unwrap();
    assert!((best - m.rho).abs() < 0.011);
    let h = 1e-3;
    let curv = -(full_ll(m.rho + h) - 2.0 * full_ll(m.rho) + full_ll(m.rho - h)) / (h * h);
    assert!((m.rho_se / (1.0 / curv.sqrt()) - 1.0).abs() < 0.35, "se {}", m.rho_se);
    assert!(m.lr_p < 0.001);
    // Islands are refused in words.
    let mut isl = ws.clone();
    isl[0].clear();
    assert!(sem(&cols, &y, &names(), &isl, &eigs).unwrap_err().contains("islands"));
}

#[test]
fn gwr_with_a_very_wide_kernel_is_ols_and_a_varying_coefficient_is_recovered() {
    let (xy, _) = grid();
    let n = xy.len();
    let dist = Dist::new(&xy, false, MAX_GWR, &|| false).unwrap();
    let mut rng = Rng::new(3);
    let z: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    // Constant coefficients: a Gaussian kernel with a bandwidth far larger than the area gives equal weights, so GWR = OLS.
    let y_const: Vec<f64> = (0..n).map(|i| 1.0 + 2.0 * z[i] + 0.3 * normal(&mut rng)).collect();
    let x = DMatrix::from_fn(n, 2, |i, j| if j == 0 { 1.0 } else { z[i] });
    let g = gwr_fit(&x, &y_const, &dist, 1e7, false, Kernel::Gaussian, true).unwrap();
    let o = ols(&[z.clone()], &y_const, &["z".into()], 0.95).unwrap();
    for i in [0usize, 100, 224] {
        assert!((g.beta[i][0] - o.beta[0]).abs() < 1e-6 && (g.beta[i][1] - o.beta[1]).abs() < 1e-6);
        assert!((g.se[i][1] - o.se[1]).abs() < 1e-6, "se {} vs {}", g.se[i][1], o.se[1]);
    }
    assert!((g.tr_s - 2.0).abs() < 1e-6 && (g.tr_sts - 2.0).abs() < 1e-6, "tr S {} tr S'S {}", g.tr_s, g.tr_sts);
    assert!((g.rss - o.rss).abs() < 1e-6);
    // The slope grows from west to east: y = (1 + 0.25 x) z + noise. GWR recovers it and beats the global model on AICc.
    let y_var: Vec<f64> = (0..n).map(|i| (1.0 + 0.25 * xy[i].0) * z[i] + 0.1 * normal(&mut rng)).collect();
    let bw = gwr_select(&x, &y_var, &dist, true, Kernel::Bisquare, Criterion::Aicc).unwrap();
    assert!(bw >= 7.0 && bw < n as f64, "bandwidth {bw}");
    let g = gwr_fit(&x, &y_var, &dist, bw, true, Kernel::Bisquare, true).unwrap();
    let err: f64 = (0..n).map(|i| (g.beta[i][1] - (1.0 + 0.25 * xy[i].0)).abs()).sum::<f64>() / n as f64;
    assert!(err < 0.25, "mean absolute error of the local slope {err}");
    let global = gwr_fit(&x, &y_var, &dist, 1e7, false, Kernel::Gaussian, false).unwrap();
    assert!(g.aicc < global.aicc - 3.0, "GWR AICc {} vs global {}", g.aicc, global.aicc);
    assert!(g.enp > 2.0 && g.enp < 60.0);
    assert!(g.local_r2.iter().all(|r| r.is_finite()));
    // Cross-validation also picks a finite, not-global bandwidth.
    let bw_cv = gwr_select(&x, &y_var, &dist, true, Kernel::Bisquare, Criterion::Cv).unwrap();
    assert!(bw_cv < n as f64);
    // Too small a bandwidth for the number of predictors is reported, not silently wrong.
    assert!(gwr_fit(&x, &y_var, &dist, 2.0, true, Kernel::Bisquare, false).is_err());
}
