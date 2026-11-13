//! Statistics sheets (docs/STATISTICS_WORKSPACE_PLAN.md): a sheet is a document saved in the project (`__stat_sheets`, created IF NOT EXISTS so
//! older projects still open), an ordered list of cards. A card names a `Statistics › …` tool, its inputs and options, and keeps the last
//! result with the data versions it was computed at. Nothing here recomputes by itself: a card whose layers changed is marked out of date by
//! the UI (versions differ) and runs again only when the user asks.

use super::result::{ColumnKind, Dropped, StatColumn, StatResult, StatTable, MAX_TABLE_ROWS};
use crate::db::ident;
use crate::graph::{Graph, Status};
use crate::{sources, Engine};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// A tool is a statistics tool (and so can be a card) when its category starts with this.
pub const CATEGORY_PREFIX: &str = "Statistics ›";

/// Current format of a stored sheet. Newer files are refused with a sentence, older ones fill defaults.
pub const SHEET_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatSheet {
    pub id: String,
    pub name: String,
    #[serde(default = "one")]
    pub version: u32,
    /// In sheet order, top first.
    #[serde(default)]
    pub cards: Vec<StatCard>,
}

fn one() -> u32 {
    SHEET_VERSION
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatCard {
    /// Unique within the sheet.
    pub id: String,
    /// `ToolSpec.id` of a `Statistics › …` tool.
    pub tool: String,
    /// What the user called the card; the tool's name when empty.
    #[serde(default)]
    pub title: Option<String>,
    /// Tool input port id -> the layer or table feeding it.
    #[serde(default)]
    pub inputs: BTreeMap<String, CardInput>,
    /// Tool parameters by `Param.id`, exactly what a node of that tool would hold.
    #[serde(default)]
    pub options: Map<String, Value>,
    /// The last run, with the data versions it was computed at. `None` until the card has run.
    #[serde(default)]
    pub result: Option<StatResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardInput {
    pub layer: String,
    /// "Selected features only": the feature ids picked when the card was set up. They are stored, so the card reruns on the same rows
    /// whatever the map selection is later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fids: Option<Vec<i64>>,
}

impl StatSheet {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() || self.id.len() > 64 || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("A statistics sheet needs an id of letters, digits, '-' and '_'");
        }
        if self.name.trim().is_empty() {
            bail!("Give the statistics sheet a name");
        }
        if self.version > SHEET_VERSION {
            bail!("This sheet was made by a newer version of GIS Studio (format {}); this one reads up to {SHEET_VERSION}", self.version);
        }
        let mut seen = std::collections::HashSet::new();
        for c in &self.cards {
            if c.id.trim().is_empty() || !seen.insert(c.id.as_str()) {
                bail!("Each card on a sheet needs its own id ('{}' is empty or used twice)", c.id);
            }
        }
        Ok(())
    }
}

/// Fields of a layer or table as the Data pane shows them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FieldProfile {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    /// Rows where the value is NULL (or NaN for a floating-point field). Empty text counts as present.
    pub missing: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DataProfile {
    pub layer: String,
    /// Rows counted: the layer's features (after its definition query), or the given feature ids.
    pub rows: i64,
    pub fields: Vec<FieldProfile>,
}

/// Each call takes its own pooled connection, and two commits touching the same sheet id fail with a constraint error in DuckDB.
static SHEET_WRITES: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

