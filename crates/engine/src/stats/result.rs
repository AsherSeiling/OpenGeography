//! The one result every statistics tool returns (docs/STATISTICS_WORKSPACE_PLAN.md, "ST0 result"): tables, chart data, notes, how many rows
//! were used and why others were dropped, and the data version the numbers came from. Chart *data*, not SVG: `stats/chart.rs` (ST3) draws it.
//! Field names are snake_case in the project file and on the wire; `ui/src/ipc/api.ts` mirrors them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Rows a result table keeps before it says it was cut (a sheet stores results in the project file).
pub const MAX_TABLE_ROWS: usize = 5_000;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct StatResult {
    /// One sentence naming the exact method ("Welch two-sample t-test, two-sided").
    pub method: String,
    /// Method references as text, then the input layers' sources.
    #[serde(default)]
    pub citations: Vec<String>,
    /// Rows used. `None` when the tool does not report it; the card then says so instead of implying every row was used.
    #[serde(default)]
    pub n_used: Option<u64>,
    /// Every dropped row, by reason. Never empty when rows were dropped.
    #[serde(default)]
    pub dropped: Vec<Dropped>,
    #[serde(default)]
    pub tables: Vec<StatTable>,
    #[serde(default)]
    pub charts: Vec<ChartSpec>,
    /// Assumption checks, the spatial-independence warning, multiple-testing and density-binning notes.
    #[serde(default)]
    pub notes: Vec<String>,
    /// Shown when a random method ran.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Layer id -> that layer's data version when this was computed. A card compares it with the layers now to mark itself out of date.
    #[serde(default)]
    pub data_version: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Dropped {
    pub reason: String,
    pub count: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct StatTable {
    pub title: String,
    pub columns: Vec<StatColumn>,
    /// Full precision; the UI rounds.
    pub rows: Vec<Vec<Value>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Number,
    Integer,
    Text,
    Bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatColumn {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub kind: ColumnKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Histogram,
    Box,
    Scatter,
    Qq,
    Bar,
    Line,
    Heatmap,
}

/// Chart data. Each mark of each series is drawn with `data-mark="<index>"` (index into that series' `marks`, series in order); a mark carries the
/// feature ids it stands for, or a bin key the engine turns into ids on click (a density bin of a large scatter plot).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChartSpec {
    pub kind: ChartKind,
    pub title: String,
    pub x: Axis,
    pub y: Axis,
    pub series: Vec<ChartSeries>,
    /// Said under the chart (density binning, a clipped axis).
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChartSeries {
    pub name: String,
    pub marks: Vec<Mark>,
}

/// One drawn thing. `x`/`y` place it; `x2`/`y2` give its far corner (a histogram bar spans x..x2 up to y, a box spans y..y2).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Mark {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Features this mark stands for (linked selection).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fids: Vec<i64>,
    /// A density bin key, when the mark stands for too many features to list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
}
