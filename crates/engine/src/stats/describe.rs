//! Descriptive statistics of a column of numbers (ST2). Pure functions on slices, accurate enough for the NIST Statistical Reference Datasets
//! (the "NumAcc" sets are built to defeat the one-pass textbook formulas): sums are Neumaier-compensated and the variance is the corrected
//! two-pass form.
//!
//! Definitions (also stated in every result):
//! * mean, sample standard deviation and variance with n - 1 in the denominator;
//! * quantiles are R type 7 (Hyndman and Fan 1996), the same as DuckDB's `quantile_cont`;
//! * skewness is the adjusted Fisher-Pearson coefficient G1 and kurtosis the sample excess kurtosis G2 (Joanes and Gill 1998; the values
//!   Excel's SKEW and KURT, SAS and SPSS report);
//! * the mode is the most frequent value (the smallest when several tie), and absent when no value repeats;
//! * weighted statistics use reliability (precision) weights: mean = sum(w x) / sum(w), variance = sum(w (x - mean)^2) / (sum(w) - sum(w^2) / sum(w)).

use std::collections::HashMap;

/// Neumaier's improved Kahan summation.
#[derive(Default, Clone, Copy)]
pub struct Sum {
    s: f64,
    c: f64,
}

impl Sum {
    pub fn add(&mut self, x: f64) {
        let t = self.s + x;
        if self.s.abs() >= x.abs() {
            self.c += (self.s - t) + x;
        } else {
            self.c += (x - t) + self.s;
        }
        self.s = t;
    }
    pub fn value(&self) -> f64 {
        self.s + self.c
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Describe {
    pub n: usize,
    pub sum: f64,
    pub mean: f64,
    pub variance: Option<f64>,
    pub sd: Option<f64>,
    /// Standard error of the mean.
    pub se: Option<f64>,
    /// 95 % confidence interval of the mean (t distribution, n - 1 degrees of freedom).
    pub ci95: Option<(f64, f64)>,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub range: f64,
    pub iqr: f64,
    pub skewness: Option<f64>,
    pub kurtosis: Option<f64>,
    /// Standard deviation over the mean; absent when the mean is 0.
    pub cv: Option<f64>,
    pub mode: Option<f64>,
}

/// Quantile of a sorted slice, R type 7: h = (n - 1) p, interpolating between the two neighbours.
pub fn quantile7(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    let h = (n - 1) as f64 * p.clamp(0.0, 1.0);
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

/// Mean of a slice (compensated).
pub fn mean(x: &[f64]) -> f64 {
    let mut s = Sum::default();
    x.iter().for_each(|v| s.add(*v));
    s.value() / x.len() as f64
}

/// Sample variance (n - 1), corrected two-pass. None for fewer than 2 values.
pub fn variance(x: &[f64]) -> Option<f64> {
    let n = x.len();
    if n < 2 {
        return None;
    }
    let m = mean(x);
    let (mut s2, mut s1) = (Sum::default(), Sum::default());
    for v in x {
        let d = v - m;
        s2.add(d * d);
        s1.add(d);
    }
    Some(((s2.value() - s1.value() * s1.value() / n as f64) / (n - 1) as f64).max(0.0))
}

pub fn describe(x: &[f64]) -> Option<Describe> {
    let n = x.len();
    if n == 0 {
        return None;
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut sum = Sum::default();
    x.iter().for_each(|v| sum.add(*v));
    let mean = sum.value() / n as f64;
    let variance = variance(x);
    let sd = variance.map(f64::sqrt);
    let se = sd.map(|s| s / (n as f64).sqrt());
    let ci95 = match (se, n) {
        (Some(se), n) if n >= 2 => {
            use statrs::distribution::{ContinuousCDF, StudentsT};
            StudentsT::new(0.0, 1.0, (n - 1) as f64).ok().map(|t| {
                let q = t.inverse_cdf(0.975);
                (mean - q * se, mean + q * se)
            })
        }
        _ => None,
    };
    let nf = n as f64;
    // Central moments about the mean, compensated.
    let (mut m3, mut m4) = (Sum::default(), Sum::default());
    for v in x {
        let d = v - mean;
        m3.add(d * d * d);
        m4.add(d * d * d * d);
    }
    let skewness = match sd {
        Some(s) if n >= 3 && s > 0.0 => Some(nf / ((nf - 1.0) * (nf - 2.0)) * m3.value() / (s * s * s)),
        _ => None,
    };
    let kurtosis = match sd {
        Some(s) if n >= 4 && s > 0.0 => {
            let s4 = s * s * s * s;
            Some(nf * (nf + 1.0) / ((nf - 1.0) * (nf - 2.0) * (nf - 3.0)) * m4.value() / s4 - 3.0 * (nf - 1.0) * (nf - 1.0) / ((nf - 2.0) * (nf - 3.0)))
        }
        _ => None,
    };
    let mut counts: HashMap<u64, usize> = HashMap::new();
    for v in x {
        // -0.0 and 0.0 are one value.
        *counts.entry((if *v == 0.0 { 0.0f64 } else { *v }).to_bits()).or_insert(0) += 1;
    }
    let best = counts.values().copied().max().unwrap_or(1);
    let mode = if best > 1 { counts.iter().filter(|(_, c)| **c == best).map(|(b, _)| f64::from_bits(*b)).fold(f64::INFINITY, f64::min).into() } else { None };
    let (q1, median, q3) = (quantile7(&sorted, 0.25), quantile7(&sorted, 0.5), quantile7(&sorted, 0.75));
    Some(Describe {
        n,
        sum: sum.value(),
        mean,
        variance,
        sd,
        se,
        ci95,
        min: sorted[0],
        q1,
        median,
        q3,
        max: sorted[n - 1],
        range: sorted[n - 1] - sorted[0],
        iqr: q3 - q1,
        skewness,
        kurtosis,
        cv: match sd {
            Some(s) if mean != 0.0 => Some(s / mean),
            _ => None,
        },
        mode,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Weighted {
    pub sum_weights: f64,
    /// (sum w)^2 / sum(w^2): how many equally weighted values the weights are worth.
    pub effective_n: f64,
    pub mean: f64,
    pub variance: Option<f64>,
    pub sd: Option<f64>,
}

/// Weighted mean and reliability-weights variance. Weights must be non-negative; pairs with weight 0 count for nothing.
pub fn weighted(x: &[f64], w: &[f64]) -> Option<Weighted> {
    let (mut sw, mut sw2, mut swx) = (Sum::default(), Sum::default(), Sum::default());
    for (v, wi) in x.iter().zip(w) {
        sw.add(*wi);
        sw2.add(wi * wi);
        swx.add(wi * v);
    }
    let (w1, w2) = (sw.value(), sw2.value());
    if !(w1 > 0.0) {
        return None;
    }
    let mean = swx.value() / w1;
    let mut ss = Sum::default();
    for (v, wi) in x.iter().zip(w) {
        ss.add(wi * (v - mean) * (v - mean));
    }
    let denom = w1 - w2 / w1;
    let variance = if denom > 0.0 { Some(ss.value() / denom) } else { None };
    Some(Weighted { sum_weights: w1, effective_n: w1 * w1 / w2, mean, variance, sd: variance.map(f64::sqrt) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_type_7_matches_r() {
        // R: quantile(c(1, 2, 3, 4, 10), c(.25, .5, .9)) gives 2, 3, 7.6
        let x = [1.0, 2.0, 3.0, 4.0, 10.0];
        assert_eq!(quantile7(&x, 0.25), 2.0);
        assert_eq!(quantile7(&x, 0.5), 3.0);
        assert!((quantile7(&x, 0.9) - 7.6).abs() < 1e-12);
    }

    #[test]
    fn excel_documentation_skew_and_kurt() {
        // Microsoft's documented examples: SKEW = 0.359543071, KURT = -0.151799637 for this data.
        let d = describe(&[3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 5.0, 6.0, 4.0, 7.0]).unwrap();
        assert!((d.skewness.unwrap() - 0.359543071).abs() < 1e-8, "{:?}", d.skewness);
        assert!((d.kurtosis.unwrap() + 0.151799637).abs() < 1e-8, "{:?}", d.kurtosis);
    }

    #[test]
    fn weighted_mean_and_variance_by_hand() {
        // x = 1, 2, 4 with weights 1, 2, 1: mean = (1 + 4 + 4) / 4 = 2.25; sum w (x - m)^2 = 1.5625 + 0.125 + 3.0625 = 4.75;
        // denominator = 4 - 6 / 4 = 2.5; variance = 1.9.
        let w = weighted(&[1.0, 2.0, 4.0], &[1.0, 2.0, 1.0]).unwrap();
        assert!((w.mean - 2.25).abs() < 1e-12 && (w.variance.unwrap() - 1.9).abs() < 1e-12 && (w.effective_n - 16.0 / 6.0).abs() < 1e-12);
    }
}
