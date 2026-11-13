//! Hypothesis tests and their effect sizes (package ST4). Pure functions on slices; the tools in `nodes/stat_tests.rs` load the data, call these and
//! report. Every definition is the textbook one and is checked against stored R output in `tests/stats_tests.rs`.
//!
//! Welch is the default two-sample t-test (Delacre, Lakens and Leys 2017). One-sided intervals are half-infinite, as in R (infinite ends become
//! null in a result table).

use super::describe::{mean, variance};
use super::dist::*;
use statrs::distribution::{Beta, Binomial, ChiSquared, ContinuousCDF, Discrete, DiscreteCDF, FisherSnedecor, Normal, StudentsT};

pub type Res<T> = Result<T, String>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alt {
    Two,
    Less,
    Greater,
}

impl Alt {
    pub fn parse(s: Option<&str>) -> Alt {
        match s {
            Some("less") => Alt::Less,
            Some("greater") => Alt::Greater,
            _ => Alt::Two,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Alt::Two => "two-sided",
            Alt::Less => "one-sided (less)",
            Alt::Greater => "one-sided (greater)",
        }
    }
}

fn std_normal() -> Normal {
    Normal::new(0.0, 1.0).unwrap()
}

pub fn pnorm(z: f64) -> f64 {
    std_normal().cdf(z)
}

pub fn qnorm(p: f64) -> f64 {
    std_normal().inverse_cdf(p)
}

fn tdist(df: f64) -> StudentsT {
    StudentsT::new(0.0, 1.0, df).unwrap()
}

fn p_from_t(t: f64, df: f64, alt: Alt) -> f64 {
    let d = tdist(df);
    match alt {
        Alt::Two => (2.0 * d.cdf(-t.abs())).min(1.0),
        Alt::Less => d.cdf(t),
        Alt::Greater => d.cdf(-t),
    }
}

fn p_from_z(z: f64, alt: Alt) -> f64 {
    match alt {
        Alt::Two => (2.0 * pnorm(-z.abs())).min(1.0),
        Alt::Less => pnorm(z),
        Alt::Greater => pnorm(-z),
    }
}

/// Chi-square upper tail.
pub fn chisq_sf(x: f64, df: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    1.0 - ChiSquared::new(df).unwrap().cdf(x)
}

pub fn f_sf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    1.0 - FisherSnedecor::new(d1, d2).unwrap().cdf(x)
}

/// Interval for an estimate with a t distribution, in the direction of the alternative.
fn t_interval(est: f64, se: f64, df: f64, alt: Alt, conf: f64) -> (f64, f64) {
    let d = tdist(df);
    match alt {
        Alt::Two => {
            let q = d.inverse_cdf(1.0 - (1.0 - conf) / 2.0);
            (est - q * se, est + q * se)
        }
        Alt::Less => (f64::NEG_INFINITY, est + d.inverse_cdf(conf) * se),
        Alt::Greater => (est - d.inverse_cdf(conf) * se, f64::INFINITY),
    }
}

/// Average ranks (1-based) and the sizes of every tie group of size > 1.
pub fn ranks(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap());
    let mut r = vec![0.0; x.len()];
    let mut ties = vec![];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && x[idx[j + 1]] == x[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for k in i..=j {
            r[idx[k]] = avg;
        }
        if j > i {
            ties.push((j - i + 1) as f64);
        }
        i = j + 1;
    }
    (r, ties)
}

fn tie_term(ties: &[f64]) -> f64 {
    ties.iter().map(|t| t * t * t - t).sum()
}

// ------------------------------------------------------------------ t tests

#[derive(Clone, Debug)]
pub struct TTest {
    pub t: f64,
    pub df: f64,
    pub p: f64,
    /// Mean, mean difference, or paired mean difference.
    pub estimate: f64,
    pub se: f64,
    pub ci: (f64, f64),
    /// Hedges' g (small-sample corrected standardised mean difference) and its approximate interval.
    pub g: f64,
    pub g_ci: (f64, f64),
}

fn hedges_j(df: f64) -> f64 {
    1.0 - 3.0 / (4.0 * df - 1.0)
}

fn g_interval(g: f64, var: f64, conf: f64) -> (f64, f64) {
    let z = qnorm(1.0 - (1.0 - conf) / 2.0);
    (g - z * var.sqrt(), g + z * var.sqrt())
}

/// One-sample t-test of the mean against `mu` (also the paired test, applied to the differences).
pub fn t_one(x: &[f64], mu: f64, alt: Alt, conf: f64) -> Res<TTest> {
    let n = x.len() as f64;
    if x.len() < 2 {
        return Err("A t-test needs at least 2 values.".into());
    }
    let m = mean(x);
    let v = variance(x).unwrap();
    if v <= 0.0 {
        return Err("All values are identical, so the standard deviation is 0 and the t statistic is undefined.".into());
    }
    let sd = v.sqrt();
    let se = sd / n.sqrt();
    let df = n - 1.0;
    let t = (m - mu) / se;
    let g = hedges_j(df) * (m - mu) / sd;
    let (lo, hi) = t_interval(m, se, df, alt, conf);
    let g_ci = g_interval(g, 1.0 / n + g * g / (2.0 * n), conf);
    Ok(TTest { t, df, p: p_from_t(t, df, alt), estimate: m, se, ci: (lo, hi), g, g_ci })
}