impl Engine {
    /// Saved sheets, in name order.
    pub fn list_stat_sheets(&self) -> Result<Vec<StatSheet>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT json::VARCHAR FROM __stat_sheets")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                let s: StatSheet = serde_json::from_str(&r?).map_err(|e| anyhow!("A saved statistics sheet could not be read: {e}"))?;
                out.push(s);
            }
            out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
            Ok(out)
        })
    }

    /// Validate and store a sheet (insert or replace).
    pub fn save_stat_sheet(&self, sheet: &StatSheet) -> Result<()> {
        sheet.validate()?;
        let json = serde_json::to_string(sheet)?;
        let _one_at_a_time = SHEET_WRITES.lock();
        self.with_conn(|c| {
            c.execute("INSERT OR REPLACE INTO __stat_sheets VALUES (?, ?::JSON, now()::TIMESTAMP)", duckdb::params![sheet.id, json])?;
            Ok(())
        })
    }

    pub fn delete_stat_sheet(&self, id: &str) -> Result<()> {
        let _one_at_a_time = SHEET_WRITES.lock();
        self.with_conn(|c| {
            c.execute("DELETE FROM __stat_sheets WHERE id = ?", duckdb::params![id])?;
            Ok(())
        })
    }

    /// Change only the name, leaving the cards as saved (a rename never overwrites cards the UI has not saved yet).
    pub fn rename_stat_sheet(&self, id: &str, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("Give the statistics sheet a name");
        }
        let _one_at_a_time = SHEET_WRITES.lock();
        self.with_conn(|c| {
            let json: String = c
                .query_row("SELECT json::VARCHAR FROM __stat_sheets WHERE id = ?", [id], |r| r.get(0))
                .map_err(|_| anyhow!("That statistics sheet no longer exists"))?;
            let mut s: StatSheet = serde_json::from_str(&json)?;
            s.name = name.trim().to_string();
            c.execute("UPDATE __stat_sheets SET json = ?::JSON, updated = now()::TIMESTAMP WHERE id = ?", duckdb::params![serde_json::to_string(&s)?, id])?;
            Ok(())
        })
    }

    /// Statistics tools a card can use: the `ToolSpec`s whose category starts with "Statistics ›". Same specs as the node tree.
    pub fn is_stat_tool(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t.spec().id == tool && t.spec().category.starts_with(CATEGORY_PREFIX))
    }

    /// Fields with types and missing counts for the Data pane.
    pub fn stat_data_profile(&self, layer: &str, fids: Option<&[i64]>) -> Result<DataProfile> {
        let info = self.layer(layer)?;
        if info.raster.is_some() {
            bail!("'{}' is a raster; statistics sheets work on layers and tables", info.name);
        }
        let mut cond = Vec::new();
        if let Some(q) = info.def_condition() {
            cond.push(format!("({q})"));
        }
        if let Some(ids) = fids {
            cond.push(if ids.is_empty() { "FALSE".into() } else { format!("__fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")) });
        }
        let w = if cond.is_empty() { String::new() } else { format!("WHERE {}", cond.join(" AND ")) };
        let mut sel = vec!["count(*)".to_string()];
        for f in &info.fields {
            let q = ident(&f.name);
            let t = f.ty.to_ascii_uppercase();
            let float = t.contains("DOUBLE") || t.contains("FLOAT") || t.contains("REAL");
            sel.push(if float { format!("count(*) FILTER (WHERE {q} IS NULL OR isnan({q}))") } else { format!("count(*) FILTER (WHERE {q} IS NULL)") });
        }
        self.with_conn(|c| {
            let sql = format!("SELECT {} FROM {} {w}", sel.join(", "), ident(&info.table));
            let mut stmt = c.prepare(&sql)?;
            let mut it = stmt.query([])?;
            let r = it.next()?.ok_or_else(|| anyhow!("The layer returned no count"))?;
            let rows: i64 = r.get(0)?;
            let mut fields = Vec::new();
            for (i, f) in info.fields.iter().enumerate() {
                fields.push(FieldProfile { name: f.name.clone(), ty: f.ty.clone(), missing: r.get(i + 1)? });
            }
            Ok(DataProfile { layer: layer.to_string(), rows, fields })
        })
    }

    /// Run one card: the tool runs through the same executor as a node tree (an input node per port, then the tool), and its first output table
    /// becomes the result's first table. A tool that reports its own `StatResult` (ST2 onward) will replace this adapter for that tool; until
    /// then `n_used` is left unreported rather than guessed, and the result says so. Cancellable like a model run.
    pub fn run_stat_card(&self, card: &StatCard) -> Result<StatResult> {
        let tool = self.tools.iter().find(|t| t.spec().id == card.tool).ok_or_else(|| anyhow!("Unknown statistics tool '{}'", card.tool))?;
        let spec = tool.spec();
        if !spec.category.starts_with(CATEGORY_PREFIX) {
            bail!("'{}' is not a statistics tool, so it cannot be a card on a statistics sheet", spec.name);
        }
        if spec.inputs.is_empty() {
            bail!("'{}' takes no input, so it has nothing to run on a sheet", spec.name);
        }
        // Versions before the run, and again after: numbers computed while a layer changed would claim a version they are not from.
        let mut versions = BTreeMap::new();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for port in &spec.inputs {
            let input = card.inputs.get(port.id).ok_or_else(|| anyhow!("Choose a layer for '{}'", port.label))?;
            let info = self.layer(&input.layer).map_err(|_| anyhow!("The layer chosen for '{}' no longer exists", port.label))?;
            if info.raster.is_some() {
                bail!("'{}' is a raster; statistics sheets work on layers and tables", info.name);
            }
            versions.insert(info.id.clone(), info.version);
            let mut params = json!({ "layer": info.id });
            if let Some(f) = &input.fids {
                params["fids"] = json!(f);
            }
            let node = format!("in_{}", port.id);
            nodes.push(json!({ "id": node, "tool": "input_layer", "params": params }));
            edges.push(json!({ "source": node, "target": "card", "targetHandle": port.id }));
        }
        nodes.push(json!({ "id": "card", "tool": card.tool, "params": card.options }));
        let graph: Graph = serde_json::from_value(json!({ "nodes": nodes, "edges": edges }))?;
        let results = self.run_graph(&graph, Some("card"), &mut |_| {})?;
        let r = results.iter().find(|r| r.id == "card").ok_or_else(|| anyhow!("The tool did not run"))?;
        match r.status {
            Status::Done | Status::Cached => {}
            Status::Cancelled => bail!("Cancelled"),
            _ => bail!("{}", r.error.clone().unwrap_or_else(|| "The tool could not run".into())),
        }
        for (id, v) in &versions {
            if self.layer(id).map(|l| l.version).ok() != Some(*v) {
                bail!("A layer changed while the card was running, so the result would not match it. Run the card again.");
            }
        }
        let table_name = r.output.clone().ok_or_else(|| anyhow!("'{}' produced no table", spec.name))?;
        let mut notes = Vec::new();
        let table = self.with_conn(|c| read_table(c, &table_name, spec.name, &mut notes))?;
        let mut citations = Vec::new();
        self.with_conn(|c| {
            let entries = sources::list(c)?;
            for e in entries.iter().filter(|e| e.uses.iter().any(|u| versions.contains_key(&u.layer_id))) {
                let ci = &e.citation;
                citations.push([ci.author.as_str(), ci.year.as_str(), ci.title.as_str(), ci.url.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(". "));
            }
            Ok(())
        })?;
        notes.push("This tool does not report how many rows it used or dropped, so none is claimed here.".into());
        Ok(StatResult {
            method: spec.description.to_string(),
            citations,
            n_used: None,
            dropped: Vec::<Dropped>::new(),
            tables: vec![table],
            charts: vec![],
            notes,
            seed: None,
            data_version: versions,
        })
    }
}

