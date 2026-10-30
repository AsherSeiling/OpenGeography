//! Attribute tables and charts in layouts (QGIS Print Layout "Attribute table",
//! ArcGIS Pro "Table frame" / "Chart frame"). Drawn from the layer's data with
//! the same bundled fonts as the rest of the page. Chart colours are the
//! Okabe–Ito palette (Okabe & Ito 2002), chosen to stay distinguishable with
//! the common forms of colour blindness.

use super::model::Placement;
use super::slots::TABLE_PT;
use super::surround::{ellipsize, open_group, text_slot, text_width, PT_MM};
use super::svg::{color, esc, mm};
use crate::db::ident;
use crate::layers::LayerInfo;
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use std::fmt::Write;

/// Okabe–Ito: blue, orange, green, pink, sky blue, vermillion, yellow; grey for "Other".
pub const PALETTE: [&str; 7] = ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442"];
const OTHER: &str = "#999999";

pub struct TableSpec<'a> {
    pub fields: &'a [String],
    pub sort_field: &'a str,
    pub descending: bool,
    pub max_rows: usize,
    pub size: f64,
    pub zebra: bool,
}

pub struct ChartSpec<'a> {
    /// "bar" | "pie".
    pub kind: &'a str,
    pub category: &'a str,
    /// Empty = count the features per category.
    pub value: &'a str,
    /// "sum" | "avg" | "count" (empty = sum with a value field, else count).
    pub agg: &'a str,
    pub top_n: usize,
    pub title: &'a str,
    pub size: f64,
}

pub fn check_chart(kind: &str, agg: &str) -> Result<()> {
    if !matches!(kind, "bar" | "pie") {
        bail!("Unknown chart type '{kind}' (bar or pie)");
    }
    if !matches!(agg, "" | "sum" | "avg" | "count") {
        bail!("Unknown chart summary '{agg}' (sum, avg or count)");
    }
    Ok(())
}

fn where_def(info: &LayerInfo) -> String {
    info.definition_query.as_deref().filter(|q| !q.trim().is_empty()).map_or(String::new(), |q| format!(" WHERE ({q})"))
}

fn is_numeric(ty: &str) -> bool {
    let t = ty.to_ascii_uppercase();
    ["INT", "DOUBLE", "FLOAT", "DECIMAL", "REAL", "NUMERIC"].iter().any(|k| t.contains(k))
}

fn num_text(s: &str) -> String {
    match s.parse::<f64>() {
        Ok(v) if v.fract() == 0.0 && v.abs() < 1e15 => format!("{}", v as i64),
        Ok(v) => format!("{}", (v * 1000.0).round() / 1000.0),
        Err(_) => s.to_string(),
    }
}

fn find_layer(lookup: &dyn Fn(&str) -> Option<LayerInfo>, id: &str, what: &str, at: &Placement, warnings: &mut Vec<String>) -> Option<LayerInfo> {
    if id.is_empty() {
        warnings.push(format!("{what} '{}' has no layer yet; choose one in Properties", at.id));
        return None;
    }
    let info = lookup(id);
    if info.is_none() {
        warnings.push(format!("{what} '{}': layer {id} not found; not drawn", at.id));
    }
    info
}

