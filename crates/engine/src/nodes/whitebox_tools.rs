//! WhiteboxTools as model nodes (Wave 1, package T1): terrain, distance, stream-network and focal-filter tools run by the bundled
//! WhiteboxTools v2.4.0 program (MIT, John Lindsay; https://github.com/jblindsay/whitebox-tools) through `whitebox.rs`.
//!
//! One table (`DEFS`) holds each tool's ports, settings and WhiteboxTools flags; `Wbt` turns a row into a `Tool`, so `ToolSpec` stays the
//! single definition. Every raster result comes back on the first input's grid and gets that input's CRS written into it (never guessed).
//! Tools whose answer depends on the size of a cell in metres refuse a raster in degrees and tell the user to project it first (the same
//! rule as terrain on geographic DEMs; WhiteboxTools would read degrees as metres without any warning).
//! Viewshed, the cost tools and the Euclidean tools have their own structs because they take points or run two programs in turn.

use super::hydrology_tools::{band_range, restamp};
use super::*;
use crate::raster;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum Pd {
    None,
    Num(f64),
    Bool(bool),
    Str(&'static str),
}

/// A raster (or points) input and the WhiteboxTools flag it feeds.
pub struct WIn {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: PortKind,
    pub flag: &'static str,
}

/// A setting. `flags` are every WhiteboxTools flag it sets (a window size sets both `filterx` and `filtery`). `min` is the smallest
/// allowed number (NaN = no limit); `whole` requires a whole number.
pub struct WP {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub def: Pd,
    pub flags: &'static [&'static str],
    pub options: &'static [(&'static str, &'static str)],
    pub min: f64,
    pub whole: bool,
    pub help: &'static str,
}

#[derive(Clone, Copy)]
pub enum Geo {
    /// Cell counts and elevation differences only: any CRS works.
    Any,
    /// Needs the cell size in metres or feet.
    Projected,
    /// Needs a projected raster unless this setting has this value (flow accumulation in cells).
    ProjectedUnless(&'static str, &'static str),
}

pub struct WDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    /// WhiteboxTools' own tool name.
    pub tool: &'static str,
    pub ins: &'static [WIn],
    pub params: &'static [WP],
    pub out_flag: &'static str,
    /// Flags that are always passed (e.g. the ESRI pointer scheme).
    pub fixed: &'static [(&'static str, &'static str)],
    pub geo: Geo,
    pub look: ResultLook,
    /// The result is in the same units as the first input (heights), so it keeps the vertical datum label.
    pub keeps_heights: bool,
    pub keywords: &'static [&'static str],
}

const R: PortKind = PortKind::Raster;
const NAN: f64 = f64::NAN;
const NOFIX: &[(&str, &str)] = &[];
const PNT: &[(&str, &str)] = &[("esri_pntr", "true")];

const fn rin(id: &'static str, label: &'static str, flag: &'static str) -> WIn {
    WIn { id, label, kind: R, flag }
}
const DEM: WIn = rin("in", "Elevation raster (DEM)", "dem");
const FILTER_IN: WIn = rin("in", "Raster", "input");
const PNTR: WIn = rin("pntr", "Flow direction raster (ESRI codes)", "d8_pntr");
const STREAMS: WIn = rin("streams", "Streams raster", "streams");

const fn wp(id: &'static str, label: &'static str, kind: ParamKind, def: Pd, flags: &'static [&'static str]) -> WP {
    WP { id, label, kind, def, flags, options: &[], min: NAN, whole: false, help: "" }
}
const fn num(id: &'static str, label: &'static str, def: f64, flags: &'static [&'static str], min: f64, whole: bool, help: &'static str) -> WP {
    WP { id, label, kind: ParamKind::Number, def: Pd::Num(def), flags, options: &[], min, whole, help }
}
/// Square moving window, in cells.
const fn window(def: f64) -> WP {
    num("size", "Window size (cells)", def, &["filterx", "filtery"], 1.0, true, "Width and height of the square moving window, in cells. An even size is made one cell larger so the window is centred on the cell.")
}

