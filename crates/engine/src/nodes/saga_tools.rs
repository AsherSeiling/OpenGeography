//! SAGA GIS tools as model nodes (package T2): valley bottom flatness (MRVBF/MRRTF), TPI-based landform classes, potential incoming solar
//! radiation and the channel network, run by the bundled `saga_cmd` (GPL, https://saga-gis.sourceforge.io, shipped unmodified as a separate
//! program, never linked) through `saga.rs`.
//!
//! The grid goes to SAGA as `.sgrd/.sdat` (written by our GDAL), the result comes back through GDAL and gets the input's CRS written into it
//! (never guessed or taken from SAGA). Tools that depend on the cell size in metres refuse a raster in degrees. One table (`DEFS`) holds each
//! tool; `Saga` turns a row into a `Tool`, so `ToolSpec` stays the single definition. Tools WhiteboxTools (T1) or GDAL already cover are not repeated
//! here (TRI is the `ruggedness` node; Geomorphons is the other landform classifier).

use super::hydrology_tools::{restamp, band_range};
use super::whitebox_tools::{check_rasters, geo_refusal, nodata_note};
use super::*;
use crate::raster;
use std::path::Path;

#[derive(Clone, Copy)]
enum Pd {
    Num(f64),
    Str(&'static str),
}

/// A setting and the SAGA flag it sets. `max` is NaN for no upper limit.
struct SP {
    id: &'static str,
    label: &'static str,
    kind: ParamKind,
    def: Pd,
    flag: &'static str,
    options: &'static [(&'static str, &'static str)],
    min: f64,
    max: f64,
    whole: bool,
    help: &'static str,
}

const NAN: f64 = f64::NAN;

const fn num(id: &'static str, label: &'static str, def: f64, flag: &'static str, min: f64, max: f64, whole: bool, help: &'static str) -> SP {
    SP { id, label, kind: ParamKind::Number, def: Pd::Num(def), flag, options: &[], min, max, whole, help }
}

struct SDef {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    description: &'static str,
    library: &'static str,
    tool: &'static str,
    /// SAGA's flag for the elevation input.
    input_flag: &'static str,
    params: &'static [SP],
    /// (value, label, SAGA output flag). With more than one a "Result" setting picks which one comes back; all are requested.
    outs: &'static [(&'static str, &'static str, &'static str)],
    /// Output shapes SAGA insists on writing; they are discarded.
    shapes: &'static [&'static str],
    /// Flags that are always passed.
    fixed: &'static [(&'static str, &'static str)],
    /// Needs the cell size in metres (or feet): refuses a raster in degrees.
    projected: bool,
    keywords: &'static [&'static str],
}

