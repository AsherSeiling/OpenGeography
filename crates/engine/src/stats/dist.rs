//! Distribution functions the tests need beyond `statrs`: the studentized range (Tukey HSD), non-central chi-square and F (confidence intervals of
//! effect sizes), and the exact null distributions of the Mann-Whitney and Wilcoxon signed-rank statistics.
//!
//! * Studentized range: P(Q <= q) = integral over s of f(s) W(q s; k) ds, where W(w; k) = k integral phi(z) [Phi(z) - Phi(z - w)]^(k-1) dz is the
//!   distribution of the range of k standard normals and s = sqrt(chi-square_df / df) (Tukey 1953; Copenhaver and Holland 1988, who evaluate the
//!   same integrals). Both integrals are done with composite Gauss-Legendre rules.
//! * Non-central chi-square and F: Poisson mixtures of central distributions (Johnson, Kotz and Balakrishnan 1995, ch. 29 and 30).
//! * Mann-Whitney: the coefficients of the Gaussian binomial (m+n choose m)_q count the arrangements with each U (Mann and Whitney 1947).
//!   Wilcoxon signed-rank: subset-sum counts of the ranks 1..n (Wilcoxon 1945).

use statrs::distribution::{ChiSquared, Continuous, ContinuousCDF, Normal};
use statrs::function::beta::beta_reg;
use statrs::function::gamma::ln_gamma;

/// Nodes and weights of the n-point Gauss-Legendre rule on [-1, 1] (Newton iteration on the Legendre polynomial).
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        out.push((x, 2.0 / ((1.0 - x * x) * dp * dp)));
    }
    out
}

/// Composite Gauss-Legendre integral of f over [a, b] with `panels` panels.
fn integrate(f: impl Fn(f64) -> f64, a: f64, b: f64, panels: usize, rule: &[(f64, f64)]) -> f64 {
    let h = (b - a) / panels as f64;
    let mut total = 0.0;
    for p in 0..panels {
        let (lo, hi) = (a + p as f64 * h, a + (p + 1) as f64 * h);
        let (mid, half) = ((lo + hi) / 2.0, (hi - lo) / 2.0);
        total += half * rule.iter().map(|(x, w)| w * f(mid + half * x)).sum::<f64>();
    }
    total
}

fn norm() -> Normal {
    Normal::new(0.0, 1.0).unwrap()
}

/// Distribution function of the range of k independent standard normals.
fn range_cdf(w: f64, k: usize, rule: &[(f64, f64)]) -> f64 {
    if w <= 0.0 {
        return 0.0;
    }
    let n = norm();
    let kf = k as f64;
    let v = kf * integrate(|z| n.pdf(z) * (n.cdf(z) - n.cdf(z - w)).max(0.0).powi(k as i32 - 1), -9.0, 9.0 + w.min(0.0), 36, rule);
    v.clamp(0.0, 1.0)
}

/// P(Q <= q) for the studentized range of `k` means with `df` degrees of freedom for the error (df = infinity: pass f64::INFINITY).
pub fn ptukey(q: f64, k: usize, df: f64) -> f64 {
    if q <= 0.0 || k < 2 {
        return 0.0;
    }
    let rule = gauss_legendre(24);
    if !df.is_finite() || df > 20_000.0 {
        return range_cdf(q, k, &rule);
    }
    let chi = ChiSquared::new(df).unwrap();
    let (lo, hi) = ((chi.inverse_cdf(1e-13) / df).sqrt(), (chi.inverse_cdf(1.0 - 1e-13) / df).sqrt());
    // s has density 2 (df/2)^(df/2) / Gamma(df/2) s^(df-1) exp(-df s^2 / 2).
    let ln_c = (2.0f64).ln() + df / 2.0 * (df / 2.0).ln() - ln_gamma(df / 2.0);
    let dens = |s: f64| (ln_c + (df - 1.0) * s.ln() - df * s * s / 2.0).exp();
    let v = integrate(|s| dens(s) * range_cdf(q * s, k, &rule), lo, hi, 24, &rule);
    v.clamp(0.0, 1.0)
}