pub fn table(out: &mut String, warnings: &mut Vec<String>, conn: &Connection, lookup: &dyn Fn(&str) -> Option<LayerInfo>, at: &Placement, layer: &str, o: &TableSpec) -> Result<()> {
    let Some(info) = find_layer(lookup, layer, "Table", at, warnings) else { return Ok(()) };
    let size = if o.size > 0.0 { o.size } else { TABLE_PT };
    let head_pt = at.slot_pt("table.header", size);
    let cell_pt = at.slot_pt("table.cells", size);
    let row_h = head_pt.max(cell_pt) * PT_MM * 1.7;
    // Fields: the chosen ones that exist; else the first five.
    let mut cols: Vec<(String, bool)> = Vec::new();
    for f in o.fields {
        match info.fields.iter().find(|x| &x.name == f) {
            Some(x) => cols.push((x.name.clone(), is_numeric(&x.ty))),
            None => warnings.push(format!("Table '{}': {} has no field '{f}'; skipped", at.id, info.name)),
        }
    }
    if o.fields.is_empty() {
        cols = info.fields.iter().filter(|f| !f.name.starts_with("__")).take(5).map(|f| (f.name.clone(), is_numeric(&f.ty))).collect();
    }
    if cols.is_empty() {
        warnings.push(format!("Table '{}': {} has no fields to show", at.id, info.name));
        return Ok(());
    }
    let max_fit = (((at.h - row_h * 1.2) / row_h).floor().max(0.0)) as usize;
    let limit = o.max_rows.clamp(1, 500).min(max_fit.max(1));
    let sort = info.fields.iter().find(|f| f.name == o.sort_field).map(|f| format!(" ORDER BY {} {} NULLS LAST", ident(&f.name), if o.descending { "DESC" } else { "ASC" })).unwrap_or_default();
    let sel = cols.iter().map(|(n, _)| format!("CAST({} AS VARCHAR)", ident(n))).collect::<Vec<_>>().join(", ");
    let def = where_def(&info);
    let sql = format!("SELECT {sel} FROM {}{def}{sort} LIMIT {limit}", ident(&info.table));
    let total: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {}{def}", ident(&info.table)), [], |r| r.get(0)).map_err(|e| anyhow!("Table of {} could not be read: {e}", info.name))?;
    let mut st = conn.prepare(&sql).map_err(|e| anyhow!("Table of {} could not be read: {e}", info.name))?;
    let rows: Vec<Vec<String>> = st
        .query_map([], |r| (0..cols.len()).map(|i| r.get::<_, Option<String>>(i).map(|v| v.unwrap_or_default())).collect::<Result<Vec<_>, _>>())
        .map_err(|e| anyhow!("Table of {} could not be read: {e}", info.name))?
        .collect::<Result<_, _>>()?;
    if (rows.len() as i64) < total {
        warnings.push(format!("Table '{}' shows {} of {} rows; enlarge it or raise the row limit", at.id, rows.len(), total));
    }
    // Column widths from the widest text, shrunk to the box.
    let pad = 1.2;
    let mut w: Vec<f64> = cols
        .iter()
        .enumerate()
        .map(|(i, (n, numeric))| {
            let cell = rows.iter().map(|r| text_width(&if *numeric { num_text(&r[i]) } else { r[i].clone() }, "sans", cell_pt)).fold(0.0, f64::max);
            cell.max(text_width(n, "sans-bold", head_pt)) + 2.0 * pad
        })
        .collect();
    let total_w: f64 = w.iter().sum();
    if total_w > at.w {
        let k = at.w / total_w;
        w.iter_mut().for_each(|x| *x *= k);
    }
    open_group(out, "table", at);
    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#e6e6e6\"/>", mm(at.x), mm(at.y), mm(w.iter().sum()), mm(row_h));
    let mut x = at.x;
    for (i, (n, numeric)) in cols.iter().enumerate() {
        let t = ellipsize(n, "sans-bold", head_pt, w[i] - 2.0 * pad);
        let (tx, anchor) = if *numeric { (x + w[i] - pad, "end") } else { (x + pad, "start") };
        text_slot(out, tx, at.y + row_h * 0.72, &t, "sans-bold", head_pt, anchor, at, "table.header");
        x += w[i];
    }
    for (ri, r) in rows.iter().enumerate() {
        let y = at.y + row_h * (ri as f64 + 1.0);
        if o.zebra && ri % 2 == 1 {
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f4f4f4\"/>", mm(at.x), mm(y), mm(w.iter().sum()), mm(row_h));
        }
        let mut x = at.x;
        for (i, (_, numeric)) in cols.iter().enumerate() {
            let raw = if *numeric { num_text(&r[i]) } else { r[i].clone() };
            let t = ellipsize(&raw, "sans", cell_pt, w[i] - 2.0 * pad);
            let (tx, anchor) = if *numeric { (x + w[i] - pad, "end") } else { (x + pad, "start") };
            text_slot(out, tx, y + row_h * 0.72, &t, "sans", cell_pt, anchor, at, "table.cells");
            x += w[i];
        }
    }
    let bottom = at.y + row_h * (rows.len() as f64 + 1.0);
    let _ = write!(out, "<path d=\"M{} {}L{} {}M{} {}L{} {}\" stroke=\"#555555\" stroke-width=\"0.2\" fill=\"none\"/>", mm(at.x), mm(at.y + row_h), mm(at.x + w.iter().sum::<f64>()), mm(at.y + row_h), mm(at.x), mm(bottom), mm(at.x + w.iter().sum::<f64>()), mm(bottom));
    out.push_str("</g>");
    Ok(())
}