static DEFS: &[SDef] = &[
    SDef { id: "saga_mrvbf", name: "Valley Bottom Flatness (MRVBF, MRRTF)", category: "Raster › Terrain and Contours", library: "ta_morphometry", tool: "8", input_flag: "DEM",
        description: "Multiresolution Index of Valley Bottom Flatness (Gallant and Dowling 2003; SAGA GIS: ta_morphometry, tool 8). Finds flat low-lying land such as valley floors and floodplains by combining slope and elevation percentile over a series of coarser and coarser scales. MRVBF is 0 on slopes and rises from about 0.5 on small, thin valley bottoms to 6 or more on very broad flat ones (each whole step is a threefold coarser scale). MRRTF is the same idea for flat high ground (ridge tops). Unitless. The slope threshold is in per cent (rise over run times 100), so the DEM must be projected with heights in the same unit as the cells (metres). SAGA's defaults are the paper's. Runs the SAGA program (GPL) as a separate process.",
        params: &[
            num("slope", "Initial slope threshold (%)", 16.0, "T_SLOPE", 0.0, 100.0, false, "Slope below which a cell can be flat valley floor at the finest scale. 16 % is the paper's value for a 25 m DEM; use about 116 / cell size (m) for others."),
            num("pctl_valley", "Elevation percentile threshold, lowness", 0.4, "T_PCTL_V", 0.0, 1.0, false, ""),
            num("pctl_ridge", "Elevation percentile threshold, upness", 0.35, "T_PCTL_R", 0.0, 1.0, false, ""),
            num("slope_shape", "Slope shape parameter", 4.0, "P_SLOPE", NAN, NAN, false, ""),
            num("pctl_shape", "Percentile shape parameter", 3.0, "P_PCTL", NAN, NAN, false, ""),
            num("max_res", "Maximum resolution (% of the finest scale to stop at)", 100.0, "MAX_RES", 0.0, 100.0, false, "100 uses every scale; lower values stop before the coarsest ones."),
        ],
        outs: &[("mrvbf", "Valley bottom flatness (MRVBF)", "MRVBF"), ("mrrtf", "Ridge top flatness (MRRTF)", "MRRTF")], shapes: &[], fixed: &[("UPDATE", "0"), ("CLASSIFY", "0")], projected: true,
        keywords: &["mrvbf", "valley floor", "floodplain", "flat low land", "ridge top flatness", "valley bottom", "mrrtf"] },
    SDef { id: "saga_tpi_landforms", name: "TPI Landform Classification", category: "Raster › Terrain and Contours", library: "ta_morphometry", tool: "19", input_flag: "DEM",
        description: "Classify the land into 10 landform classes from the Topographic Position Index at two scales (Weiss 2001; Guisan, Weiss and Weiss 1999; SAGA GIS: ta_morphometry, tool 19): 1 streams, 2 midslope drainages and shallow valleys, 3 upland drainages and headwaters, 4 u-shaped valleys, 5 plains, 6 open slopes, 7 upper slopes and mesas, 8 local ridges and hills in valleys, 9 midslope ridges and small hills, 10 mountain tops and high ridges. The small and large scale are neighbourhood radii in the DEM's map units (metres for a metric CRS), with the part inside the minimum left out, so the DEM must be projected. Different from Geomorphons, which looks along lines of sight.",
        params: &[
            num("small_min", "Small scale, inner radius (map units)", 0.0, "RADIUS_A_MIN", 0.0, NAN, false, "Cells closer than this are left out of the small neighbourhood (0 = start at the cell)."),
            num("small_max", "Small scale, outer radius (map units)", 100.0, "RADIUS_A_MAX", 0.0, NAN, false, ""),
            num("large_min", "Large scale, inner radius (map units)", 0.0, "RADIUS_B_MIN", 0.0, NAN, false, ""),
            num("large_max", "Large scale, outer radius (map units)", 1000.0, "RADIUS_B_MAX", 0.0, NAN, false, "Should be clearly larger than the small scale."),
        ],
        outs: &[("landforms", "Landform classes", "LANDFORMS")], shapes: &[], fixed: &[("DW_WEIGHTING", "0")], projected: true,
        keywords: &["landform classification", "topographic position", "tpi", "weiss", "ridge", "valley", "slope position", "mesa"] },
    SDef { id: "saga_solar_radiation", name: "Potential Incoming Solar Radiation", category: "Raster › Terrain and Contours", library: "ta_lighting", tool: "2", input_flag: "GRD_DEM",
        description: "Sunshine energy that could reach each cell of the DEM over a period, in kWh per square metre, from the sun's path, slope, aspect and the shadows cast by the terrain (Wilson and Gallant 2000; Böhner and Antonić 2009; SAGA GIS: ta_lighting, tool 2). Atmosphere is the lumped transmittance method (a fixed share of the beam gets through, no weather or cloud). The latitude is taken from the middle of the DEM, transformed to WGS 84, and is stated in a note; the period is days of the year (1 to 365) with the sun followed from sunrise to sunset in half-hour steps. Heights are read as metres, so the DEM must be projected with metre heights. Direct is the beam, diffuse is the sky light, total is both.",
        params: &[
            num("first_day", "First day of the year", 172.0, "", 1.0, 365.0, true, "1 = 1 January, 172 = 21 June, 355 = 21 December."),
            num("last_day", "Last day of the year", 172.0, "", 1.0, 365.0, true, "The same as the first day for one day; later for a range (the result is the sum over the range)."),
            num("day_step", "Step between days in a range", 5.0, "DAYS_STEP", 1.0, NAN, true, "A range is sampled every this many days and scaled up to the full range."),
            num("transmittance", "Atmospheric transmittance (%)", 70.0, "LUMPED", 0.0, 100.0, false, "Share of the sun's energy that reaches the ground through a clear atmosphere."),
            SP { id: "shadow", label: "Shadows", kind: ParamKind::Enum, def: Pd::Str("1"), flag: "SHADOW", options: &[("0", "Slim (a cell is shaded if the sun is blocked at its centre)"), ("1", "Fat (default: also at the edges)"), ("2", "None (ignore cast shadows)")], min: NAN, max: NAN, whole: false, help: "" },
        ],
        outs: &[("total", "Total insolation (kWh/m²)", "GRD_TOTAL"), ("direct", "Direct insolation (kWh/m²)", "GRD_DIRECT"), ("diffuse", "Diffuse insolation (kWh/m²)", "GRD_DIFFUS")], shapes: &[],
        fixed: &[("UNITS", "0"), ("LOCATION", "0"), ("METHOD", "2"), ("LOCALSVF", "1"), ("SOLARCONST", "1367")], projected: true,
        keywords: &["insolation", "sunlight", "solar energy", "shade", "sun exposure", "solar potential", "irradiance", "kwh"] },
    SDef { id: "saga_channel_network", name: "Channel Network (Strahler Order)", category: "Hydrology", library: "ta_channels", tool: "5", input_flag: "DEM",
        description: "Trace the drainage channels of a DEM in one step and give each channel cell its Strahler order (O'Callaghan and Mark 1984 flow directions; Strahler 1957; SAGA GIS: ta_channels, tool 5). A cell is a channel where the threshold order is reached, so a larger threshold keeps only the bigger rivers. Cells outside the channels have no data. The DEM should be free of pits (use Fill Depressions or Breach Depressions first). Counts cells only, so any CRS works. For the channel lines and basins as vectors, use the Whitebox stream tools.",
        params: &[num("threshold", "Threshold (Strahler order)", 5.0, "THRESHOLD", 1.0, NAN, true, "Channels begin where the Strahler order of the drainage network reaches this value; 1 keeps every flow path.")],
        outs: &[("order", "Strahler order", "ORDER")], shapes: &["SEGMENTS", "BASINS"], fixed: &[("SUBBASINS", "0")], projected: false,
        keywords: &["streams", "rivers", "drainage network", "stream network", "strahler", "channels"] },
];

