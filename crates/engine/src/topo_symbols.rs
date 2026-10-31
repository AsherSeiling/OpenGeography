//! The topographic symbol catalogue: the symbols of the USGS "Topographic Map Symbols" sheet, redrawn as plain geometry
//! (strokes, fills, patterns, markers), plus the overlay symbols of a military installation map (DMA series V741S, Fort
//! Campbell). One definition here; the UI reads it through `topo_symbols` and merges a symbol's [`SymbolStyle`] into a
//! layer's style, so any layer can wear any symbol. Categories keep the headings printed on the sheet.
//!
//! Two eras share the catalogue: **classic** (the printed look of 1970s–80s quadrangles: red-filled primary roads, pink
//! built-up-area tint) and **modern** (grey built-up tint, thinner road fills). The USGS sheet and the DMA map are works of
//! the U.S. Government; nothing is copied as an image.

use crate::layout::model::{LayerStyle, StrokeSpec};
use serde::Serialize;

// Colours (sRGB), after the printed sheet.
pub const BROWN: &str = "#a8662f";
pub const BLUE: &str = "#169bd0";
pub const BLUE_TINT: &str = "#c8ebf7";
pub const GREEN_TINT: &str = "#cfe8c5";
pub const GREEN: &str = "#4f9a42";
pub const RED: &str = "#c8202f";
pub const BLACK: &str = "#000000";
pub const PINK_TINT: &str = "#f6d3ca";
pub const GREY_TINT: &str = "#e4e4e4";
pub const TAN_TINT: &str = "#f1dcc3";

/// What a symbol sets on a layer's style; the UI merges it over the layer's own style (fields left out stay as they are).
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SymbolStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strokes: Option<Vec<StrokeSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_spacing: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TopoSymbol {
    pub id: &'static str,
    /// The heading on the USGS sheet ("Roads and related features"), or "Military overlay".
    pub category: &'static str,
    pub name: &'static str,
    /// "line" | "area" | "point": the kind of layer it suits.
    pub geom: &'static str,
    pub style: SymbolStyle,
}

fn st(color: &str, width: f64) -> StrokeSpec {
    StrokeSpec { color: color.into(), width, dash: Vec::new(), cap: None }
}
fn dashed(color: &str, width: f64, dash: &[f64]) -> StrokeSpec {
    StrokeSpec { color: color.into(), width, dash: dash.to_vec(), cap: Some("butt".into()) }
}
/// Cross-ties / ticks: a thick stroke with a hairline dash paints short lines across the road line.
fn ticks(color: &str, length: f64, gap: f64) -> StrokeSpec {
    StrokeSpec { color: color.into(), width: length, dash: vec![0.9, gap], cap: Some("butt".into()) }
}

fn line(id: &'static str, category: &'static str, name: &'static str, strokes: Vec<StrokeSpec>) -> TopoSymbol {
    TopoSymbol { id, category, name, geom: "line", style: SymbolStyle { fill: strokes.last().map(|s| s.color.clone()), strokes: Some(strokes), ..Default::default() } }
}

#[allow(clippy::too_many_arguments)]
fn area(id: &'static str, category: &'static str, name: &'static str, fill: &str, opacity: f64, outline: Option<(&str, f64)>, pattern: Option<(&str, f64)>, strokes: Option<Vec<StrokeSpec>>) -> TopoSymbol {
    let (stroke, stroke_width) = match outline {
        Some((c, w)) => (Some(c.to_string()), Some(w)),
        None => (Some(fill.to_string()), Some(0.0)),
    };
    TopoSymbol { id, category, name, geom: "area", style: SymbolStyle { fill: Some(fill.into()), stroke, stroke_width, opacity: Some(opacity), strokes, pattern: pattern.map(|p| p.0.to_string()), pattern_spacing: pattern.map(|p| p.1), ..Default::default() } }
}

fn point(id: &'static str, category: &'static str, name: &'static str, marker: &str, color: &str, radius: f64) -> TopoSymbol {
    TopoSymbol { id, category, name, geom: "point", style: SymbolStyle { fill: Some(color.into()), marker: Some(marker.into()), radius: Some(radius), stroke: Some(color.into()), ..Default::default() } }
}

