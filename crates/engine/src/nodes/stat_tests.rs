//! Statistics › Tests (package ST4): t-tests, one-way ANOVA with Tukey HSD and Welch ANOVA, chi-square, proportions, rank tests, Shapiro-Wilk,
//! Levene and correlations. The maths is in `stats/inference.rs` and `stats/dist.rs` and is checked against stored R output
//! (`tests/stats_tests.rs`). Every result names its exact method, rows used and dropped, an effect size with an interval beside each p-value,
//! assumption-check notes, and the spatial-independence note when the input has geometry.

use super::stat_support::*;
use super::*;
use crate::stats::describe::{mean, variance};
use crate::stats::inference::*;
use crate::stats::multiple::{adjust as adjust_p, adjust_label};
use crate::stats::{ColumnKind as K, Dropped};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const C_WELCH: &str = "Welch, B. L. (1947). The generalization of 'Student's' problem when several different population variances are involved. Biometrika 34, 28-35.";
const C_DELACRE: &str = "Delacre, M., Lakens, D. and Leys, C. (2017). Why psychologists should by default use Welch's t-test instead of Student's t-test. International Review of Social Psychology 30(1), 92-101.";
const C_HEDGES: &str = "Hedges, L. V. and Olkin, I. (1985). Statistical Methods for Meta-Analysis. Academic Press (Hedges' g and its standard error).";
const C_SW: &str = "Royston, P. (1995). Remark AS R94: a remark on algorithm AS 181: the W-test for normality. Applied Statistics 44(4), 547-551 (Shapiro-Wilk).";
const C_ANOVA: &str = "Fisher, R. A. (1925). Statistical Methods for Research Workers. Oliver and Boyd (one-way analysis of variance).";
const C_ETA: &str = "Steiger, J. H. (2004). Beyond the F test: effect size confidence intervals and tests of close fit in the analysis of variance and contrast analysis. Psychological Methods 9(2), 164-182 (eta-squared interval from the non-central F distribution).";
const C_TUKEY: &str = "Tukey, J. W. (1949). Comparing individual means in the analysis of variance. Biometrics 5, 99-114; Kramer, C. Y. (1956). Extension of multiple range tests to group means with unequal numbers of replications. Biometrics 12, 307-310.";
const C_WELCHA: &str = "Welch, B. L. (1951). On the comparison of several mean values: an alternative approach. Biometrika 38, 330-336.";
const C_LEVENE: &str = "Brown, M. B. and Forsythe, A. B. (1974). Robust tests for the equality of variances. Journal of the American Statistical Association 69, 364-367; Levene, H. (1960). In Contributions to Probability and Statistics, Stanford University Press.";
const C_CHI: &str = "Pearson, K. (1900). On the criterion that a given system of deviations from the probable in the case of a correlated system of variables is such that it can be reasonably supposed to have arisen from random sampling. Philosophical Magazine 50, 157-175.";
const C_CRAMER: &str = "Cramer, H. (1946). Mathematical Methods of Statistics. Princeton University Press (V); Cohen, J. (1988). Statistical Power Analysis for the Behavioral Sciences, 2nd ed. (w); the interval inverts the non-central chi-square distribution (Smithson 2003).";
const C_WILSON: &str = "Wilson, E. B. (1927). Probable inference, the law of succession, and statistical inference. Journal of the American Statistical Association 22, 209-212.";
const C_CP: &str = "Clopper, C. J. and Pearson, E. S. (1934). The use of confidence or fiducial limits illustrated in the case of the binomial. Biometrika 26, 404-413.";
const C_NEWCOMBE: &str = "Newcombe, R. G. (1998). Interval estimation for the difference between independent proportions: comparison of eleven methods. Statistics in Medicine 17, 873-890 (method 10).";
const C_MW: &str = "Mann, H. B. and Whitney, D. R. (1947). On a test of whether one of two random variables is stochastically larger than the other. Annals of Mathematical Statistics 18, 50-60; Hanley, J. A. and McNeil, B. J. (1982). Radiology 143, 29-36 (interval of the area under the curve, here converted to the rank-biserial correlation).";
const C_WSR: &str = "Wilcoxon, F. (1945). Individual comparisons by ranking methods. Biometrics Bulletin 1, 80-83.";
const C_KW: &str = "Kruskal, W. H. and Wallis, W. A. (1952). Use of ranks in one-criterion variance analysis. Journal of the American Statistical Association 47, 583-621; Tomczak, M. and Tomczak, E. (2014). Trends in Sport Sciences 21(1), 19-25 (epsilon-squared).";
const C_PEARSON: &str = "Fisher, R. A. (1921). On the 'probable error' of a coefficient of correlation deduced from a small sample. Metron 1, 3-32 (Fisher z interval).";
const C_SPEARMAN: &str = "Spearman, C. (1904). American Journal of Psychology 15, 72-101; Bonett, D. G. and Wright, T. A. (2000). Sample size requirements for estimating Pearson, Kendall and Spearman correlations. Psychometrika 65, 23-28.";
const C_KENDALL: &str = "Kendall, M. G. (1938). A new measure of rank correlation. Biometrika 30, 81-93; Kendall, M. G. (1970). Rank Correlation Methods, 4th ed. (tie-corrected variance); Fieller, E. C., Hartley, H. O. and Pearson, E. S. (1957). Biometrika 44, 470-481 (interval).";
const C_HOLM: &str = "Holm, S. (1979). A simple sequentially rejective multiple test procedure. Scandinavian Journal of Statistics 6, 65-70.";
const C_BH: &str = "Benjamini, Y. and Hochberg, Y. (1995). Controlling the false discovery rate. Journal of the Royal Statistical Society B 57, 289-300.";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(TTestTool), Box::new(AnovaTool), Box::new(ChiSqTool), Box::new(ProportionTool), Box::new(RankTool), Box::new(ShapiroTool), Box::new(LeveneTool), Box::new(CorrelationTool)]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "stat_ttest" => &["t-test", "student", "welch", "paired", "compare means", "difference of means", "hypothesis test", "p-value"],
        "stat_anova" => &["anova", "analysis of variance", "tukey", "post hoc", "compare groups", "f test", "welch anova", "eta squared"],
        "stat_chisq" => &["chi-square", "chi squared", "independence", "goodness of fit", "contingency", "cramer", "association"],
        "stat_proportion" => &["proportion", "binomial", "rate", "percentage test", "wilson", "clopper pearson", "compare proportions"],
        "stat_rank_test" => &["mann-whitney", "wilcoxon", "kruskal", "non-parametric", "rank test", "signed rank", "rank sum"],
        "stat_shapiro" => &["normality", "shapiro", "wilk", "normal distribution test"],
        "stat_levene" => &["levene", "equal variances", "homogeneity of variance", "brown-forsythe", "homoscedasticity"],
        "stat_correlation" => &["correlation", "pearson", "spearman", "kendall", "association", "r value", "rank correlation"],
        _ => &[],
    }
}

