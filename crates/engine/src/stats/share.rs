//! Sharing a statistics sheet (ST7): tables as CSV, charts as SVG or PNG, and a report of the whole sheet with each card's method, rows used and
//! dropped, notes, seed and citations. The chart drawing is the one renderer in `chart.rs`, so a file, a layout and the sheet show the same picture.

use super::chart::{esc, render_svg, ChartStyle};
use super::result::{ChartSpec, StatResult, StatTable};
use super::sheet::{StatCard, StatSheet};
use crate::layout::export;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::fmt::Write;

/// Chart size in CSS pixels when a file is made (the sheet shows 720 x 400).
pub const FILE_W: f64 = 720.0;
pub const FILE_H: f64 = 400.0;

/// Whether a card's numbers still match the data.
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    NotRun,
    Current,
    /// Names of the layers that changed since the card ran.
    OutOfDate(Vec<String>),
    /// Layers the card used that no longer exist.
    Missing(Vec<String>),
}

impl CardState {
    /// A sentence for a warning or a report, or `None` when the card is current.
    pub fn sentence(&self, what: &str) -> Option<String> {
        match self {
            CardState::Current => None,
            CardState::NotRun => Some(format!("{what} has not been run yet, so it shows nothing")),
            CardState::OutOfDate(l) => Some(format!("{what} is out of date: {} changed after it was run. Run the card again to update it", l.join(", "))),
            CardState::Missing(l) => Some(format!("{what} used {}, which no longer exists", l.join(", "))),
        }
    }
}

/// `lookup` gives a layer's name and data version, or `None` when it is gone.
pub fn card_state(card: &StatCard, lookup: &dyn Fn(&str) -> Option<(String, u64)>) -> CardState {
    let Some(r) = &card.result else { return CardState::NotRun };
    let (mut changed, mut gone) = (Vec::new(), Vec::new());
    for (id, v) in &r.data_version {
        match lookup(id) {
            Some((_, now)) if now == *v => {}
            Some((name, _)) => changed.push(name),
            None => gone.push(id.clone()),
        }
    }
    if !gone.is_empty() {
        CardState::Missing(gone)
    } else if !changed.is_empty() {
        CardState::OutOfDate(changed)
    } else {
        CardState::Current
    }
}

/// A table cell as text for people: whole numbers as they are, other numbers to four significant digits.
pub fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => {
                let f = n.as_f64().unwrap_or(f64::NAN);
                if !f.is_finite() {
                    return String::new();
                }
                if f == 0.0 {
                    return "0".into();
                }
                let digits = (3 - f.abs().log10().floor() as i32).clamp(0, 12) as usize;
                if f.abs() >= 1e9 || f.abs() < 1e-4 {
                    format!("{f:.3e}")
                } else {
                    let t = format!("{f:.digits$}");
                    if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t }
                }
            }
        },
        other => other.to_string(),
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// A result table as CSV (RFC 4180, UTF-8, CRLF). Numbers keep full precision; a column's unit is in its heading; an empty cell is a missing value.
pub fn table_csv(t: &StatTable) -> String {
    let mut o = String::new();
    let head: Vec<String> = t.columns.iter().map(|c| csv_field(&match &c.unit { Some(u) if !u.is_empty() => format!("{} ({u})", c.name), _ => c.name.clone() })).collect();
    o.push_str(&head.join(","));
    o.push_str("\r\n");
    for r in &t.rows {
        let row: Vec<String> = (0..t.columns.len())
            .map(|i| match r.get(i) {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Null) | None => String::new(),
                Some(Value::String(s)) => csv_field(s),
                Some(v) => csv_field(&v.to_string()),
            })
            .collect();
        o.push_str(&row.join(","));
        o.push_str("\r\n");
    }
    o
}

fn result_of(card: &StatCard) -> Result<&StatResult> {
    card.result.as_ref().ok_or_else(|| anyhow!("This card has not been run yet. Run it, then export."))
}

/// A chart as a standalone SVG on a white page, with the dark ink a printer wants.
pub fn chart_file_svg(spec: &ChartSpec) -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", render_svg(spec, FILE_W, FILE_H, &ChartStyle::paper()))
}