/// The marks of a patterned area are drawn in the symbol's outline colour: set it (the outline itself stays off unless the symbol has one).
fn marks(mut s: TopoSymbol, color: &str) -> TopoSymbol {
    if s.style.stroke_width.unwrap_or(0.0) == 0.0 {
        s.style.stroke = Some(color.into());
    }
    s
}

/// The whole catalogue for an era ("classic" or "modern"; anything else is classic).
pub fn symbols(era: &str) -> Vec<TopoSymbol> {
    let classic = era != "modern";
    let built_up = if classic { PINK_TINT } else { GREY_TINT };
    const ROADS: &str = "Roads and related features";
    const BND: &str = "Boundaries";
    const RIV: &str = "Rivers, lakes, and canals";
    const CON: &str = "Contours";
    const RAIL: &str = "Railroads and related features";
    const VEG: &str = "Vegetation";
    const SUB: &str = "Submerged areas and bogs";
    const SURF: &str = "Surface features";
    const BLD: &str = "Buildings and related features";
    const CTL: &str = "Control data and monuments";
    const LAND: &str = "Land surveys";
    const TRANS: &str = "Transmission lines and pipelines";
    const MINE: &str = "Mines and caves";
    const COAST: &str = "Coastal features";
    let road_fill = if classic { RED } else { "#d8484f" };
    vec![
        // ---- Roads (red-filled with black casing in the classic print; double black lines for light duty)
        line("road_primary", ROADS, "Primary highway", vec![st(BLACK, 4.2), st(road_fill, 2.6)]),
        line("road_secondary", ROADS, "Secondary highway", vec![st(BLACK, 3.4), st(road_fill, 1.7)]),
        line("road_light_paved", ROADS, "Light duty road, paved", vec![st(BLACK, 2.8), st("#ffffff", 1.4)]),
        line("road_light_gravel", ROADS, "Light duty road, gravel", vec![st(BLACK, 2.8), st("#ffffff", 1.4), dashed(BLACK, 0.5, &[5.0, 3.0])]),
        line("road_unimproved", ROADS, "Unimproved road", vec![dashed(BLACK, 2.4, &[5.0, 3.0]), dashed("#ffffff", 1.0, &[5.0, 3.0])]),
        line("road_4wd", ROADS, "4WD road", vec![dashed(BLACK, 1.6, &[3.0, 3.0])]),
        line("road_trail", ROADS, "Trail", vec![dashed(BLACK, 1.0, &[3.5, 2.5])]),
        line("road_median", ROADS, "Highway or road with median strip", vec![st(BLACK, 6.4), st(road_fill, 5.0), st(BLACK, 1.6)]),
        line("road_construction", ROADS, "Highway or road under construction", vec![dashed(road_fill, 2.4, &[7.0, 3.0]), dashed(BLACK, 0.5, &[7.0, 3.0])]),
        // ---- Boundaries (black dash patterns)
        line("bnd_national", BND, "National boundary", vec![dashed(BLACK, 1.2, &[10.0, 2.0, 2.0, 2.0])]),
        line("bnd_state", BND, "State or territorial boundary", vec![dashed(BLACK, 1.0, &[9.0, 2.0, 2.0, 2.0, 2.0, 2.0])]),
        line("bnd_county", BND, "County or equivalent", vec![dashed(BLACK, 0.9, &[12.0, 3.0, 4.0, 3.0])]),
        line("bnd_township", BND, "Civil township or equivalent", vec![dashed(BLACK, 0.7, &[8.0, 3.0])]),
        line("bnd_city", BND, "Incorporated city or equivalent", vec![dashed(BLACK, 0.7, &[3.0, 2.0])]),
        line("bnd_park_small", BND, "Small park (county or city)", vec![dashed(BLACK, 0.6, &[3.0, 3.0])]),
        line("bnd_forest_admin", BND, "Forest Service administrative area", vec![dashed(BLACK, 0.9, &[10.0, 2.0, 2.0, 2.0])]),
        area("bnd_federal_park", BND, "Federally administered park, reservation, or monument", PINK_TINT, 0.8, Some((RED, 0.8)), None, Some(vec![dashed(RED, 0.9, &[9.0, 2.0, 2.0, 2.0])])),
        area("bnd_national_forest", BND, "National Forest System land status, Forest Service lands", PINK_TINT, 0.55, Some((BLACK, 0.5)), None, None),
        // ---- Hydrography
        line("hyd_stream", RIV, "Perennial stream", vec![st(BLUE, 1.0)]),
        line("hyd_stream_int", RIV, "Intermittent stream", vec![dashed(BLUE, 0.9, &[6.0, 2.0, 1.0, 2.0])]),
        line("hyd_canal", RIV, "Canal, flume, or aqueduct", vec![st(BLUE, 1.2), ticks(BLUE, 3.0, 12.0)]),
        line("hyd_aqueduct_tunnel", RIV, "Aqueduct tunnel", vec![dashed(BLUE, 1.0, &[4.0, 2.0])]),
        line("hyd_disappearing", RIV, "Disappearing stream", vec![dashed(BLUE, 0.9, &[2.0, 3.0])]),
        area("hyd_lake", RIV, "Perennial lake/pond", BLUE_TINT, 1.0, Some((BLUE, 0.8)), None, None),
        area("hyd_lake_int", RIV, "Intermittent lake/pond", BLUE_TINT, 1.0, Some((BLUE, 0.7)), Some(("dots", 3.0)), None),
        area("hyd_lake_dry", RIV, "Dry lake/pond", TAN_TINT, 1.0, Some((BROWN, 0.7)), Some(("sand", 3.0)), None),
        area("hyd_river", RIV, "Perennial river", BLUE_TINT, 1.0, Some((BLUE, 0.8)), None, None),
        area("hyd_wash", RIV, "Wide wash", TAN_TINT, 1.0, Some((BROWN, 0.5)), Some(("sand", 3.0)), None),
        point("hyd_spring", RIV, "Spring or seep", "ring", BLUE, 2.0),
        point("hyd_well", BLD, "Well (other than water), windmill or wind generator", "ring", BLACK, 2.0),
        // ---- Contours
        line("con_index", CON, "Index contour", vec![st(BROWN, 1.5)]),
        line("con_intermediate", CON, "Intermediate contour", vec![st(BROWN, 0.7)]),
        line("con_supplementary", CON, "Supplementary contour", vec![dashed(BROWN, 0.6, &[4.0, 2.0])]),
        line("con_approx", CON, "Approximate or indefinite contour", vec![dashed(BROWN, 0.7, &[6.0, 3.0])]),
        line("con_bathy_index", CON, "Bathymetric index contour", vec![st(BLUE, 1.2)]),
        line("con_bathy", CON, "Bathymetric intermediate contour", vec![st(BLUE, 0.6)]),
        line("con_divide", CON, "Continental divide", vec![dashed(BROWN, 1.6, &[9.0, 3.0])]),
        // ---- Railroads
        line("rail_standard", RAIL, "Standard gauge railroad, single track", vec![st(BLACK, 1.0), ticks(BLACK, 6.0, 12.0)]),
        line("rail_multiple", RAIL, "Standard gauge railroad, multiple track", vec![st(BLACK, 4.4), st("#ffffff", 2.4), ticks(BLACK, 7.0, 12.0)]),
        line("rail_narrow", RAIL, "Narrow gauge railroad", vec![st(BLACK, 1.0), ticks(BLACK, 3.4, 12.0)]),
        line("rail_siding", RAIL, "Railroad siding", vec![st(BLACK, 0.8), ticks(BLACK, 4.0, 18.0)]),
        // ---- Vegetation (tints and stipples)
        area("veg_woodland", VEG, "Woodland", GREEN_TINT, 1.0, None, None, None),
        marks(area("veg_scrub", VEG, "Shrubland", GREEN_TINT, 0.6, None, Some(("scrub", 3.5)), None), GREEN),
        marks(area("veg_orchard", VEG, "Orchard", "#ffffff", 1.0, None, Some(("orchard", 4.5)), None), GREEN),
        marks(area("veg_vineyard", VEG, "Vineyard", "#ffffff", 1.0, None, Some(("vineyard", 5.0)), None), GREEN),
        // ---- Submerged areas and bogs
        marks(area("sub_marsh", SUB, "Marsh or swamp", "#ffffff", 1.0, None, Some(("marsh", 6.0)), None), BLUE),
        marks(area("sub_swamp_wooded", SUB, "Wooded marsh or swamp", GREEN_TINT, 1.0, None, Some(("marsh", 6.0)), None), BLUE),
        marks(area("sub_marsh_submerged", SUB, "Submerged marsh or swamp", BLUE_TINT, 1.0, None, Some(("marsh", 6.0)), None), BLUE),
        marks(area("sub_inundation", SUB, "Land subject to inundation", BLUE_TINT, 0.7, None, Some(("horizontal", 3.0)), None), BLUE),
        // ---- Surface features
        line("surf_levee", SURF, "Levee", vec![st(BROWN, 0.8), ticks(BROWN, 3.0, 8.0)]),
        marks(area("surf_sand", SURF, "Sand or mud", TAN_TINT, 1.0, None, Some(("sand", 3.0)), None), BROWN),
        marks(area("surf_gravel", SURF, "Gravel beach or glacial moraine", "#ffffff", 1.0, None, Some(("gravel", 3.2)), None), BROWN),
        area("surf_disturbed", SURF, "Disturbed surface", TAN_TINT, 1.0, None, None, None),
        area("surf_tailings_pond", SURF, "Tailings pond", TAN_TINT, 1.0, Some((BROWN, 0.6)), Some(("marsh", 5.0)), None),
        // ---- Buildings and built-up areas
        area("bld_built_up", BLD, if classic { "Built-up area (classic pink tint)" } else { "Built-up area" }, built_up, 1.0, None, None, None),
        area("bld_building", BLD, "Building (large, to scale)", BLACK, 1.0, Some((BLACK, 0.4)), None, None),
        point("bld_house", BLD, "Building", "square", BLACK, 1.8),
        point("bld_school", BLD, "School; house of worship", "church", BLACK, 2.2),
        point("bld_tank", BLD, "Tank", "circle", BLACK, 2.4),
        point("bld_cemetery", BLD, "Cemetery", "cross", BLACK, 2.2),
        point("bld_campground", BLD, "Campground", "triangle", RED, 2.6),
        point("bld_picnic", BLD, "Picnic area", "square", RED, 2.2),
        area("bld_airport_unpaved", BLD, "Unpaved landing strip", "#ffffff", 1.0, Some((BLACK, 0.5)), None, Some(vec![dashed(BLACK, 0.6, &[5.0, 3.0])])),
        // ---- Control data and monuments
        point("ctl_bench_mark", CTL, "Bench mark / vertical control (×)", "x_thin", BLACK, 2.4),
        point("ctl_spot_elevation", CTL, "Spot elevation", "x_thin", BLACK, 1.6),
        point("ctl_horizontal", CTL, "Horizontal control, third-order or better", "tri_open", BLACK, 2.6),
        point("ctl_monument", CTL, "Boundary monument; mineral or location monument", "triangle", BLACK, 1.8),
        point("ctl_unmonumented", CTL, "Unmonumented control", "plus_thin", BLACK, 2.2),
        point("ctl_principal", CTL, "Principal point", "ring", BLACK, 2.6),
        // ---- Land surveys (public land survey system, red)
        line("pls_township", LAND, "Range or Township line", vec![st(RED, 1.1)]),
        line("pls_township_approx", LAND, "Range or Township line, location approximate", vec![dashed(RED, 1.0, &[8.0, 2.0])]),
        line("pls_section", LAND, "Section line", vec![st(RED, 0.6)]),
        line("pls_section_approx", LAND, "Section line, location approximate", vec![dashed(RED, 0.6, &[8.0, 2.0])]),
        line("pls_protracted", LAND, "Protracted survey line", vec![dashed(RED, 0.6, &[2.5, 1.5])]),
        line("pls_fence", LAND, "Fence or field lines", vec![dashed(BLACK, 0.7, &[5.0, 3.0])]),
        // ---- Transmission lines and pipelines
        line("tx_power", TRANS, "Power transmission line", vec![st(BLACK, 0.7), ticks(BLACK, 4.0, 60.0)]),
        line("tx_telephone", TRANS, "Telephone line", vec![dashed(BLACK, 0.6, &[6.0, 3.0])]),
        line("tx_pipeline", TRANS, "Aboveground pipeline", vec![st(BLACK, 0.8)]),
        line("tx_pipeline_under", TRANS, "Underground pipeline", vec![dashed(BLACK, 0.8, &[6.0, 3.0])]),
        // ---- Mines and caves
        point("mine_quarry", MINE, "Quarry or open pit mine", "x", BLACK, 2.2),
        point("mine_prospect", MINE, "Prospect", "x_thin", BLACK, 1.8),
        marks(area("mine_tailings", MINE, "Tailings", TAN_TINT, 1.0, None, Some(("gravel", 3.0)), None), BROWN),
        // ---- Coastal features
        line("coast_shoreline", COAST, "Shoreline", vec![st(BLUE, 1.0)]),
        line("coast_seawall", COAST, "Seawall", vec![st(BLUE, 1.2), ticks(BLUE, 3.0, 6.0)]),
        point("coast_rock", COAST, "Rock, bare or awash; dangerous to navigation", "plus_thin", BLUE, 2.2),
        // ---- Military overlay (DMA series V741S, Fort Campbell Military Installation Map)
        line("mil_range_boundary", MIL, "Range boundary", vec![st(RED, 1.6)]),
        area("mil_impact_area", MIL, "Impact area and demolition area", "#f2a9a9", 0.55, Some((RED, 1.4)), None, None),
        area("mil_drop_zone", MIL, "Drop zone", "#ffffff", 0.0, Some((RED, 1.2)), None, None),
        area("mil_recreation", MIL, "Recreation / environment area (off limits for tactical training)", "#bfe6f2", 0.55, Some((BLUE, 1.0)), None, None),
        area("mil_training_area", MIL, "Training area", "#cde7bb", 0.6, Some((GREEN, 0.9)), None, None),
        area("mil_landing_zone", MIL, "Landing zone", "#7a7a7a", 0.85, Some((BLACK, 0.9)), None, None),
        area("mil_cantonment", MIL, "Cantonment / built-up military area", "#9fb9cc", 0.7, Some(("#3d5f7a", 0.8)), None, None),
        line("mil_restricted_boundary", MIL, "Restricted area boundary line", vec![dashed(BLACK, 1.4, &[8.0, 3.0, 2.0, 3.0])]),
        line("mil_atc_boundary", MIL, "Air traffic control boundary", vec![st("#4aa6d6", 3.2)]),
        point("mil_firing_point", MIL, "Firing point", "triangle", RED, 3.6),
        point("mil_observation_point", MIL, "Observation point", "tri_open", RED, 3.6),
        line("mil_grid_line", MIL, "Military grid line", vec![st("#8a6d3b", 0.5)]),
    ]
}