/// Two-sample t-test of mean(x) - mean(y) = 0. Welch unless `equal_var`.
pub fn t_two(x: &[f64], y: &[f64], equal_var: bool, alt: Alt, conf: f64) -> Res<TTest> {
    if x.len() < 2 || y.len() < 2 {
        return Err("Each group needs at least 2 values for a t-test.".into());
    }
    let (n1, n2) = (x.len() as f64, y.len() as f64);
    let (m1, m2) = (mean(x), mean(y));
    let (v1, v2) = (variance(x).unwrap(), variance(y).unwrap());
    if v1 <= 0.0 && v2 <= 0.0 {
        return Err("Both groups have no spread (every value in each group is identical), so the t statistic is undefined.".into());
    }
    let sp2 = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
    let (se, df) = if equal_var {
        ((sp2 * (1.0 / n1 + 1.0 / n2)).sqrt(), n1 + n2 - 2.0)
    } else {
        let (a, b) = (v1 / n1, v2 / n2);
        ((a + b).sqrt(), (a + b).powi(2) / (a * a / (n1 - 1.0) + b * b / (n2 - 1.0)))
    };
    let diff = m1 - m2;
    let t = diff / se;
    let g = hedges_j(n1 + n2 - 2.0) * diff / sp2.sqrt();
    let g_ci = g_interval(g, (n1 + n2) / (n1 * n2) + g * g / (2.0 * (n1 + n2)), conf);
    let (lo, hi) = t_interval(diff, se, df, alt, conf);
    Ok(TTest { t, df, p: p_from_t(t, df, alt), estimate: diff, se, ci: (lo, hi), g, g_ci })
}

// -------------------------------------------------------------------- ANOVA

#[derive(Clone, Debug)]
pub struct Anova {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
    pub ss_between: f64,
    pub ss_within: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub eta2: f64,
    pub eta2_ci: (f64, f64),
    pub omega2: f64,
}

fn check_groups(groups: &[Vec<f64>], min_each: usize) -> Res<()> {
    if groups.len() < 2 {
        return Err("At least 2 groups are needed.".into());
    }
    if let Some(g) = groups.iter().position(|g| g.len() < min_each) {
        return Err(format!("Group {} has {} value(s); every group needs at least {min_each}.", g + 1, groups[g].len()));
    }
    Ok(())
}

pub fn anova(groups: &[Vec<f64>], conf: f64) -> Res<Anova> {
    check_groups(groups, 1)?;
    let k = groups.len() as f64;
    let n_tot: f64 = groups.iter().map(|g| g.len() as f64).sum();
    if n_tot <= k {
        return Err("Every group has a single value, so there is no within-group variation to compare against.".into());
    }
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let grand = mean(&all);
    let ssb: f64 = groups.iter().map(|g| g.len() as f64 * (mean(g) - grand).powi(2)).sum();
    let ssw: f64 = groups.iter().map(|g| g.iter().map(|v| (v - mean(g)).powi(2)).sum::<f64>()).sum();
    if ssw <= 0.0 {
        return Err("No value varies within its group, so the F statistic is undefined.".into());
    }
    let (df1, df2) = (k - 1.0, n_tot - k);
    let (msb, msw) = (ssb / df1, ssw / df2);
    let f = msb / msw;
    let eta2 = ssb / (ssb + ssw);
    let omega2 = ((ssb - df1 * msw) / (ssb + ssw + msw)).max(0.0);
    let (l_lo, l_hi) = nc_interval(|l| pncf(f, df1, df2, l), conf);
    let to_eta = |l: f64| l / (l + n_tot);
    Ok(Anova { f, df1, df2, p: f_sf(f, df1, df2), ss_between: ssb, ss_within: ssw, ms_between: msb, ms_within: msw, eta2, eta2_ci: (to_eta(l_lo), to_eta(l_hi)), omega2 })
}

