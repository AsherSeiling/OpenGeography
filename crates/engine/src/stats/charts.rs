//! The data side of the chart tools: read the columns, drop what cannot be plotted (and count it), compute the bins, quartiles, trend and
//! correlations, and return a `ChartSpec` (for `chart.rs`), the same numbers as a table, and the notes that say exactly what was done.
//! Each builder is a pure function of the values, so the numbers are tested without a database. Marks carry a `bin` key always and their feature
//! ids when there are not too many (`FID_CAP`); `stat_chart_bin_fids` rebuilds with no cap and returns the ids of the mark a key names.

use super::result::*;
use crate::db::ident;
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde_json::{json, Map, Value};

/// Scatter plots above this many points are drawn as a density grid, and say so.
pub const POINT_LIMIT: usize = 5_000;
/// Cells of the density grid per side.
pub const GRID: usize = 50;
/// Feature ids a chart lists in its marks; more than this and a mark carries only its bin key.
pub const FID_CAP: usize = 20_000;
const MATRIX_POINTS: usize = 1_500;
const MAX_GROUPS: usize = 50;

#[derive(Debug, Clone)]
pub struct Built {
    pub method: String,
    pub citations: Vec<String>,
    pub chart: ChartSpec,
    pub table: StatTable,
    pub notes: Vec<String>,
    pub n_used: u64,
    pub dropped: Vec<Dropped>,
}

/// Where the rows come from.
pub struct Source<'a> {
    pub conn: &'a Connection,
    pub table: &'a str,
    /// The layer's definition query, when it has one.
    pub cond: Option<String>,
    pub fids: Option<&'a [i64]>,
    /// Does the table have a `__fid` column (a layer does; a node's output does not)?
    pub has_fid: bool,
    /// Feature ids a mark may list.
    pub cap: usize,
}

fn where_clause(s: &Source) -> String {
    let mut parts = Vec::new();
    if let Some(c) = &s.cond {
        parts.push(format!("({c})"));
    }
    if let (Some(ids), true) = (s.fids, s.has_fid) {
        parts.push(if ids.is_empty() { "FALSE".to_string() } else { format!("__fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")) });
    }
    if parts.is_empty() { String::new() } else { format!("WHERE {}", parts.join(" AND ")) }
}

struct Rows {
    fids: Vec<i64>,
    /// One vector per requested numeric column.
    nums: Vec<Vec<Option<f64>>>,
    total: usize,
}

fn read_numeric(s: &Source, cols: &[String]) -> Result<Rows> {
    let fid = if s.has_fid { "__fid".to_string() } else { "CAST(-1 AS BIGINT)".to_string() };
    let sel: Vec<String> = cols.iter().map(|c| format!("TRY_CAST({} AS DOUBLE)", ident(c))).collect();
    let sql = format!("SELECT {fid}{}{} FROM {} {}", if sel.is_empty() { "" } else { ", " }, sel.join(", "), ident(s.table), where_clause(s));
    let sql = if s.has_fid { format!("{sql} ORDER BY __fid") } else { sql };
    let mut stmt = s.conn.prepare(&sql).map_err(|e| anyhow!("The chart could not read its fields: {e}"))?;
    let mut it = stmt.query([])?;
    let mut rows = Rows { fids: vec![], nums: vec![vec![]; cols.len()], total: 0 };
    while let Some(r) = it.next()? {
        rows.fids.push(r.get::<_, i64>(0)?);
        for i in 0..cols.len() {
            rows.nums[i].push(r.get::<_, Option<f64>>(i + 1)?.filter(|v| v.is_finite()));
        }
        rows.total += 1;
    }
    Ok(rows)
}

fn read_text(s: &Source, col: &str) -> Result<Vec<Option<String>>> {
    let sql = format!("SELECT CAST({} AS VARCHAR) FROM {} {} {}", ident(col), ident(s.table), where_clause(s), if s.has_fid { "ORDER BY __fid" } else { "" });
    let mut stmt = s.conn.prepare(&sql).map_err(|e| anyhow!("The chart could not read its fields: {e}"))?;
    let mut it = stmt.query([])?;
    let mut out = vec![];
    while let Some(r) = it.next()? {
        out.push(r.get::<_, Option<String>>(0)?);
    }
    Ok(out)
}

fn field(p: &Map<String, Value>, id: &str, label: &str) -> Result<String> {
    match p.get(id).and_then(|v| v.as_str()).map(str::trim) {
        Some(s) if !s.is_empty() => Ok(s.to_string()),
        _ => bail!("Choose a field for {label}."),
    }
}

