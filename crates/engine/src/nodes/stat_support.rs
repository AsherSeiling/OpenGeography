//! Shared pieces of the statistics tools (ST2 onward): loading columns with exact accounting of missing and non-numeric values, building a
//! `StatResult`, turning its first table into the tool's output table, and the notes every test carries.

use super::*;
use crate::stats::{ColumnKind, Dropped, StatColumn, StatResult, StatTable};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Most rows a statistics tool loads into memory. Totals, means and quantiles of bigger tables belong in the SQL tools; the tool says so.
pub const MAX_ROWS: i64 = 20_000_000;

pub const SPATIAL_NOTE: &str = "These rows are places on a map. Features near each other are usually not independent, which breaks the independence assumption of this test, so its p-value and interval are likely too optimistic. Check for spatial autocorrelation (Moran's I) before relying on them.";

pub struct LCol {
    pub name: String,
    /// Numeric columns: None for a missing or unusable value.
    pub num: Vec<Option<f64>>,
    /// Text columns: None for NULL or empty text.
    pub txt: Vec<Option<String>>,
    /// Rows where the value is NULL (or empty text).
    pub null: u64,
    /// Rows with a value that is not a finite number (numeric columns only).
    pub bad: u64,
}

pub struct Loaded {
    pub rows: usize,
    pub cols: Vec<LCol>,
}

/// Load columns of `rel` in one pass so their rows line up. `numeric` columns are cast to numbers (text that is not a number counts as
/// `bad`), the others are read as text. Refuses tables over [`MAX_ROWS`] and unknown fields, in words.
pub fn load(ctx: &Ctx, rel: &Rel, wanted: &[(&str, bool)]) -> Result<Loaded> {
    let known = ctx.columns(rel)?;
    for (f, _) in wanted {
        if !known.iter().any(|c| c == f) {
            bail!("The input has no field named '{f}'. Fields are: {}", known.join(", "));
        }
    }
    let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&rel.table)), [], |r| r.get(0))?;
    if total > MAX_ROWS {
        bail!("This table has {total} rows; this tool holds the values in memory and stops at {MAX_ROWS}. Filter or aggregate first.");
    }
    let mut sel = Vec::new();
    for (f, num) in wanted {
        let q = ident(f);
        if *num {
            sel.push(format!("TRY_CAST({q} AS DOUBLE)"));
            sel.push(format!("({q} IS NULL OR trim(CAST({q} AS VARCHAR)) = '')"));
        } else {
            sel.push(format!("CAST({q} AS VARCHAR)"));
        }
    }
    let mut cols: Vec<LCol> = wanted.iter().map(|(f, _)| LCol { name: f.to_string(), num: Vec::with_capacity(total as usize), txt: vec![], null: 0, bad: 0 }).collect();
    let mut stmt = ctx.conn.prepare(&format!("SELECT {} FROM {}", sel.join(", "), ident(&rel.table)))?;
    let mut it = stmt.query([])?;
    let mut rows = 0usize;
    while let Some(r) = it.next()? {
        let mut k = 0;
        for (ci, (_, num)) in wanted.iter().enumerate() {
            let c = &mut cols[ci];
            if *num {
                let v: Option<f64> = r.get(k)?;
                let was_null: bool = r.get(k + 1)?;
                k += 2;
                if was_null {
                    c.null += 1;
                    c.num.push(None);
                } else {
                    match v {
                        Some(x) if x.is_finite() => c.num.push(Some(x)),
                        _ => {
                            c.bad += 1;
                            c.num.push(None);
                        }
                    }
                }
            } else {
                let v: Option<String> = r.get(k)?;
                k += 1;
                match v.filter(|s| !s.trim().is_empty()) {
                    Some(s) => c.txt.push(Some(s)),
                    None => {
                        c.null += 1;
                        c.txt.push(None);
                    }
                }
            }
        }
        rows += 1;
    }
    Ok(Loaded { rows, cols })
}

/// "missing" and "not a number" entries for the dropped list, for one column.
pub fn drop_reasons(c: &LCol, what: &str) -> Vec<Dropped> {
    let mut v = vec![];
    if c.null > 0 {
        v.push(Dropped { reason: format!("{what}: empty or missing"), count: c.null });
    }
    if c.bad > 0 {
        v.push(Dropped { reason: format!("{what}: not a finite number"), count: c.bad });
    }
    v
}

/// Order text levels the way a reader expects: numerically when every level is a number, otherwise alphabetically.
pub fn sort_levels(v: &mut Vec<String>) {
    if v.iter().all(|s| s.trim().parse::<f64>().is_ok()) {
        v.sort_by(|a, b| a.trim().parse::<f64>().unwrap().partial_cmp(&b.trim().parse::<f64>().unwrap()).unwrap());
    } else {
        v.sort();
    }
}

/// Split a comma-separated list of field names.
pub fn split_fields(s: &str) -> Vec<String> {
    s.split(',').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect()
}

pub fn col(name: &str, kind: ColumnKind) -> StatColumn {
    StatColumn { name: name.to_string(), unit: None, kind }
}

pub fn table(title: &str, cols: Vec<StatColumn>, rows: Vec<Vec<Value>>) -> StatTable {
    StatTable { title: title.to_string(), columns: cols, rows }
}

/// A JSON number, or null for a missing or non-finite one.
pub fn num(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { Value::Null }
}