// ------------------------------------------------------------------ helpers

fn conf_of(p: &Params) -> Result<f64> {
    let c = p.f64("conf").unwrap_or(0.95);
    if !(c > 0.5 && c < 1.0) {
        bail!("'Confidence level' must be between 0.5 and 1 (for example 0.95)");
    }
    Ok(c)
}

fn pct(c: f64) -> String {
    format!("{}", (c * 1000.0).round() / 10.0)
}

fn spec_input() -> Vec<Port> {
    vec![Port { id: "in", label: "Input", kind: PortKind::Table }]
}

fn input<'a>(inputs: &'a Inputs, name: &str) -> Result<&'a Rel> {
    inputs.get("in").ok_or_else(|| anyhow!("{name} needs an input table"))
}

fn conf_param() -> Param {
    Param::new("conf", "Confidence level", ParamKind::Number).default(0.95).help("For the interval beside each estimate, for example 0.95.")
}

fn alt_param() -> Param {
    Param::new("alternative", "Alternative", ParamKind::Enum).default("two.sided").options(&[("two.sided", "Two-sided"), ("less", "One-sided: less"), ("greater", "One-sided: greater")])
}

fn s_err(e: String) -> anyhow::Error {
    anyhow!(e)
}

struct Grouped {
    labels: Vec<String>,
    vals: Vec<Vec<f64>>,
    dropped: Vec<Dropped>,
    used: usize,
}

/// Values of a numeric field split by the levels of a group field. Rows with a missing value or group are dropped and counted.
fn grouped(ctx: &Ctx, rel: &Rel, value: &str, group: &str) -> Result<Grouped> {
    if value == group {
        bail!("The value field and the group field must be different");
    }
    let data = load(ctx, rel, &[(value, true), (group, false)])?;
    let (vc, gc) = (&data.cols[0], &data.cols[1]);
    let mut map: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut no_group = 0u64;
    for r in 0..data.rows {
        match (vc.num[r], &gc.txt[r]) {
            (Some(v), Some(g)) => map.entry(g.clone()).or_default().push(v),
            (Some(_), None) => no_group += 1,
            _ => {}
        }
    }
    let mut labels: Vec<String> = map.keys().cloned().collect();
    sort_levels(&mut labels);
    if labels.len() > 500 {
        bail!("'{group}' has {} groups; at most 500 can be compared at a time. Group the values first.", labels.len());
    }
    let vals: Vec<Vec<f64>> = labels.iter().map(|l| map[l].clone()).collect();
    let mut dropped = drop_reasons(vc, value);
    if no_group > 0 {
        dropped.push(Dropped { reason: format!("{group}: no group value (rows with a {value} value)"), count: no_group });
    }
    let used = vals.iter().map(|v| v.len()).sum();
    Ok(Grouped { labels, vals, dropped, used })
}

/// Two numeric fields, rows where both are numbers.
fn pairs(ctx: &Ctx, rel: &Rel, x: &str, y: &str) -> Result<(Vec<f64>, Vec<f64>, Vec<Dropped>)> {
    if x == y {
        bail!("Choose two different fields");
    }
    let data = load(ctx, rel, &[(x, true), (y, true)])?;
    let (mut a, mut b) = (vec![], vec![]);
    let (mut miss_x, mut miss_y) = (0u64, 0u64);
    for r in 0..data.rows {
        match (data.cols[0].num[r], data.cols[1].num[r]) {
            (Some(p), Some(q)) => {
                a.push(p);
                b.push(q);
            }
            (None, _) => miss_x += 1,
            (Some(_), None) => miss_y += 1,
        }
    }
    let mut dropped = vec![];
    if miss_x > 0 {
        dropped.push(Dropped { reason: format!("{x}: empty, missing or not a finite number"), count: miss_x });
    }
    if miss_y > 0 {
        dropped.push(Dropped { reason: format!("{y}: empty, missing or not a finite number (rows with {x} present)"), count: miss_y });
    }
    Ok((a, b, dropped))
}

/// Add a note per sample that departs from normality at 0.05, or one reassuring note when none does.
fn normality_notes(samples: &[(String, &[f64])], follow_up: &str, notes: &mut Vec<String>) {
    let mut checked = 0;
    let mut flagged = 0;
    for (label, x) in samples {
        if let Ok((w, p)) = shapiro_wilk(x) {
            checked += 1;
            if p < 0.05 {
                flagged += 1;
                notes.push(format!("Normality check (Shapiro-Wilk) on {label}: W = {w:.3}, p = {p:.4}. The values depart from normality; {follow_up}"));
            }
        }
    }
    if checked > 0 && flagged == 0 {
        notes.push(format!("Normality check (Shapiro-Wilk, 3 or more values): no sample departs from normality at the 0.05 level ({checked} checked). With very few values this check has little power."));
    } else if checked == 0 {
        notes.push("Normality could not be checked: each sample has fewer than 3 values.".into());
    }
}

fn finite(v: f64) -> Value {
    num(v)
}

// -------------------------------------------------------------------- t-test

struct TTestTool;

