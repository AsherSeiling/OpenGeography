//! Statistics › Describe (package ST2): descriptive statistics (overall, by group, weighted), frequency tables and cross-tabulations.
//! Numbers come from `stats/describe.rs`, checked against the NIST Statistical Reference Datasets. Each tool reports a full `StatResult`
//! (method, citations, rows used, every dropped row by reason) and returns its first table as the node's output.

use super::stat_support::*;
use super::*;
use crate::stats::describe::{describe, weighted, Describe};
use crate::stats::{ColumnKind as K, Dropped};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const C_QUANTILE: &str = "Hyndman, R. J. and Fan, Y. (1996). Sample quantiles in statistical packages. The American Statistician 50(4), 361-365 (type 7).";
const C_MOMENTS: &str = "Joanes, D. N. and Gill, C. A. (1998). Comparing measures of sample skewness and kurtosis. The Statistician 47(1), 183-189 (G1 and G2).";
const C_NIST: &str = "NIST/SEMATECH e-Handbook of Statistical Methods and the NIST Statistical Reference Datasets, https://www.itl.nist.gov/div898/strd/ (the accuracy tests of this tool).";

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(DescribeTool), Box::new(FrequencyTool), Box::new(CrosstabTool)]
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    match id {
        "stat_describe" => &["descriptive statistics", "mean", "median", "standard deviation", "quartiles", "skewness", "kurtosis", "summary", "weighted mean", "by group", "coefficient of variation"],
        "stat_frequency" => &["frequency table", "counts", "percent", "categories", "value counts", "tabulate"],
        "stat_crosstab" => &["cross tabulation", "contingency table", "two-way table", "pivot", "counts by two fields"],
        _ => &[],
    }
}

struct DescribeTool;

