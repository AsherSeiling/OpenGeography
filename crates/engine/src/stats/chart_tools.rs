//! The chart tools: one `ToolSpec` each under "Statistics › Charts", so a chart works as a node in a tree (its output table is the numbers
//! behind the chart) and as a card on a statistics sheet (where `Engine::run_chart_card` returns the `ChartSpec` too). One definition, two uses.

use super::charts::{self, Source};
use super::result::{ColumnKind, StatTable};
use crate::nodes::*;
use anyhow::{bail, Result};
use serde_json::Value;

pub const CATEGORY: &str = "Statistics › Charts";

pub fn tools() -> Vec<Box<dyn Tool>> {
    charts::CHART_TOOLS.iter().map(|id| Box::new(ChartTool(id)) as Box<dyn Tool>).collect()
}

pub struct ChartTool(pub &'static str);

fn fld(id: &'static str, label: &'static str) -> Param {
    Param::new(id, label, ParamKind::Field).field_of("in")
}

impl Tool for ChartTool {
    fn spec(&self) -> ToolSpec {
        let (name, description, params): (&'static str, &'static str, Vec<Param>) = match self.0 {
            "chart_histogram" => (
                "Histogram",
                "Counts of a numeric field in equal-width bins. Bin width follows the Freedman-Diaconis rule unless you set the number of bins.",
                vec![fld("field", "Field"), Param::new("bins", "Number of bins (optional)", ParamKind::Number).optional().help("Leave empty for the Freedman-Diaconis rule.")],
            ),
            "chart_box" => ("Box Plot", "Quartiles, Tukey whiskers and outliers of a numeric field, overall or for each value of a group field.", vec![fld("field", "Field"), fld("group", "Group by (optional)").optional()]),
            "chart_scatter" => (
                "Scatter Plot",
                "Two numeric fields against each other, with an optional least-squares trend line. Above 5,000 points it draws a density grid instead.",
                vec![fld("x", "X field"), fld("y", "Y field"), Param::new("trend", "Trend line", ParamKind::Bool).default(true)],
            ),
            "chart_qq" => ("Normal Q-Q Plot", "Ordered values of a numeric field against normal quantiles, to see how close to normal they look.", vec![fld("field", "Field")]),
            "chart_bar" => (
                "Bar Chart",
                "A bar for each value of a category field: how many rows, or the sum or mean of a numeric field.",
                vec![
                    fld("category", "Category field"),
                    Param::new("agg", "Statistic", ParamKind::Enum).options(&[("count", "Count of rows"), ("sum", "Sum of the value field"), ("mean", "Mean of the value field")]).default("count"),
                    fld("value", "Value field (for sum or mean)").optional(),
                ],
            ),
            "chart_line" => ("Line Chart", "A numeric field against another, joined in order of the X field.", vec![fld("x", "X field"), fld("y", "Y field")]),
            "chart_scatter_matrix" => (
                "Scatter Plot Matrix",
                "Every pair of two to four numeric fields as small scatter plots.",
                vec![fld("f1", "Field 1"), fld("f2", "Field 2"), fld("f3", "Field 3 (optional)").optional(), fld("f4", "Field 4 (optional)").optional()],
            ),
            _ => (
                "Correlation Heat Map",
                "Pearson correlations between two to six numeric fields, as a colour grid.",
                vec![fld("f1", "Field 1"), fld("f2", "Field 2"), fld("f3", "Field 3 (optional)").optional(), fld("f4", "Field 4 (optional)").optional(), fld("f5", "Field 5 (optional)").optional(), fld("f6", "Field 6 (optional)").optional()],
            ),
        };
        ToolSpec { id: self.0, name, category: CATEGORY, role: Role::Tool, description, inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }], output: Some(PortKind::Table), params }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let table = &inputs["in"].table;
        let src = Source { conn: ctx.conn, table, cond: None, fids: None, has_fid: false, cap: 0 };
        let built = charts::build(self.0, &src, p.0)?;
        Ok(Output::Select { sql: values_select(&built.table)?, crs: None, view: false })
    }
}

fn lit(v: &Value, kind: ColumnKind) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Number(n) => match kind {
            ColumnKind::Integer => format!("{}::BIGINT", n.as_f64().unwrap_or(0.0) as i64),
            _ => format!("{:?}::DOUBLE", n.as_f64().unwrap_or(f64::NAN)),
        },
        Value::Bool(b) => b.to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        other => format!("'{}'", other.to_string().replace('\'', "''")),
    }
}

/// A table of values as a `SELECT` (what the node outputs).
fn values_select(t: &StatTable) -> Result<String> {
    if t.rows.is_empty() {
        bail!("The chart has no data to put in a table.");
    }
    let rows: Vec<String> = t.rows.iter().map(|r| format!("({})", r.iter().zip(&t.columns).map(|(v, c)| lit(v, c.kind)).collect::<Vec<_>>().join(", "))).collect();
    let names: Vec<String> = t.columns.iter().map(|c| crate::db::ident(&c.name)).collect();
    Ok(format!("SELECT * FROM (VALUES {}) AS t({})", rows.join(", "), names.join(", ")))
}