impl Tool for TTestTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_ttest",
            name: "t-Test",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Student's t-test of means: one sample against a value, two groups of a field (Welch's unequal-variance test by default, or the pooled-variance test), or two paired fields. Reports the t statistic, degrees of freedom, p-value, the difference with its confidence interval, and Hedges' g with an interval. Checks each sample with the Shapiro-Wilk test and notes departures.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("mode", "Test", ParamKind::Enum).default("two").options(&[("one", "One sample against a value"), ("two", "Two groups (by a group field)"), ("paired", "Paired fields")]),
                Param::new("value", "Value field", ParamKind::Field).field_of("in").help("The numeric field (the first field for paired data)."),
                Param::new("group", "Group field", ParamKind::Field).field_of("in").optional().help("Two groups: a field with exactly two values."),
                Param::new("value2", "Second field", ParamKind::Field).field_of("in").optional().help("Paired data: the field measured second."),
                Param::new("mu", "Value to compare with", ParamKind::Number).default(0).help("One sample only."),
                Param::new("variance", "Variances", ParamKind::Enum).default("welch").options(&[("welch", "Unequal (Welch, recommended)"), ("pooled", "Equal (Student, pooled)")]).help("Two groups only."),
                alt_param(),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The t-test")?;
        let value = p.req_str("value", "Value field")?;
        let mode = p.str("mode").unwrap_or_else(|| "two".into());
        let alt = Alt::parse(p.str("alternative").as_deref());
        let conf = conf_of(p)?;
        let mu = p.f64("mu").unwrap_or(0.0);
        let pooled = p.str("variance").as_deref() == Some("pooled");
        let mut notes = vec![];
        let (res, label, n1, n2, m1, m2, method, dropped, used);
        match mode.as_str() {
            "one" => {
                let d = load(ctx, rel, &[(&value, true)])?;
                let xs: Vec<f64> = d.cols[0].num.iter().flatten().copied().collect();
                res = t_one(&xs, mu, alt, conf).map_err(s_err)?;
                label = format!("mean of {value} vs {mu}");
                (n1, n2, m1, m2) = (xs.len(), None, mean(&xs), None);
                method = format!("One-sample Student t-test of the mean against {mu}, {}", alt.label());
                dropped = drop_reasons(&d.cols[0], &value);
                used = xs.len();
                normality_notes(&[(value.clone(), &xs)], "consider the Wilcoxon signed-rank test.", &mut notes);
                notes.push("Estimate = mean - value compared with; Hedges' g = (mean - value) / sd with the small-sample correction.".into());
            }
            "paired" => {
                let v2 = p.req_str("value2", "Second field")?;
                let (a, b, dr) = pairs(ctx, rel, &value, &v2)?;
                let diff: Vec<f64> = a.iter().zip(&b).map(|(x, y)| x - y).collect();
                res = t_one(&diff, 0.0, alt, conf).map_err(s_err)?;
                label = format!("{value} - {v2}");
                (n1, n2, m1, m2) = (a.len(), Some(b.len()), mean(&a), Some(mean(&b)));
                method = format!("Paired Student t-test on the differences {value} - {v2}, {}", alt.label());
                dropped = dr;
                used = a.len();
                normality_notes(&[("the paired differences".to_string(), &diff)], "consider the Wilcoxon signed-rank test.", &mut notes);
                notes.push("Estimate = mean paired difference; Hedges' g is the standardised mean difference of the pairs (d_z = mean difference / sd of the differences, corrected for small samples).".into());
            }
            _ => {
                let group = p.req_str("group", "Group field")?;
                let g = grouped(ctx, rel, &value, &group)?;
                if g.labels.len() != 2 {
                    bail!("A two-group t-test needs a group field with exactly 2 values; '{group}' has {}: {}. Use One-way ANOVA for more groups.", g.labels.len(), g.labels.join(", "));
                }
                res = t_two(&g.vals[0], &g.vals[1], pooled, alt, conf).map_err(s_err)?;
                label = format!("{} - {}", g.labels[0], g.labels[1]);
                (n1, n2, m1, m2) = (g.vals[0].len(), Some(g.vals[1].len()), mean(&g.vals[0]), Some(mean(&g.vals[1])));
                method = if pooled { format!("Two-sample Student t-test with pooled variance, {}", alt.label()) } else { format!("Welch two-sample t-test (unequal variances), {}", alt.label()) };
                normality_notes(&[(g.labels[0].clone(), &g.vals[0]), (g.labels[1].clone(), &g.vals[1])], "consider the Mann-Whitney test.", &mut notes);
                if pooled {
                    if let Ok(l) = levene(&g.vals, true) {
                        if l.p < 0.05 {
                            notes.push(format!("Levene (Brown-Forsythe) test of equal variances: F = {:.3}, p = {:.4}. The variances differ, so the pooled test is not appropriate; use Welch.", l.f, l.p));
                        }
                    }
                } else {
                    notes.push("Welch's test does not assume equal variances (Delacre et al. 2017). Estimate = mean of the first group - mean of the second; Hedges' g uses the pooled standard deviation with the small-sample correction.".into());
                }
                dropped = g.dropped;
                used = g.used;
            }
        }
        let mut cols = vec![col("comparison", K::Text), col("n1", K::Integer), col("n2", K::Integer)];
        for n in ["mean1", "mean2", "estimate", "ci_low", "ci_high", "t", "df", "p", "hedges_g", "g_ci_low", "g_ci_high"] {
            cols.push(col(n, K::Number));
        }
        let row = vec![json!(label), json!(n1), n2.map_or(Value::Null, |v| json!(v)), num(m1), opt(m2), num(res.estimate), finite(res.ci.0), finite(res.ci.1), num(res.t), num(res.df), num(res.p), num(res.g), num(res.g_ci.0), num(res.g_ci.1)];
        let mut r = result(method, &[C_DELACRE, C_WELCH, C_HEDGES, C_SW]);
        r.tables.push(table("t-test", cols, vec![row]));
        r.n_used = Some(used as u64);
        r.dropped = dropped;
        r.notes = notes;
        r.notes.push(format!("The interval is a {} % confidence interval for the estimate; the interval for g is approximate (normal, Hedges-Olkin standard error). The test assumes independent observations.", pct(conf)));
        finish(ctx, rel, r, true)
    }
}

// --------------------------------------------------------------------- ANOVA

struct AnovaTool;