#[derive(Clone, Debug)]
pub struct WelchAnova {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// Welch (1951) one-way ANOVA for unequal variances.
pub fn welch_anova(groups: &[Vec<f64>]) -> Res<WelchAnova> {
    check_groups(groups, 2)?;
    let k = groups.len() as f64;
    let (mut w, mut wm) = (vec![], vec![]);
    for g in groups {
        let v = variance(g).unwrap();
        if v <= 0.0 {
            return Err("A group has no spread (identical values), so Welch's weights are undefined.".into());
        }
        w.push(g.len() as f64 / v);
        wm.push(mean(g));
    }
    let wsum: f64 = w.iter().sum();
    let mw: f64 = w.iter().zip(&wm).map(|(a, b)| a * b).sum::<f64>() / wsum;
    let a: f64 = w.iter().zip(&wm).map(|(wi, mi)| wi * (mi - mw).powi(2)).sum::<f64>() / (k - 1.0);
    let lam: f64 = groups.iter().zip(&w).map(|(g, wi)| (1.0 - wi / wsum).powi(2) / (g.len() as f64 - 1.0)).sum();
    let b = 1.0 + 2.0 * (k - 2.0) / (k * k - 1.0) * lam;
    let f = a / b;
    let df2 = (k * k - 1.0) / (3.0 * lam);
    Ok(WelchAnova { f, df1: k - 1.0, df2, p: f_sf(f, k - 1.0, df2) })
}

#[derive(Clone, Debug)]
pub struct TukeyRow {
    pub a: usize,
    pub b: usize,
    /// mean(b) - mean(a), the order R's TukeyHSD uses (later group minus earlier).
    pub diff: f64,
    pub ci: (f64, f64),
    pub p_adj: f64,
}

/// Tukey-Kramer all-pairs comparison after a one-way ANOVA (Tukey 1949; Kramer 1956 for unequal sizes).
pub fn tukey(groups: &[Vec<f64>], conf: f64) -> Res<Vec<TukeyRow>> {
    let a = anova(groups, conf)?;
    let k = groups.len();
    let qcrit = qtukey(conf, k, a.df2);
    let mut rows = vec![];
    for i in 0..k {
        for j in i + 1..k {
            let (ni, nj) = (groups[i].len() as f64, groups[j].len() as f64);
            let se = (a.ms_within / 2.0 * (1.0 / ni + 1.0 / nj)).sqrt();
            let diff = mean(&groups[j]) - mean(&groups[i]);
            let p_adj = (1.0 - ptukey(diff.abs() / se, k, a.df2)).clamp(0.0, 1.0);
            rows.push(TukeyRow { a: i, b: j, diff, ci: (diff - qcrit * se, diff + qcrit * se), p_adj });
        }
    }
    Ok(rows)
}

#[derive(Clone, Debug)]
pub struct LeveneResult {
    pub f: f64,
    pub df1: f64,
    pub df2: f64,
    pub p: f64,
}

/// Levene's test of equal variances; `median` centre is the Brown-Forsythe form (Brown and Forsythe 1974), the mean centre is Levene's original.
pub fn levene(groups: &[Vec<f64>], median: bool) -> Res<LeveneResult> {
    check_groups(groups, 2)?;
    let z: Vec<Vec<f64>> = groups
        .iter()
        .map(|g| {
            let c = if median {
                let mut s = g.clone();
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                super::describe::quantile7(&s, 0.5)
            } else {
                mean(g)
            };
            g.iter().map(|v| (v - c).abs()).collect()
        })
        .collect();
    let a = anova(&z, 0.95).map_err(|_| "Every group's values are identical or the groups are too small, so the variance test is undefined.".to_string())?;
    Ok(LeveneResult { f: a.f, df1: a.df1, df2: a.df2, p: a.p })
}

// --------------------------------------------------------------- chi-square

#[derive(Clone, Debug)]
pub struct ChiSq {
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
    pub n: f64,
    pub expected: Vec<Vec<f64>>,
    /// Cramer's V (independence) or Cohen's w (goodness of fit), with a non-central chi-square interval.
    pub effect: f64,
    pub effect_ci: (f64, f64),
    pub min_expected: f64,
    pub share_below_5: f64,
}

/// Pearson chi-square test of independence on a table of counts; Yates' continuity correction only for 2 x 2 and only when asked.
pub fn chisq_independence(counts: &[Vec<f64>], yates: bool, conf: f64) -> Res<ChiSq> {
    let (r, c) = (counts.len(), counts.first().map_or(0, |x| x.len()));
    if r < 2 || c < 2 {
        return Err("A test of independence needs at least 2 values in each field.".into());
    }
    let n: f64 = counts.iter().flatten().sum();
    let rs: Vec<f64> = counts.iter().map(|row| row.iter().sum()).collect();
    let cs: Vec<f64> = (0..c).map(|j| counts.iter().map(|row| row[j]).sum()).collect();
    let expected: Vec<Vec<f64>> = (0..r).map(|i| (0..c).map(|j| rs[i] * cs[j] / n).collect()).collect();
    let mut chi2 = 0.0;
    let corr = if yates && r == 2 && c == 2 {
        counts.iter().flatten().zip(expected.iter().flatten()).map(|(o, e)| (o - e).abs()).fold(f64::INFINITY, f64::min).min(0.5)
    } else {
        0.0
    };
    for i in 0..r {
        for j in 0..c {
            let (o, e) = (counts[i][j], expected[i][j]);
            chi2 += ((o - e).abs() - corr).powi(2) / e;
        }
    }
    let df = ((r - 1) * (c - 1)) as f64;
    let m = (r.min(c) - 1) as f64;
    let v = (chi2 / (n * m)).sqrt();
    let (l_lo, l_hi) = nc_interval(|l| pnchisq(chi2, df, l), conf);
    let to_v = |l: f64| ((l + df) / (n * m)).sqrt().min(1.0);
    let flat: Vec<f64> = expected.iter().flatten().copied().collect();
    let min_e = flat.iter().copied().fold(f64::INFINITY, f64::min);
    let below = flat.iter().filter(|e| **e < 5.0).count() as f64 / flat.len() as f64;
    Ok(ChiSq { chi2, df, p: chisq_sf(chi2, df), n, expected, effect: v, effect_ci: (to_v(l_lo).min(v), to_v(l_hi).max(v)), min_expected: min_e, share_below_5: below })
}

/// Chi-square goodness of fit of counts against expected proportions (rescaled to sum to 1).
pub fn chisq_gof(observed: &[f64], props: &[f64], conf: f64) -> Res<ChiSq> {
    if observed.len() < 2 {
        return Err("A goodness-of-fit test needs at least 2 categories.".into());
    }
    if props.len() != observed.len() {
        return Err(format!("{} expected proportions were given but the field has {} categories.", props.len(), observed.len()));
    }
    if props.iter().any(|p| !(*p > 0.0) || !p.is_finite()) {
        return Err("Expected proportions must all be positive numbers.".into());
    }
    let n: f64 = observed.iter().sum();
    let ps: f64 = props.iter().sum();
    let expected: Vec<f64> = props.iter().map(|p| n * p / ps).collect();
    let chi2: f64 = observed.iter().zip(&expected).map(|(o, e)| (o - e).powi(2) / e).sum();
    let df = (observed.len() - 1) as f64;
    let (l_lo, l_hi) = nc_interval(|l| pnchisq(chi2, df, l), conf);
    let w = (chi2 / n).sqrt();
    let to_w = |l: f64| ((l + df) / n).sqrt();
    let min_e = expected.iter().copied().fold(f64::INFINITY, f64::min);
    let below = expected.iter().filter(|e| **e < 5.0).count() as f64 / expected.len() as f64;
    Ok(ChiSq { chi2, df, p: chisq_sf(chi2, df), n, expected: vec![expected], effect: w, effect_ci: (to_w(l_lo).min(w), to_w(l_hi).max(w)), min_expected: min_e, share_below_5: below })
}

// -------------------------------------------------------------- proportions

/// Wilson score interval (Wilson 1927).
pub fn wilson(x: f64, n: f64, conf: f64) -> (f64, f64) {
    let z = qnorm(1.0 - (1.0 - conf) / 2.0);
    let p = x / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    ((c - h).max(0.0), (c + h).min(1.0))
}

/// Clopper-Pearson exact interval (Clopper and Pearson 1934).
pub fn clopper_pearson(x: f64, n: f64, conf: f64) -> (f64, f64) {
    let a = 1.0 - conf;
    let lo = if x == 0.0 { 0.0 } else { Beta::new(x, n - x + 1.0).unwrap().inverse_cdf(a / 2.0) };
    let hi = if x == n { 1.0 } else { Beta::new(x + 1.0, n - x).unwrap().inverse_cdf(1.0 - a / 2.0) };
    (lo, hi)
}

#[derive(Clone, Debug)]
pub struct BinomTest {
    pub p: f64,
    pub estimate: f64,
    pub ci_exact: (f64, f64),
    pub ci_wilson: (f64, f64),
    /// Cohen's h against p0.
    pub h: f64,
}

/// Exact binomial test of one proportion against p0 (two-sided p sums every outcome no more likely than the observed, as R's binom.test does).
pub fn binom_test(x: u64, n: u64, p0: f64, alt: Alt, conf: f64) -> Res<BinomTest> {
    if n == 0 || x > n {
        return Err("No observations.".into());
    }
    if !(p0 > 0.0 && p0 < 1.0) {
        return Err("The hypothesised proportion must be strictly between 0 and 1.".into());
    }
    if n > 20_000_000 {
        return Err("The exact binomial test is limited to 20 million observations.".into());
    }
    let b = Binomial::new(p0, n).unwrap();
    let p = match alt {
        Alt::Less => b.cdf(x),
        Alt::Greater => 1.0 - if x == 0 { 0.0 } else { b.cdf(x - 1) },
        Alt::Two => {
            let d = b.pmf(x);
            let cut = d * (1.0 + 1e-7);
            if (x as f64 - n as f64 * p0).abs() < 1e-12 {
                1.0
            } else {
                // Sum the smaller tail first for accuracy: all outcomes with pmf <= observed.
                (0..=n).map(|y| b.pmf(y)).filter(|v| *v <= cut).sum::<f64>().min(1.0)
            }
        }
    };
    let est = x as f64 / n as f64;
    let h = 2.0 * est.sqrt().asin() - 2.0 * p0.sqrt().asin();
    Ok(BinomTest { p, estimate: est, ci_exact: clopper_pearson(x as f64, n as f64, conf), ci_wilson: wilson(x as f64, n as f64, conf), h })
}

#[derive(Clone, Debug)]
pub struct PropEqual {
    pub chi2: f64,
    pub df: f64,
    pub p: f64,
}

/// Chi-square test that k proportions are equal (R's prop.test without continuity correction).
pub fn prop_equal(x: &[f64], n: &[f64]) -> Res<PropEqual> {
    if x.len() < 2 || x.len() != n.len() {
        return Err("At least 2 groups are needed to compare proportions.".into());
    }
    let (sx, sn): (f64, f64) = (x.iter().sum(), n.iter().sum());
    let p = sx / sn;
    if p <= 0.0 || p >= 1.0 {
        return Err("Every observation has the same outcome, so there is no difference to test.".into());
    }
    let chi2: f64 = x.iter().zip(n).map(|(xi, ni)| (xi - ni * p).powi(2) / (ni * p * (1.0 - p))).sum();
    let df = (x.len() - 1) as f64;
    Ok(PropEqual { chi2, df, p: chisq_sf(chi2, df) })
}

/// Difference of two proportions with Newcombe's hybrid score interval (Newcombe 1998, method 10).
pub fn newcombe_diff(x1: f64, n1: f64, x2: f64, n2: f64, conf: f64) -> (f64, (f64, f64)) {
    let (p1, p2) = (x1 / n1, x2 / n2);
    let (l1, u1) = wilson(x1, n1, conf);
    let (l2, u2) = wilson(x2, n2, conf);
    let d = p1 - p2;
    (d, (d - ((p1 - l1).powi(2) + (u2 - p2).powi(2)).sqrt(), d + ((u1 - p1).powi(2) + (p2 - l2).powi(2)).sqrt()))
}

// ------------------------------------------------------------ rank-based tests

#[derive(Clone, Debug)]
pub struct RankTest {
    /// The statistic in R's terms: W (Mann-Whitney, for the first group), V (signed rank), H (Kruskal-Wallis).
    pub stat: f64,
    pub z: Option<f64>,
    pub df: Option<f64>,
    pub p: f64,
    pub exact: bool,
    /// Rank-biserial correlation (two groups, paired) or epsilon-squared (Kruskal-Wallis).
    pub effect: f64,
    pub effect_ci: Option<(f64, f64)>,
    pub ties: bool,
}

fn fisher_back(z: f64) -> f64 {
    z.tanh()
}

/// Mann-Whitney U test (Wilcoxon rank-sum). Exact for fewer than 50 per group without ties, otherwise the normal approximation with continuity
/// and tie corrections (as R's wilcox.test). The effect size is the rank-biserial correlation 2 W / (n1 n2) - 1 with an interval from the
/// Hanley-McNeil (1982) standard error of the area under the curve W / (n1 n2).
pub fn mann_whitney(x: &[f64], y: &[f64], alt: Alt, conf: f64) -> Res<RankTest> {
    let (n1, n2) = (x.len() as f64, y.len() as f64);
    if x.is_empty() || y.is_empty() {
        return Err("Each group needs at least 1 value.".into());
    }
    let all: Vec<f64> = x.iter().chain(y).copied().collect();
    let (r, ties) = ranks(&all);
    let w = r[..x.len()].iter().sum::<f64>() - n1 * (n1 + 1.0) / 2.0;
    let has_ties = !ties.is_empty();
    let exact = !has_ties && x.len() < 50 && y.len() < 50;
    let (p, z) = if exact {
        let c = mw_counts(x.len(), y.len());
        let le = cum_le(&c, w);
        let ge = 1.0 - cum_le(&c, w - 1.0);
        (
            match alt {
                Alt::Two => (2.0 * le.min(ge)).min(1.0),
                Alt::Less => le,
                Alt::Greater => ge,
            },
            None,
        )
    } else {
        let n = n1 + n2;
        let sigma = (n1 * n2 / 12.0 * ((n + 1.0) - tie_term(&ties) / (n * (n - 1.0)))).sqrt();
        if sigma <= 0.0 {
            return Err("Every value is identical, so the ranks cannot separate the groups.".into());
        }
        let d = w - n1 * n2 / 2.0;
        let corr = match alt {
            Alt::Two => d.signum() * 0.5,
            Alt::Greater => 0.5,
            Alt::Less => -0.5,
        };
        let z = (d - corr) / sigma;
        (p_from_z(z, alt), Some(z))
    };
    let auc = w / (n1 * n2);
    let rb = 2.0 * auc - 1.0;
    let q1 = auc / (2.0 - auc);
    let q2 = 2.0 * auc * auc / (1.0 + auc);
    let var = (auc * (1.0 - auc) + (n1 - 1.0) * (q1 - auc * auc) + (n2 - 1.0) * (q2 - auc * auc)) / (n1 * n2);
    let ci = if var > 0.0 {
        let zc = qnorm(1.0 - (1.0 - conf) / 2.0);
        let s = var.sqrt();
        Some((2.0 * (auc - zc * s).max(0.0) - 1.0, 2.0 * (auc + zc * s).min(1.0) - 1.0))
    } else {
        None
    };
    Ok(RankTest { stat: w, z, df: None, p, exact, effect: rb, effect_ci: ci, ties: has_ties })
}

/// Wilcoxon signed-rank test of the differences `d` (already x - mu, or x - y for pairs). Zeros are dropped (count returned). Exact below 50
/// without ties, otherwise normal with continuity and tie corrections. Effect: matched-pairs rank-biserial correlation.
pub fn signed_rank(d: &[f64], alt: Alt, conf: f64) -> Res<(RankTest, usize)> {
    let nz: Vec<f64> = d.iter().copied().filter(|v| *v != 0.0).collect();
    let zeros = d.len() - nz.len();
    if nz.is_empty() {
        return Err("Every difference is exactly 0, so there is nothing to rank.".into());
    }
    let n = nz.len() as f64;
    let abs: Vec<f64> = nz.iter().map(|v| v.abs()).collect();
    let (r, ties) = ranks(&abs);
    let v: f64 = nz.iter().zip(&r).filter(|(d, _)| **d > 0.0).map(|(_, r)| r).sum();
    let has_ties = !ties.is_empty();
    let exact = !has_ties && zeros == 0 && nz.len() < 50;
    let (p, z) = if exact {
        let c = signrank_counts(nz.len());
        let le = cum_le(&c, v);
        let ge = 1.0 - cum_le(&c, v - 1.0);
        (
            match alt {
                Alt::Two => (2.0 * le.min(ge)).min(1.0),
                Alt::Less => le,
                Alt::Greater => ge,
            },
            None,
        )
    } else {
        let sigma = (n * (n + 1.0) * (2.0 * n + 1.0) / 24.0 - tie_term(&ties) / 48.0).sqrt();
        let dv = v - n * (n + 1.0) / 4.0;
        let corr = match alt {
            Alt::Two => dv.signum() * 0.5,
            Alt::Greater => 0.5,
            Alt::Less => -0.5,
        };
        let z = (dv - corr) / sigma;
        (p_from_z(z, alt), Some(z))
    };
    let total = n * (n + 1.0) / 2.0;
    let rb = (2.0 * v - total) / total;
    let se = (2.0 * (2.0 * n + 1.0) / (3.0 * n * (n + 1.0))).sqrt();
    let ci = if n >= 4.0 && rb.abs() < 1.0 {
        let zc = qnorm(1.0 - (1.0 - conf) / 2.0);
        let fz = rb.atanh();
        let s = se / (1.0 - rb * rb);
        Some((fisher_back(fz - zc * s), fisher_back(fz + zc * s)))
    } else {
        None
    };
    Ok((RankTest { stat: v, z, df: None, p, exact, effect: rb, effect_ci: ci, ties: has_ties }, zeros))
}

/// Kruskal-Wallis rank-sum test with the tie correction (Kruskal and Wallis 1952); effect size epsilon-squared = H / (N - 1) (Tomczak and Tomczak 2014).
pub fn kruskal(groups: &[Vec<f64>]) -> Res<RankTest> {
    check_groups(groups, 1)?;
    let all: Vec<f64> = groups.iter().flatten().copied().collect();
    let n = all.len() as f64;
    let (r, ties) = ranks(&all);
    let mut start = 0;
    let mut h = 0.0;
    for g in groups {
        let rs: f64 = r[start..start + g.len()].iter().sum();
        h += rs * rs / g.len() as f64;
        start += g.len();
    }
    h = 12.0 / (n * (n + 1.0)) * h - 3.0 * (n + 1.0);
    let corr = 1.0 - tie_term(&ties) / (n * n * n - n);
    if corr <= 0.0 {
        return Err("Every value is identical, so the ranks cannot separate the groups.".into());
    }
    h /= corr;
    let df = (groups.len() - 1) as f64;
    Ok(RankTest { stat: h, z: None, df: Some(df), p: chisq_sf(h, df), exact: false, effect: h / (n - 1.0), effect_ci: None, ties: !ties.is_empty() })
}

// ------------------------------------------------------------ Shapiro-Wilk

fn poly(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, k| acc * x + k)
}