impl Tool for DescribeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_describe",
            name: "Describe Fields",
            category: "Statistics › Describe",
            role: Role::Tool,
            description: "Descriptive statistics of one or more numeric fields, overall or by group: count, missing, mean (with its standard error and 95 % interval), median, mode, standard deviation, variance, coefficient of variation, minimum, quartiles, maximum, range, interquartile range, skewness and excess kurtosis, and with a weight field the weighted mean and standard deviation. The standard deviation and variance use n - 1; quartiles are R type 7 (Hyndman and Fan 1996); skewness and kurtosis are the adjusted G1 and G2 (Joanes and Gill 1998). Every dropped value is counted by reason, per field.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("fields", "Numeric fields", ParamKind::Text).help("Field names separated by commas."),
                Param::new("group", "Group by", ParamKind::Field).field_of("in").optional().help("One row of results per group value. Rows with no group value are left out and counted."),
                Param::new("weight", "Weight field", ParamKind::Field).field_of("in").optional().help("Adds weighted statistics using reliability weights (not counts of repeated observations). Rows whose weight is missing, not a number or negative are left out of the weighted columns and counted."),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Describe Fields needs an input table"))?;
        let fields = split_fields(&p.req_str("fields", "Numeric fields")?);
        if fields.is_empty() {
            bail!("'Numeric fields' needs at least one field name");
        }
        let group = p.str("group");
        let weight = p.str("weight");
        let mut wanted: Vec<(&str, bool)> = fields.iter().map(|f| (f.as_str(), true)).collect();
        if let Some(g) = &group {
            wanted.push((g, false));
        }
        if let Some(w) = &weight {
            wanted.push((w, true));
        }
        let data = load(ctx, rel, &wanted)?;
        let gi = group.as_ref().map(|_| fields.len());
        let wi = weight.as_ref().map(|_| fields.len() + gi.is_some() as usize);

        // Group membership: row indices per group label, in a readable order.
        let mut dropped: Vec<Dropped> = vec![];
        let mut members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut used_rows = data.rows;
        match gi {
            Some(g) => {
                let gc = &data.cols[g];
                for (r, v) in gc.txt.iter().enumerate() {
                    if let Some(v) = v {
                        members.entry(v.clone()).or_default().push(r);
                    }
                }
                if gc.null > 0 {
                    dropped.push(Dropped { reason: format!("{}: no group value", gc.name), count: gc.null });
                    used_rows -= gc.null as usize;
                }
            }
            None => {
                members.insert(String::new(), (0..data.rows).collect());
            }
        }
        let mut labels: Vec<String> = members.keys().cloned().collect();
        sort_levels(&mut labels);
        if labels.len() > 500 {
            bail!("'{}' has {} groups; describe at most 500 at a time. Group the values first.", group.clone().unwrap_or_default(), labels.len());
        }
        for (f, c) in fields.iter().zip(&data.cols) {
            dropped.extend(drop_reasons(c, f));
        }
        if let Some(wi) = wi {
            let wc = &data.cols[wi];
            let negative = wc.num.iter().flatten().filter(|v| **v < 0.0).count() as u64;
            let nr = drop_reasons(wc, &format!("{} (weights; weighted columns only)", wc.name));
            dropped.extend(nr);
            if negative > 0 {
                dropped.push(Dropped { reason: format!("{} (weights; weighted columns only): negative", wc.name), count: negative });
            }
        }

        let mut cols = vec![];
        if let Some(g) = &group {
            cols.push(col(g, K::Text));
        }
        cols.push(col("field", K::Text));
        for (n, k) in [("n", K::Integer), ("missing", K::Integer)] {
            cols.push(col(n, k));
        }
        for n in ["mean", "se_mean", "ci95_low", "ci95_high", "median", "mode", "sd", "variance", "cv", "min", "q1", "q3", "max", "range", "iqr", "skewness", "kurtosis", "sum"] {
            cols.push(col(n, K::Number));
        }
        if weight.is_some() {
            for n in ["sum_weights", "effective_n", "weighted_mean", "weighted_sd"] {
                cols.push(col(n, K::Number));
            }
        }
        let mut rows = vec![];
        for label in &labels {
            let idx = &members[label];
            for (fi, f) in fields.iter().enumerate() {
                let c = &data.cols[fi];
                let (mut xs, mut ws, mut x_for_w) = (vec![], vec![], vec![]);
                let mut missing = 0u64;
                for &r in idx {
                    match c.num[r] {
                        Some(v) => {
                            xs.push(v);
                            if let Some(wi) = wi {
                                if let Some(w) = data.cols[wi].num[r].filter(|w| *w >= 0.0) {
                                    x_for_w.push(v);
                                    ws.push(w);
                                }
                            }
                        }
                        None => missing += 1,
                    }
                }
                let d: Option<Describe> = describe(&xs);
                let mut row: Vec<Value> = vec![];
                if group.is_some() {
                    row.push(json!(label));
                }
                row.push(json!(f));
                row.push(json!(xs.len()));
                row.push(json!(missing));
                match &d {
                    Some(d) => {
                        row.extend([num(d.mean), opt(d.se), opt(d.ci95.map(|c| c.0)), opt(d.ci95.map(|c| c.1)), num(d.median), opt(d.mode), opt(d.sd), opt(d.variance), opt(d.cv), num(d.min), num(d.q1), num(d.q3), num(d.max), num(d.range), num(d.iqr), opt(d.skewness), opt(d.kurtosis), num(d.sum)]);
                    }
                    None => row.extend(std::iter::repeat(Value::Null).take(18)),
                }
                if weight.is_some() {
                    match weighted(&x_for_w, &ws) {
                        Some(w) => row.extend([num(w.sum_weights), num(w.effective_n), num(w.mean), opt(w.sd)]),
                        None => row.extend(std::iter::repeat(Value::Null).take(4)),
                    }
                }
                rows.push(row);
            }
        }
        let mut r = result(
            format!(
                "Descriptive statistics{}{}: sample standard deviation and variance (n - 1), quartiles by R type 7, skewness G1 and excess kurtosis G2, mean interval from Student's t.",
                if group.is_some() { " by group" } else { "" },
                if weight.is_some() { ", with reliability-weighted mean and standard deviation" } else { "" }
            ),
            &[C_QUANTILE, C_MOMENTS, C_NIST],
        );
        r.n_used = Some(used_rows as u64);
        r.dropped = dropped;
        r.tables.push(table("Descriptive statistics", cols, rows));
        if weight.is_some() {
            r.notes.push("Weighted columns use reliability weights: mean = sum(w x) / sum(w), variance = sum(w (x - mean)^2) / (sum(w) - sum(w^2) / sum(w)). If your weights count repeated observations, the unweighted columns on expanded data are the ones to use.".into());
        }
        r.notes.push("Skewness needs at least 3 values and kurtosis at least 4, and both need some spread; the mode is empty when no value repeats (the smallest is shown when several tie). The coefficient of variation is empty when the mean is 0.".into());
        finish(ctx, rel, r, false)
    }
}

struct FrequencyTool;