/// The card's chart or table `index` for a file, checked in words.
fn pick<'a, T>(items: &'a [T], index: usize, what: &str) -> Result<&'a T> {
    items.get(index).ok_or_else(|| {
        if items.is_empty() {
            anyhow!("This card has no {what} to export")
        } else {
            anyhow!("This card has {} {what}{}; number {} does not exist", items.len(), if items.len() == 1 { "" } else { "s" }, index + 1)
        }
    })
}

impl Engine {
    /// Write result table `index` of a card as CSV.
    pub fn export_stat_table(&self, card: &StatCard, index: usize, path: &str) -> Result<()> {
        let t = pick(&result_of(card)?.tables, index, "table")?;
        std::fs::write(path, table_csv(t)).map_err(|e| anyhow!("The table could not be written to {path}: {e}"))
    }

    /// Write chart `index` of a card as "svg" or "png" (PNG at `dpi` of 96-per-inch pixels, 20 to 1200).
    pub fn export_stat_chart(&self, card: &StatCard, index: usize, format: &str, dpi: f64, path: &str) -> Result<()> {
        let spec = pick(&result_of(card)?.charts, index, "chart")?;
        let svg = chart_file_svg(spec);
        let bytes = match format {
            "svg" => svg.into_bytes(),
            "png" => export::svg_to_png(&svg, dpi)?,
            other => bail!("Unknown chart format '{other}' (svg or png)"),
        };
        std::fs::write(path, bytes).map_err(|e| anyhow!("The chart could not be written to {path}: {e}"))
    }

    fn card_state_now(&self, card: &StatCard) -> CardState {
        card_state(card, &|id| self.layer(id).ok().map(|l| (l.name, l.version)))
    }

    /// Write a report of the whole sheet as one self-contained HTML page (print it to PDF from any browser). Returns warnings: cards not run or out of date.
    pub fn export_stat_report(&self, sheet: &StatSheet, date: &str, path: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        let states: Vec<CardState> = sheet.cards.iter().map(|c| self.card_state_now(c)).collect();
        for (c, s) in sheet.cards.iter().zip(&states) {
            if let Some(w) = s.sentence(&format!("'{}'", card_title(c))) {
                warnings.push(w);
            }
        }
        let layer_name = |id: &str| self.layer(id).map(|l| l.name).unwrap_or_else(|_| id.to_string());
        let html = report_html(sheet, &states, date, &layer_name);
        std::fs::write(path, html).map_err(|e| anyhow!("The report could not be written to {path}: {e}"))?;
        Ok(warnings)
    }
}

pub fn card_title(c: &StatCard) -> String {
    c.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| c.tool.clone())
}

fn html_table(o: &mut String, t: &StatTable) {
    let _ = write!(o, "<table><caption>{}</caption><thead><tr>", esc(&t.title));
    for c in &t.columns {
        let _ = write!(o, "<th>{}</th>", esc(&match &c.unit { Some(u) if !u.is_empty() => format!("{} ({u})", c.name), _ => c.name.clone() }));
    }
    o.push_str("</tr></thead><tbody>");
    for r in &t.rows {
        o.push_str("<tr>");
        for (i, c) in t.columns.iter().enumerate() {
            let numeric = matches!(c.kind, super::result::ColumnKind::Number | super::result::ColumnKind::Integer);
            let _ = write!(o, "<td{}>{}</td>", if numeric { " class=\"n\"" } else { "" }, esc(&r.get(i).map(cell_text).unwrap_or_default()));
        }
        o.push_str("</tr>");
    }
    o.push_str("</tbody></table>");
}