pub static DEFS: &[WDef] = &[
    // ---- Curvature (Zevenbergen and Thorne 1987 / Florinsky; WhiteboxTools)
    WDef { id: "plan_curvature", name: "Plan Curvature", category: "Raster › Terrain and Contours", tool: "PlanCurvature", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Curvature of the contour lines across the surface (Zevenbergen and Thorne 1987 as implemented in WhiteboxTools: PlanCurvature): positive on ridges where contours bend outward and flow diverges, negative in hollows where they bend inward and flow converges. On a circular bowl it equals minus 1 over the radius of the contour. Result in 1/metre when the heights and the cell size are in metres. Needs a projected DEM.",
        params: &[wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"])],
        keywords: &["contour curvature", "convergence", "divergence", "ridge", "valley", "surface shape"] },
    WDef { id: "profile_curvature", name: "Profile Curvature", category: "Raster › Terrain and Contours", tool: "ProfileCurvature", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Curvature in the direction of steepest slope (Zevenbergen and Thorne 1987 as implemented in WhiteboxTools: ProfileCurvature): positive on convex slopes where flow speeds up, negative on concave slopes where it slows down. Equals the second derivative along the slope divided by (1 + slope squared) to the power 1.5. Result in 1/metre when heights and cells are in metres. Needs a projected DEM.",
        params: &[wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"])],
        keywords: &["slope curvature", "convex", "concave", "flow acceleration", "surface shape"] },
    WDef { id: "total_curvature", name: "Total Curvature", category: "Raster › Terrain and Contours", tool: "TotalCurvature", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Total curvature as defined by Florinsky: the sum of the squared second derivatives of the surface, z_xx squared plus twice z_xy squared plus z_yy squared (WhiteboxTools: TotalCurvature). Zero on a plane, never negative, not corrected for slope. Result in 1/metre squared when heights and cells are in metres. Needs a projected DEM.",
        params: &[wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"])],
        keywords: &["curvature", "surface shape", "convex", "concave"] },
    WDef { id: "tangential_curvature", name: "Tangential Curvature", category: "Raster › Terrain and Contours", tool: "TangentialCurvature", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Curvature of the surface in the plane perpendicular to the steepest slope direction (Florinsky), which controls whether flow converges or diverges: negative where it converges (WhiteboxTools: TangentialCurvature). Result in 1/metre when heights and cells are in metres. Needs a projected DEM.",
        params: &[wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"])],
        keywords: &["curvature", "flow convergence", "surface shape"] },
    WDef { id: "gaussian_curvature", name: "Gaussian Curvature", category: "Raster › Terrain and Contours", tool: "GaussianCurvature", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "The product of the two principal curvatures (Gauss): positive on domes and bowls, negative on saddles and passes (WhiteboxTools: GaussianCurvature). Result in 1/metre squared when heights and cells are in metres. Needs a projected DEM.",
        params: &[wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"])],
        keywords: &["curvature", "saddle", "dome", "surface shape"] },
    // ---- Topographic position (window sizes are in cells, so any CRS works)
    WDef { id: "tpi", name: "Topographic Position Index (TPI)", category: "Raster › Terrain and Contours", tool: "DiffFromMeanElev", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: true,
        description: "Elevation of each cell minus the mean elevation in a square window around it (Weiss 2001; Jenness 2006). Positive on ridges and hilltops, negative in valleys, near zero on flats and mid-slopes. Result in the DEM's height unit (WhiteboxTools: DiffFromMeanElev). The window is in cells, so its ground size is the window times the cell size.",
        params: &[window(11.0)], keywords: &["topographic position", "ridge", "valley", "landform", "high pass", "difference from mean"] },
    WDef { id: "dev", name: "Deviation from Mean Elevation (DEV)", category: "Raster › Terrain and Contours", tool: "DevFromMeanElev", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "How far each cell is from the mean elevation of a square window, in standard deviations of that window (Wilson and Gallant 2000). Unitless. Large positive values are local highs, large negative values local lows (WhiteboxTools: DevFromMeanElev).",
        params: &[window(11.0)], keywords: &["z score", "local relief", "topographic position", "landform"] },
    WDef { id: "rel_topographic_position", name: "Relative Topographic Position", category: "Raster › Terrain and Contours", tool: "RelativeTopographicPosition", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Where each cell sits between the lowest (-1) and highest (+1) elevation of a square window around it, passing through the window mean at 0 (Newman et al. 2018). Unitless (WhiteboxTools: RelativeTopographicPosition).",
        params: &[window(11.0)], keywords: &["rtp", "local position", "landform"] },
    WDef { id: "elev_rel_minmax", name: "Elevation Relative to Min and Max", category: "Raster › Terrain and Contours", tool: "ElevRelativeToMinMax", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Each cell's elevation as a fraction of the whole DEM's range: 0 at the lowest cell, 1 at the highest. Unitless (WhiteboxTools: ElevRelativeToMinMax).",
        params: &[], keywords: &["normalize elevation", "percent of range", "relative height"] },
    WDef { id: "geomorphons", name: "Geomorphons (Landform Classes)", category: "Raster › Terrain and Contours", tool: "Geomorphons", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Classify every cell into one of 10 landforms (1 flat, 2 peak, 3 ridge, 4 shoulder, 5 spur, 6 slope, 7 hollow, 8 footslope, 9 valley, 10 pit) from the pattern of its line-of-sight neighbours (Jasiewicz and Stepinski 2013; WhiteboxTools: Geomorphons). The look-up distance is in cells and the flatness threshold in degrees of slope angle, which needs a projected DEM.",
        params: &[
            num("search", "Look-up distance (cells)", 100.0, &["search"], 1.0, true, ""),
            num("threshold", "Flatness threshold (degrees)", 1.0, &["threshold"], 0.0, false, ""),
            WP { id: "forms", label: "Ten landform classes", kind: ParamKind::Bool, def: Pd::Bool(true), flags: &["forms"], options: &[], min: NAN, whole: false, help: "Off gives the ternary pattern code instead of the 10 classes." },
        ],
        keywords: &["landforms", "terrain classification", "peak", "ridge", "valley", "pit"] },
    // ---- Hydrology
    WDef { id: "breach_depressions", name: "Breach Depressions", category: "Hydrology", tool: "BreachDepressions", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Make a DEM drain by cutting channels through pits and barriers instead of filling them (Lindsay 2016; WhiteboxTools: BreachDepressions). Usually preferred to filling because it changes far fewer cells. Heights stay in the DEM's unit. Run Flow Direction on the result. Maximum breach length is in cells.",
        params: &[
            num("max_depth", "Maximum breach depth (height units, 0 = no limit)", 0.0, &[], 0.0, false, ""),
            num("max_length", "Maximum breach length (cells, 0 = no limit)", 0.0, &[], 0.0, true, ""),
            wp("fill_pits", "Fill single-cell pits first", ParamKind::Bool, Pd::Bool(true), &["fill_pits"]),
        ],
        keywords: &["hydro conditioning", "sinks", "pits", "carve", "alternative to fill"] },
    WDef { id: "fd8_flow_accumulation", name: "Flow Accumulation (FD8, multiple flow)", category: "Hydrology", tool: "FD8FlowAccumulation", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::ProjectedUnless("out_type", "cells"), look: ResultLook::Derived, keeps_heights: false,
        description: "Upstream area through every cell, splitting flow between all downslope neighbours (Freeman 1991; Quinn et al. 1991; WhiteboxTools: FD8FlowAccumulation). Gives smoother, more realistic patterns on hillslopes than D8. Result in cells, in square metres (catchment area) or in metres (specific contributing area) when the DEM's cell size is in metres. Area results need a projected DEM; counts of cells work in any CRS. Use a depression-filled or breached DEM.",
        params: &[
            WP { id: "out_type", label: "Result", kind: ParamKind::Enum, def: Pd::Str("cells"), flags: &["out_type"], options: &[("cells", "Number of upstream cells"), ("catchment area", "Catchment area"), ("specific contributing area", "Specific contributing area")], min: NAN, whole: false, help: "" },
            num("exponent", "Convergence exponent", 1.1, &["exponent"], 0.0, false, "Larger values concentrate flow more (Freeman 1991 uses 1.1)."),
            wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"]),
        ],
        keywords: &["multiple flow direction", "contributing area", "upstream area", "runoff"] },
    WDef { id: "dinf_flow_accumulation", name: "Flow Accumulation (D-infinity)", category: "Hydrology", tool: "DInfFlowAccumulation", ins: &[rin("in", "Elevation raster (DEM)", "input")], out_flag: "output", fixed: NOFIX, geo: Geo::ProjectedUnless("out_type", "cells"), look: ResultLook::Derived, keeps_heights: false,
        description: "Upstream area through every cell using Tarboton's (1997) D-infinity flow directions, which split flow between at most two neighbours (WhiteboxTools: DInfFlowAccumulation). Result in cells, in square metres (catchment area) or in metres (specific contributing area) when the DEM's cell size is in metres. Area results need a projected DEM. Use a depression-filled or breached DEM.",
        params: &[
            WP { id: "out_type", label: "Result", kind: ParamKind::Enum, def: Pd::Str("sca"), flags: &["out_type"], options: &[("cells", "Number of upstream cells"), ("ca", "Catchment area"), ("sca", "Specific contributing area")], min: NAN, whole: false, help: "" },
            wp("log", "Log-transform the result", ParamKind::Bool, Pd::Bool(false), &["log"]),
        ],
        keywords: &["tarboton", "dinf", "contributing area", "upstream area"] },
    WDef { id: "wetness_index", name: "Topographic Wetness Index", category: "Hydrology", tool: "WetnessIndex", ins: &[rin("sca", "Specific contributing area raster", "sca"), rin("slope", "Slope raster (degrees)", "slope")], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "ln(a / tan b) (Beven and Kirkby 1979), high where water collects and soils stay wet. Takes a specific contributing area raster (Flow Accumulation FD8 or D-infinity, 'specific contributing area', in metres) and a slope raster in degrees (the Slope tool). Unitless (WhiteboxTools: WetnessIndex). Both inputs must be on the same grid.",
        params: &[], keywords: &["twi", "soil moisture", "saturation", "wetness"] },
    WDef { id: "stream_power_index", name: "Stream Power Index", category: "Hydrology", tool: "StreamPowerIndex", ins: &[rin("sca", "Specific contributing area raster", "sca"), rin("slope", "Slope raster (degrees)", "slope")], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Relative stream power, SCA^m x tan(slope) (Moore et al. 1991): where flowing water has most power to erode. Takes a specific contributing area raster and a slope raster in degrees. Unitless relative index (WhiteboxTools: StreamPowerIndex).",
        params: &[num("exponent", "SCA exponent (m)", 1.0, &["exponent"], 0.0, false, "")], keywords: &["spi", "erosion power", "erosivity"] },
    WDef { id: "hand", name: "Height Above Nearest Drainage (HAND)", category: "Hydrology", tool: "ElevationAboveStream", ins: &[DEM, STREAMS], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: true,
        description: "Height of each cell above the stream cell it drains to, following the downslope path (Rennó et al. 2008; Nobre et al. 2011; WhiteboxTools: ElevationAboveStream). Result in the DEM's height unit. Low HAND values are flood-prone. Streams come from Extract Streams; both rasters must be on the same grid.",
        params: &[], keywords: &["hand", "flood height", "above drainage", "height above stream", "flood prone"] },
    WDef { id: "downslope_distance_to_stream", name: "Downslope Distance to Stream", category: "Hydrology", tool: "DownslopeDistanceToStream", ins: &[DEM, STREAMS], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Length of the flow path from each cell down to the nearest stream cell, in the raster's map unit (metres for a metric CRS) (WhiteboxTools: DownslopeDistanceToStream). Needs a projected DEM; the DEM and streams must be on the same grid.",
        params: &[wp("dinf", "Use D-infinity flow paths instead of D8", ParamKind::Bool, Pd::Bool(false), &["dinf"])],
        keywords: &["flow path length", "distance to river", "flow distance"] },
    WDef { id: "strahler_order", name: "Stream Order (Strahler)", category: "Hydrology", tool: "StrahlerStreamOrder", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Strahler (1957) order of each stream link: headwater links are 1, and the order rises by one only where two links of equal order join. Unitless (WhiteboxTools: StrahlerStreamOrder). Takes the D8 flow direction raster (ESRI codes) and the streams raster from Extract Streams.",
        params: &[], keywords: &["stream ranking", "river order", "hierarchy"] },
    WDef { id: "shreve_magnitude", name: "Stream Magnitude (Shreve)", category: "Hydrology", tool: "ShreveStreamMagnitude", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Shreve (1966) magnitude of each stream link: the number of headwater links upstream of it. Unitless (WhiteboxTools: ShreveStreamMagnitude). Takes the D8 flow direction raster (ESRI codes) and the streams raster from Extract Streams.",
        params: &[], keywords: &["stream order", "headwaters", "link magnitude"] },
    WDef { id: "horton_order", name: "Stream Order (Horton)", category: "Hydrology", tool: "HortonStreamOrder", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Horton (1945) order, as modified by Strahler: a stream keeps the order of its main stem all the way to the outlet. Unitless (WhiteboxTools: HortonStreamOrder). Takes the D8 flow direction raster (ESRI codes) and the streams raster.",
        params: &[], keywords: &["stream ranking", "river order", "main stem"] },
    WDef { id: "stream_link_id", name: "Stream Link Identifier", category: "Hydrology", tool: "StreamLinkIdentifier", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Give each stream link, the reach between two junctions, its own number (WhiteboxTools: StreamLinkIdentifier). Takes the D8 flow direction raster (ESRI codes) and the streams raster.",
        params: &[], keywords: &["reach", "segment", "stream segments", "junctions"] },
    WDef { id: "subbasins", name: "Sub-basins (per Stream Link)", category: "Hydrology", tool: "Subbasins", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "The area draining to each stream link, numbered like Stream Link Identifier (WhiteboxTools: Subbasins). Takes the D8 flow direction raster (ESRI codes) and the streams raster.",
        params: &[], keywords: &["catchments", "watershed per reach", "drainage area"] },
    WDef { id: "strahler_basins", name: "Strahler-order Basins", category: "Hydrology", tool: "StrahlerOrderBasins", ins: &[PNTR, STREAMS], out_flag: "output", fixed: PNT, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Basins labelled with the Strahler order of the stream that drains them (WhiteboxTools: StrahlerOrderBasins). Takes the D8 flow direction raster (ESRI codes) and the streams raster.",
        params: &[], keywords: &["watersheds by order", "basins", "stream order"] },
    WDef { id: "isobasins", name: "Equal-size Basins (Isobasins)", category: "Hydrology", tool: "Isobasins", ins: &[DEM], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Divide a DEM into drainage basins of nearly the same area (WhiteboxTools: Isobasins). The target size is in cells; the ground area is that times the cell area. Use a depression-filled or breached DEM.",
        params: &[num("size", "Target basin size (cells)", 1000.0, &["size"], 1.0, true, "")], keywords: &["equal catchments", "split watershed", "subdivide basins"] },
    // ---- Focal statistics and filters (windows in cells)
    WDef { id: "focal_mean", name: "Focal Mean (Mean Filter)", category: "Raster › Focal", tool: "MeanFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Replace each cell with the mean of a square window around it (WhiteboxTools: MeanFilter). Smooths noise; the result keeps the input's unit. No-data cells are left out of the mean.",
        params: &[window(3.0)], keywords: &["smooth", "low pass", "blur", "neighbourhood mean", "moving average"] },
    WDef { id: "focal_median", name: "Focal Median (Median Filter)", category: "Raster › Focal", tool: "MedianFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Replace each cell with the median of a square window around it (WhiteboxTools: MedianFilter). Removes spikes while keeping edges; the result keeps the input's unit.",
        params: &[window(3.0)], keywords: &["despeckle", "remove spikes", "salt and pepper", "neighbourhood median"] },
    WDef { id: "focal_max", name: "Focal Maximum", category: "Raster › Focal", tool: "MaximumFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Replace each cell with the largest value in a square window around it (WhiteboxTools: MaximumFilter). The result keeps the input's unit.",
        params: &[window(3.0)], keywords: &["dilate", "neighbourhood maximum", "expand highs"] },
    WDef { id: "focal_min", name: "Focal Minimum", category: "Raster › Focal", tool: "MinimumFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Replace each cell with the smallest value in a square window around it (WhiteboxTools: MinimumFilter). The result keeps the input's unit.",
        params: &[window(3.0)], keywords: &["erode", "neighbourhood minimum", "expand lows"] },
    WDef { id: "focal_range", name: "Focal Range", category: "Raster › Focal", tool: "RangeFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Largest minus smallest value in a square window around each cell (WhiteboxTools: RangeFilter): a simple measure of local relief when the input is a DEM. The result is in the input's unit.",
        params: &[window(3.0)], keywords: &["local relief", "roughness", "neighbourhood range"] },
    WDef { id: "focal_stddev", name: "Focal Standard Deviation", category: "Raster › Focal", tool: "StandardDeviationFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Derived, keeps_heights: false,
        description: "Standard deviation of the values in a square window around each cell (WhiteboxTools: StandardDeviationFilter): texture or roughness. The result is in the input's unit.",
        params: &[window(3.0)], keywords: &["texture", "roughness", "variability", "neighbourhood spread"] },
    WDef { id: "focal_majority", name: "Focal Majority (Mode)", category: "Raster › Focal", tool: "MajorityFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: false,
        description: "Replace each cell with the most frequent value in a square window around it (WhiteboxTools: MajorityFilter). Cleans speckle from a classified raster.",
        params: &[window(3.0)], keywords: &["mode", "clean classification", "smooth classes", "despeckle"] },
    WDef { id: "focal_gaussian", name: "Gaussian Filter", category: "Raster › Focal", tool: "GaussianFilter", ins: &[FILTER_IN], out_flag: "output", fixed: NOFIX, geo: Geo::Any, look: ResultLook::Source, keeps_heights: true,
        description: "Smooth a raster with a Gaussian kernel (WhiteboxTools: GaussianFilter). The standard deviation is in cells; the result keeps the input's unit.",
        params: &[num("sigma", "Sigma (cells)", 0.75, &["sigma"], 0.01, false, "")], keywords: &["smooth", "blur", "low pass"] },
    // ---- Distance on a grid (single-input tools; cost tools and Viewshed are separate structs below)
    WDef { id: "euclidean_distance", name: "Euclidean Distance", category: "Raster › Distance", tool: "EuclideanDistance", ins: &[rin("in", "Target raster (non-zero cells are targets)", "input")], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Straight-line distance from each cell to the nearest target cell, in the raster's map unit (metres for a metric CRS, feet for a foot CRS) (Shih and Wu 2004; WhiteboxTools: EuclideanDistance). Targets are the cells with a non-zero value; zero and no-data cells are measured. Needs a projected raster: in degrees it would be wrong, so project it first.",
        params: &[], keywords: &["distance to", "proximity", "buffer raster", "how far"] },
    WDef { id: "euclidean_allocation", name: "Euclidean Allocation", category: "Raster › Distance", tool: "EuclideanAllocation", ins: &[rin("in", "Target raster (non-zero cells are targets)", "input")], out_flag: "output", fixed: NOFIX, geo: Geo::Projected, look: ResultLook::Derived, keeps_heights: false,
        description: "Give each cell the value of the nearest target cell, by straight-line distance (Shih and Wu 2004; WhiteboxTools: EuclideanAllocation): a raster version of Thiessen polygons. Targets are the cells with a non-zero value. Needs a projected raster: project it first.",
        params: &[], keywords: &["thiessen", "nearest source", "voronoi raster", "proximity zones"] },
];

pub fn tools() -> Vec<Box<dyn Tool>> {
    let mut v: Vec<Box<dyn Tool>> = DEFS.iter().map(|d| Box::new(Wbt(d)) as Box<dyn Tool>).collect();
    v.push(Box::new(Viewshed));
    v.push(Box::new(CostTool(CostKind::Distance)));
    v.push(Box::new(CostTool(CostKind::Path)));
    v.push(Box::new(CostTool(CostKind::Allocation)));
    v
}

/// Search words for the tools of this file (called from `keywords.rs`).
pub fn keywords(id: &str) -> &'static [&'static str] {
    if let Some(d) = DEFS.iter().find(|d| d.id == id) {
        return d.keywords;
    }
    match id {
        "viewshed" => &["visibility", "line of sight", "what can be seen", "intervisibility", "observer"],
        "cost_distance" => &["least cost", "friction", "travel cost", "accumulated cost"],
        "cost_path" => &["least cost path", "route", "corridor", "friction"],
        "cost_allocation" => &["nearest by cost", "service area", "catchment by cost"],
        _ => &[],
    }
}

pub(super) fn geo_refusal(name: &str) -> String {
    format!("{name} measures distances or slopes in the raster's map units, and this raster is in degrees, so the result would be wrong (degrees read as metres). Project the raster to a metre-based coordinate system first (Warp or Project).")
}

/// The raster inputs of a tool, checked: they all exist, have a CRS, and sit on one grid in one CRS. Returns (path, crs) of the first.
pub(super) fn check_rasters<'a>(name: &str, ins: &[(&str, &'a Rel)]) -> Result<(&'a Path, &'a str)> {
    let mut first: Option<(&Path, &str)> = None;
    for (label, rel) in ins {
        let p = Path::new(need_raster(rel).map_err(|_| anyhow!("{name}: the input '{label}' must be a raster"))?);
        let crs = need_raster_crs(rel)?;
        match first {
            None => first = Some((p, crs)),
            Some((f, _)) => {
                let (a, b) = (crate::raster_ops::grid_of(f)?, crate::raster_ops::grid_of(p)?);
                crate::raster_ops::same_grid(&a, &b, name)?;
            }
        }
    }
    first.ok_or_else(|| anyhow!("{name} needs a raster input"))
}

/// (cells, no-data cells) of band 1, read a block of rows at a time.
fn count_nodata(path: &Path) -> Option<(u64, u64)> {
    let ds = gdal::Dataset::open(path).ok()?;
    let band = ds.rasterband(1).ok()?;
    let nd = band.no_data_value();
    let (cols, rows) = ds.raster_size();
    let step = (4_000_000 / cols.max(1)).clamp(1, rows.max(1));
    let (mut n, mut bad) = (0u64, 0u64);
    let mut r = 0;
    while r < rows {
        let h = step.min(rows - r);
        let buf = band.read_as::<f64>((0, r as isize), (cols, h), (cols, h), None).ok()?;
        for v in buf.data() {
            n += 1;
            if !v.is_finite() || nd.is_some_and(|d| *v == d) {
                bad += 1;
            }
        }
        r += h;
    }
    Some((n, bad))
}

/// A note on how many cells of the result have no data (the input's no-data cells and cells the program could not compute).
pub(super) fn nodata_note(ctx: &Ctx, name: &str, out: &Path) {
    if let Some((n, bad)) = count_nodata(out) {
        if bad > 0 {
            ctx.add_note(format!("{name}: {bad} of {n} cells in the result have no data (no-data cells in the input, and cells at the edge or beside no-data that the method cannot compute)."));
        }
    }
}

fn output_raster(out: PathBuf, crs: &Option<String>) -> Output {
    Output::Raster { path: out.display().to_string(), crs: crs.clone() }
}

fn run_wbt(tool: &str, args: &[(&str, String)], out: &Path) -> Result<()> {
    if let Err(e) = crate::whitebox::run(tool, args) {
        let _ = std::fs::remove_file(out);
        return Err(e);
    }
    Ok(())
}

pub struct Wbt(pub &'static WDef);

impl Tool for Wbt {
    fn result_look(&self) -> ResultLook {
        self.0.look
    }
    fn spec(&self) -> ToolSpec {
        let d = self.0;
        ToolSpec {
            id: d.id,
            name: d.name,
            category: d.category,
            role: Role::Tool,
            description: d.description,
            inputs: d.ins.iter().map(|i| Port { id: i.id, label: i.label, kind: i.kind }).collect(),
            output: Some(PortKind::Raster),
            params: d
                .params
                .iter()
                .map(|p| {
                    let mut q = Param::new(p.id, p.label, p.kind).options(p.options);
                    q.help = p.help;
                    match p.def {
                        Pd::None => q,
                        Pd::Num(v) => q.default(v),
                        Pd::Bool(v) => q.default(v),
                        Pd::Str(v) => q.default(v),
                    }
                })
                .collect(),
        }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let d = self.0;
        let rels: Vec<(&str, &Rel)> = d.ins.iter().map(|i| Ok((i.label, inputs.get(i.id).ok_or_else(|| anyhow!("{} needs the input '{}'", d.name, i.label))?))).collect::<Result<_>>()?;
        let (first, crs) = check_rasters(d.name, &rels)?;
        let geographic = crate::crs::is_geographic(crs);
        match d.geo {
            Geo::Projected if geographic => bail!("{}", geo_refusal(d.name)),
            Geo::ProjectedUnless(k, v) if geographic && p.str(k).as_deref().unwrap_or(v) != v => bail!("{}", geo_refusal(d.name)),
            _ => {}
        }
        let out = raster::new_store_path(d.id, "tif");
        let mut args: Vec<(&str, String)> = rels.iter().zip(d.ins).map(|((_, r), i)| Ok((i.flag, need_raster(r)?.to_string()))).collect::<Result<_>>()?;
        args.push((d.out_flag, out.display().to_string()));
        for (k, v) in d.fixed {
            args.push((k, v.to_string()));
        }
        for def in d.params {
            match (def.kind, def.def) {
                (ParamKind::Bool, _) => {
                    let on = if p.0.contains_key(def.id) { p.bool(def.id) } else { matches!(def.def, Pd::Bool(true)) };
                    if on {
                        for f in def.flags {
                            args.push((f, "true".into()));
                        }
                    }
                }
                (ParamKind::Number, dv) => {
                    let v = p.f64(def.id).or(if let Pd::Num(x) = dv { Some(x) } else { None }).unwrap_or(f64::NAN);
                    if !v.is_finite() {
                        bail!("'{}' must be a number", def.label);
                    }
                    if !def.min.is_nan() && v < def.min {
                        bail!("'{}' must be at least {}", def.label, def.min);
                    }
                    if def.whole && v.fract() != 0.0 {
                        bail!("'{}' must be a whole number", def.label);
                    }
                    // 0 means "no limit" for the optional breach limits (they have no WhiteboxTools flag unless set).
                    let flags: Vec<&str> = if def.flags.is_empty() { vec![def.id] } else { def.flags.to_vec() };
                    if !(v == 0.0 && def.min == 0.0 && matches!(def.id, "max_depth" | "max_length")) {
                        for f in flags {
                            args.push((f, if def.whole { format!("{}", v as i64) } else { v.to_string() }));
                        }
                    }
                }
                (_, dv) => {
                    let v = p.str(def.id).or(if let Pd::Str(s) = dv { Some(s.to_string()) } else { None });
                    if let Some(v) = v {
                        for f in def.flags {
                            args.push((f, v.clone()));
                        }
                    }
                }
            }
        }
        run_wbt(d.tool, &args, &out)?;
        restamp(&out, first, crs)?;
        raster::build_overviews(&out)?;
        if d.keeps_heights {
            let srcs: Vec<&Path> = rels.iter().filter_map(|(_, r)| r.raster.as_deref().map(Path::new)).take(1).collect();
            crate::vertical::inherit(&out, &srcs);
        }
        nodata_note(ctx, d.name, &out);
        Ok(output_raster(out, &rels[0].1.crs))
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Viewshed

/// Station points reprojected into the raster's CRS and written as a shapefile for WhiteboxTools. Returns the file and the stations'
/// coordinates in the raster's CRS, plus how many features had no usable point.
fn stations_shp(ctx: &Ctx, pts: &Rel, rcrs: &str, ext: [f64; 4]) -> Result<(PathBuf, Vec<(f64, f64)>, i64)> {
    let pcrs = need_crs(pts)?;
    let rres = crate::crs::resolve(rcrs);
    let geom = if crate::crs::same(pcrs, rcrs) { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, always_xy := true)", lit(&crate::crs::resolve(pcrs)), lit(&rres)) };
    let pointed = format!("SELECT CASE WHEN ST_GeometryType(geom)::VARCHAR = 'POINT' THEN geom ELSE ST_PointOnSurface(geom) END AS geom FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)", ident(&pts.table));
    let total: i64 = ctx.conn.query_row(&format!("SELECT count(*) FROM {}", ident(&pts.table)), [], |r| r.get(0))?;
    let mut stmt = ctx.conn.prepare(&format!("SELECT ST_X(g), ST_Y(g) FROM (SELECT {geom} AS g FROM ({pointed})) WHERE g IS NOT NULL"))?;
    let xy: Vec<(f64, f64)> = stmt.query_map([], |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    let xy: Vec<(f64, f64)> = xy.into_iter().filter(|(x, y)| x.is_finite() && y.is_finite()).collect();
    let skipped = total - xy.len() as i64;
    if xy.is_empty() {
        bail!("The observer layer has no point that can be placed on the elevation raster");
    }
    // WhiteboxTools stops with a panic when any station lies outside the DEM, so only the stations inside are written.
    let inside = format!("ST_X(g) BETWEEN {} AND {} AND ST_Y(g) BETWEEN {} AND {}", ext[0], ext[2], ext[1], ext[3]);
    let shp = raster::new_store_path("stations", "shp");
    ctx.conn.execute_batch(&format!(
        "COPY (SELECT g AS geom FROM (SELECT {geom} AS g FROM ({pointed})) WHERE g IS NOT NULL AND {inside}) TO {} (FORMAT GDAL, DRIVER 'ESRI Shapefile', SRS {}, USE_TMP_FILE false)",
        lit(shp.to_str().unwrap()), lit(&rres)
    ))?;
    Ok((shp, xy, skipped))
}

fn remove_shp(shp: &Path) {
    for ext in ["shp", "shx", "dbf", "prj", "cpg"] {
        let _ = std::fs::remove_file(shp.with_extension(ext));
    }
}

pub struct Viewshed;
impl Tool for Viewshed {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "viewshed", name: "Viewshed", category: "Raster › Terrain and Contours", role: Role::Tool,
            description: "Which cells can be seen from one or more observer points (WhiteboxTools: Viewshed). The result is 1 where a cell is visible from at least one observer and 0 where it is hidden (a line of sight blocked by the terrain). Observer points are transformed into the DEM's coordinate system first; observers outside the DEM are reported. The observer height is above the ground in the DEM's height unit, which must be the same as its horizontal unit (metres for a metric projected CRS); the target is taken at ground level. Earth curvature and refraction are not applied. Needs a projected DEM.",
            inputs: vec![Port { id: "in", label: "Elevation raster (DEM)", kind: PortKind::Raster }, Port { id: "obs", label: "Observer points", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![Param::new("height", "Observer height above ground (DEM height unit)", ParamKind::Number).default(1.7)] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (dem, pts) = (&inputs["in"], &inputs["obs"]);
        let (src, crs) = check_rasters("Viewshed", &[("Elevation raster (DEM)", dem)])?;
        if crate::crs::is_geographic(crs) {
            bail!("{}", geo_refusal("Viewshed"));
        }
        let h = p.f64("height").unwrap_or(1.7);
        if !h.is_finite() || h < 0.0 {
            bail!("'Observer height' must be a number, 0 or more");
        }
        let g = crate::raster_ops::grid_of(src)?;
        if g.gt[2] != 0.0 || g.gt[4] != 0.0 {
            bail!("Viewshed does not work on a rotated raster. Use Warp to make it north-up first.");
        }
        let (x0, y1) = (g.gt[0], g.gt[3]);
        let (x1, y0) = (x0 + g.gt[1] * g.cols as f64, y1 + g.gt[5] * g.rows as f64);
        let ext = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
        let (shp, xy, skipped) = stations_shp(ctx, pts, crs, ext)?;
        let outside: Vec<usize> = xy.iter().enumerate().filter(|(_, (x, y))| *x < x0.min(x1) || *x > x0.max(x1) || *y < y0.min(y1) || *y > y0.max(y1)).map(|(i, _)| i + 1).collect();
        if outside.len() == xy.len() {
            remove_shp(&shp);
            bail!("All {} observer point(s) are outside the elevation raster, so nothing can be seen from them", xy.len());
        }
        if !outside.is_empty() {
            ctx.add_note(format!("Viewshed: {} of {} observer point(s) lie outside the DEM and were not used (point numbers {:?}, in the order of the layer).", outside.len(), xy.len(), outside));
        }
        if skipped > 0 {
            ctx.add_note(format!("Viewshed: {skipped} feature(s) of the observer layer had no usable point and were skipped."));
        }
        let out = raster::new_store_path("viewshed", "tif");
        let r = run_wbt("Viewshed", &[("dem", src.display().to_string()), ("stations", shp.display().to_string()), ("output", out.display().to_string()), ("height", h.to_string())], &out);
        remove_shp(&shp);
        r?;
        restamp(&out, src, crs)?;
        raster::build_overviews(&out)?;
        nodata_note(ctx, "Viewshed", &out);
        Ok(output_raster(out, &dem.crs))
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Cost distance, least-cost path, cost allocation

#[derive(Clone, Copy, PartialEq)]
pub enum CostKind {
    Distance,
    Path,
    Allocation,
}

pub struct CostTool(pub CostKind);

macro_rules! cost_units {
    () => {
        "Cost distance is the sum, along the cheapest route, of the cost of each cell crossed times the ground distance travelled, taking the average of the two cells' costs for each step (diagonal steps are longer); its unit is the cost unit times the map unit (for example minutes per metre times metres gives minutes). The cost raster must be positive; cells with no data cannot be crossed."
    };
}

impl Tool for CostTool {
    fn result_look(&self) -> ResultLook {
        ResultLook::Derived
    }
    fn spec(&self) -> ToolSpec {
        let src = Port { id: "source", label: "Source raster (non-zero cells)", kind: PortKind::Raster };
        let cost = Port { id: "cost", label: "Cost raster (cost per unit distance)", kind: PortKind::Raster };
        match self.0 {
            CostKind::Distance => ToolSpec { id: "cost_distance", name: "Cost Distance", category: "Raster › Distance", role: Role::Tool,
                description: concat!("Accumulated least cost of travelling from the nearest source cell to every cell over a cost surface (WhiteboxTools: CostDistance; Dijkstra's algorithm on the 8-neighbour grid). Sources are the cells with a non-zero value. ", cost_units!(), " Needs a projected raster."),
                inputs: vec![src, cost], output: Some(PortKind::Raster), params: vec![] },
            CostKind::Path => ToolSpec { id: "cost_path", name: "Least-Cost Path", category: "Raster › Distance", role: Role::Tool,
                description: concat!("The cheapest route from each destination cell back to the nearest source over a cost surface (WhiteboxTools: CostPathway, after CostDistance). The result is a raster that is 1 along the paths and no data elsewhere. Sources and destinations are the cells with a non-zero value. ", cost_units!(), " Needs a projected raster."),
                inputs: vec![src, cost, Port { id: "dest", label: "Destination raster (non-zero cells)", kind: PortKind::Raster }], output: Some(PortKind::Raster), params: vec![] },
            CostKind::Allocation => ToolSpec { id: "cost_allocation", name: "Cost Allocation", category: "Raster › Distance", role: Role::Tool,
                description: concat!("Give each cell the value of the source cell it is cheapest to reach (WhiteboxTools: CostAllocation, after CostDistance): a service-area raster by travel cost. Sources are the cells with a non-zero value. ", cost_units!(), " Needs a projected raster."),
                inputs: vec![src, cost], output: Some(PortKind::Raster), params: vec![] },
        }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let name = match self.0 { CostKind::Distance => "Cost Distance", CostKind::Path => "Least-Cost Path", CostKind::Allocation => "Cost Allocation" };
        let (s, c) = (&inputs["source"], &inputs["cost"]);
        let mut ins: Vec<(&str, &Rel)> = vec![("Source raster", s), ("Cost raster", c)];
        if self.0 == CostKind::Path {
            ins.push(("Destination raster", &inputs["dest"]));
        }
        let (first, crs) = check_rasters(name, &ins)?;
        if crate::crs::is_geographic(crs) {
            bail!("{}", geo_refusal(name));
        }
        if let Some((lo, _, _)) = band_range(Path::new(need_raster(c)?)) {
            if lo <= 0.0 {
                bail!("{name}: the cost raster has cells with a cost of {lo}. Costs must be greater than 0; use Reclassify or the raster calculator to raise them.");
            }
        }
        let (sp, cp) = (need_raster(s)?.to_string(), need_raster(c)?.to_string());
        let accum = raster::new_store_path("costacc", "tif");
        let back = raster::new_store_path("costback", "tif");
        let clean = |paths: &[&Path]| paths.iter().for_each(|p| { let _ = std::fs::remove_file(p); });
        if let Err(e) = crate::whitebox::run("CostDistance", &[("source", sp.clone()), ("cost", cp), ("out_accum", accum.display().to_string()), ("out_backlink", back.display().to_string())]) {
            clean(&[&accum, &back]);
            return Err(e);
        }
        let result = match self.0 {
            CostKind::Distance => {
                clean(&[&back]);
                accum.clone()
            }
            CostKind::Path => {
                let out = raster::new_store_path("costpath", "tif");
                let r = run_wbt("CostPathway", &[("destination", need_raster(&inputs["dest"])?.to_string()), ("backlink", back.display().to_string()), ("output", out.display().to_string())], &out);
                clean(&[&accum, &back]);
                r?;
                out
            }
            CostKind::Allocation => {
                let out = raster::new_store_path("costalloc", "tif");
                let r = run_wbt("CostAllocation", &[("source", sp), ("backlink", back.display().to_string()), ("output", out.display().to_string())], &out);
                clean(&[&accum, &back]);
                r?;
                out
            }
        };
        restamp(&result, first, crs)?;
        raster::build_overviews(&result)?;
        if self.0 != CostKind::Path {
            if let Some((n, bad)) = count_nodata(&result) {
                if bad > 0 {
                    ctx.add_note(format!("{name}: {bad} of {n} cells could not be reached from any source (no data), because of no-data cost cells or disconnected areas."));
                }
            }
        }
        Ok(output_raster(result, &s.crs))
    }
}