/// Shapiro-Wilk W and its p-value for 3 <= n <= 5000 (Royston 1995, algorithm AS R94, which approximates Shapiro and Wilk's 1965 coefficients).
pub fn shapiro_wilk(x: &[f64]) -> Res<(f64, f64)> {
    let n = x.len();
    if n < 3 {
        return Err("The Shapiro-Wilk test needs at least 3 values.".into());
    }
    if n > 5000 {
        return Err("The Shapiro-Wilk test is defined for at most 5000 values (Royston 1995); test a random sample or use a Q-Q plot.".into());
    }
    let mut s = x.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let range = s[n - 1] - s[0];
    if range <= 0.0 {
        return Err("All values are identical, so W is undefined.".into());
    }
    let nf = n as f64;
    let nn2 = n / 2;
    // Coefficients a_1..a_nn2 for the pairs (x_(n+1-i) - x_(i)).
    let mut a = vec![0.0; nn2 + 1];
    if n == 3 {
        a[1] = 0.5f64.sqrt();
    } else {
        let mut m = vec![0.0; nn2 + 1];
        let mut summ2 = 0.0;
        for i in 1..=nn2 {
            m[i] = qnorm((i as f64 - 0.375) / (nf + 0.25));
            summ2 += m[i] * m[i];
        }
        summ2 *= 2.0;
        let ssumm2 = summ2.sqrt();
        let rsn = 1.0 / nf.sqrt();
        const C1: [f64; 6] = [0.0, 0.221157, -0.147981, -2.07119, 4.434685, -2.706056];
        const C2: [f64; 6] = [0.0, 0.042981, -0.293762, -1.752461, 5.682633, -3.582633];
        let a1 = poly(&C1, rsn) - m[1] / ssumm2;
        let (i1, fac);
        let mut a2 = 0.0;
        if n > 5 {
            i1 = 3;
            a2 = -m[2] / ssumm2 + poly(&C2, rsn);
            fac = ((summ2 - 2.0 * m[1] * m[1] - 2.0 * m[2] * m[2]) / (1.0 - 2.0 * a1 * a1 - 2.0 * a2 * a2)).sqrt();
        } else {
            i1 = 2;
            fac = ((summ2 - 2.0 * m[1] * m[1]) / (1.0 - 2.0 * a1 * a1)).sqrt();
        }
        a[1] = a1;
        if n > 5 {
            a[2] = a2;
        }
        for i in i1..=nn2 {
            a[i] = m[i] / -fac;
        }
    }
    let mut num = 0.0;
    for i in 1..=nn2 {
        num += a[i] * (s[n - i] - s[i - 1]);
    }
    let mean_x = mean(&s);
    let ss: f64 = s.iter().map(|v| (v - mean_x).powi(2)).sum();
    let w = (num * num / ss).min(1.0);
    let p = if n == 3 {
        let pi6 = 6.0 / std::f64::consts::PI;
        let stqr = (0.75f64).sqrt().asin();
        (pi6 * (w.sqrt().asin() - stqr)).max(0.0)
    } else {
        let y = (1.0 - w).ln();
        let xx = nf.ln();
        let (yy, m, sd) = if n <= 11 {
            let gamma = -2.273 + 0.459 * nf;
            if y >= gamma {
                return Ok((w, 1e-99));
            }
            const C3: [f64; 4] = [0.544, -0.39978, 0.025054, -6.714e-4];
            const C4: [f64; 4] = [1.3822, -0.77857, 0.062767, -0.0020322];
            (-(gamma - y).ln(), poly(&C3, nf), poly(&C4, nf).exp())
        } else {
            const C5: [f64; 4] = [-1.5861, -0.31082, -0.083751, 0.0038915];
            const C6: [f64; 3] = [-0.4803, -0.082676, 0.0030302];
            (y, poly(&C5, xx), poly(&C6, xx).exp())
        };
        1.0 - pnorm((yy - m) / sd)
    };
    Ok((w, p.clamp(0.0, 1.0)))
}