impl Tool for FrequencyTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_frequency",
            name: "Frequency Table",
            category: "Statistics › Describe",
            role: Role::Tool,
            description: "How often each value of a field occurs: count, percent of all rows, percent of rows with a value, and cumulative percent. Empty values are shown as their own row and are not part of the percentages of valid rows. For categories or a few distinct numbers; a field with thousands of different values is refused (use a histogram).",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("field", "Field", ParamKind::Field).field_of("in"),
                Param::new("order", "Order", ParamKind::Enum).default("value").options(&[("value", "By value"), ("count", "By count, most frequent first")]),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Frequency Table needs an input table"))?;
        let field = p.req_str("field", "Field")?;
        if !ctx.columns(rel)?.iter().any(|c| *c == field) {
            bail!("The input has no field named '{field}'");
        }
        let q = ident(&field);
        let t = ident(&rel.table);
        let clean = format!("({q} IS NOT NULL AND trim(CAST({q} AS VARCHAR)) <> '')");
        let mut stmt = ctx.conn.prepare(&format!("SELECT CAST({q} AS VARCHAR), count(*) FROM {t} WHERE {clean} GROUP BY 1"))?;
        let mut levels: Vec<(String, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
        let valid: i64 = levels.iter().map(|l| l.1).sum();
        let missing = total - valid;
        if levels.len() > 5000 {
            bail!("'{field}' has {} different values; a frequency table is for categories (at most 5000). Use a histogram for a measurement.", levels.len());
        }
        if p.str("order").as_deref() == Some("count") {
            levels.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        } else {
            let mut names: Vec<String> = levels.iter().map(|l| l.0.clone()).collect();
            sort_levels(&mut names);
            let pos: std::collections::HashMap<&String, usize> = names.iter().enumerate().map(|(i, n)| (n, i)).collect();
            levels.sort_by_key(|l| pos[&l.0]);
        }
        let cols = vec![col("value", K::Text), col("count", K::Integer), col("percent", K::Number), col("valid_percent", K::Number), col("cumulative_percent", K::Number)];
        let mut rows = vec![];
        let mut cum = 0i64;
        for (v, n) in &levels {
            cum += n;
            rows.push(vec![json!(v), json!(n), num(100.0 * *n as f64 / total as f64), num(100.0 * *n as f64 / valid as f64), num(100.0 * cum as f64 / valid as f64)]);
        }
        if missing > 0 {
            rows.push(vec![json!("(missing)"), json!(missing), num(100.0 * missing as f64 / total as f64), Value::Null, Value::Null]);
        }
        let mut r = result("Frequency table: counts of each value, percentages of all rows and of rows with a value.", &[]);
        r.n_used = Some(valid as u64);
        if missing > 0 {
            r.dropped.push(Dropped { reason: format!("{field}: empty or missing"), count: missing as u64 });
        }
        r.tables.push(table(&format!("Frequencies of {field}"), cols, rows));
        finish(ctx, rel, r, false)
    }
}

struct CrosstabTool;

impl Tool for CrosstabTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "stat_crosstab",
            name: "Cross-tabulation",
            category: "Statistics › Describe",
            role: Role::Tool,
            description: "Counts of rows for every combination of the values of two fields (a contingency table), with row and column totals, as counts or as percentages of the row, column or grand total. Rows where either field is empty are left out and counted. For categories (at most 200 values per field); for a test of association use Chi-square Test of Independence.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
            output: Some(PortKind::Table),
            params: vec![
                Param::new("row_field", "Row field", ParamKind::Field).field_of("in"),
                Param::new("col_field", "Column field", ParamKind::Field).field_of("in"),
                Param::new("show", "Show", ParamKind::Enum).default("count").options(&[("count", "Counts"), ("row", "Percent of row total"), ("column", "Percent of column total"), ("total", "Percent of all rows used")]),
            ],
        }
    }
    fn reports_stat(&self) -> bool {
        true
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let rel = inputs.get("in").ok_or_else(|| anyhow!("Cross-tabulation needs an input table"))?;
        let (a, b) = (p.req_str("row_field", "Row field")?, p.req_str("col_field", "Column field")?);
        if a == b {
            bail!("Choose two different fields");
        }
        let ct = contingency(ctx, rel, &a, &b)?;
        let show = p.str("show").unwrap_or_else(|| "count".into());
        let row_tot: Vec<f64> = ct.counts.iter().map(|r| r.iter().sum()).collect();
        let col_tot: Vec<f64> = (0..ct.col_levels.len()).map(|j| ct.counts.iter().map(|r| r[j]).sum()).collect();
        let grand: f64 = row_tot.iter().sum();
        let pct = show != "count";
        let kind = if pct { K::Number } else { K::Integer };
        let cell = |v: f64, rt: f64, ct_: f64| -> Value {
            match show.as_str() {
                "row" => num(100.0 * v / rt),
                "column" => num(100.0 * v / ct_),
                "total" => num(100.0 * v / grand),
                _ => json!(v as i64),
            }
        };
        let mut cols = vec![col(&a, K::Text)];
        for l in &ct.col_levels {
            cols.push(col(l, kind));
        }
        cols.push(col("Total", kind));
        let mut rows = vec![];
        for (i, rl) in ct.row_levels.iter().enumerate() {
            let mut row = vec![json!(rl)];
            for j in 0..ct.col_levels.len() {
                row.push(cell(ct.counts[i][j], row_tot[i], col_tot[j]));
            }
            row.push(cell(row_tot[i], row_tot[i], grand));
            rows.push(row);
        }
        let mut totals = vec![json!("Total")];
        for j in 0..ct.col_levels.len() {
            totals.push(cell(col_tot[j], col_tot[j], col_tot[j]));
        }
        totals.push(cell(grand, grand, grand));
        rows.push(totals);
        let mut r = result(
            format!("Cross-tabulation of {a} (rows) by {b} (columns): {}.", match show.as_str() { "row" => "percent of each row total", "column" => "percent of each column total", "total" => "percent of all rows used", _ => "counts" }),
            &[],
        );
        r.n_used = Some(ct.used as u64);
        r.dropped = ct.dropped;
        r.tables.push(table(&format!("{a} by {b}"), cols, rows));
        if ct.col_levels.is_empty() {
            r.notes.push("No row has a value in both fields, so the table is empty.".into());
        }
        finish(ctx, rel, r, false)
    }
}