pub fn opt(v: Option<f64>) -> Value {
    v.map_or(Value::Null, num)
}

/// A new result with its method and references filled in.
pub fn result(method: impl Into<String>, citations: &[&str]) -> StatResult {
    StatResult { method: method.into(), citations: citations.iter().map(|s| s.to_string()).collect(), n_used: None, dropped: vec![], tables: vec![], charts: vec![], notes: vec![], seed: None, data_version: BTreeMap::new() }
}

/// The tool's first table as a SELECT, so the same tool works as a node in a tree (its output is a table layer).
pub fn table_sql(t: &StatTable) -> String {
    let ty = |k: ColumnKind| match k {
        ColumnKind::Number => "DOUBLE",
        ColumnKind::Integer => "BIGINT",
        ColumnKind::Text => "VARCHAR",
        ColumnKind::Bool => "BOOLEAN",
    };
    let names: Vec<String> = (0..t.columns.len()).map(|i| format!("c{i}")).collect();
    let proj: Vec<String> = t.columns.iter().enumerate().map(|(i, c)| format!("CAST(c{i} AS {}) AS {}", ty(c.kind), ident(&c.name))).collect();
    if t.rows.is_empty() {
        return format!("SELECT {} WHERE FALSE", t.columns.iter().map(|c| format!("CAST(NULL AS {}) AS {}", ty(c.kind), ident(&c.name))).collect::<Vec<_>>().join(", "));
    }
    let lit_of = |v: &Value| match v {
        Value::Null => "NULL".to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => lit(s),
        other => lit(&other.to_string()),
    };
    let rows: Vec<String> = t.rows.iter().map(|r| format!("({})", r.iter().map(lit_of).collect::<Vec<_>>().join(", "))).collect();
    format!("SELECT {} FROM (VALUES {}) AS t({})", proj.join(", "), rows.join(", "), names.join(", "))
}

/// Report a finished result: hand it to the caller (a sheet card) and return the first table as the node's output.
pub fn finish(ctx: &Ctx, rel: &Rel, mut r: StatResult, spatial: bool) -> Result<Output> {
    if spatial && rel.has_geom {
        r.notes.push(SPATIAL_NOTE.to_string());
    }
    let first = r.tables.first().cloned().ok_or_else(|| anyhow!("The tool produced no table"))?;
    let sql = table_sql(&first);
    ctx.set_stat(r);
    Ok(Output::Select { sql, crs: None, view: false })
}

/// Counts of a pair of categorical fields (rows with either field empty are dropped and counted).
pub struct Contingency {
    pub row_levels: Vec<String>,
    pub col_levels: Vec<String>,
    pub counts: Vec<Vec<f64>>,
    pub total_rows: i64,
    pub used: i64,
    pub dropped: Vec<Dropped>,
}

pub const MAX_LEVELS: usize = 200;

pub fn contingency(ctx: &Ctx, rel: &Rel, a: &str, b: &str) -> Result<Contingency> {
    let known = ctx.columns(rel)?;
    for f in [a, b] {
        if !known.iter().any(|c| c == f) {
            bail!("The input has no field named '{f}'. Fields are: {}", known.join(", "));
        }
    }
    let t = ident(&rel.table);
    let (qa, qb) = (ident(a), ident(b));
    let clean = |q: &str| format!("({q} IS NOT NULL AND trim(CAST({q} AS VARCHAR)) <> '')");
    let (total_rows, miss_a, miss_b): (i64, i64, i64) = ctx.conn.query_row(
        &format!("SELECT count(*), count(*) FILTER (WHERE NOT {ca}), count(*) FILTER (WHERE {ca} AND NOT {cb}) FROM {t}", ca = clean(&qa), cb = clean(&qb)),
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let mut stmt = ctx.conn.prepare(&format!("SELECT CAST({qa} AS VARCHAR), CAST({qb} AS VARCHAR), count(*) FROM {t} WHERE {} AND {} GROUP BY 1, 2", clean(&qa), clean(&qb)))?;
    let cells: Vec<(String, String, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut rl: Vec<String> = cells.iter().map(|c| c.0.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let mut cl: Vec<String> = cells.iter().map(|c| c.1.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    for (f, n) in [(a, rl.len()), (b, cl.len())] {
        if n > MAX_LEVELS {
            bail!("'{f}' has {n} different values; this tool is for categories (at most {MAX_LEVELS}). Group the values first, or use a histogram for a measurement.");
        }
    }
    sort_levels(&mut rl);
    sort_levels(&mut cl);
    let mut counts = vec![vec![0.0; cl.len()]; rl.len()];
    for (x, y, n) in &cells {
        let (i, j) = (rl.iter().position(|v| v == x).unwrap(), cl.iter().position(|v| v == y).unwrap());
        counts[i][j] = *n as f64;
    }
    let mut dropped = vec![];
    if miss_a > 0 {
        dropped.push(Dropped { reason: format!("{a}: empty or missing"), count: miss_a as u64 });
    }
    if miss_b > 0 {
        dropped.push(Dropped { reason: format!("{b}: empty or missing (rows with {a} present)"), count: miss_b as u64 });
    }
    Ok(Contingency { row_levels: rl, col_levels: cl, counts, total_rows, used: total_rows - miss_a - miss_b, dropped })
}
