//! Geoprocessing tools used by the Model (node editor).
//!
//! Adding a tool = adding one file here and one line in `registry()`.
//! A tool declares its ports and parameters in `spec()`; the UI builds the
//! node and its parameter form from that spec, so no UI code is needed.
//! `build()` returns SQL; the executor materializes and caches the result.

mod add_to_map;
mod attribute_join;
mod buffer;
mod calculate_field;
mod calculate_geometry;
mod clip;
mod dissolve;
mod export;
mod filter;
mod input_layer;
mod intersect;
mod layer_output;
mod project;
mod spatial_join;
mod erase;
mod merge;
mod select_by_location;
mod feature_to_point;
mod bounding_geometry;
mod simplify;
mod explode;
mod repair;
mod summarize_within;
mod near;
mod fishnet;
mod summary_statistics;

use crate::db::{ident, lit};
use crate::layers::LayerInfo;
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::Serialize;
use serde_json::{Map, Value};
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PortKind {
    /// Features with geometry.
    Vector,
    /// Any table, with or without geometry.
    Table,
}

#[derive(Serialize, Clone, Debug)]
pub struct Port {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PortKind,
}

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ParamKind {
    Number,
    Text,
    Expression,
    Enum,
    /// A field of the table connected to `field_of`.
    Field,
    Layer,
    Crs,
    Bool,
    SavePath,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Param {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: Value,
    pub options: Vec<(&'static str, &'static str)>,
    pub field_of: Option<&'static str>,
    pub optional: bool,
    pub help: &'static str,
    /// Set by the UI, not shown to the user.
    pub hidden: bool,
}

impl Param {
    pub fn new(id: &'static str, label: &'static str, kind: ParamKind) -> Self {
        Self { id, label, kind, default: Value::Null, options: vec![], field_of: None, optional: false, help: "", hidden: false }
    }
    pub fn default(mut self, v: impl Into<Value>) -> Self {
        self.default = v.into();
        self
    }
    pub fn options(mut self, o: &[(&'static str, &'static str)]) -> Self {
        self.options = o.to_vec();
        self
    }
    pub fn field_of(mut self, port: &'static str) -> Self {
        self.field_of = Some(port);
        self
    }
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
    pub fn help(mut self, h: &'static str) -> Self {
        self.help = h;
        self
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Blue oval in the Model: brings data in.
    Data,
    /// Yellow rectangle: transforms data.
    Tool,
    /// Ends a branch: adds to map or writes a file.
    Output,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub role: Role,
    pub description: &'static str,
    pub inputs: Vec<Port>,
    pub output: Option<PortKind>,
    pub params: Vec<Param>,
}

/// A node's materialized output.
#[derive(Clone, Debug)]
pub struct Rel {
    pub table: String,
    pub crs: Option<String>,
    pub has_geom: bool,
}

pub enum Output {
    /// A SELECT the executor materializes. `view` = cheap pass-through.
    Select { sql: String, crs: Option<String>, view: bool },
    /// Create/replace a map layer.
    Layer { name: String, sql: String, crs: Option<String> },
    /// The result of a layer's own node tree, drawn in place of that layer.
    Result { owner: String, sql: String, crs: Option<String> },
    /// Side effect done (e.g. file written).
    Done(String),
}

pub struct Ctx<'a> {
    pub conn: &'a Connection,
    pub layers: &'a HashMap<String, LayerInfo>,
    reprojected: Cell<bool>,
}

impl<'a> Ctx<'a> {
    pub fn new(conn: &'a Connection, layers: &'a HashMap<String, LayerInfo>) -> Self {
        Self { conn, layers, reprojected: Cell::new(false) }
    }

    pub fn was_reprojected(&self) -> bool {
        self.reprojected.get()
    }

    /// Non-geometry column names of a relation.
    pub fn columns(&self, rel: &Rel) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(&rel.table)))?;
        let names = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(names.into_iter().filter(|n| n != "geom").collect())
    }

    /// `geom` of `rel` expressed in `target` CRS, reprojecting if needed.
    pub fn geom_in(&self, rel: &Rel, alias: &str, target: &str) -> Result<String> {
        let src = need_crs(rel)?;
        let col = format!("{alias}.geom");
        if src.eq_ignore_ascii_case(target) {
            Ok(col)
        } else {
            self.reprojected.set(true);
            Ok(format!("ST_Transform({col}, {}, {}, true)", lit(src), lit(target)))
        }
    }

    /// Lon/lat of the center of a relation's extent (to pick a UTM zone).
    pub fn center_lonlat(&self, rel: &Rel) -> Result<(f64, f64)> {
        let crs = need_crs(rel)?;
        let sql = format!(
            "SELECT ST_X(c), ST_Y(c) FROM (SELECT ST_Transform(ST_Point((min(ST_XMin(geom)) + max(ST_XMax(geom))) / 2, (min(ST_YMin(geom)) + max(ST_YMax(geom))) / 2), {}, 'EPSG:4326', true) AS c FROM {})",
            lit(crs),
            ident(&rel.table)
        );
        let r: (Option<f64>, Option<f64>) = self.conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        match r {
            (Some(x), Some(y)) => Ok((x, y)),
            _ => bail!("Input has no features"),
        }
    }

    /// UTM zone (WGS 84) at the center of a relation, for metric operations.
    pub fn utm(&self, rel: &Rel) -> Result<String> {
        let (lon, lat) = self.center_lonlat(rel)?;
        Ok(crate::crs::utm_epsg(lon, lat))
    }

    /// Dominant geometry dimension of a relation (0 point, 1 line, 2 polygon).
    pub fn dimension(&self, rel: &Rel) -> Result<i32> {
        let d: Option<i32> = self.conn.query_row(
            &format!("SELECT max(ST_Dimension(geom)) FROM {}", ident(&rel.table)),
            [],
            |r| r.get(0),
        )?;
        Ok(d.unwrap_or(2))
    }
}