pub fn tools() -> Vec<Box<dyn Tool>> {
    DEFS.iter().map(|d| Box::new(Saga(d)) as Box<dyn Tool>).collect()
}

pub fn keywords(id: &str) -> &'static [&'static str] {
    DEFS.iter().find(|d| d.id == id).map(|d| d.keywords).unwrap_or(&[])
}

struct Saga(&'static SDef);

impl Tool for Saga {
    fn result_look(&self) -> ResultLook {
        ResultLook::Derived
    }
    fn spec(&self) -> ToolSpec {
        let d = self.0;
        let mut params: Vec<Param> = vec![];
        if d.outs.len() > 1 {
            params.push(Param::new("result", "Result", ParamKind::Enum).options(result_options(d.id)).default(d.outs[0].0));
        }
        for p in d.params {
            let mut q = Param::new(p.id, p.label, p.kind).options(p.options);
            q.help = p.help;
            params.push(match p.def {
                Pd::Num(v) => q.default(v),
                Pd::Str(v) => q.default(v),
            });
        }
        ToolSpec {
            id: d.id,
            name: d.name,
            category: d.category,
            role: Role::Tool,
            description: d.description,
            inputs: vec![Port { id: "in", label: "Elevation raster (DEM)", kind: PortKind::Raster }],
            output: Some(PortKind::Raster),
            params,
        }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let d = self.0;
        let rel = inputs.get("in").ok_or_else(|| anyhow!("{} needs the elevation raster", d.name))?;
        let (src, crs) = check_rasters(d.name, &[("Elevation raster (DEM)", rel)])?;
        if d.projected && crate::crs::is_geographic(crs) {
            bail!("{}", geo_refusal(d.name));
        }
        if !crate::saga::available() {
            bail!("{} needs SAGA GIS, which is not installed with this copy of GIS Studio. Reinstall the app, or set GIS_STUDIO_SAGA to the saga_cmd program.", d.name);
        }
        // Settings, checked.
        let mut vals: Vec<(&SP, f64)> = vec![];
        for sp in d.params {
            if !matches!(sp.kind, ParamKind::Number) {
                continue;
            }
            let v = p.f64(sp.id).or(if let Pd::Num(x) = sp.def { Some(x) } else { None }).unwrap_or(f64::NAN);
            if !v.is_finite() {
                bail!("'{}' must be a number", sp.label);
            }
            if !sp.min.is_nan() && v < sp.min {
                bail!("'{}' must be at least {}", sp.label, sp.min);
            }
            if !sp.max.is_nan() && v > sp.max {
                bail!("'{}' must be at most {}", sp.label, sp.max);
            }
            if sp.whole && v.fract() != 0.0 {
                bail!("'{}' must be a whole number", sp.label);
            }
            vals.push((sp, v));
        }
        let get = |id: &str| vals.iter().find(|(s, _)| s.id == id).map(|(_, v)| *v).unwrap_or(f64::NAN);
        if d.id == "saga_tpi_landforms" && (get("small_min") > get("small_max") || get("large_min") > get("large_max")) {
            bail!("{}: an inner radius cannot be larger than the outer radius of the same scale", d.name);
        }
        if d.id == "saga_solar_radiation" && get("first_day") > get("last_day") {
            bail!("{}: the first day must not come after the last day", d.name);
        }
        let result = if d.outs.len() > 1 {
            let v = p.str("result").unwrap_or_else(|| d.outs[0].0.to_string());
            d.outs.iter().find(|o| o.0 == v).ok_or_else(|| anyhow!("'Result' must be one of: {}", d.outs.iter().map(|o| o.0).collect::<Vec<_>>().join(", ")))?
        } else {
            &d.outs[0]
        };

        let work = crate::saga::Work::new()?;
        let dem = work.import(src, "dem")?;
        let mut args: Vec<(&str, String)> = vec![(d.input_flag, dem.display().to_string())];
        for o in d.outs {
            args.push((o.2, work.grid(o.0).display().to_string()));
        }
        for f in d.shapes {
            args.push((f, work.path(&format!("{}.shp", f.to_ascii_lowercase())).display().to_string()));
        }
        for (k, v) in d.fixed {
            args.push((k, v.to_string()));
        }
        for (sp, v) in &vals {
            if !sp.flag.is_empty() {
                args.push((sp.flag, if sp.whole { format!("{}", *v as i64) } else { v.to_string() }));
            }
        }
        for sp in d.params {
            if matches!(sp.kind, ParamKind::Enum) {
                let v = p.str(sp.id).or(if let Pd::Str(s) = sp.def { Some(s.to_string()) } else { None }).unwrap_or_default();
                if !sp.options.iter().any(|o| o.0 == v) {
                    bail!("'{}' has an unknown value '{v}'", sp.label);
                }
                args.push((sp.flag, v));
            }
        }
        if d.id == "saga_solar_radiation" {
            // The latitude is worked out here, from the DEM's own CRS: SAGA's own CRS lookup is not part of the shipped program.
            let g = crate::raster_ops::grid_of(src)?;
            let (cx, cy) = (g.gt[0] + g.gt[1] * g.cols as f64 / 2.0, g.gt[3] + g.gt[5] * g.rows as f64 / 2.0);
            let (lon, lat) = crate::crs::transform_point(ctx.conn, cx, cy, crs, "EPSG:4326")?;
            if !lat.is_finite() || lat.abs() > 90.0 {
                bail!("{}: could not work out the latitude of the DEM from its coordinate system", d.name);
            }
            ctx.add_note(format!("{}: latitude {:.3}° (longitude {:.3}°), the middle of the DEM in WGS 84, was used for the sun's position. Heights were read as metres.", d.name, lat, lon));
            let (a, b) = (get("first_day") as i64, get("last_day") as i64);
            // 2023 is not a leap year, so day 1..365 maps to the same date every year; SAGA works from the date.
            let date = |doy: i64| -> String {
                let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
                let (mut m, mut dd) = (0usize, doy);
                while dd > months[m] {
                    dd -= months[m];
                    m += 1;
                }
                format!("2023-{:02}-{:02}", m + 1, dd)
            };
            args.push(("LATITUDE", format!("{lat}")));
            args.push(("PERIOD", if a == b { "1".into() } else { "2".into() }));
            args.push(("DAY", date(a)));
            args.push(("DAY_STOP", date(b)));
            args.push(("HOUR_RANGE_MIN", "0".into()));
            args.push(("HOUR_RANGE_MAX", "24".into()));
            args.push(("HOUR_STEP", "0.5".into()));
        }
        if let Err(e) = crate::saga::run(d.library, d.tool, &args) {
            // SAGA stops with an unrelated message when no cell reaches the threshold (it cannot vectorise an empty network).
            if d.id == "saga_channel_network" && e.to_string().contains("Vectorizing Grid Classes") {
                bail!("{}: no cell reaches a Strahler order of {} in this DEM, so there are no channels. Lower the threshold, or check that the DEM is large enough and free of pits.", d.name, get("threshold"));
            }
            return Err(e);
        }

        let out = raster::new_store_path(d.id, "tif");
        if let Err(e) = work.export(result.0, &out) {
            let _ = std::fs::remove_file(&out);
            return Err(e);
        }
        if let Err(e) = restamp(&out, src, crs) {
            let _ = std::fs::remove_file(&out);
            return Err(e);
        }
        raster::build_overviews(&out)?;
        if band_range(&out).is_none() {
            ctx.add_note(format!("{}: the result has no valid cells (the DEM may be flat, empty or all no-data).", d.name));
        }
        nodata_note(ctx, d.name, Path::new(&out));
        Ok(Output::Raster { path: out.display().to_string(), crs: rel.crs.clone() })
    }
}

/// The "Result" choices of a tool with several outputs (static data, so the spec can borrow them).
fn result_options(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "saga_mrvbf" => &[("mrvbf", "Valley bottom flatness (MRVBF)"), ("mrrtf", "Ridge top flatness (MRRTF)")],
        "saga_solar_radiation" => &[("total", "Total insolation (kWh/m²)"), ("direct", "Direct insolation (kWh/m²)"), ("diffuse", "Diffuse insolation (kWh/m²)")],
        _ => &[],
    }
}