const MIL: &str = "Military overlay";

/// Look a symbol up by id.
pub fn find(id: &str, era: &str) -> Option<TopoSymbol> {
    symbols(era).into_iter().find(|s| s.id == id)
}

impl SymbolStyle {
    /// A layer style wearing this symbol (the base supplies what the symbol does not set).
    pub fn apply(&self, base: &LayerStyle) -> LayerStyle {
        let mut s = base.clone();
        if let Some(v) = &self.fill {
            s.fill = v.clone();
        }
        if let Some(v) = &self.stroke {
            s.stroke = v.clone();
        }
        if let Some(v) = self.stroke_width {
            s.stroke_width = v;
        }
        if let Some(v) = self.opacity {
            s.opacity = v;
        }
        if let Some(v) = self.radius {
            s.radius = v;
        }
        s.strokes = self.strokes.clone();
        s.pattern = self.pattern.clone();
        s.pattern_spacing = self.pattern_spacing;
        s.marker = self.marker.clone();
        s
    }
}

/// A small SVG sample of a symbol (7 × 5 mm viewBox, scaled by the caller), drawn by the layout renderer itself so it matches the page exactly.
pub fn preview_svg(sym: &TopoSymbol) -> String {
    let style = sym.style.apply(&LayerStyle::default());
    crate::layout::surround::swatch_svg(&style, sym.geom)
}