impl Tool for AnovaTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_anova",
            name: "One-way ANOVA",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Compares the means of a numeric field across the groups of another field: the F test with eta-squared (and its interval) and omega-squared, Welch's ANOVA for unequal variances, and Tukey HSD all-pairs comparisons with adjusted p-values and intervals. Notes Levene's test of equal variances and Shapiro-Wilk normality per group.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("value", "Value field", ParamKind::Field).field_of("in"),
                Param::new("group", "Group field", ParamKind::Field).field_of("in"),
                Param::new("posthoc", "Pairwise comparisons", ParamKind::Enum).default("tukey").options(&[("tukey", "Tukey HSD"), ("none", "None")]),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The ANOVA")?;
        let (value, group) = (p.req_str("value", "Value field")?, p.req_str("group", "Group field")?);
        let conf = conf_of(p)?;
        let g = grouped(ctx, rel, &value, &group)?;
        if g.labels.len() < 2 {
            bail!("'{group}' has only {} value; ANOVA needs at least 2 groups.", g.labels.len());
        }
        let a = anova(&g.vals, conf).map_err(s_err)?;
        let mut r = result(
            format!("One-way analysis of variance of {value} by {group} ({} groups): F test with eta-squared and omega-squared; Welch ANOVA; Tukey-Kramer HSD pairwise comparisons.", g.labels.len()),
            &[C_ANOVA, C_ETA, C_WELCHA, C_TUKEY],
        );
        let n_tot: f64 = g.used as f64;
        let rows = vec![
            vec![json!("Between groups"), num(a.df1), num(a.ss_between), num(a.ms_between), num(a.f), num(a.p), num(a.eta2), num(a.eta2_ci.0), num(a.eta2_ci.1), num(a.omega2)],
            vec![json!("Within groups"), num(a.df2), num(a.ss_within), num(a.ms_within), Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, Value::Null],
        ];
        let mut cols = vec![col("source", K::Text)];
        for n in ["df", "sum_sq", "mean_sq", "F", "p", "eta2", "eta2_ci_low", "eta2_ci_high", "omega2"] {
            cols.push(col(n, K::Number));
        }
        r.tables.push(table("ANOVA", cols, rows));
        let summary: Vec<Vec<Value>> = g.labels.iter().zip(&g.vals).map(|(l, v)| vec![json!(l), json!(v.len()), num(mean(v)), opt(variance(v).map(f64::sqrt))]).collect();
        r.tables.push(table("Group summary", vec![col("group", K::Text), col("n", K::Integer), col("mean", K::Number), col("sd", K::Number)], summary));
        match welch_anova(&g.vals) {
            Ok(w) => r.tables.push(table("Welch ANOVA", vec![col("F", K::Number), col("df1", K::Number), col("df2", K::Number), col("p", K::Number)], vec![vec![num(w.f), num(w.df1), num(w.df2), num(w.p)]])),
            Err(e) => r.notes.push(format!("Welch ANOVA not computed: {e}")),
        }
        if p.str("posthoc").as_deref() != Some("none") {
            let t = tukey(&g.vals, conf).map_err(s_err)?;
            let rows = t.iter().map(|x| vec![json!(format!("{} - {}", g.labels[x.b], g.labels[x.a])), num(x.diff), num(x.ci.0), num(x.ci.1), num(x.p_adj)]).collect();
            r.tables.push(table("Tukey HSD", vec![col("comparison", K::Text), col("difference", K::Number), col("ci_low", K::Number), col("ci_high", K::Number), col("p_adjusted", K::Number)], rows));
            r.notes.push(format!("Tukey HSD: p-values and {} % intervals are adjusted for all {} pairwise comparisons together; each difference is the later group minus the earlier.", pct(conf), t.len()));
        }
        let samples: Vec<(String, &[f64])> = g.labels.iter().cloned().zip(g.vals.iter().map(|v| v.as_slice())).collect();
        normality_notes(&samples, "consider the Kruskal-Wallis test.", &mut r.notes);
        if let Ok(l) = levene(&g.vals, true) {
            if l.p < 0.05 {
                r.notes.push(format!("Levene (Brown-Forsythe) test of equal variances: F = {:.3}, p = {:.4}. The group variances differ; prefer the Welch ANOVA row, and treat Tukey intervals with care.", l.f, l.p));
            } else {
                r.notes.push(format!("Levene (Brown-Forsythe) test of equal variances: F = {:.3}, p = {:.4}; no evidence the variances differ.", l.f, l.p));
            }
        }
        r.notes.push(format!("Eta-squared = share of the total variation explained by the groups (interval from the non-central F distribution, {} %); omega-squared is its less biased version. N = {n_tot}.", pct(conf)));
        r.n_used = Some(g.used as u64);
        r.dropped = g.dropped;
        finish(ctx, rel, r, true)
    }
}

// ---------------------------------------------------------------- chi-square

struct ChiSqTool;

impl Tool for ChiSqTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_chisq",
            name: "Chi-square Test",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Pearson chi-square test. Independence: are two categorical fields associated (counts table, expected counts, standardised residuals, Cramer's V with an interval). Goodness of fit: do the values of one field follow given proportions (default equal), with Cohen's w. Warns when expected counts are below 5.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("mode", "Test", ParamKind::Enum).default("independence").options(&[("independence", "Independence of two fields"), ("gof", "Goodness of fit of one field")]),
                Param::new("field_a", "Field", ParamKind::Field).field_of("in").help("The row field, or the field for goodness of fit."),
                Param::new("field_b", "Second field", ParamKind::Field).field_of("in").optional().help("Independence only: the column field."),
                Param::new("expected", "Expected proportions", ParamKind::Text).optional().help("Goodness of fit only: one number per value in the order shown in the result (numbers or text order of the values), separated by commas; they are rescaled to add to 1. Empty means equal proportions."),
                Param::new("yates", "Yates continuity correction (2 x 2)", ParamKind::Bool).help("Off gives the plain Pearson statistic."),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The chi-square test")?;
        let a = p.req_str("field_a", "Field")?;
        let conf = conf_of(p)?;
        let mode = p.str("mode").unwrap_or_else(|| "independence".into());
        let mut r;
        if mode == "gof" {
            let known = ctx.columns(rel)?;
            if !known.iter().any(|c| c == &a) {
                bail!("The input has no field named '{a}'. Fields are: {}", known.join(", "));
            }
            let (t, q) = (ident(&rel.table), ident(&a));
            let clean = format!("({q} IS NOT NULL AND trim(CAST({q} AS VARCHAR)) <> '')");
            let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
            let mut stmt = ctx.conn.prepare(&format!("SELECT CAST({q} AS VARCHAR), count(*) FROM {t} WHERE {clean} GROUP BY 1"))?;
            let cells: Vec<(String, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
            if cells.len() > MAX_LEVELS {
                bail!("'{a}' has {} different values; this test is for categories (at most {MAX_LEVELS}).", cells.len());
            }
            let mut levels: Vec<String> = cells.iter().map(|c| c.0.clone()).collect();
            sort_levels(&mut levels);
            let observed: Vec<f64> = levels.iter().map(|l| cells.iter().find(|c| &c.0 == l).unwrap().1 as f64).collect();
            let props: Vec<f64> = match p.str("expected") {
                Some(s) => s.split(',').map(|v| v.trim().parse::<f64>().map_err(|_| anyhow!("'{}' in 'Expected proportions' is not a number", v.trim()))).collect::<Result<_>>()?,
                None => vec![1.0; levels.len()],
            };
            let c = chisq_gof(&observed, &props, conf).map_err(s_err)?;
            r = result(format!("Pearson chi-square goodness-of-fit test of {a} against {}", if p.str("expected").is_some() { "the given proportions (rescaled to sum to 1)" } else { "equal proportions" }), &[C_CHI, C_CRAMER]);
            r.tables.push(test_table(&c, "Cohen's w", conf));
            let rows = levels.iter().enumerate().map(|(i, l)| vec![json!(l), num(observed[i]), num(c.expected[0][i]), num((observed[i] - c.expected[0][i]) / c.expected[0][i].sqrt())]).collect();
            r.tables.push(table("Observed and expected", vec![col("value", K::Text), col("observed", K::Number), col("expected", K::Number), col("std_residual", K::Number)], rows));
            let used: f64 = observed.iter().sum();
            r.n_used = Some(used as u64);
            if total as f64 > used {
                r.dropped.push(Dropped { reason: format!("{a}: empty or missing"), count: (total as f64 - used) as u64 });
            }
            low_expected(&c, &mut r.notes);
        } else {
            let b = p.req_str("field_b", "Second field")?;
            if a == b {
                bail!("Choose two different fields");
            }
            let ct = contingency(ctx, rel, &a, &b)?;
            let c = chisq_independence(&ct.counts, p.bool("yates"), conf).map_err(s_err)?;
            let yates = p.bool("yates") && ct.row_levels.len() == 2 && ct.col_levels.len() == 2;
            r = result(format!("Pearson chi-square test of independence of {a} and {b}{} ({} x {} table)", if yates { " with Yates' continuity correction" } else { "" }, ct.row_levels.len(), ct.col_levels.len()), &[C_CHI, C_CRAMER]);
            r.tables.push(test_table(&c, "Cramer's V", conf));
            let mut rows = vec![];
            for (i, rl) in ct.row_levels.iter().enumerate() {
                for (j, cl) in ct.col_levels.iter().enumerate() {
                    let (o, e) = (ct.counts[i][j], c.expected[i][j]);
                    rows.push(vec![json!(rl), json!(cl), num(o), num(e), num((o - e) / e.sqrt())]);
                }
            }
            r.tables.push(table("Observed and expected", vec![col(&a, K::Text), col(&b, K::Text), col("observed", K::Number), col("expected", K::Number), col("std_residual", K::Number)], rows));
            r.n_used = Some(ct.used as u64);
            r.dropped = ct.dropped;
            low_expected(&c, &mut r.notes);
            r.notes.push("Standardised residual = (observed - expected) / sqrt(expected); values beyond about 2 show which cells drive the association.".into());
        }
        r.notes.push("Cramer's V and Cohen's w are effect sizes (Cohen's guide for w: 0.1 small, 0.3 medium, 0.5 large); each interval comes from the non-central chi-square distribution and is approximate. Fisher's exact test is not provided: with small expected counts, combine categories.".into());
        r.notes.push(format!("Intervals are {} % intervals.", pct(conf)));
        finish(ctx, rel, r, true)
    }
}

