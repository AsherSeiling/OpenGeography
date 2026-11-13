//! A statistics sheet's chart or table on a layout page (ST7). The element holds only the sheet and card ids; the drawing is read from the card's
//! last result in the project, with the same chart renderer the sheet uses. A card that is out of date, has not been run or is gone says so in
//! the warnings (the preflight list), and the page still shows the last numbers it has.

use super::model::Placement;
use super::surround::{ellipsize, open_group, text_slot, text_width, PT_MM};
use super::svg::mm;
use crate::layers::LayerInfo;
use crate::stats::chart::{render_nested, ChartStyle};
use crate::stats::result::ColumnKind;
use crate::stats::share::{card_state, card_title, cell_text};
use crate::stats::StatSheet;
use anyhow::{anyhow, Result};
use duckdb::Connection;
use std::fmt::Write;

const TABLE_PT: f64 = 7.0;
/// The chart is drawn in a pixel space this many pixels per millimetre (96 dpi), so its 11 px labels print at about 8 pt.
const PX_PER_MM: f64 = 96.0 / 25.4;

pub struct Spec<'a> {
    pub sheet: &'a str,
    pub card: &'a str,
    pub part: &'a str,
    pub index: usize,
    pub size: f64,
}

fn load_sheet(conn: &Connection, id: &str) -> Result<Option<StatSheet>> {
    let mut st = conn.prepare("SELECT json::VARCHAR FROM __stat_sheets WHERE id = ?")?;
    let mut rows = st.query([id])?;
    match rows.next()? {
        Some(r) => {
            let json: String = r.get(0)?;
            Ok(Some(serde_json::from_str(&json).map_err(|e| anyhow!("Statistics sheet could not be read: {e}"))?))
        }
        None => Ok(None),
    }
}

fn placeholder(out: &mut String, at: &Placement, text: &str) {
    open_group(out, "stat-card", at);
    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f0f0f0\" stroke=\"#999999\" stroke-width=\"0.25\" stroke-dasharray=\"2 1\"/>", mm(at.x), mm(at.y), mm(at.w), mm(at.h));
    let pt = (at.h * 0.18 / PT_MM).clamp(5.0, 9.0);
    let t = ellipsize(text, "sans", pt, at.w - 2.0);
    text_slot(out, at.x + at.w / 2.0, at.y + at.h / 2.0, &t, "sans", pt, "middle", at, "stat_card.note");
    out.push_str("</g>");
}

pub fn draw(out: &mut String, warnings: &mut Vec<String>, conn: &Connection, lookup: &dyn Fn(&str) -> Option<LayerInfo>, at: &Placement, o: &Spec) -> Result<()> {
    if o.sheet.is_empty() || o.card.is_empty() {
        warnings.push(format!("Statistics element '{}' has no card yet; choose one in Properties", at.id));
        placeholder(out, at, "Choose a statistics card");
        return Ok(());
    }
    let Some(sheet) = load_sheet(conn, o.sheet)? else {
        warnings.push(format!("Statistics element '{}': the sheet {} is gone; not drawn", at.id, o.sheet));
        placeholder(out, at, "Statistics sheet not found");
        return Ok(());
    };
    let Some(card) = sheet.cards.iter().find(|c| c.id == o.card) else {
        warnings.push(format!("Statistics element '{}': the card is no longer on sheet '{}'; not drawn", at.id, sheet.name));
        placeholder(out, at, "Statistics card not found");
        return Ok(());
    };
    let name = format!("{} › {}", sheet.name, card_title(card));
    let state = card_state(card, &|id| lookup(id).map(|l| (l.name, l.version)));
    if let Some(w) = state.sentence(&format!("Statistics element '{}' ({name})", at.id)) {
        warnings.push(w);
    }
    let Some(r) = &card.result else {
        placeholder(out, at, "Card not run yet");
        return Ok(());
    };
    let table = o.part == "table";
    if table {
        match r.tables.get(o.index) {
            Some(t) => draw_table(out, warnings, at, t, o.size),
            None => {
                warnings.push(format!("Statistics element '{}': {name} has no table number {}", at.id, o.index + 1));
                placeholder(out, at, "No such table");
            }
        }
    } else {
        match r.charts.get(o.index) {
            Some(c) => {
                open_group(out, "stat-card", at);
                out.push_str(&render_nested(c, at.w * PX_PER_MM, at.h * PX_PER_MM, &ChartStyle::paper(), (at.x, at.y, at.w, at.h)));
                out.push_str("</g>");
            }
            None => {
                warnings.push(format!("Statistics element '{}': {name} has no chart number {}", at.id, o.index + 1));
                placeholder(out, at, "No such chart");
            }
        }
    }
    Ok(())
}