/// Every symbol of an era on one page (swatch and name, grouped by heading): a printable key and the visual check of the catalogue.
pub fn contact_sheet_svg(era: &str) -> String {
    use crate::layout::svg::esc;
    use std::fmt::Write;
    let all = symbols(era);
    let (cols, cw, ch, sw, sh) = (2usize, 105.0, 8.4, 14.0, 10.0);
    let mut body = String::new();
    let mut y = 10.0;
    let mut cats: Vec<&str> = Vec::new();
    for s in &all {
        if !cats.contains(&s.category) {
            cats.push(s.category);
        }
    }
    for c in cats {
        let _ = write!(body, "<text x=\"6\" y=\"{}\" font-family=\"Noto Sans\" font-weight=\"700\" font-size=\"4\">{}</text>", y + 3.0, esc(c));
        y += 6.0;
        let items: Vec<&TopoSymbol> = all.iter().filter(|s| s.category == c).collect();
        for (i, s) in items.iter().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let (x, yy) = (6.0 + col as f64 * cw, y + row as f64 * ch);
            let inner = preview_svg(s).replacen("width=\"70\" height=\"50\"", &format!("x=\"{x}\" y=\"{yy}\" width=\"{sw}\" height=\"{sh}\""), 1);
            body.push_str(&inner);
            let _ = write!(body, "<text x=\"{}\" y=\"{}\" font-family=\"Noto Sans\" font-size=\"3\">{}</text>", x + sw + 2.0, yy + 5.6, esc(s.name));
        }
        y += (items.len().div_ceil(cols)) as f64 * ch + 4.0;
    }
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\"><rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>{body}</svg>", w = 6.0 + cols as f64 * cw, h = y + 6.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_symbol_draws_something() {
        let all = symbols("classic");
        let mut ids: Vec<_> = all.iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "duplicate ids");
        assert!(all.len() >= 80, "{}", all.len());
        for s in &all {
            assert!(matches!(s.geom, "line" | "area" | "point"), "{}", s.id);
            match s.geom {
                "line" => assert!(s.style.strokes.as_ref().is_some_and(|v| !v.is_empty()), "{} has no strokes", s.id),
                "point" => assert!(s.style.marker.is_some() && s.style.radius.is_some(), "{}", s.id),
                _ => assert!(s.style.fill.is_some(), "{}", s.id),
            }
            let svg = preview_svg(s);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && svg.len() > 60, "{}: {svg}", s.id);
        }
    }

    #[test]
    fn categories_follow_the_sheet_and_the_eras_differ_only_where_the_print_did() {
        let classic = symbols("classic");
        for c in ["Roads and related features", "Boundaries", "Rivers, lakes, and canals", "Contours", "Railroads and related features", "Vegetation", "Land surveys", "Military overlay"] {
            assert!(classic.iter().any(|s| s.category == c), "{c}");
        }
        let modern = symbols("modern");
        assert_eq!(classic.len(), modern.len());
        let built = |v: &[TopoSymbol]| v.iter().find(|s| s.id == "bld_built_up").unwrap().style.fill.clone().unwrap();
        assert_eq!(built(&classic), PINK_TINT);
        assert_eq!(built(&modern), GREY_TINT);
        let diff: Vec<_> = classic.iter().zip(&modern).filter(|(a, b)| a.style != b.style).map(|(a, _)| a.id).collect();
        assert_eq!(diff, ["road_primary", "road_secondary", "road_median", "road_construction", "bld_built_up"]);
    }

    #[test]
    #[ignore = "writes the contact sheet to $TOPO_SHEET_DIR for a visual check"]
    fn write_contact_sheets() {
        let dir = std::env::var("TOPO_SHEET_DIR").unwrap_or_else(|_| "/tmp".into());
        for era in ["classic", "modern"] {
            let png = crate::layout::export::svg_to_png(&contact_sheet_svg(era), 110.0).unwrap();
            std::fs::write(format!("{dir}/topo_symbols_{era}.png"), png).unwrap();
        }
    }

    #[test]
    fn a_symbol_wears_on_a_layer_and_keeps_what_it_does_not_set() {
        let road = find("road_primary", "classic").unwrap();
        let base = LayerStyle { label: None, ..Default::default() };
        let s = road.style.apply(&base);
        assert_eq!(s.strokes.as_ref().unwrap().len(), 2);
        assert_eq!(s.strokes.as_ref().unwrap()[1].color, RED);
        assert!(find("nope", "classic").is_none());
    }
}