/// The q with P(Q <= q) = p (bisection; the distribution function is increasing).
pub fn qtukey(p: f64, k: usize, df: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 5.0);
    while ptukey(hi, k, df) < p {
        hi *= 2.0;
        if hi > 1e4 {
            break;
        }
    }
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if ptukey(mid, k, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

fn poisson_weights(half_lambda: f64) -> Vec<(usize, f64)> {
    if half_lambda <= 0.0 {
        return vec![(0, 1.0)];
    }
    let top = (half_lambda + 12.0 * half_lambda.sqrt() + 60.0) as usize;
    (0..=top).map(|j| (j, (-half_lambda + j as f64 * half_lambda.ln() - ln_gamma(j as f64 + 1.0)).exp())).collect()
}

/// P(X <= x) for non-central chi-square with `df` degrees of freedom and non-centrality `lambda`.
pub fn pnchisq(x: f64, df: f64, lambda: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    poisson_weights(lambda / 2.0).iter().map(|(j, w)| w * ChiSquared::new(df + 2.0 * *j as f64).unwrap().cdf(x)).sum::<f64>().clamp(0.0, 1.0)
}

/// P(F <= x) for non-central F with d1, d2 degrees of freedom and non-centrality `lambda`.
pub fn pncf(x: f64, d1: f64, d2: f64, lambda: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let y = d1 * x / (d1 * x + d2);
    poisson_weights(lambda / 2.0).iter().map(|(j, w)| w * beta_reg(d1 / 2.0 + *j as f64, d2 / 2.0, y)).sum::<f64>().clamp(0.0, 1.0)
}

/// Confidence limits (lower, upper) of a non-centrality parameter from an observed statistic whose distribution function in lambda is `cdf(lambda)`
/// (decreasing in lambda): the lambda at which the observed value is the (1 - alpha/2) and (alpha/2) quantile (Steiger and Fouladi 1997).
pub fn nc_interval(cdf: impl Fn(f64) -> f64, conf: f64) -> (f64, f64) {
    let alpha = 1.0 - conf;
    let solve = |target: f64| -> f64 {
        if cdf(0.0) < target {
            return 0.0;
        }
        let mut hi = 1.0;
        while cdf(hi) > target && hi < 1e9 {
            hi *= 2.0;
        }
        let mut lo = 0.0;
        for _ in 0..80 {
            let mid = (lo + hi) / 2.0;
            if cdf(mid) > target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    };
    (solve(1.0 - alpha / 2.0), solve(alpha / 2.0))
}

/// Counts of the Mann-Whitney U statistic (number of pairs x > y) over all arrangements of m and n values: index u = 0..=m*n.
pub fn mw_counts(m: usize, n: usize) -> Vec<f64> {
    let top = m * n;
    let mut c = vec![0.0; top + 1];
    c[0] = 1.0;
    // Multiply by prod_{i=1..m} (1 - q^(n+i)) / (1 - q^i).
    for i in 1..=m {
        for u in (n + i..=top).rev() {
            c[u] -= c[u - (n + i)];
        }
        for u in i..=top {
            c[u] += c[u - i];
        }
    }
    c
}

/// Counts of the signed-rank statistic V = sum of positive ranks over the 2^n sign patterns: index v = 0..=n(n+1)/2.
pub fn signrank_counts(n: usize) -> Vec<f64> {
    let top = n * (n + 1) / 2;
    let mut c = vec![0.0; top + 1];
    c[0] = 1.0;
    for r in 1..=n {
        for s in (r..=top).rev() {
            c[s] += c[s - r];
        }
    }
    c
}

/// P(count index <= k) for a counts vector.
pub fn cum_le(counts: &[f64], k: f64) -> f64 {
    let total: f64 = counts.iter().sum();
    if k < 0.0 {
        return 0.0;
    }
    let upto = (k.floor() as usize).min(counts.len() - 1);
    counts[..=upto].iter().sum::<f64>() / total
}

/// Smallest k with P(index <= k) >= p (the quantile of a counts vector).
pub fn quantile_of(counts: &[f64], p: f64) -> usize {
    let total: f64 = counts.iter().sum();
    let mut acc = 0.0;
    for (k, c) in counts.iter().enumerate() {
        acc += c / total;
        if acc >= p - 1e-12 {
            return k;
        }
    }
    counts.len() - 1
}

/// Counts of the number of inversions of a permutation of n items (Mahonian numbers): index 0..=n(n-1)/2. Kendall's tau without ties.
pub fn inversion_counts(n: usize) -> Vec<f64> {
    let top = n * (n.saturating_sub(1)) / 2;
    let mut c = vec![0.0; top + 1];
    c[0] = 1.0;
    for i in 2..=n {
        let mut next = vec![0.0; top + 1];
        for (u, v) in c.iter().enumerate() {
            if *v == 0.0 {
                continue;
            }
            for add in 0..i {
                if u + add <= top {
                    next[u + add] += v;
                }
            }
        }
        c = next;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studentized_range_matches_published_tables() {
        // Upper 5 % points of the studentized range (Pearson and Hartley, Biometrika Tables): q(3, df=10) = 3.877, q(4, df=20) = 3.958,
        // q(2, df=infinity) = 2.772 (= sqrt(2) * 1.95996).
        assert!((ptukey(3.877, 3, 10.0) - 0.95).abs() < 5e-4, "{}", ptukey(3.877, 3, 10.0));
        assert!((ptukey(3.958, 4, 20.0) - 0.95).abs() < 5e-4, "{}", ptukey(3.958, 4, 20.0));
        assert!((qtukey(0.95, 2, f64::INFINITY) - 2.0f64.sqrt() * 1.959964).abs() < 1e-4);
    }

    #[test]
    fn exact_rank_distributions_by_hand() {
        // m = 2, n = 2: U takes 0..4 with counts 1, 1, 2, 1, 1.
        assert_eq!(mw_counts(2, 2), vec![1.0, 1.0, 2.0, 1.0, 1.0]);
        // n = 3 signed ranks: sums 0,1,2,3,3,4,5,6 -> counts 1,1,1,2,1,1,1.
        assert_eq!(signrank_counts(3), vec![1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0]);
        assert_eq!(inversion_counts(3), vec![1.0, 2.0, 2.0, 1.0]);
    }

    #[test]
    fn noncentral_chisq_reduces_to_central_and_has_the_right_mean_shift() {
        let c = ChiSquared::new(4.0).unwrap();
        assert!((pnchisq(5.0, 4.0, 0.0) - c.cdf(5.0)).abs() < 1e-12);
        // A larger non-centrality shifts the distribution right, so the cdf falls with it and stays below the central one.
        let (a, b) = (pnchisq(10.0, 3.0, 2.0), pnchisq(10.0, 3.0, 6.0));
        assert!(b < a && a < ChiSquared::new(3.0).unwrap().cdf(10.0));
        assert!((pnchisq(1e4, 3.0, 6.0) - 1.0).abs() < 1e-9);
    }
}