/// The report page. `layer_name` turns a layer id into the name people know it by.
pub fn report_html(sheet: &StatSheet, states: &[CardState], date: &str, layer_name: &dyn Fn(&str) -> String) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>{t}</title><style>\
body{{font:14px/1.45 'Noto Sans',Arial,sans-serif;color:#1a1a1a;max-width:860px;margin:2em auto;padding:0 1em}}\
h1{{font-size:24px}}h2{{font-size:18px;margin-top:2em;border-bottom:1px solid #999}}h3{{font-size:14px;margin-bottom:.2em}}\
table{{border-collapse:collapse;margin:.6em 0}}caption{{text-align:left;font-weight:600;padding:.2em 0}}\
th,td{{border-bottom:1px solid #ccc;padding:.2em .7em;text-align:left}}th{{background:#eee}}td.n{{text-align:right}}\
.warn{{background:#fff3cd;border-left:4px solid #e69f00;padding:.3em .7em}}.meta{{color:#555}}figure{{margin:.6em 0}}\
@media print{{h2{{break-after:avoid}}figure,table{{break-inside:avoid}}}}</style></head><body>",
        t = esc(&sheet.name)
    );
    let _ = write!(o, "<h1>{}</h1><p class=\"meta\">Statistics report made by GIS Studio{}. {} card{}.</p>", esc(&sheet.name), if date.is_empty() { String::new() } else { format!(" on {}", esc(date)) }, sheet.cards.len(), if sheet.cards.len() == 1 { "" } else { "s" });
    let mut cites: Vec<String> = Vec::new();
    for (i, c) in sheet.cards.iter().enumerate() {
        let title = card_title(c);
        let _ = write!(o, "<h2>{}. {}</h2>", i + 1, esc(&title));
        if let Some(w) = states.get(i).and_then(|s| s.sentence("This card")) {
            let _ = write!(o, "<p class=\"warn\">{}.</p>", esc(&w));
        }
        let ins: Vec<String> = c.inputs.values().map(|x| format!("{}{}", layer_name(&x.layer), x.fids.as_ref().map(|f| format!(" ({} selected features)", f.len())).unwrap_or_default())).collect();
        if !ins.is_empty() {
            let _ = write!(o, "<p class=\"meta\">Data: {}</p>", esc(&ins.join(", ")));
        }
        let Some(r) = &c.result else { continue };
        let _ = write!(o, "<p><b>Method.</b> {}</p>", esc(&r.method));
        let used = match r.n_used {
            Some(n) => format!("Rows used: {n}."),
            None => "This tool does not report how many rows it used.".to_string(),
        };
        let dropped: u64 = r.dropped.iter().map(|d| d.count).sum();
        let _ = write!(o, "<p>{}{}{}</p>", esc(&used), if dropped > 0 { esc(&format!(" Rows left out: {dropped} ({}).", r.dropped.iter().map(|d| format!("{} {}", d.count, d.reason)).collect::<Vec<_>>().join("; "))) } else { String::new() }, r.seed.map(|s| format!(" Random seed: {s}.")).unwrap_or_default());
        for t in &r.tables {
            html_table(&mut o, t);
        }
        for ch in &r.charts {
            let _ = write!(o, "<figure>{}</figure>", render_svg(ch, FILE_W, FILE_H, &ChartStyle { ink: "#1a1a1a".into(), paper: Some("#ffffff".into()), font: "'Noto Sans', Arial, sans-serif".into() }));
        }
        if !r.notes.is_empty() {
            o.push_str("<ul>");
            for n in &r.notes {
                let _ = write!(o, "<li>{}</li>", esc(n));
            }
            o.push_str("</ul>");
        }
        for ci in &r.citations {
            if !cites.contains(ci) {
                cites.push(ci.clone());
            }
        }
    }
    o.push_str("<h2>Methods and citations</h2>");
    let mut methods: Vec<&str> = Vec::new();
    for c in &sheet.cards {
        if let Some(r) = &c.result {
            if !methods.contains(&r.method.as_str()) {
                methods.push(&r.method);
            }
        }
    }
    if methods.is_empty() {
        o.push_str("<p>No card has been run.</p>");
    } else {
        o.push_str("<h3>Methods</h3><ul>");
        for m in methods {
            let _ = write!(o, "<li>{}</li>", esc(m));
        }
        o.push_str("</ul>");
    }
    if !cites.is_empty() {
        o.push_str("<h3>References and data sources</h3><ol>");
        for c in &cites {
            let _ = write!(o, "<li>{}</li>", esc(c));
        }
        o.push_str("</ol>");
    }
    o.push_str("</body></html>\n");
    o
}
