//! Layout document model (docs/LAYOUTS_PLAN.md L2). Page units are
//! millimetres, origin top-left, y down; font sizes are points.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const LAYOUT_VERSION: u32 = 1;

fn one() -> u32 {
    LAYOUT_VERSION
}
fn earth() -> String {
    "earth".into()
}
fn yes() -> bool {
    true
}
fn white() -> String {
    "#ffffff".into()
}
fn black() -> String {
    "#000000".into()
}
fn sans() -> String {
    "sans".into()
}
fn map_layers() -> FrameLayers {
    FrameLayers::Follow("map".into())
}
fn none() -> String {
    "none".into()
}
fn size_pt() -> f64 {
    10.0
}
fn left() -> String {
    "left".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Layout {
    pub id: String,
    pub name: String,
    #[serde(default = "one")]
    pub version: u32,
    /// Planetary body (L4); only "earth" until phase F.
    #[serde(default = "earth")]
    pub body: String,
    pub page: Page,
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Map series (C3.2); stored, not rendered yet.
    #[serde(default)]
    pub series: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Page {
    pub width: f64,
    pub height: f64,
    #[serde(default = "mm")]
    pub units: String,
    #[serde(default)]
    pub margin: f64,
    #[serde(default = "white")]
    pub background: String,
}

fn mm() -> String {
    "mm".into()
}

/// Position on the page, shared by every element kind.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Placement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Degrees clockwise about the element's centre.
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "yes")]
    pub visible: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum FrameLayers {
    /// "map": follow the map's draw order and visibility.
    Follow(String),
    /// Explicit list, top layer first.
    List(Vec<FrameLayer>),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrameLayer {
    pub id: String,
    #[serde(default = "yes")]
    pub visible: bool,
}

/// Graticule (degrees) or projected grid (CRS units) of a map frame.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GridSpec {
    /// "auto" or the spacing: degrees for a graticule, CRS units for a grid.
    #[serde(default = "auto")]
    pub interval: GridInterval,
    /// Edge labels.
    #[serde(default = "yes")]
    pub labels: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum GridInterval {
    Auto(String),
    Every(f64),
}

fn auto() -> GridInterval {
    GridInterval::Auto("auto".into())
}

impl GridSpec {
    /// Parse the `graticule` / `grid` value of a frame (`what` names it in errors).
    pub fn parse(v: &serde_json::Value, what: &str) -> Result<GridSpec> {
        let g: GridSpec = serde_json::from_value(v.clone()).map_err(|e| anyhow!("Invalid {what}: {e}"))?;
        match &g.interval {
            GridInterval::Auto(s) if s == "auto" => {}
            GridInterval::Every(v) if v.is_finite() && *v > 0.0 => {}
            _ => bail!("Invalid {what}: the interval must be \"auto\" or a positive number"),
        }
        Ok(g)
    }

    /// The fixed spacing, or None for "auto".
    pub fn step(&self) -> Option<f64> {
        match self.interval {
            GridInterval::Every(v) => Some(v),
            GridInterval::Auto(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Border {
    pub width: f64,
    #[serde(default = "black")]
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Element {
    MapFrame {
        #[serde(flatten)]
        at: Placement,
        /// Explicit frame CRS; never guessed (L3). Empty = not set.
        #[serde(default)]
        crs: String,
        /// Frame centre in frame-CRS units.
        center: [f64; 2],
        /// Nominal scale denominator.
        scale: f64,
        #[serde(default = "map_layers")]
        layers: FrameLayers,
        /// "none" | "usgsTopo" | "usgsImagery" | "usgsRelief" (OSM is refused: print).
        #[serde(default = "none")]
        basemap: String,
        /// Lines of latitude and longitude ([`GridSpec`]).
        #[serde(default)]
        graticule: Option<serde_json::Value>,
        /// Projected grid in frame-CRS units ([`GridSpec`]).
        #[serde(default)]
        grid: Option<serde_json::Value>,
        #[serde(default)]
        border: Option<Border>,
        /// Label edits made in this layout only, by layer id (L3).
        #[serde(default, rename = "labelOverrides", skip_serializing_if = "BTreeMap::is_empty")]
        label_overrides: BTreeMap<String, LabelOverrides>,
    },
    Text {
        #[serde(flatten)]
        at: Placement,
        text: String,
        /// "sans" | "sans-bold" | "sans-italic".
        #[serde(default = "sans")]
        font: String,
        #[serde(default = "size_pt")]
        size: f64,
        /// "left" | "center" | "right".
        #[serde(default = "left")]
        align: String,
        #[serde(default = "black")]
        color: String,
    },
    Legend {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        columns: u32,
    },
    ScaleBar {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        #[serde(default)]
        units: String,
        #[serde(default)]
        segments: u32,
        /// Also print the ratio ("1:24,000") under the bar (C3.0).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        ratio: bool,
    },
    NorthArrow {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        #[serde(default)]
        style: String,
    },
    Sources {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        #[serde(default)]
        size: f64,
    },
}

impl Element {
    pub fn at(&self) -> &Placement {
        match self {
            Element::MapFrame { at, .. }
            | Element::Text { at, .. }
            | Element::Legend { at, .. }
            | Element::ScaleBar { at, .. }
            | Element::NorthArrow { at, .. }
            | Element::Sources { at, .. } => at,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Element::MapFrame { .. } => "map_frame",
            Element::Text { .. } => "text",
            Element::Legend { .. } => "legend",
            Element::ScaleBar { .. } => "scale_bar",
            Element::NorthArrow { .. } => "north_arrow",
            Element::Sources { .. } => "sources",
        }
    }
}

impl Layout {
    /// Parse a stored layout. An unknown element `kind` is an error.
    pub fn from_json(json: &str) -> Result<Layout> {
        let l: Layout = serde_json::from_str(json).map_err(|e| anyhow!("Invalid layout: {e}"))?;
        l.validate()?;
        Ok(l)
    }

    /// Structural checks done on save and before every render.
    pub fn validate(&self) -> Result<()> {
        if self.version != LAYOUT_VERSION {
            bail!("Layout '{}' has version {}; this build reads version {LAYOUT_VERSION}", self.name, self.version);
        }
        super::geo::Body::get(&self.body)?;
        if self.page.units != "mm" {
            bail!("Page units must be \"mm\" (got \"{}\")", self.page.units);
        }
        if !(self.page.width.is_finite() && self.page.height.is_finite() && self.page.width > 0.0 && self.page.height > 0.0) {
            bail!("Page size must be positive");
        }
        let mut ids = BTreeSet::new();
        for e in &self.elements {
            let a = e.at();
            if !ids.insert(a.id.as_str()) {
                bail!("Duplicate element id '{}'", a.id);
            }
            if ![a.x, a.y, a.w, a.h, a.rotation].iter().all(|v| v.is_finite()) || a.w <= 0.0 || a.h <= 0.0 {
                bail!("Element '{}' needs a finite position and a positive size", a.id);
            }
            if let Element::MapFrame { scale, center, layers, basemap, graticule, grid, .. } = e {
                super::basemap::check(basemap)?;
                if let Some(g) = graticule {
                    GridSpec::parse(g, "graticule")?;
                }
                if let Some(g) = grid {
                    GridSpec::parse(g, "grid")?;
                }
                if !(scale.is_finite() && *scale > 0.0) {
                    bail!("Map frame '{}' needs a positive scale", a.id);
                }
                if !center.iter().all(|v| v.is_finite()) {
                    bail!("Map frame '{}' needs a finite centre", a.id);
                }
                if let FrameLayers::Follow(s) = layers {
                    if s != "map" {
                        bail!("Map frame '{}': layers must be \"map\" or a list", a.id);
                    }
                }
            }
        }
        for e in &self.elements {
            let frame = match e {
                Element::Legend { frame, .. } | Element::ScaleBar { frame, .. } | Element::NorthArrow { frame, .. } | Element::Sources { frame, .. } => frame,
                _ => continue,
            };
            let ok = self.elements.iter().any(|f| matches!(f, Element::MapFrame { at, .. } if &at.id == frame));
            if !ok {
                bail!("Element '{}' refers to '{frame}', which is not a map frame", e.at().id);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- styles

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StyleKind {
    #[default]
    Single,
    Categorized,
    Graduated,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StyleClass {
    pub value: String,
    pub color: String,
    pub label: String,
}

/// Mirror of `LayerStyle` in ui/src/store/store.ts. A drift test compares it
/// with `tests/fixtures/layer_style.json`, which a vitest checks too.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayerStyle {
    pub kind: StyleKind,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f64,
    pub opacity: f64,
    pub radius: f64,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub classes: Option<Vec<StyleClass>>,
    #[serde(default)]
    pub breaks: Option<Vec<f64>>,
    #[serde(default)]
    pub colors: Option<Vec<String>>,
    /// Feature labels (L1); layouts draw them from L2 on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<LabelStyle>,
}

/// Label edits of one layer in one map frame (L3): a partial style for this layout only, and per-feature edits keyed by `__fid`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LabelOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<LabelPatch>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<String, FeatureLabel>,
}

/// Unset fields fall back to the layer's `LayerStyle.label` (the map's setting).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LabelPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halo_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halo_width: Option<f64>,
}

/// One label's edits. Any of `dxMm`, `dyMm`, `rotation`, `text`, `sizePt` pins the label: placed first, never dropped by collision.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeatureLabel {
    /// Move from the automatic position, page mm (right and down are positive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dx_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dy_mm: Option<f64>,
    /// Degrees on the page, clockwise; replaces the automatic angle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_pt: Option<f64>,
    /// Not drawn, and the automatic label does not come back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

impl FeatureLabel {
    pub fn pinned(&self) -> bool {
        self.dx_mm.is_some() || self.dy_mm.is_some() || self.rotation.is_some() || self.text.is_some() || self.size_pt.is_some()
    }
}

/// Mirror of `LabelStyle` in ui/src/store/store.ts.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabelStyle {
    pub enabled: bool,
    pub field: String,
    /// Text size, pt.
    pub size: f64,
    pub color: String,
    pub halo_color: String,
    /// Halo width, px; 0 = none.
    pub halo_width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scale: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scale: Option<u64>,
}

impl Default for LayerStyle {
    fn default() -> Self {
        LayerStyle {
            kind: StyleKind::Single,
            fill: "#7aa8d8".into(),
            stroke: "#4d4d4d".into(),
            stroke_width: 0.7,
            opacity: 1.0,
            radius: 3.5,
            field: None,
            classes: None,
            breaks: None,
            colors: None,
            label: None,
        }
    }
}

/// Everything one render needs from the UI besides the layout itself.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RenderRequest {
    pub layout: Layout,
    /// Per layer id. BTreeMap: output order never depends on hashing.
    #[serde(default)]
    pub styles: BTreeMap<String, LayerStyle>,
    /// Draw order, top layer first (as in the Contents pane).
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub visible: BTreeMap<String, bool>,
    /// Text token `{project}`.
    #[serde(default)]
    pub project: String,
    /// Text token `{date}`; supplied by the caller so renders stay reproducible.
    #[serde(default)]
    pub date: String,
    /// Resolution of raster layers and base maps drawn into frames, dpi
    /// (default 300, clamped to 72–300: PDF images are capped at 300 dpi).
    #[serde(default)]
    pub raster_dpi: Option<f64>,
}