fn opt_field(p: &Map<String, Value>, id: &str) -> Option<String> {
    p.get(id).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn dropped(reason: String, n: usize) -> Vec<Dropped> {
    if n > 0 { vec![Dropped { reason, count: n as u64 }] } else { vec![] }
}

fn axis(label: &str) -> Axis {
    Axis { label: label.to_string(), ..Default::default() }
}

fn col(name: &str, kind: ColumnKind) -> StatColumn {
    StatColumn { name: name.to_string(), unit: None, kind }
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Quantile by R's type 7 (Hyndman and Fan 1996), the same as DuckDB's `quantile_cont`. `sorted` is ascending and not empty.
pub fn quantile7(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    sorted[lo] + (h - h.floor()) * (sorted[hi] - sorted[lo])
}

/// The standard normal quantile (Acklam's rational approximation, relative error under 1.2e-9: far below what a Q–Q plot can show).
pub fn qnorm(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02, 1.383577518672690e+02, -3.066479806614716e+01, 2.506628277459239e+00];
    const B: [f64; 5] = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02, 6.680131188771972e+01, -1.328068155288572e+01];
    const C: [f64; 6] = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00, -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    const D: [f64; 4] = [7.784695709041462e-03, 3.224671290700398e-01, 2.445134137142996e+00, 3.754408661907416e+00];
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    let lo = 0.02425;
    if p < lo {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5]) / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - lo {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -qnorm(1.0 - p)
    }
}

const CITE_PALETTE: &str = "Okabe, M. and Ito, K. (2008). Color Universal Design (CUD): how to make figures and presentations that are friendly to colorblind people.";
const CITE_Q7: &str = "Hyndman, R. J. and Fan, Y. (1996). Sample quantiles in statistical packages. The American Statistician 50(4), 361-365 (type 7).";

fn mark_fids(fids: Vec<i64>, cap: usize) -> Vec<i64> {
    if fids.len() <= cap { fids } else { vec![] }
}

// ---------------------------------------------------------------- histogram

pub fn histogram(fids: &[i64], vals: &[f64], bins: Option<usize>, name: &str, cap: usize) -> Result<Built> {
    let n = vals.len();
    if n == 0 {
        bail!("'{name}' has no numeric values to plot.");
    }
    let mut sorted = vals.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let (min, max) = (sorted[0], sorted[n - 1]);
    let mut notes = vec![];
    let k = if min == max {
        notes.push(format!("Every value is {}, so there is one bin.", super::chart::num(min)));
        1
    } else if let Some(b) = bins {
        let b = b.clamp(1, 500);
        notes.push(format!("{b} equal-width bins, set by you."));
        b
    } else {
        let iqr = quantile7(&sorted, 0.75) - quantile7(&sorted, 0.25);
        let (k, how) = if iqr > 0.0 {
            let width = 2.0 * iqr * (n as f64).powf(-1.0 / 3.0);
            (((max - min) / width).ceil() as usize, format!("Bin width by the Freedman-Diaconis rule, 2 x IQR x n^(-1/3) = {}", super::chart::num(width)))
        } else {
            (((n as f64).log2().ceil() as usize) + 1, "The IQR is 0, so the number of bins is Sturges' rule, ceil(log2 n) + 1".to_string())
        };
        let capped = k.clamp(1, 200);
        notes.push(format!("{how}: {k} bins{}.", if capped != k { format!(", shown as {capped} (the most this chart draws)") } else { String::new() }));
        capped
    };
    let (a, width) = if min == max { (min - 0.5, 1.0) } else { (min, (max - min) / k as f64) };
    let mut counts = vec![0usize; k];
    let mut ids: Vec<Vec<i64>> = vec![vec![]; k];
    for (i, v) in vals.iter().enumerate() {
        let b = (((v - a) / width).floor() as usize).min(k - 1);
        counts[b] += 1;
        ids[b].push(fids[i]);
    }
    notes.push("Bins include their left edge; the last bin includes both edges.".into());
    let mut marks = vec![];
    let mut rows = vec![];
    for b in 0..k {
        let (x, x2) = (a + b as f64 * width, a + (b + 1) as f64 * width);
        marks.push(Mark {
            x: Some(x),
            x2: Some(x2),
            y: Some(counts[b] as f64),
            label: Some(format!("{} to {}: {}", super::chart::num(x), super::chart::num(x2), counts[b])),
            fids: mark_fids(std::mem::take(&mut ids[b]), if n <= cap { usize::MAX } else { 0 }),
            bin: Some(format!("hist:{b}")),
            ..Default::default()
        });
        rows.push(vec![json!(x), json!(x2), json!(counts[b])]);
    }
    Ok(Built {
        method: format!("Histogram of {name}: counts of values in equal-width bins."),
        citations: vec!["Freedman, D. and Diaconis, P. (1981). On the histogram as a density estimator: L2 theory. Zeitschrift fur Wahrscheinlichkeitstheorie 57, 453-476.".into(), CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Histogram, title: format!("Histogram of {name}"), x: axis(name), y: axis("Count"), series: vec![ChartSeries { name: name.into(), style: SeriesStyle::Marks, marks }], notes: vec![] },
        table: StatTable { title: "Histogram bins".into(), columns: vec![col("bin_start", ColumnKind::Number), col("bin_end", ColumnKind::Number), col("count", ColumnKind::Integer)], rows },
        notes,
        n_used: n as u64,
        dropped: vec![],
    })
}