fn test_table(c: &ChiSq, effect_name: &str, _conf: f64) -> crate::stats::StatTable {
    table(
        "Chi-square test",
        vec![col("n", K::Number), col("chi_square", K::Number), col("df", K::Number), col("p", K::Number), col(effect_name, K::Number), col("ci_low", K::Number), col("ci_high", K::Number), col("min_expected", K::Number), col("share_expected_below_5", K::Number)],
        vec![vec![num(c.n), num(c.chi2), num(c.df), num(c.p), num(c.effect), num(c.effect_ci.0), num(c.effect_ci.1), num(c.min_expected), num(c.share_below_5)]],
    )
}

fn low_expected(c: &ChiSq, notes: &mut Vec<String>) {
    if c.share_below_5 > 0.2 || c.min_expected < 1.0 {
        notes.push(format!("{:.0} % of the expected counts are below 5 (smallest {:.2}). The chi-square approximation is unreliable here; combine sparse categories before relying on the p-value.", 100.0 * c.share_below_5, c.min_expected));
    }
}

// --------------------------------------------------------------- proportions

struct ProportionTool;

impl Tool for ProportionTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_proportion",
            name: "Proportion Test",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Tests proportions of rows with a chosen outcome value. One group: exact binomial test against a proportion, with Clopper-Pearson and Wilson intervals and Cohen's h. Several groups: chi-square test that the proportions are equal, a Wilson interval per group, and for two groups the difference with Newcombe's interval and Cohen's h.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("outcome", "Outcome field", ParamKind::Field).field_of("in"),
                Param::new("success", "Count as success", ParamKind::Text).help("The value of the outcome field that counts as a success (compared as text, for example 1, yes or forest; a true/false field uses true or false)."),
                Param::new("group", "Group field", ParamKind::Field).field_of("in").optional().help("Empty: one group of all rows."),
                Param::new("p0", "Proportion to compare with", ParamKind::Number).default(0.5).help("One group only."),
                alt_param(),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The proportion test")?;
        let outcome = p.req_str("outcome", "Outcome field")?;
        let success = p.req_str("success", "Count as success")?;
        let conf = conf_of(p)?;
        let group = p.str("group");
        let mut wanted: Vec<(&str, bool)> = vec![(&outcome, false)];
        if let Some(g) = &group {
            wanted.push((g, false));
        }
        let d = load(ctx, rel, &wanted)?;
        let mut order: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let (mut no_outcome, mut no_group) = (0u64, 0u64);
        for r in 0..d.rows {
            let Some(o) = &d.cols[0].txt[r] else {
                no_outcome += 1;
                continue;
            };
            let gl = match &group {
                Some(_) => match &d.cols[1].txt[r] {
                    Some(g) => g.clone(),
                    None => {
                        no_group += 1;
                        continue;
                    }
                },
                None => "all rows".to_string(),
            };
            let e = order.entry(gl).or_insert((0.0, 0.0));
            e.1 += 1.0;
            if o.trim() == success {
                e.0 += 1.0;
            }
        }
        let mut labels: Vec<String> = order.keys().cloned().collect();
        sort_levels(&mut labels);
        if labels.is_empty() {
            bail!("No row has an outcome value, so there is nothing to test.");
        }
        let x: Vec<f64> = labels.iter().map(|l| order[l].0).collect();
        let n: Vec<f64> = labels.iter().map(|l| order[l].1).collect();
        let used: f64 = n.iter().sum();
        let mut r;
        let alt = Alt::parse(p.str("alternative").as_deref());
        if labels.len() == 1 {
            let p0 = p.f64("p0").unwrap_or(0.5);
            let b = binom_test(x[0] as u64, n[0] as u64, p0, alt, conf).map_err(s_err)?;
            r = result(format!("Exact binomial test that the proportion of {outcome} = {success} is {p0}, {}", alt.label()), &[C_CP, C_WILSON]);
            r.tables.push(table(
                "Binomial test",
                vec![col("successes", K::Integer), col("n", K::Integer), col("proportion", K::Number), col("p0", K::Number), col("p", K::Number), col("ci_low", K::Number), col("ci_high", K::Number), col("wilson_low", K::Number), col("wilson_high", K::Number), col("cohens_h", K::Number)],
                vec![vec![json!(x[0] as i64), json!(n[0] as i64), num(b.estimate), num(p0), num(b.p), num(b.ci_exact.0), num(b.ci_exact.1), num(b.ci_wilson.0), num(b.ci_wilson.1), num(b.h)]],
            ));
            r.notes.push(format!("ci_low/ci_high are the Clopper-Pearson exact {} % interval (conservative); the Wilson interval is shown beside it. Cohen's h = 2 asin(sqrt(p)) - 2 asin(sqrt(p0)) is the effect size (0.2 small, 0.5 medium, 0.8 large).", pct(conf)));
        } else {
            let e = prop_equal(&x, &n).map_err(s_err)?;
            r = result(format!("Chi-square test that the proportion of {outcome} = {success} is equal in the {} groups of {}", labels.len(), group.clone().unwrap_or_default()), &[C_CHI, C_WILSON, C_NEWCOMBE]);
            let mut rows = vec![];
            let mut cols = vec![col("groups", K::Integer), col("chi_square", K::Number), col("df", K::Number), col("p", K::Number)];
            let mut row = vec![json!(labels.len()), num(e.chi2), num(e.df), num(e.p)];
            if labels.len() == 2 {
                let (diff, ci) = newcombe_diff(x[0], n[0], x[1], n[1], conf);
                let h = 2.0 * (x[0] / n[0]).sqrt().asin() - 2.0 * (x[1] / n[1]).sqrt().asin();
                for nme in ["difference", "ci_low", "ci_high", "cohens_h"] {
                    cols.push(col(nme, K::Number));
                }
                row.extend([num(diff), num(ci.0), num(ci.1), num(h)]);
                r.notes.push(format!("Difference = proportion in {} minus proportion in {}, with Newcombe's hybrid score {} % interval; Cohen's h is the effect size.", labels[0], labels[1], pct(conf)));
            } else {
                r.notes.push("The test says whether any proportion differs; follow up pairs with the Proportion Test on two groups at a time and adjust the p-values for the number of pairs.".into());
            }
            rows.push(row);
            r.tables.push(table("Chi-square test of equal proportions", cols, rows));
            if n.iter().any(|v| *v < 10.0) || (0..x.len()).any(|i| (n[i] * x.iter().sum::<f64>() / n.iter().sum::<f64>()) < 5.0 || (n[i] * (1.0 - x.iter().sum::<f64>() / n.iter().sum::<f64>())) < 5.0) {
                r.notes.push("Some group has an expected count below 5 successes or failures; the chi-square approximation is unreliable there.".into());
            }
        }
        let rows = labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let w = wilson(x[i], n[i], conf);
                vec![json!(l), json!(x[i] as i64), json!(n[i] as i64), num(x[i] / n[i]), num(w.0), num(w.1)]
            })
            .collect();
        r.tables.push(table("Proportions by group", vec![col("group", K::Text), col("successes", K::Integer), col("n", K::Integer), col("proportion", K::Number), col("wilson_low", K::Number), col("wilson_high", K::Number)], rows));
        r.n_used = Some(used as u64);
        if no_outcome > 0 {
            r.dropped.push(Dropped { reason: format!("{outcome}: empty or missing"), count: no_outcome });
        }
        if no_group > 0 {
            r.dropped.push(Dropped { reason: format!("{}: no group value (rows with an outcome)", group.clone().unwrap_or_default()), count: no_group });
        }
        finish(ctx, rel, r, true)
    }
}