// ------------------------------------------------------------- correlation

#[derive(Clone, Debug)]
pub struct Corr {
    pub r: f64,
    pub stat: f64,
    pub stat_name: &'static str,
    pub df: Option<f64>,
    pub p: f64,
    pub ci: Option<(f64, f64)>,
    pub exact: bool,
    pub ties: bool,
}

fn fisher_ci(r: f64, se: f64, conf: f64) -> (f64, f64) {
    let z = qnorm(1.0 - (1.0 - conf) / 2.0);
    let fz = r.atanh();
    ((fz - z * se).tanh(), (fz + z * se).tanh())
}

fn pearson_r(x: &[f64], y: &[f64]) -> Res<f64> {
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return Err("One of the fields has no spread (identical values), so a correlation is undefined.".into());
    }
    Ok((sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0))
}

fn corr_t(r: f64, n: f64) -> (f64, f64, f64) {
    let df = n - 2.0;
    let t = r * (df / (1.0 - r * r).max(1e-300)).sqrt();
    (t, df, p_from_t(t, df, Alt::Two))
}

/// Pearson product-moment correlation with a Fisher-z interval (Fisher 1921).
pub fn pearson(x: &[f64], y: &[f64], conf: f64) -> Res<Corr> {
    if x.len() < 3 {
        return Err("A correlation needs at least 3 pairs.".into());
    }
    let r = pearson_r(x, y)?;
    let n = x.len() as f64;
    let (t, df, p) = corr_t(r, n);
    let ci = if n > 3.0 { Some(fisher_ci(r, 1.0 / (n - 3.0).sqrt(), conf)) } else { None };
    Ok(Corr { r, stat: t, stat_name: "t", df: Some(df), p, ci, exact: false, ties: false })
}

