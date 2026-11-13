//! MGWR on simulated data with a locally varying coefficient (z1, slowly changing with x) and a constant one (z2): the constant term gets a much
//! wider bandwidth, the coefficients are recovered, and the hat-matrix form agrees with the fitted values (internal consistency).

use engine::stats::mgwr::mgwr;
use engine::stats::spatial_reg::*;
use engine::stats::weights::Rng;
use nalgebra::DMatrix;

fn normal(rng: &mut Rng) -> f64 {
    let (u, v) = (rng.f64().max(1e-12), rng.f64());
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

#[test]
fn mgwr_gives_the_constant_covariate_a_wider_bandwidth() {
    let side = 14usize;
    let n = side * side;
    let xy: Vec<(f64, f64)> = (0..n).map(|i| ((i % side) as f64, (i / side) as f64)).collect();
    let dist = Dist::new(&xy, false, MAX_MGWR, &|| false).unwrap();
    let mut rng = Rng::new(21);
    let z1: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    let z2: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
    let y: Vec<f64> = (0..n).map(|i| 1.0 + (0.5 + 0.2 * xy[i].0) * z1[i] + 2.0 * z2[i] + 0.1 * normal(&mut rng)).collect();
    let x = DMatrix::from_fn(n, 3, |i, j| match j {
        0 => 1.0,
        1 => z1[i],
        _ => z2[i],
    });
    let m = mgwr(&x, &y, &dist, true, Kernel::Bisquare, Criterion::Aicc, &|| false).unwrap();
    assert!(m.bandwidths[2] > m.bandwidths[1], "constant term bandwidth {} should exceed the varying one {}", m.bandwidths[2], m.bandwidths[1]);
    // The constant coefficient is nearly flat around 2; the varying one follows 0.5 + 0.2 x.
    let mean2 = m.beta.iter().map(|b| b[2]).sum::<f64>() / n as f64;
    assert!((mean2 - 2.0).abs() < 0.15, "mean z2 coefficient {mean2}");
    let err: f64 = (0..n).map(|i| (m.beta[i][1] - (0.5 + 0.2 * xy[i].0)).abs()).sum::<f64>() / n as f64;
    assert!(err < 0.3, "z1 coefficient error {err}");
    // Internal consistency: the fitted values are the sum of coefficient x covariate, and the model fits well with sensible degrees of freedom.
    for i in [0usize, 57, 195] {
        let direct: f64 = (0..3).map(|j| m.beta[i][j] * x[(i, j)]).sum();
        assert!((direct - m.fitted[i]).abs() < 1e-6, "{direct} vs {}", m.fitted[i]);
    }
    assert!(m.tr_s > 3.0 && m.tr_s < n as f64 / 2.0, "tr S {}", m.tr_s);
    assert!((m.enp.iter().sum::<f64>() - m.tr_s).abs() < 1e-9);
    assert!(m.se.iter().all(|r| r.iter().all(|v| v.is_finite() && *v > 0.0)));
    assert!(m.rss / (n as f64) < 0.1);
    // Too many features are refused in words.
    let big: Vec<(f64, f64)> = (0..MAX_MGWR + 1).map(|i| (i as f64, 0.0)).collect();
    assert!(Dist::new(&big, false, MAX_MGWR, &|| false).is_err());
}