// ----------------------------------------------------------------- rank tests

struct RankTool;

impl Tool for RankTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_rank_test",
            name: "Rank Tests",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Distribution-free tests that need no normality: Mann-Whitney (two groups), Wilcoxon signed-rank (one sample against a value, or two paired fields) and Kruskal-Wallis (several groups). Exact p-values for small samples without ties, otherwise the normal approximation with continuity and tie corrections. Reports the rank-biserial correlation (with an interval) or epsilon-squared as the effect size.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("mode", "Test", ParamKind::Enum).default("mw").options(&[("mw", "Mann-Whitney (two groups)"), ("signed", "Wilcoxon signed-rank (one sample or paired)"), ("kruskal", "Kruskal-Wallis (several groups)")]),
                Param::new("value", "Value field", ParamKind::Field).field_of("in"),
                Param::new("group", "Group field", ParamKind::Field).field_of("in").optional().help("Mann-Whitney and Kruskal-Wallis."),
                Param::new("value2", "Second field", ParamKind::Field).field_of("in").optional().help("Signed-rank on paired data: leave empty to test one field against a value."),
                Param::new("mu", "Value to compare with", ParamKind::Number).default(0).help("Signed-rank, one field."),
                alt_param(),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The rank test")?;
        let value = p.req_str("value", "Value field")?;
        let mode = p.str("mode").unwrap_or_else(|| "mw".into());
        let alt = Alt::parse(p.str("alternative").as_deref());
        let conf = conf_of(p)?;
        let mut r;
        let mut notes = vec![];
        let (dropped, used);
        let cols_two = |stat: &str| {
            let mut c = vec![col("comparison", K::Text), col("n1", K::Integer), col("n2", K::Integer), col(stat, K::Number), col("z", K::Number), col("p", K::Number), col("exact", K::Bool), col("rank_biserial", K::Number), col("ci_low", K::Number), col("ci_high", K::Number)];
            c.shrink_to_fit();
            c
        };
        match mode.as_str() {
            "mw" => {
                let group = p.req_str("group", "Group field")?;
                let g = grouped(ctx, rel, &value, &group)?;
                if g.labels.len() != 2 {
                    bail!("Mann-Whitney needs a group field with exactly 2 values; '{group}' has {}: {}. Use Kruskal-Wallis for more groups.", g.labels.len(), g.labels.join(", "));
                }
                let t = mann_whitney(&g.vals[0], &g.vals[1], alt, conf).map_err(s_err)?;
                r = result(format!("Mann-Whitney U (Wilcoxon rank-sum) test, {}, {}", alt.label(), if t.exact { "exact p-value" } else { "normal approximation with continuity and tie corrections" }), &[C_MW]);
                r.tables.push(table(
                    "Mann-Whitney test",
                    cols_two("W"),
                    vec![vec![json!(format!("{} vs {}", g.labels[0], g.labels[1])), json!(g.vals[0].len()), json!(g.vals[1].len()), num(t.stat), opt(t.z), num(t.p), json!(t.exact), num(t.effect), opt(t.effect_ci.map(|c| c.0)), opt(t.effect_ci.map(|c| c.1))]],
                ));
                notes.push(format!("W counts the pairs (one value from each group) in which the first group's value is larger (ties count one half). Rank-biserial correlation = 2W/(n1 n2) - 1: +1 means every value in {} exceeds every value in {}; its interval comes from the Hanley-McNeil standard error and is approximate.", g.labels[0], g.labels[1]));
                if t.ties {
                    notes.push("There are tied values, so the exact test is not used.".into());
                }
                notes.push("The test compares whole distributions; it reads as a test of medians only when the two distributions have the same shape.".into());
                dropped = g.dropped;
                used = g.used;
            }
            "signed" => {
                let (d, dr, label, n2) = if let Some(v2) = p.str("value2") {
                    let (a, b, dr) = pairs(ctx, rel, &value, &v2)?;
                    (a.iter().zip(&b).map(|(x, y)| x - y).collect::<Vec<f64>>(), dr, format!("{value} - {v2}"), Some(b.len()))
                } else {
                    let mu = p.f64("mu").unwrap_or(0.0);
                    let data = load(ctx, rel, &[(&value, true)])?;
                    (data.cols[0].num.iter().flatten().map(|v| v - mu).collect(), drop_reasons(&data.cols[0], &value), format!("{value} - {mu}"), None)
                };
                let (t, zeros) = signed_rank(&d, alt, conf).map_err(s_err)?;
                r = result(format!("Wilcoxon signed-rank test on {label}, {}, {}", alt.label(), if t.exact { "exact p-value" } else { "normal approximation with continuity and tie corrections" }), &[C_WSR]);
                r.tables.push(table(
                    "Wilcoxon signed-rank test",
                    cols_two("V"),
                    vec![vec![json!(label), json!(d.len()), n2.map_or(Value::Null, |v| json!(v)), num(t.stat), opt(t.z), num(t.p), json!(t.exact), num(t.effect), opt(t.effect_ci.map(|c| c.0)), opt(t.effect_ci.map(|c| c.1))]],
                ));
                notes.push("V = sum of the ranks of the positive differences. Effect size: matched-pairs rank-biserial correlation (sum of positive ranks minus sum of negative ranks) / total; its interval uses the null-hypothesis variance of V and is approximate.".into());
                if zeros > 0 {
                    notes.push(format!("{zeros} difference(s) were exactly 0 and were left out of the ranking (the usual convention); the exact test is then not used."));
                    r.dropped.push(Dropped { reason: "difference exactly 0 (not ranked)".into(), count: zeros as u64 });
                }
                dropped = dr;
                used = d.len();
            }
            _ => {
                let group = p.req_str("group", "Group field")?;
                let g = grouped(ctx, rel, &value, &group)?;
                if g.labels.len() < 2 {
                    bail!("Kruskal-Wallis needs at least 2 groups.");
                }
                let t = kruskal(&g.vals).map_err(s_err)?;
                r = result(format!("Kruskal-Wallis rank-sum test of {value} across {} groups of {group}, tie-corrected, chi-square approximation", g.labels.len()), &[C_KW]);
                r.tables.push(table(
                    "Kruskal-Wallis test",
                    vec![col("groups", K::Integer), col("H", K::Number), col("df", K::Number), col("p", K::Number), col("epsilon_squared", K::Number)],
                    vec![vec![json!(g.labels.len()), num(t.stat), opt(t.df), num(t.p), num(t.effect)]],
                ));
                notes.push("Effect size epsilon-squared = H / (N - 1), the share of rank variation explained by the groups. No interval is given: there is no closed-form interval for it. The chi-square approximation is poor when groups have fewer than 5 values.".into());
                if g.vals.iter().any(|v| v.len() < 5) {
                    notes.push("Some group has fewer than 5 values; the p-value is approximate.".into());
                }
                dropped = g.dropped;
                used = g.used;
            }
        }
        r.n_used = Some(used as u64);
        let mut all = dropped;
        all.extend(std::mem::take(&mut r.dropped));
        r.dropped = all;
        r.notes = notes;
        r.notes.push(format!("Intervals are {} % intervals.", pct(conf)));
        finish(ctx, rel, r, true)
    }
}