fn draw_table(out: &mut String, warnings: &mut Vec<String>, at: &Placement, t: &crate::stats::StatTable, size: f64) {
    let pt = if size > 0.0 { size } else { TABLE_PT };
    let row_h = pt * PT_MM * 1.7;
    let pad = 1.2;
    if t.columns.is_empty() {
        placeholder(out, at, "Empty table");
        return;
    }
    let max_fit = (((at.h - row_h * 1.2) / row_h).floor().max(0.0)) as usize;
    let shown = t.rows.len().min(max_fit.max(1));
    if shown < t.rows.len() {
        warnings.push(format!("Statistics table '{}' shows {} of {} rows; enlarge it or lower the text size", at.id, shown, t.rows.len()));
    }
    let heads: Vec<String> = t.columns.iter().map(|c| match &c.unit { Some(u) if !u.is_empty() => format!("{} ({u})", c.name), _ => c.name.clone() }).collect();
    let num: Vec<bool> = t.columns.iter().map(|c| matches!(c.kind, ColumnKind::Number | ColumnKind::Integer)).collect();
    let cells: Vec<Vec<String>> = t.rows.iter().take(shown).map(|r| (0..t.columns.len()).map(|i| r.get(i).map(cell_text).unwrap_or_default()).collect()).collect();
    let mut w: Vec<f64> = (0..t.columns.len()).map(|i| cells.iter().map(|r| text_width(&r[i], "sans", pt)).fold(text_width(&heads[i], "sans-bold", pt), f64::max) + 2.0 * pad).collect();
    let total: f64 = w.iter().sum();
    if total > at.w {
        let k = at.w / total;
        w.iter_mut().for_each(|x| *x *= k);
    }
    let width: f64 = w.iter().sum();
    open_group(out, "stat-card", at);
    let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#e6e6e6\"/>", mm(at.x), mm(at.y), mm(width), mm(row_h));
    let mut x = at.x;
    for i in 0..t.columns.len() {
        let txt = ellipsize(&heads[i], "sans-bold", pt, w[i] - 2.0 * pad);
        let (tx, anchor) = if num[i] { (x + w[i] - pad, "end") } else { (x + pad, "start") };
        text_slot(out, tx, at.y + row_h * 0.72, &txt, "sans-bold", pt, anchor, at, "stat_card.header");
        x += w[i];
    }
    for (ri, r) in cells.iter().enumerate() {
        let y = at.y + row_h * (ri as f64 + 1.0);
        if ri % 2 == 1 {
            let _ = write!(out, "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f4f4f4\"/>", mm(at.x), mm(y), mm(width), mm(row_h));
        }
        let mut x = at.x;
        for i in 0..t.columns.len() {
            let txt = ellipsize(&r[i], "sans", pt, w[i] - 2.0 * pad);
            let (tx, anchor) = if num[i] { (x + w[i] - pad, "end") } else { (x + pad, "start") };
            text_slot(out, tx, y + row_h * 0.72, &txt, "sans", pt, anchor, at, "stat_card.cells");
            x += w[i];
        }
    }
    let bottom = at.y + row_h * (shown as f64 + 1.0);
    let _ = write!(out, "<path d=\"M{} {}L{} {}M{} {}L{} {}\" stroke=\"#555555\" stroke-width=\"0.2\" fill=\"none\"/>", mm(at.x), mm(at.y + row_h), mm(at.x + width), mm(at.y + row_h), mm(at.x), mm(bottom), mm(at.x + width), mm(bottom));
    out.push_str("</g>");
}