/// A node output table as a result table: its visible columns, numbers kept as numbers, at most `MAX_TABLE_ROWS` rows (the cut is said).
fn read_table(c: &duckdb::Connection, table: &str, title: &str, notes: &mut Vec<String>) -> Result<StatTable> {
    let t = ident(table);
    let mut cols: Vec<(String, ColumnKind)> = Vec::new();
    {
        let mut stmt = c.prepare(&format!("SELECT column_name, column_type FROM (DESCRIBE SELECT * FROM {t})"))?;
        let mut it = stmt.query([])?;
        while let Some(r) = it.next()? {
            let (name, ty): (String, String) = (r.get(0)?, r.get::<_, String>(1)?.to_ascii_uppercase());
            if name.starts_with("__") || ty.contains("GEOMETRY") {
                continue;
            }
            let kind = if ty == "BOOLEAN" {
                ColumnKind::Bool
            } else if ty.contains("DOUBLE") || ty.contains("FLOAT") || ty.contains("DECIMAL") || ty == "REAL" {
                ColumnKind::Number
            } else if ty.contains("INT") {
                ColumnKind::Integer
            } else {
                ColumnKind::Text
            };
            cols.push((name, kind));
        }
    }
    if cols.is_empty() {
        bail!("The tool's output has no columns to show");
    }
    let total: i64 = c.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))?;
    let select: Vec<String> = cols
        .iter()
        .map(|(n, k)| {
            let ty = match k {
                ColumnKind::Number => "DOUBLE",
                ColumnKind::Integer => "BIGINT",
                ColumnKind::Bool => "BOOLEAN",
                ColumnKind::Text => "VARCHAR",
            };
            format!("CAST({} AS {ty})", ident(n))
        })
        .collect();
    let mut stmt = c.prepare(&format!("SELECT {} FROM {t} LIMIT {MAX_TABLE_ROWS}", select.join(", ")))?;
    let mut it = stmt.query([])?;
    let mut rows = Vec::new();
    let mut non_finite = 0usize;
    while let Some(r) = it.next()? {
        let mut row = Vec::with_capacity(cols.len());
        for (i, (_, k)) in cols.iter().enumerate() {
            row.push(match k {
                ColumnKind::Number => match r.get::<_, Option<f64>>(i)? {
                    Some(v) if v.is_finite() => json!(v),
                    Some(_) => {
                        non_finite += 1;
                        Value::Null
                    }
                    None => Value::Null,
                },
                ColumnKind::Integer => r.get::<_, Option<i64>>(i)?.map_or(Value::Null, |v| json!(v)),
                ColumnKind::Bool => r.get::<_, Option<bool>>(i)?.map_or(Value::Null, |v| json!(v)),
                ColumnKind::Text => r.get::<_, Option<String>>(i)?.map_or(Value::Null, |v| json!(v)),
            });
        }
        rows.push(row);
    }
    if total as usize > rows.len() {
        notes.push(format!("The table has {total} rows; the first {} are kept here.", rows.len()));
    }
    if non_finite > 0 {
        notes.push(format!("{non_finite} value(s) were infinite or not a number and are shown empty."));
    }
    Ok(StatTable { title: title.to_string(), columns: cols.into_iter().map(|(name, kind)| StatColumn { name, unit: None, kind }).collect(), rows })
}
