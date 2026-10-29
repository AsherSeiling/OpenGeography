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
fn twenty() -> usize {
    20
}
fn eight() -> usize {
    8
}
fn bar() -> String {
    "bar".into()
}
fn rectangle() -> String {
    "rectangle".into()
}
fn one_f() -> f64 {
    1.0
}
fn half() -> f64 {
    0.5
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
    /// Text token `{author}`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Editor guides (mm); the snapping targets of the Layout view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Map series (C3.2): see [`super::series::SeriesSpec`].
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
    /// Print bleed, mm (0–10): the page background and anything placed past the trim extends this far.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bleed: f64,
    /// Draw crop (trim) marks outside the bleed.
    #[serde(default, rename = "cropMarks", skip_serializing_if = "std::ops::Not::not")]
    pub crop_marks: bool,
}

/// An alignment guide in the layout editor; not drawn in exports.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Guide {
    /// "x" (vertical line at x mm) or "y" (horizontal line at y mm).
    pub axis: String,
    pub pos: f64,
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
    /// Background, frame and drop shadow under the element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decor: Option<Decor>,
    /// Elements with the same group id are selected and moved together in the editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Background, frame and shadow of any layout element (QGIS item "Background / Frame / Drop shadow"; ArcGIS Pro element background, border, shadow).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Decor {
    pub background: Option<String>,
    pub frame_color: Option<String>,
    /// Frame line width, mm; 0 = no frame.
    pub frame_width: f64,
    pub corner_radius: f64,
    pub shadow: bool,
    /// Shadow offset right and down, mm.
    pub shadow_offset: f64,
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
        /// Overview/inset: draw the extent of another map frame on this one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        indicator: Option<ExtentIndicator>,
        /// The layout's map series moves this frame to each index feature (C3.2).
        #[serde(default, rename = "followsSeries", skip_serializing_if = "std::ops::Not::not")]
        follows_series: bool,
    },
    Text {
        #[serde(flatten)]
        at: Placement,
        text: String,
        /// "sans" | "sans-bold" | "sans-italic".
        #[serde(default = "sans")]
        font: String,
        /// An installed font family (`fonts::list`) used in place of the bundled face; the weight/style of `font` still applies. Missing → Noto Sans.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        family: Option<String>,
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
        /// Renamed or hidden entries, by generated key (see `LegendRowInfo`).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        overrides: BTreeMap<String, LegendOverride>,
        /// Extra hand-made entries at the end.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        custom: Vec<LegendCustom>,
        /// Area swatch shape: "" / "square" | "rounded" | "circle".
        #[serde(default, skip_serializing_if = "String::is_empty")]
        patch: String,
        /// Entry text size in points; 0 = default (8).
        #[serde(default, rename = "textSize", skip_serializing_if = "is_zero")]
        text_size: f64,
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
        /// "alternating" (default) | "hollow" | "line" | "ticks" | "double".
        #[serde(default, skip_serializing_if = "String::is_empty")]
        style: String,
        /// Style "double": the second bar's units.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        units2: String,
        /// Also print "1 cm on the map = N units".
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        verbal: bool,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        color: String,
    },
    NorthArrow {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        /// One of `surround::ARROW_STYLES`; empty = "simple".
        #[serde(default)]
        style: String,
        /// "true" (default) or "grid".
        #[serde(default, skip_serializing_if = "String::is_empty")]
        north: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        color: String,
        /// Style "custom": picture data URI whose top points north.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        source: String,
    },
    Sources {
        #[serde(flatten)]
        at: Placement,
        frame: String,
        #[serde(default)]
        size: f64,
    },
    /// Attribute data of a layer as a table.
    Table {
        #[serde(flatten)]
        at: Placement,
        /// Layer id.
        #[serde(default)]
        layer: String,
        /// Fields to show, in order; empty = the first five.
        #[serde(default)]
        fields: Vec<String>,
        #[serde(default, rename = "sortField")]
        sort_field: String,
        #[serde(default)]
        descending: bool,
        /// Most rows to show (also limited by the box height).
        #[serde(default = "twenty", rename = "maxRows")]
        max_rows: usize,
        /// Text size, pt; 0 = 7.
        #[serde(default)]
        size: f64,
        #[serde(default = "yes")]
        zebra: bool,
    },
    /// A bar or pie chart of a layer's categories.
    Chart {
        #[serde(flatten)]
        at: Placement,
        #[serde(default)]
        layer: String,
        /// "bar" | "pie".
        #[serde(default = "bar", rename = "chartType")]
        chart_type: String,
        /// Field that makes the categories.
        #[serde(default)]
        category: String,
        /// Numeric field to add up; empty = count features.
        #[serde(default)]
        value: String,
        /// "sum" | "avg" | "count".
        #[serde(default)]
        agg: String,
        /// Largest categories shown (1–12).
        #[serde(default = "eight", rename = "topN")]
        top_n: usize,
        #[serde(default)]
        title: String,
        #[serde(default)]
        size: f64,
    },
    /// A rectangle, ellipse, triangle, line or arrow.
    Shape {
        #[serde(flatten)]
        at: Placement,
        /// "rectangle" | "ellipse" | "triangle" | "line" | "arrow".
        #[serde(default = "rectangle")]
        shape: String,
        #[serde(default = "white")]
        fill: String,
        #[serde(default = "one_f", rename = "fillOpacity")]
        fill_opacity: f64,
        #[serde(default = "black")]
        stroke: String,
        /// Line width, mm.
        #[serde(default = "half", rename = "strokeWidth")]
        stroke_width: f64,
        /// "solid" | "dashed" | "dotted".
        #[serde(default)]
        dash: String,
        #[serde(default, rename = "cornerRadius")]
        corner_radius: f64,
    },
    /// A logo or picture (PNG, JPEG or SVG), embedded in the layout as a data URI.
    Picture {
        #[serde(flatten)]
        at: Placement,
        /// `data:image/png|jpeg|svg+xml;base64,…`; empty = placeholder.
        #[serde(default)]
        source: String,
        /// "contain" | "cover" | "stretch".
        #[serde(default)]
        fit: String,
        #[serde(default = "one_f")]
        opacity: f64,
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
            | Element::Sources { at, .. }
            | Element::Shape { at, .. }
            | Element::Table { at, .. }
            | Element::Chart { at, .. }
            | Element::Picture { at, .. } => at,
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
            Element::Shape { .. } => "shape",
            Element::Table { .. } => "table",
            Element::Chart { .. } => "chart",
            Element::Picture { .. } => "picture",
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
        if !(0.0..=10.0).contains(&self.page.bleed) {
            bail!("Bleed must be between 0 and 10 mm");
        }
        if !(self.page.width.is_finite() && self.page.height.is_finite() && self.page.width > 0.0 && self.page.height > 0.0) {
            bail!("Page size must be positive");
        }
        if let Some(s) = &self.series {
            super::series::SeriesSpec::parse(s)?;
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
            if let Element::Chart { chart_type, agg, .. } = e {
                super::tabular::check_chart(chart_type, agg)?;
            }
            if let Element::Shape { shape, .. } = e {
                super::shapes::check_shape(shape)?;
            }
            if let Element::ScaleBar { style, units2, .. } = e {
                if !style.is_empty() && !super::surround::BAR_STYLES.contains(&style.as_str()) {
                    bail!("Unknown scale bar style '{style}' ({})", super::surround::BAR_STYLES.join(", "));
                }
                if !units2.is_empty() && !["km", "mi", "m", "ft"].contains(&units2.as_str()) {
                    bail!("Unknown scale bar units '{units2}' (km, mi, m or ft)");
                }
            }
            if let Element::NorthArrow { style, north, source, .. } = e {
                if !style.is_empty() && !super::surround::ARROW_STYLES.contains(&style.as_str()) {
                    bail!("Unknown north arrow style '{style}' ({})", super::surround::ARROW_STYLES.join(", "));
                }
                if !matches!(north.as_str(), "" | "true" | "grid") {
                    bail!("North arrow direction must be \"true\" or \"grid\"");
                }
                super::shapes::check_picture(source)?;
            }
            if let Element::MapFrame { indicator: Some(ind), .. } = e {
                if ind.frame == a.id {
                    bail!("Map frame '{}' cannot show its own extent; choose another frame", a.id);
                }
                if !self.elements.iter().any(|f| matches!(f, Element::MapFrame { at, .. } if at.id == ind.frame)) {
                    bail!("Map frame '{}' shows the extent of '{}', which is not a map frame", a.id, ind.frame);
                }
            }
            if let Element::Legend { patch, .. } = e {
                if !matches!(patch.as_str(), "" | "square" | "rounded" | "circle") {
                    bail!("Unknown legend patch '{patch}' (square, rounded or circle)");
                }
            }
            if let Element::Picture { source, .. } = e {
                super::shapes::check_picture(source)?;
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
    /// Raster symbology; the layout draws rasters exactly as the map does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<crate::raster_style::RasterStyle>,
}

/// An overview map's rectangle showing where another frame looks (QGIS "Overview", ArcGIS "Extent indicator").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtentIndicator {
    /// Id of the map frame whose extent is drawn.
    pub frame: String,
    #[serde(default = "indicator_color")]
    pub color: String,
    /// Outline width, mm.
    #[serde(default = "indicator_width")]
    pub width: f64,
    /// Fill opacity 0–1 (0 = outline only).
    #[serde(default)]
    pub fill_opacity: f64,
}

fn indicator_color() -> String {
    "#d95f02".into()
}
fn indicator_width() -> f64 {
    0.6
}

/// A legend entry the user renamed or hid.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct LegendOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

/// A hand-made legend entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LegendCustom {
    pub label: String,
    #[serde(default)]
    pub color: String,
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
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
    /// Only features with INDEX = 1 (index contours) are labelled.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub index_only: bool,
    /// An installed font family (`fonts::list`); None = the bundled Noto Sans. A family that is missing falls back to Noto Sans and the layout says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// Point labels (QGIS "Placement › Around point / Offset / Cartographic"): "right" (default), "left", "above", "below", "center", "aboveRight", "aboveLeft", "belowRight", "belowLeft".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// Extra gap between the symbol and the label, px.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub distance: f64,
    /// Line labels (QGIS "Above line / On line / Below line"): "above", "on" (default), "below".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_pos: Option<String>,
    /// QGIS "Rendering › Priority", 0 (lowest) to 10; labels with a higher number are placed first. Default 5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    /// QGIS "Allow overlaps": draw even where it covers another label.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_overlap: bool,
    /// QGIS "Formatting › Wrap on character", as a width in characters: lines are broken at spaces to fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap_at: Option<u32>,
    /// Multi-line alignment: "left", "center" (default), "right".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// QGIS "Background".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<LabelBackground>,
    /// QGIS "Shadow".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<LabelShadow>,
    /// QGIS data-defined rotation (degrees clockwise), size (pt) and colour ("#rrggbb"), read from fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_field: Option<String>,
    /// Contour labels: on a line layer, repeat the label along each line (see `ContourLabels`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour: Option<ContourLabels>,
}