// ------------------------------------------------------------- Shapiro-Wilk

struct ShapiroTool;

impl Tool for ShapiroTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_shapiro",
            name: "Normality Test (Shapiro-Wilk)",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Shapiro-Wilk test that a numeric field comes from a normal distribution, overall or per group (3 to 5000 values each). A small p-value says the values depart from normality; with many values even small departures are detected, so read it with a Q-Q plot.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("value", "Value field", ParamKind::Field).field_of("in"),
                Param::new("group", "Group by", ParamKind::Field).field_of("in").optional(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The normality test")?;
        let value = p.req_str("value", "Value field")?;
        let (labels, vals, dropped, used) = match p.str("group") {
            Some(g) => {
                let gr = grouped(ctx, rel, &value, &g)?;
                (gr.labels, gr.vals, gr.dropped, gr.used)
            }
            None => {
                let d = load(ctx, rel, &[(&value, true)])?;
                let xs: Vec<f64> = d.cols[0].num.iter().flatten().copied().collect();
                let n = xs.len();
                (vec![value.clone()], vec![xs], drop_reasons(&d.cols[0], &value), n)
            }
        };
        let mut rows = vec![];
        let mut notes = vec![];
        for (l, v) in labels.iter().zip(&vals) {
            match shapiro_wilk(v) {
                Ok((w, pv)) => rows.push(vec![json!(l), json!(v.len()), num(w), num(pv)]),
                Err(e) => {
                    rows.push(vec![json!(l), json!(v.len()), Value::Null, Value::Null]);
                    notes.push(format!("{l}: {e}"));
                }
            }
        }
        let mut r = result("Shapiro-Wilk test of normality (W statistic, Royston 1995 approximation of the p-value)", &[C_SW]);
        r.tables.push(table("Shapiro-Wilk", vec![col("group", K::Text), col("n", K::Integer), col("W", K::Number), col("p", K::Number)], rows));
        r.n_used = Some(used as u64);
        r.dropped = dropped;
        r.notes = notes;
        r.notes.push("W close to 1 means the values are close to a normal sample. The null hypothesis is that the values are normal, so a large p-value does not prove normality; with few values the test has little power, with thousands it flags trivial departures.".into());
        finish(ctx, rel, r, true)
    }
}

// -------------------------------------------------------------------- Levene

struct LeveneTool;

impl Tool for LeveneTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_levene",
            name: "Equal Variances Test (Levene)",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Levene's test that the groups of a field have equal variances. The default centres on the group median (Brown-Forsythe), which is robust to non-normal data; the mean centre is Levene's original.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("value", "Value field", ParamKind::Field).field_of("in"),
                Param::new("group", "Group field", ParamKind::Field).field_of("in"),
                Param::new("center", "Centre", ParamKind::Enum).default("median").options(&[("median", "Median (Brown-Forsythe)"), ("mean", "Mean (Levene)")]),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "Levene's test")?;
        let (value, group) = (p.req_str("value", "Value field")?, p.req_str("group", "Group field")?);
        let median = p.str("center").as_deref() != Some("mean");
        let g = grouped(ctx, rel, &value, &group)?;
        let l = levene(&g.vals, median).map_err(s_err)?;
        let mut r = result(format!("Levene's test of equal variances of {value} across {} groups of {group}, {} centre", g.labels.len(), if median { "median (Brown-Forsythe)" } else { "mean" }), &[C_LEVENE]);
        r.tables.push(table("Levene test", vec![col("groups", K::Integer), col("F", K::Number), col("df1", K::Number), col("df2", K::Number), col("p", K::Number)], vec![vec![json!(g.labels.len()), num(l.f), num(l.df1), num(l.df2), num(l.p)]]));
        let rows = g.labels.iter().zip(&g.vals).map(|(l, v)| vec![json!(l), json!(v.len()), opt(variance(v)), opt(variance(v).map(f64::sqrt))]).collect();
        r.tables.push(table("Group variances", vec![col("group", K::Text), col("n", K::Integer), col("variance", K::Number), col("sd", K::Number)], rows));
        r.notes.push("A small p-value says the group variances differ; use Welch's t-test or ANOVA instead of the pooled versions. The statistic is a one-way ANOVA F on the absolute deviations from each group's centre.".into());
        r.n_used = Some(g.used as u64);
        r.dropped = g.dropped;
        finish(ctx, rel, r, true)
    }
}