/// Spearman rank correlation (Pearson on average ranks) with the t approximation for p and the Bonett-Wright (2000) interval.
pub fn spearman(x: &[f64], y: &[f64], conf: f64) -> Res<Corr> {
    if x.len() < 3 {
        return Err("A correlation needs at least 3 pairs.".into());
    }
    let (rx, tx) = ranks(x);
    let (ry, ty) = ranks(y);
    let r = pearson_r(&rx, &ry)?;
    let n = x.len() as f64;
    let (t, df, p) = corr_t(r, n);
    let ci = if n > 3.0 { Some(fisher_ci(r, ((1.0 + r * r / 2.0) / (n - 3.0)).sqrt(), conf)) } else { None };
    Ok(Corr { r, stat: t, stat_name: "t", df: Some(df), p, ci, exact: false, ties: !tx.is_empty() || !ty.is_empty() })
}

/// Kendall's tau-b. Exact p (n < 50, no ties) from the distribution of inversions, otherwise the tie-corrected normal approximation of Kendall
/// (1970). The interval is Fisher's z with the Fieller, Hartley and Pearson (1957) standard error sqrt(0.437 / (n - 4)).
pub fn kendall(x: &[f64], y: &[f64], conf: f64) -> Res<Corr> {
    let n = x.len();
    if n < 3 {
        return Err("A correlation needs at least 3 pairs.".into());
    }
    if n > 20_000 {
        return Err("Kendall's tau compares every pair of rows and is limited to 20,000 rows; use Spearman for larger tables.".into());
    }
    let (mut conc, mut disc) = (0.0f64, 0.0f64);
    for i in 0..n {
        for j in i + 1..n {
            let s = (x[i] - x[j]).signum() * (y[i] - y[j]).signum();
            if (x[i] - x[j]) == 0.0 || (y[i] - y[j]) == 0.0 {
                continue;
            }
            if s > 0.0 {
                conc += 1.0;
            } else {
                disc += 1.0;
            }
        }
    }
    let nf = n as f64;
    let n0 = nf * (nf - 1.0) / 2.0;
    let (_, tx) = ranks(x);
    let (_, ty) = ranks(y);
    let n1: f64 = tx.iter().map(|t| t * (t - 1.0) / 2.0).sum();
    let n2: f64 = ty.iter().map(|t| t * (t - 1.0) / 2.0).sum();
    if n0 - n1 <= 0.0 || n0 - n2 <= 0.0 {
        return Err("One of the fields has no spread (identical values), so a correlation is undefined.".into());
    }
    let s = conc - disc;
    let tau = s / ((n0 - n1) * (n0 - n2)).sqrt();
    let has_ties = !tx.is_empty() || !ty.is_empty();
    let ci = if nf > 4.0 { Some(fisher_ci(tau.clamp(-0.999999, 0.999999), (0.437 / (nf - 4.0)).sqrt(), conf)) } else { None };
    if !has_ties && n < 50 {
        let c = inversion_counts(n);
        // Concordant pairs q = (S + n0) / 2 follow the same law as n0 minus inversions.
        let q = ((s + n0) / 2.0).round();
        let le = cum_le(&c, q);
        let ge = 1.0 - cum_le(&c, q - 1.0);
        return Ok(Corr { r: tau, stat: q, stat_name: "T (concordant pairs)", df: None, p: (2.0 * le.min(ge)).min(1.0), ci, exact: true, ties: false });
    }
    let v0 = nf * (nf - 1.0) * (2.0 * nf + 5.0);
    let vt: f64 = tx.iter().map(|t| t * (t - 1.0) * (2.0 * t + 5.0)).sum();
    let vu: f64 = ty.iter().map(|t| t * (t - 1.0) * (2.0 * t + 5.0)).sum();
    let v1: f64 = tx.iter().map(|t| t * (t - 1.0)).sum::<f64>() * ty.iter().map(|t| t * (t - 1.0)).sum::<f64>();
    let v2: f64 = tx.iter().map(|t| t * (t - 1.0) * (t - 2.0)).sum::<f64>() * ty.iter().map(|t| t * (t - 1.0) * (t - 2.0)).sum::<f64>();
    let var = (v0 - vt - vu) / 18.0 + v2 / (9.0 * nf * (nf - 1.0) * (nf - 2.0)) + v1 / (2.0 * nf * (nf - 1.0));
    let z = s / var.sqrt();
    Ok(Corr { r: tau, stat: z, stat_name: "z", df: None, p: p_from_z(z, Alt::Two), ci, exact: false, ties: has_ties })
}