/// Labels placed along contour lines at intervals, upright, with the line broken under the text or the text above it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContourLabels {
    /// Distance between labels along a line, px (96 per inch).
    pub spacing: f64,
    /// Break the line under the label (the text sits in the line) instead of above it.
    #[serde(default)]
    pub in_line: bool,
    /// Decimals of the elevation value; None = as stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    /// Text added after the value, e.g. " m" or " ft".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LabelBackground {
    pub fill: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// Space between the text and the edge, px.
    #[serde(default)]
    pub padding: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LabelShadow {
    pub color: String,
    /// Offset right and down, px.
    pub dx: f64,
    pub dy: f64,
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
            raster: None,
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
    /// Text token `{author}`.
    #[serde(default)]
    pub author: String,
    /// Resolution of raster layers and base maps drawn into frames, dpi
    /// (default 300, clamped to 72–300: PDF images are capped at 300 dpi).
    #[serde(default)]
    pub raster_dpi: Option<f64>,
    /// Draw this page (0-based) of the layout's map series instead of the frames as saved (C3.2).
    #[serde(default)]
    pub series_page: Option<usize>,
    /// The series page being drawn, for the `{page_*}` text tokens; filled in by the engine.
    #[serde(default)]
    pub page: Option<PageCtx>,
}

/// The series page a render is for: `{page_number}`, `{page_count}`, `{page_name}` and `{page_<field>}` of the index feature.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct PageCtx {
    /// 1-based.
    pub number: usize,
    pub count: usize,
    pub name: String,
    pub attrs: BTreeMap<String, String>,
}