// ---------------------------------------------------------------- box plot

pub fn boxplot(groups: Vec<(String, Vec<(i64, f64)>)>, name: &str, grouped_by: Option<&str>, cap: usize) -> Result<Built> {
    if groups.is_empty() || groups.iter().all(|g| g.1.is_empty()) {
        bail!("'{name}' has no numeric values to plot.");
    }
    let mut marks = vec![];
    let mut rows = vec![];
    let mut n_total = 0usize;
    let total: usize = groups.iter().map(|g| g.1.len()).sum();
    let mut cats = vec![];
    for (gi, (gname, items)) in groups.iter().enumerate() {
        cats.push(gname.clone());
        if items.is_empty() {
            continue;
        }
        n_total += items.len();
        let mut s: Vec<f64> = items.iter().map(|x| x.1).collect();
        s.sort_by(|a, b| a.total_cmp(b));
        let (q1, med, q3) = (quantile7(&s, 0.25), quantile7(&s, 0.5), quantile7(&s, 0.75));
        let iqr = q3 - q1;
        let (fl, fh) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
        let lo = s.iter().cloned().find(|v| *v >= fl).unwrap_or(s[0]);
        let hi = s.iter().rev().cloned().find(|v| *v <= fh).unwrap_or(s[s.len() - 1]);
        let ids: Vec<i64> = items.iter().map(|x| x.0).collect();
        marks.push(Mark {
            x: Some(gi as f64),
            y: Some(q1),
            y2: Some(q3),
            median: Some(med),
            lo: Some(lo),
            hi: Some(hi),
            label: Some(format!("{gname}: n {}, median {}", items.len(), super::chart::num(med))),
            fids: mark_fids(ids, if total <= cap { usize::MAX } else { 0 }),
            bin: Some(format!("box:{gi}")),
            ..Default::default()
        });
        let mut outliers = 0;
        for (fid, v) in items.iter().filter(|x| x.1 < lo || x.1 > hi) {
            outliers += 1;
            marks.push(Mark { x: Some(gi as f64), y: Some(*v), label: Some(format!("{gname}: {}", super::chart::num(*v))), fids: vec![*fid], bin: Some(format!("out:{fid}")), ..Default::default() });
        }
        rows.push(vec![json!(gname), json!(items.len()), json!(s[0]), json!(q1), json!(med), json!(q3), json!(s[s.len() - 1]), json!(lo), json!(hi), json!(outliers)]);
    }
    let kinds = [("group", ColumnKind::Text), ("n", ColumnKind::Integer), ("min", ColumnKind::Number), ("q1", ColumnKind::Number), ("median", ColumnKind::Number), ("q3", ColumnKind::Number), ("max", ColumnKind::Number), ("whisker_low", ColumnKind::Number), ("whisker_high", ColumnKind::Number), ("outliers", ColumnKind::Integer)];
    Ok(Built {
        method: format!("Box plot of {name}{}: quartiles by R's type 7 rule, Tukey whiskers.", grouped_by.map(|g| format!(" by {g}")).unwrap_or_default()),
        citations: vec!["Tukey, J. W. (1977). Exploratory Data Analysis. Addison-Wesley.".into(), CITE_Q7.into(), CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Box, title: format!("{name}{}", grouped_by.map(|g| format!(" by {g}")).unwrap_or_default()), x: Axis { label: grouped_by.unwrap_or("").to_string(), categories: cats, ..Default::default() }, y: axis(name), series: vec![ChartSeries { name: name.into(), style: SeriesStyle::Marks, marks }], notes: vec![] },
        table: StatTable { title: "Box plot statistics".into(), columns: kinds.iter().map(|(n, k)| col(n, *k)).collect(), rows },
        notes: vec!["The box runs from the first to the third quartile with a line at the median. Whiskers reach the most extreme value within 1.5 x IQR of the box; open circles are values beyond them (Tukey).".into()],
        n_used: n_total as u64,
        dropped: vec![],
    })
}

// ---------------------------------------------------------------- scatter

/// Ordinary least squares of y on x. None when x has no spread.
pub fn ols(pts: &[(f64, f64)]) -> Option<(f64, f64, f64)> {
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (sxx, sxy, syy) = pts.iter().fold((0.0, 0.0, 0.0), |a, p| (a.0 + (p.0 - mx).powi(2), a.1 + (p.0 - mx) * (p.1 - my), a.2 + (p.1 - my).powi(2)));
    if sxx <= 0.0 {
        return None;
    }
    let b = sxy / sxx;
    let r2 = if syy > 0.0 { sxy * sxy / (sxx * syy) } else { 1.0 };
    Some((my - b * mx, b, r2))
}