pub fn need_crs(rel: &Rel) -> Result<&str> {
    rel.crs.as_deref().ok_or_else(|| anyhow!("Input has no coordinate system. Use Define Projection on the source layer first."))
}

/// Column list for joining `b` onto `a`, renaming collisions to `name_1`.
pub fn join_columns(a_cols: &[String], b_cols: &[String], b_alias: &str) -> String {
    let taken: Vec<String> = a_cols.iter().map(|c| c.to_lowercase()).collect();
    b_cols
        .iter()
        .filter(|c| c.as_str() != "__fid")
        .map(|c| {
            let out = if taken.contains(&c.to_lowercase()) { format!("{c}_1") } else { c.clone() };
            format!("{b_alias}.{} AS {}", ident(c), ident(&out))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Typed access to a node's parameter values.
pub struct Params<'a>(pub &'a Map<String, Value>);

impl Params<'_> {
    pub fn str(&self, k: &str) -> Option<String> {
        self.0.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }
    pub fn req_str(&self, k: &str, label: &str) -> Result<String> {
        self.str(k).ok_or_else(|| anyhow!("'{label}' is required"))
    }
    pub fn f64(&self, k: &str) -> Option<f64> {
        match self.0.get(k)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn bool(&self, k: &str) -> bool {
        self.0.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

pub type Inputs<'a> = HashMap<&'a str, Rel>;

pub trait Tool: Sync + Send {
    fn spec(&self) -> ToolSpec;
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output>;
}

pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(input_layer::InputLayer),
        Box::new(filter::Filter),
        Box::new(buffer::Buffer),
        Box::new(clip::Clip),
        Box::new(intersect::Intersect),
        Box::new(dissolve::Dissolve),
        Box::new(project::Project),
        Box::new(spatial_join::SpatialJoin),
        Box::new(attribute_join::AttributeJoin),
        Box::new(calculate_field::CalculateField),
        Box::new(calculate_geometry::CalculateGeometry),
        Box::new(erase::Erase),
        Box::new(merge::Merge),
        Box::new(select_by_location::SelectByLocation),
        Box::new(feature_to_point::FeatureToPoint),
        Box::new(bounding_geometry::BoundingGeometry),
        Box::new(simplify::Simplify),
        Box::new(explode::Explode),
        Box::new(repair::Repair),
        Box::new(summarize_within::SummarizeWithin),
        Box::new(near::Near),
        Box::new(fishnet::Fishnet),
        Box::new(summary_statistics::SummaryStatistics),
        Box::new(layer_output::LayerOutput),
        Box::new(add_to_map::AddToMap),
        Box::new(export::Export),
    ]
}

/// Linear units → meters.
pub const LINEAR_UNITS: &[(&str, &str)] = &[
    ("m", "Meters"),
    ("km", "Kilometers"),
    ("ft", "Feet (international)"),
    ("usft", "US survey feet"),
    ("mi", "Miles"),
    ("yd", "Yards"),
];

pub fn to_meters(v: f64, unit: &str) -> Result<f64> {
    Ok(v * match unit {
        "m" => 1.0,
        "km" => 1000.0,
        "ft" => 0.3048,
        "usft" => 1200.0 / 3937.0,
        "mi" => 1609.344,
        "yd" => 0.9144,
        u => bail!("Unknown unit '{u}'"),
    })
}