pub fn chart(out: &mut String, warnings: &mut Vec<String>, conn: &Connection, lookup: &dyn Fn(&str) -> Option<LayerInfo>, at: &Placement, layer: &str, o: &ChartSpec) -> Result<()> {
    check_chart(o.kind, o.agg)?;
    let Some(info) = find_layer(lookup, layer, "Chart", at, warnings) else { return Ok(()) };
    let Some(cat) = info.fields.iter().find(|f| f.name == o.category) else {
        warnings.push(format!("Chart '{}': choose the field that makes the categories (Properties › Category field)", at.id));
        return Ok(());
    };
    let agg = match (o.agg, o.value.is_empty()) {
        ("count", _) | ("", true) => "count".to_string(),
        (_, true) => "count".to_string(),
        (a, false) => if a.is_empty() { "sum".into() } else { a.to_string() },
    };
    let val_sql = if agg == "count" {
        "COUNT(*)".to_string()
    } else {
        let Some(v) = info.fields.iter().find(|f| f.name == o.value).filter(|f| is_numeric(&f.ty)) else {
            warnings.push(format!("Chart '{}': the value field must be a numeric field of {}", at.id, info.name));
            return Ok(());
        };
        format!("{}(CAST({} AS DOUBLE))", if agg == "avg" { "AVG" } else { "SUM" }, ident(&v.name))
    };
    let n = o.top_n.clamp(1, 12);
    let def = where_def(&info);
    let sql = format!("SELECT COALESCE(CAST({c} AS VARCHAR), '(empty)'), CAST({val_sql} AS DOUBLE) FROM {t}{def} GROUP BY 1 ORDER BY 2 DESC NULLS LAST LIMIT {lim}", c = ident(&cat.name), t = ident(&info.table), lim = n + 1);
    let mut st = conn.prepare(&sql).map_err(|e| anyhow!("Chart of {} could not be read: {e}", info.name))?;
    let mut data: Vec<(String, f64)> = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<f64>>(1)?.unwrap_or(0.0))))
        .map_err(|e| anyhow!("Chart of {} could not be read: {e}", info.name))?
        .collect::<Result<_, _>>()?;
    let more = data.len() > n;
    data.truncate(n);
    if more {
        warnings.push(format!("Chart '{}' shows the {n} largest categories of {}", at.id, info.name));
    }
    let neg = data.iter().filter(|d| d.1 < 0.0).count();
    if neg > 0 {
        warnings.push(format!("Chart '{}': {neg} negative value{} not drawn", at.id, if neg == 1 { " is" } else { "s are" }));
        data.retain(|d| d.1 >= 0.0);
    }
    open_group(out, "chart", at);
    let base = if o.size > 0.0 { o.size } else { TABLE_PT };
    let size = at.slot_pt("chart.labels", base);
    let title_pt = at.slot_pt("chart.title", base * 1.2);
    let mut top = at.y;
    if !o.title.trim().is_empty() {
        text_slot(out, at.x, at.y + title_pt * PT_MM, &ellipsize(o.title, "sans-bold", title_pt, at.w), "sans-bold", title_pt, "start", at, "chart.title");
        top += title_pt * PT_MM * 1.6;
    }
    let max = data.iter().map(|d| d.1).fold(0.0, f64::max);
    if data.is_empty() || max <= 0.0 {
        text_slot(out, at.x, top + size * PT_MM, "No data to chart", "sans", size, "start", at, "chart.labels");
        out.push_str("</g>");
        return Ok(());
    }
    let fmt = |v: f64| num_text(&format!("{v}"));
    let fill = |i: usize| PALETTE[i % PALETTE.len()];
    match o.kind {
        "bar" => {
            let label_w = (at.w * 0.35).min(data.iter().map(|d| text_width(&d.0, "sans", size)).fold(0.0, f64::max) + 1.5);
            let val_w = data.iter().map(|d| text_width(&fmt(d.1), "sans", size)).fold(0.0, f64::max) + 1.5;
            let plot_w = (at.w - label_w - val_w).max(1.0);
            let row = ((at.y + at.h - top) / data.len() as f64).min(size * PT_MM * 2.2);
            for (i, (name, v)) in data.iter().enumerate() {
                let y = top + row * i as f64;
                text_slot(out, at.x + label_w - 1.0, y + row * 0.62, &ellipsize(name, "sans", size, label_w - 1.0), "sans", size, "end", at, "chart.labels");
                let bw = plot_w * v / max;
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>", mm(at.x + label_w), mm(y + row * 0.12), mm(bw), mm(row * 0.76), fill(0));
                text_slot(out, at.x + label_w + bw + 1.0, y + row * 0.62, &fmt(*v), "sans", size, "start", at, "chart.labels");
            }
        }
        _ => {
            let total: f64 = data.iter().map(|d| d.1).sum();
            let avail_h = at.y + at.h - top;
            let r = (avail_h / 2.0).min(at.w * 0.3).max(1.0);
            let (cx, cy) = (at.x + r, top + avail_h / 2.0);
            let mut a0 = -std::f64::consts::FRAC_PI_2;
            for (i, (_, v)) in data.iter().enumerate() {
                let da = v / total * std::f64::consts::TAU;
                if data.len() == 1 || da >= std::f64::consts::TAU - 1e-9 {
                    let _ = write!(out, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>", mm(cx), mm(cy), mm(r), fill(i));
                } else if da > 0.0 {
                    let a1 = a0 + da;
                    let large = if da > std::f64::consts::PI { 1 } else { 0 };
                    let _ = write!(out, "<path d=\"M{} {}L{} {}A{} {} 0 {large} 1 {} {}Z\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"0.3\"/>", mm(cx), mm(cy), mm(cx + r * a0.cos()), mm(cy + r * a0.sin()), mm(r), mm(r), mm(cx + r * a1.cos()), mm(cy + r * a1.sin()), fill(i));
                }
                a0 += da;
            }
            let lx = at.x + 2.0 * r + 3.0;
            let row = (avail_h / data.len() as f64).min(size * PT_MM * 2.0);
            for (i, (name, v)) in data.iter().enumerate() {
                let y = top + (avail_h - row * data.len() as f64) / 2.0 + row * i as f64;
                let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"3\" height=\"3\" fill=\"{}\"/>", mm(lx), mm(y + row / 2.0 - 1.5), fill(i));
                let t = format!("{} ({}%)", name, num_text(&format!("{}", (v / total * 1000.0).round() / 10.0)));
                text_slot(out, lx + 4.5, y + row * 0.62, &ellipsize(&t, "sans", size, (at.x + at.w - lx - 4.5).max(1.0)), "sans", size, "start", at, "chart.labels");
            }
        }
    }
    let _ = (color("#000", "#000"), esc(""), OTHER);
    out.push_str("</g>");
    Ok(())
}