pub fn scatter(pts: &[(i64, f64, f64)], xn: &str, yn: &str, trend: bool, cap: usize) -> Result<Built> {
    let n = pts.len();
    if n == 0 {
        bail!("There are no rows with a number in both '{xn}' and '{yn}'.");
    }
    let mut notes = vec![];
    let mut series = vec![];
    if n > POINT_LIMIT {
        let e = |f: fn(&(i64, f64, f64)) -> f64| pts.iter().map(f).fold((f64::MAX, f64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
        let (xe, ye) = (e(|p| p.1), e(|p| p.2));
        let (wx, wy) = (((xe.1 - xe.0) / GRID as f64).max(f64::MIN_POSITIVE), ((ye.1 - ye.0) / GRID as f64).max(f64::MIN_POSITIVE));
        let mut cells: std::collections::BTreeMap<(usize, usize), Vec<i64>> = Default::default();
        for p in pts {
            let (ix, iy) = ((((p.1 - xe.0) / wx) as usize).min(GRID - 1), (((p.2 - ye.0) / wy) as usize).min(GRID - 1));
            cells.entry((ix, iy)).or_default().push(p.0);
        }
        let marks = cells
            .into_iter()
            .map(|((ix, iy), ids)| Mark {
                x: Some(xe.0 + ix as f64 * wx),
                y: Some(ye.0 + iy as f64 * wy),
                x2: Some(xe.0 + (ix + 1) as f64 * wx),
                y2: Some(ye.0 + (iy + 1) as f64 * wy),
                value: Some(ids.len() as f64),
                label: Some(format!("{} points", thousands(ids.len()))),
                bin: Some(format!("sc:{ix},{iy}")),
                fids: mark_fids(ids, if n <= cap { usize::MAX } else { 0 }),
                ..Default::default()
            })
            .collect();
        series.push(ChartSeries { name: "Points".into(), style: SeriesStyle::Marks, marks });
        notes.push(format!("{} points are more than the {} this chart draws one by one, so they are shown as a {GRID} x {GRID} density grid: darker cells hold more points. Click a cell to select its features.", thousands(n), thousands(POINT_LIMIT)));
    } else {
        let marks = pts.iter().map(|p| Mark { x: Some(p.1), y: Some(p.2), fids: if p.0 >= 0 { vec![p.0] } else { vec![] }, bin: Some(format!("pt:{}", p.0)), ..Default::default() }).collect();
        series.push(ChartSeries { name: "Points".into(), style: SeriesStyle::Marks, marks });
    }
    let mut rows = vec![vec![json!("n"), json!(n as f64)]];
    if trend {
        let xy: Vec<(f64, f64)> = pts.iter().map(|p| (p.1, p.2)).collect();
        match ols(&xy) {
            Some((a, b, r2)) => {
                let (x0, x1) = xy.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
                series.push(ChartSeries { name: "Trend".into(), style: SeriesStyle::Line, marks: vec![Mark { x: Some(x0), y: Some(a + b * x0), ..Default::default() }, Mark { x: Some(x1), y: Some(a + b * x1), ..Default::default() }] });
                notes.push(format!("Trend line: ordinary least squares, {yn} = {} + {} x {xn}, R-squared {}, fitted to all {} points. It describes the straight-line trend; it is not a test, and nearby features are not independent.", super::chart::num(a), super::chart::num(b), super::chart::num(r2), thousands(n)));
                rows.extend([vec![json!("intercept"), json!(a)], vec![json!("slope"), json!(b)], vec![json!("r_squared"), json!(r2)]]);
            }
            None => notes.push(format!("No trend line: every value of {xn} is the same.")),
        }
    }
    Ok(Built {
        method: format!("Scatter plot of {yn} against {xn}{}.", if trend { " with an ordinary least squares trend line" } else { "" }),
        citations: vec![CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Scatter, title: format!("{yn} against {xn}"), x: axis(xn), y: axis(yn), series, notes: vec![] },
        table: StatTable { title: "Scatter summary".into(), columns: vec![col("statistic", ColumnKind::Text), col("value", ColumnKind::Number)], rows },
        notes,
        n_used: n as u64,
        dropped: vec![],
    })
}

// ---------------------------------------------------------------- Q-Q plot

pub fn qq(items: &[(i64, f64)], name: &str) -> Result<Built> {
    let n = items.len();
    if n < 3 {
        bail!("A Q-Q plot needs at least 3 values; '{name}' has {n}.");
    }
    let mut s = items.to_vec();
    s.sort_by(|a, b| a.1.total_cmp(&b.1));
    // R's ppoints: (i - a) / (n + 1 - 2a), a = 3/8 for n <= 10, else 1/2.
    let a = if n <= 10 { 0.375 } else { 0.5 };
    let z = |i: usize| qnorm((i as f64 + 1.0 - a) / (n as f64 + 1.0 - 2.0 * a));
    let mut notes = vec![format!("Plotting positions (i - {a}) / (n + 1 - {})  as in R's ppoints; theoretical quantiles are standard normal.", 2.0 * a)];
    let step = if n > POINT_LIMIT { 1 + (n - 1) / 2_000 } else { 1 };
    if step > 1 {
        notes.push(format!("{} values: every {step}th ordered value is plotted, so about 2,000 points.", thousands(n)));
    }
    let marks: Vec<Mark> = (0..n).step_by(step).map(|i| Mark { x: Some(z(i)), y: Some(s[i].1), fids: vec![s[i].0], bin: Some(format!("pt:{}", s[i].0)), ..Default::default() }).collect();
    let vals: Vec<f64> = s.iter().map(|x| x.1).collect();
    let (q1, q3) = (quantile7(&vals, 0.25), quantile7(&vals, 0.75));
    let (z1, z3) = (qnorm(0.25), qnorm(0.75));
    let slope = (q3 - q1) / (z3 - z1);
    let intercept = q1 - slope * z1;
    let (zl, zh) = (z(0), z(n - 1));
    notes.push("The line passes through the first and third quartiles. Points close to it suggest the values are roughly normal; a curve away from it means skew or heavy tails. This is a picture, not a test.".into());
    Ok(Built {
        method: format!("Normal Q-Q plot of {name}: ordered values against standard normal quantiles."),
        citations: vec!["Becker, R. A., Chambers, J. M. and Wilks, A. R. (1988). The New S Language. Wadsworth (plotting positions).".into(), "Acklam, P. J. (2003). An algorithm for computing the inverse normal cumulative distribution function.".into(), CITE_Q7.into(), CITE_PALETTE.into()],
        chart: ChartSpec {
            kind: ChartKind::Qq,
            title: format!("Normal Q-Q plot of {name}"),
            x: axis("Theoretical quantiles"),
            y: axis("Sample quantiles"),
            series: vec![
                ChartSeries { name: "Values".into(), style: SeriesStyle::Marks, marks },
                ChartSeries { name: "Line through quartiles".into(), style: SeriesStyle::Line, marks: vec![Mark { x: Some(zl), y: Some(intercept + slope * zl), ..Default::default() }, Mark { x: Some(zh), y: Some(intercept + slope * zh), ..Default::default() }] },
            ],
            notes: vec![],
        },
        table: StatTable { title: "Q-Q line".into(), columns: vec![col("statistic", ColumnKind::Text), col("value", ColumnKind::Number)], rows: vec![vec![json!("n"), json!(n as f64)], vec![json!("line_intercept"), json!(intercept)], vec![json!("line_slope"), json!(slope)]] },
        notes,
        n_used: n as u64,
        dropped: vec![],
    })
}

// ---------------------------------------------------------------- bar and line

pub fn bar(cats: &[(i64, String, Option<f64>)], agg: &str, cat_name: &str, val_name: Option<&str>, cap: usize) -> Result<Built> {
    use std::collections::BTreeMap;
    if cats.is_empty() {
        bail!("There are no rows with a value in '{cat_name}'.");
    }
    let mut groups: BTreeMap<&str, (Vec<i64>, f64, usize)> = BTreeMap::new();
    for (fid, c, v) in cats {
        let g = groups.entry(c.as_str()).or_default();
        g.0.push(*fid);
        g.2 += 1;
        g.1 += v.unwrap_or(0.0);
    }
    if groups.len() > MAX_GROUPS {
        bail!("'{cat_name}' has {} different values, more than a bar chart can show (at most {MAX_GROUPS}). Choose a field with fewer values.", groups.len());
    }
    let measure = |g: &(Vec<i64>, f64, usize)| match agg {
        "sum" => g.1,
        "mean" => g.1 / g.2 as f64,
        _ => g.2 as f64,
    };
    let y_label = match (agg, val_name) {
        ("sum", Some(v)) => format!("Sum of {v}"),
        ("mean", Some(v)) => format!("Mean of {v}"),
        _ => "Count".to_string(),
    };
    let total = cats.len();
    let mut marks = vec![];
    let mut rows = vec![];
    let mut names = vec![];
    for (i, (name, g)) in groups.iter().enumerate() {
        names.push(name.to_string());
        let v = measure(g);
        marks.push(Mark { x: Some(i as f64), y: Some(v), label: Some(format!("{name}: {}", super::chart::num(v))), fids: mark_fids(g.0.clone(), if total <= cap { usize::MAX } else { 0 }), bin: Some(format!("bar:{i}")), ..Default::default() });
        rows.push(vec![json!(name), json!(g.2), json!(v)]);
    }
    Ok(Built {
        method: format!("Bar chart of {} by {cat_name}.", y_label.to_lowercase()),
        citations: vec![CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Bar, title: format!("{y_label} by {cat_name}"), x: Axis { label: cat_name.into(), categories: names, ..Default::default() }, y: axis(&y_label), series: vec![ChartSeries { name: y_label.clone(), style: SeriesStyle::Marks, marks }], notes: vec![] },
        table: StatTable { title: "Bar chart values".into(), columns: vec![col("category", ColumnKind::Text), col("n", ColumnKind::Integer), col(&y_label, ColumnKind::Number)], rows },
        notes: vec!["Bars are in alphabetical order of the category.".into()],
        n_used: total as u64,
        dropped: vec![],
    })
}

pub fn line(pts: &[(i64, f64, f64)], xn: &str, yn: &str) -> Result<Built> {
    let n = pts.len();
    if n < 2 {
        bail!("A line chart needs at least 2 rows with a number in both '{xn}' and '{yn}'.");
    }
    let mut s = pts.to_vec();
    s.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut notes = vec![format!("Rows are joined in order of {xn}.")];
    let step = if n > POINT_LIMIT { 1 + (n - 1) / 2_000 } else { 1 };
    if step > 1 {
        notes.push(format!("{} rows: every {step}th row (and the last) is drawn, so about 2,000 points.", thousands(n)));
    }
    let mut idx: Vec<usize> = (0..n).step_by(step).collect();
    if *idx.last().unwrap() != n - 1 {
        idx.push(n - 1);
    }
    let marks = idx.iter().map(|&i| Mark { x: Some(s[i].1), y: Some(s[i].2), fids: vec![s[i].0], bin: Some(format!("pt:{}", s[i].0)), ..Default::default() }).collect();
    Ok(Built {
        method: format!("Line chart of {yn} against {xn}."),
        citations: vec![CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Line, title: format!("{yn} against {xn}"), x: axis(xn), y: axis(yn), series: vec![ChartSeries { name: yn.into(), style: SeriesStyle::LineAndMarks, marks }], notes: vec![] },
        table: StatTable { title: "Line chart points".into(), columns: vec![col(xn, ColumnKind::Number), col(yn, ColumnKind::Number)], rows: idx.iter().take(MAX_TABLE_ROWS_HERE).map(|&i| vec![json!(s[i].1), json!(s[i].2)]).collect() },
        notes,
        n_used: n as u64,
        dropped: vec![],
    })
}

const MAX_TABLE_ROWS_HERE: usize = 2_000;

// ---------------------------------------------------------------- scatter matrix and correlation heat map

pub fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    if saa <= 0.0 || sbb <= 0.0 { None } else { Some((sab / (saa * sbb).sqrt()).clamp(-1.0, 1.0)) }
}

pub fn scatter_matrix(fids: &[i64], cols: &[Vec<f64>], names: &[String], dropped_rows: usize) -> Result<Built> {
    let n = fids.len();
    if n == 0 {
        bail!("There are no rows with a number in every chosen field.");
    }
    let k = cols.len();
    let step = 1 + (n - 1) / MATRIX_POINTS;
    let mut notes = vec![];
    if step > 1 {
        notes.push(format!("{} rows: every {step}th row is plotted in each panel, so about {}.", thousands(n), thousands(MATRIX_POINTS)));
    }
    let mut series = vec![];
    for i in 0..k {
        for j in 0..k {
            if i == j {
                continue;
            }
            let marks = (0..n).step_by(step).map(|r| Mark { x: Some(cols[i][r]), y: Some(cols[j][r]), fids: vec![fids[r]], bin: Some(format!("pt:{}", fids[r])), ..Default::default() }).collect();
            series.push(ChartSeries { name: format!("{i},{j}"), style: SeriesStyle::Marks, marks });
        }
    }
    notes.push("Each panel plots the field named in its column (across) against the field named in its row (down). Only rows with a number in every field are used.".into());
    Ok(Built {
        method: format!("Scatter plot matrix of {}.", names.join(", ")),
        citations: vec![CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::ScatterMatrix, title: "Scatter plot matrix".into(), x: Axis { label: String::new(), categories: names.to_vec(), ..Default::default() }, y: Axis::default(), series, notes: vec![] },
        table: StatTable { title: "Fields".into(), columns: vec![col("field", ColumnKind::Text), col("n", ColumnKind::Integer)], rows: names.iter().map(|n_| vec![json!(n_), json!(n)]).collect() },
        notes,
        n_used: n as u64,
        dropped: dropped("a missing or non-numeric value in one of the fields".into(), dropped_rows),
    })
}

pub fn correlation_heatmap(fids: &[i64], cols: &[Vec<f64>], names: &[String], dropped_rows: usize) -> Result<Built> {
    let n = fids.len();
    if n < 3 {
        bail!("Correlations need at least 3 rows with a number in every chosen field; there are {n}.");
    }
    let k = cols.len();
    let mut marks = vec![];
    let mut rows = vec![];
    for j in 0..k {
        let mut row = vec![json!(names[j])];
        for i in 0..k {
            let r = pearson(&cols[i], &cols[j]).ok_or_else(|| anyhow!("'{}' or '{}' has no variation (every value is the same), so it has no correlation.", names[i], names[j]))?;
            marks.push(Mark { x: Some(i as f64), y: Some(j as f64), value: Some(r), label: Some(format!("{} and {}: r = {}", names[i], names[j], super::chart::num(r))), ..Default::default() });
            row.push(json!(r));
        }
        rows.push(row);
    }
    let mut columns = vec![col("field", ColumnKind::Text)];
    columns.extend(names.iter().map(|n_| col(n_, ColumnKind::Number)));
    Ok(Built {
        method: format!("Pearson correlation matrix of {}, on the {} rows where every field has a number.", names.join(", "), thousands(n)),
        citations: vec!["Pearson, K. (1895). Notes on regression and inheritance in the case of two parents. Proceedings of the Royal Society of London 58, 240-242.".into(), CITE_PALETTE.into()],
        chart: ChartSpec { kind: ChartKind::Heatmap, title: "Correlation matrix".into(), x: Axis { label: String::new(), categories: names.to_vec(), ..Default::default() }, y: Axis { label: String::new(), categories: names.to_vec(), ..Default::default() }, series: vec![ChartSeries { name: "r".into(), style: SeriesStyle::Marks, marks }], notes: vec![] },
        table: StatTable { title: "Pearson correlations".into(), columns, rows },
        notes: vec!["Blue is a negative correlation, red-orange a positive one, white none. Each cell is r for one pair, from -1 to +1. Correlations measure straight-line association only. Features near each other are usually not independent, so these are descriptive numbers; do not read them as tests.".into()],
        n_used: n as u64,
        dropped: dropped("a missing or non-numeric value in one of the fields".into(), dropped_rows),
    })
}

// ---------------------------------------------------------------- dispatch

pub const CHART_TOOLS: [&str; 8] = ["chart_histogram", "chart_box", "chart_scatter", "chart_qq", "chart_bar", "chart_line", "chart_scatter_matrix", "chart_heatmap"];

/// Complete rows of several numeric columns: (row indices kept, dropped count).
fn complete(rows: &Rows) -> (Vec<usize>, usize) {
    let keep: Vec<usize> = (0..rows.total).filter(|&i| rows.nums.iter().all(|c| c[i].is_some())).collect();
    let dropped = rows.total - keep.len();
    (keep, dropped)
}

fn missing_note(label: &str) -> String {
    format!("a missing or non-numeric value in {label}")
}

/// Build the chart a tool describes from the rows of `src`.
pub fn build(tool: &str, src: &Source, p: &Map<String, Value>) -> Result<Built> {
    let cap = src.cap;
    let mut built = match tool {
        "chart_histogram" => {
            let f = field(p, "field", "Field")?;
            let rows = read_numeric(src, std::slice::from_ref(&f))?;
            let keep: Vec<usize> = (0..rows.total).filter(|&i| rows.nums[0][i].is_some()).collect();
            let bins = p.get("bins").and_then(|v| v.as_f64()).filter(|b| *b >= 1.0).map(|b| b as usize);
            let mut b = histogram(&keep.iter().map(|&i| rows.fids[i]).collect::<Vec<_>>(), &keep.iter().map(|&i| rows.nums[0][i].unwrap()).collect::<Vec<_>>(), bins, &f, cap)?;
            b.dropped = dropped(missing_note(&f), rows.total - keep.len());
            b
        }
        "chart_box" => {
            let f = field(p, "field", "Field")?;
            let g = opt_field(p, "group");
            let rows = read_numeric(src, std::slice::from_ref(&f))?;
            let labels = match &g {
                Some(g) => Some(read_text(src, g)?),
                None => None,
            };
            let mut by: std::collections::BTreeMap<String, Vec<(i64, f64)>> = Default::default();
            let (mut miss_v, mut miss_g) = (0, 0);
            for i in 0..rows.total {
                let Some(v) = rows.nums[0][i] else {
                    miss_v += 1;
                    continue;
                };
                let name = match &labels {
                    Some(l) => match &l[i] {
                        Some(s) => s.clone(),
                        None => {
                            miss_g += 1;
                            continue;
                        }
                    },
                    None => "All".to_string(),
                };
                by.entry(name).or_default().push((rows.fids[i], v));
            }
            if by.len() > 30 {
                bail!("'{}' has {} different values, more than a box plot can show (at most 30). Choose a field with fewer values.", g.unwrap_or_default(), by.len());
            }
            let mut b = boxplot(by.into_iter().collect(), &f, g.as_deref(), cap)?;
            b.dropped = [dropped(missing_note(&f), miss_v), dropped(format!("a missing value in the group field '{}'", g.clone().unwrap_or_default()), miss_g)].concat();
            b
        }
        "chart_scatter" | "chart_line" => {
            let (xn, yn) = (field(p, "x", "X field")?, field(p, "y", "Y field")?);
            let rows = read_numeric(src, &[xn.clone(), yn.clone()])?;
            let (keep, dr) = complete(&rows);
            let pts: Vec<(i64, f64, f64)> = keep.iter().map(|&i| (rows.fids[i], rows.nums[0][i].unwrap(), rows.nums[1][i].unwrap())).collect();
            let mut b = if tool == "chart_scatter" { scatter(&pts, &xn, &yn, p.get("trend").and_then(|v| v.as_bool()).unwrap_or(true), cap)? } else { line(&pts, &xn, &yn)? };
            b.dropped = dropped(missing_note(&format!("{xn} or {yn}")), dr);
            b
        }
        "chart_qq" => {
            let f = field(p, "field", "Field")?;
            let rows = read_numeric(src, std::slice::from_ref(&f))?;
            let items: Vec<(i64, f64)> = (0..rows.total).filter_map(|i| rows.nums[0][i].map(|v| (rows.fids[i], v))).collect();
            let mut b = qq(&items, &f)?;
            b.dropped = dropped(missing_note(&f), rows.total - items.len());
            b
        }
        "chart_bar" => {
            let c = field(p, "category", "Category field")?;
            let agg = p.get("agg").and_then(|v| v.as_str()).unwrap_or("count");
            let vname = opt_field(p, "value");
            if agg != "count" && vname.is_none() {
                bail!("Choose a value field to take the {agg} of, or set Statistic to count.");
            }
            let labels = read_text(src, &c)?;
            let vals = match &vname {
                Some(v) if agg != "count" => Some(read_numeric(src, std::slice::from_ref(v))?),
                _ => None,
            };
            let fids = match &vals {
                Some(r) => r.fids.clone(),
                None => read_numeric(src, &[])?.fids,
            };
            let (mut miss_c, mut miss_v) = (0, 0);
            let mut items = vec![];
            for i in 0..labels.len() {
                let Some(l) = &labels[i] else {
                    miss_c += 1;
                    continue;
                };
                let v = match &vals {
                    Some(r) => match r.nums[0][i] {
                        Some(v) => Some(v),
                        None => {
                            miss_v += 1;
                            continue;
                        }
                    },
                    None => None,
                };
                items.push((fids[i], l.clone(), v));
            }
            let mut b = bar(&items, agg, &c, vname.as_deref(), cap)?;
            b.dropped = [dropped(format!("a missing value in '{c}'"), miss_c), dropped(missing_note(vname.as_deref().unwrap_or("the value field")), miss_v)].concat();
            b
        }
        "chart_scatter_matrix" | "chart_heatmap" => {
            let max = if tool == "chart_heatmap" { 6 } else { 4 };
            let mut names = vec![];
            for i in 1..=max {
                let key = format!("f{i}");
                if i <= 2 {
                    names.push(field(p, &key, &format!("Field {i}"))?);
                } else if let Some(f) = opt_field(p, &key) {
                    names.push(f);
                }
            }
            let mut seen = std::collections::HashSet::new();
            if let Some(d) = names.iter().find(|n| !seen.insert((*n).clone())) {
                bail!("'{d}' is chosen twice; choose a different field for each.");
            }
            let rows = read_numeric(src, &names)?;
            let (keep, dr) = complete(&rows);
            let cols: Vec<Vec<f64>> = (0..names.len()).map(|c| keep.iter().map(|&i| rows.nums[c][i].unwrap()).collect()).collect();
            let fids: Vec<i64> = keep.iter().map(|&i| rows.fids[i]).collect();
            if tool == "chart_heatmap" { correlation_heatmap(&fids, &cols, &names, dr)? } else { scatter_matrix(&fids, &cols, &names, dr)? }
        }
        other => bail!("'{other}' is not a chart tool."),
    };
    if !src.has_fid {
        // A node's table has no feature ids: its marks stand for rows, not features.
        for s in &mut built.chart.series {
            for m in &mut s.marks {
                m.fids.clear();
            }
        }
    }
    if !built.dropped.is_empty() {
        let total: u64 = built.dropped.iter().map(|d| d.count).sum();
        built.notes.push(format!("{} row{} left out because of missing or non-numeric values; they are listed above.", thousands(total as usize), if total == 1 { " was" } else { "s were" }));
    }
    Ok(built)
}