// --------------------------------------------------------------- correlation

struct CorrelationTool;

impl Tool for CorrelationTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_correlation",
            name: "Correlation",
            category: "Statistics › Tests",
            role: Role::Tool,
            description: "Correlation between numeric fields: Pearson (linear), Spearman or Kendall tau-b (rank, robust to outliers and monotone curves), for every pair of the listed fields, with a confidence interval, the test statistic and p-value, and optional Holm or Benjamini-Hochberg adjustment across the pairs. Rows are used pairwise; the number of pairs per coefficient is shown.",
            inputs: spec_input(),
            output: Some(PortKind::Table),
            params: vec![
                Param::new("fields", "Numeric fields", ParamKind::Text).help("Two or more field names separated by commas; every pair is correlated."),
                Param::new("method", "Method", ParamKind::Enum).default("pearson").options(&[("pearson", "Pearson"), ("spearman", "Spearman"), ("kendall", "Kendall tau-b")]),
                Param::new("adjust", "Adjust p-values for the number of pairs", ParamKind::Enum).default("none").options(&[("none", "No adjustment"), ("holm", "Holm"), ("bh", "Benjamini-Hochberg")]),
                conf_param(),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = input(inputs, "The correlation")?;
        let fields = split_fields(&p.req_str("fields", "Numeric fields")?);
        if fields.len() < 2 {
            bail!("'Numeric fields' needs at least two field names");
        }
        let method = p.str("method").unwrap_or_else(|| "pearson".into());
        let how = p.str("adjust").unwrap_or_else(|| "none".into());
        let conf = conf_of(p)?;
        let wanted: Vec<(&str, bool)> = fields.iter().map(|f| (f.as_str(), true)).collect();
        let d = load(ctx, rel, &wanted)?;
        let mut rows = vec![];
        let mut pv = vec![];
        let mut notes = vec![];
        let mut flagged_ties = false;
        for i in 0..fields.len() {
            for j in i + 1..fields.len() {
                let (mut x, mut y) = (vec![], vec![]);
                for r in 0..d.rows {
                    if let (Some(a), Some(b)) = (d.cols[i].num[r], d.cols[j].num[r]) {
                        x.push(a);
                        y.push(b);
                    }
                }
                let c = match method.as_str() {
                    "spearman" => spearman(&x, &y, conf),
                    "kendall" => kendall(&x, &y, conf),
                    _ => pearson(&x, &y, conf),
                };
                match c {
                    Ok(c) => {
                        flagged_ties |= c.ties;
                        pv.push(c.p);
                        rows.push(vec![json!(fields[i]), json!(fields[j]), json!(x.len()), num(c.r), opt(c.ci.map(|v| v.0)), opt(c.ci.map(|v| v.1)), num(c.stat), opt(c.df), num(c.p), Value::Null]);
                    }
                    Err(e) => {
                        pv.push(f64::NAN);
                        rows.push(vec![json!(fields[i]), json!(fields[j]), json!(x.len()), Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, Value::Null]);
                        notes.push(format!("{} and {}: {e}", fields[i], fields[j]));
                    }
                }
            }
        }
        // Adjust the pairs that produced a p-value.
        let ok: Vec<usize> = (0..pv.len()).filter(|i| pv[*i].is_finite()).collect();
        let adj = adjust_p(&ok.iter().map(|i| pv[*i]).collect::<Vec<_>>(), &how);
        for (k, i) in ok.iter().enumerate() {
            rows[*i][9] = num(adj[k]);
        }
        let (name, cite, label) = match method.as_str() {
            "spearman" => ("Spearman rank correlation", C_SPEARMAN, "rho"),
            "kendall" => ("Kendall tau-b rank correlation", C_KENDALL, "tau"),
            _ => ("Pearson product-moment correlation", C_PEARSON, "r"),
        };
        let mut cites = vec![cite];
        if let Some(l) = adjust_label(&how) {
            cites.push(if how == "holm" { C_HOLM } else { C_BH });
            notes.push(format!("p_adjusted uses the {l} adjustment across the {} pairs; the confidence intervals are not adjusted.", ok.len()));
        }
        let mut r = result(format!("{name} for {} pair(s) of fields, with {} % intervals{}", rows.len(), pct(conf), if adjust_label(&how).is_some() { " and adjusted p-values" } else { "" }), &cites);
        r.tables.push(table("Correlations", vec![col("field_x", K::Text), col("field_y", K::Text), col("n", K::Integer), col(label, K::Number), col("ci_low", K::Number), col("ci_high", K::Number), col("statistic", K::Number), col("df", K::Number), col("p", K::Number), col("p_adjusted", K::Number)], rows));
        let complete = (0..d.rows).filter(|r| d.cols.iter().all(|c| c.num[*r].is_some())).count();
        r.n_used = Some(complete as u64);
        for (f, c) in fields.iter().zip(&d.cols) {
            r.dropped.extend(drop_reasons(c, f));
        }
        r.notes = notes;
        match method.as_str() {
            "spearman" => r.notes.push("Spearman's rho is Pearson's r of the ranks (ties get average ranks). The p-value uses the t approximation with n - 2 degrees of freedom; the interval is Fisher's z with the Bonett-Wright standard error.".into()),
            "kendall" => r.notes.push("Kendall's tau-b corrects for ties. The p-value is exact for fewer than 50 pairs without ties, otherwise the tie-corrected normal approximation; the interval is Fisher's z with the Fieller-Hartley-Pearson standard error (approximate).".into()),
            _ => r.notes.push("Pearson's r measures linear association only and is sensitive to outliers; the interval is Fisher's z. The p-value assumes the pairs are approximately bivariate normal.".into()),
        }
        if flagged_ties {
            r.notes.push("Some fields have tied values.".into());
        }
        r.notes.push("Rows are used pairwise: each coefficient uses the rows where both of its fields have a value (n per pair is in the table). n_used counts rows complete in every field. Correlation is not causation, and an outlier or a few clusters can create or hide a correlation: look at the scatter plot.".into());
        finish(ctx, rel, r, true)
    }
}
