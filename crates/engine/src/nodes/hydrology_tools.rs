//! Hydrology as model nodes (phase H4), run by the bundled WhiteboxTools program (MIT, John Lindsay;
//! https://github.com/jblindsay/whitebox-tools) through `whitebox.rs`. Each node writes a new GeoTIFF in the input raster's grid and CRS.
//! The equivalent ArcGIS tools are Fill, Flow Direction, Flow Accumulation, Watershed, Basin and Stream Link thresholds on Flow Accumulation.

use super::*;
use crate::raster;
use gdal::Dataset;
use std::path::{Path, PathBuf};

fn rin(id: &'static str, label: &'static str) -> Port {
    Port { id, label, kind: PortKind::Raster }
}

/// Run a WhiteboxTools raster tool. `input_flag` names the input raster flag; `extra` are further (flag, value) pairs.
/// The output must come back on the input's grid; its CRS is set from the input so it is never guessed.
fn wbt_raster(tool: &str, input: &Rel, input_flag: &str, extra: &[(&str, String)], stem: &str) -> Result<PathBuf> {
    let src = Path::new(need_raster(input)?);
    let crs = need_raster_crs(input)?;
    let out = raster::new_store_path(stem, "tif");
    let mut args: Vec<(&str, String)> = vec![(input_flag, src.display().to_string()), ("output", out.display().to_string())];
    args.extend(extra.iter().map(|(k, v)| (*k, v.clone())));
    if let Err(e) = crate::whitebox::run(tool, &args) {
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    restamp(&out, src, crs)?;
    raster::build_overviews(&out)?;
    Ok(out)
}

/// Check the result sits on the input's grid and write the input's CRS into it.
fn restamp(out: &Path, src: &Path, crs: &str) -> Result<()> {
    let a = crate::raster_ops::grid_of(src)?;
    let b = crate::raster_ops::grid_of(out).map_err(|_| anyhow!("WhiteboxTools did not produce a raster. Check the input is a single-band elevation raster."))?;
    if (a.cols, a.rows) != (b.cols, b.rows) || a.gt.iter().zip(&b.gt).any(|(x, y)| (x - y).abs() > 1e-6 * x.abs().max(1.0)) {
        let _ = std::fs::remove_file(out);
        bail!("WhiteboxTools returned a raster on a different grid ({} × {} instead of {} × {}); the result was discarded", b.cols, b.rows, a.cols, a.rows);
    }
    let mut ds = Dataset::open_ex(out, gdal::DatasetOptions { open_flags: gdal::GdalOpenFlags::GDAL_OF_UPDATE | gdal::GdalOpenFlags::GDAL_OF_RASTER, ..Default::default() })?;
    let wkt = crate::crs::srs(&crate::crs::resolve(crs))?.to_wkt()?;
    ds.set_spatial_ref(&gdal::spatial_ref::SpatialRef::from_wkt(&wkt)?)?;
    Ok(())
}

fn rout(path: PathBuf, i: &Rel) -> Result<Output> {
    Ok(Output::Raster { path: path.display().to_string(), crs: i.crs.clone() })
}

pub struct FillDepressions;
impl Tool for FillDepressions {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "fill_depressions", name: "Fill Depressions (Sinks)", category: "Hydrology", role: Role::Tool,
            description: "Remove pits and sinks from an elevation raster so water can drain off it. Run this before Flow Direction and Flow Accumulation. With 'Fix flat areas' on, filled flats get a tiny slope toward their outlet so flow direction is defined everywhere (WhiteboxTools: FillDepressions).",
            inputs: vec![rin("in", "Elevation raster")], output: Some(PortKind::Raster),
            params: vec![Param::new("fix_flats", "Fix flat areas", ParamKind::Bool).default(true)] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let mut extra = vec![];
        if p.bool("fix_flats") {
            extra.push(("fix_flats", "true".to_string()));
        }
        let out = wbt_raster("FillDepressions", i, "dem", &extra, "fill")?;
        crate::vertical::inherit(&out, &[Path::new(need_raster(i)?)]); // a filled DEM keeps its heights
        rout(out, i)
    }
}

pub struct FlowDirection;
impl Tool for FlowDirection {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "flow_direction", name: "Flow Direction (D8)", category: "Hydrology", role: Role::Tool,
            description: "Direction water leaves each cell toward its steepest downhill neighbour, using the ESRI codes 1 = east, 2 = south-east, 4 = south, 8 = south-west, 16 = west, 32 = north-west, 64 = north, 128 = north-east (WhiteboxTools: D8Pointer). Use a depression-filled DEM.",
            inputs: vec![rin("in", "Filled elevation raster")], output: Some(PortKind::Raster), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        rout(wbt_raster("D8Pointer", i, "dem", &[("esri_pntr", "true".to_string())], "flowdir")?, i)
    }
}

pub struct FlowAccumulation;
impl Tool for FlowAccumulation {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "flow_accumulation", name: "Flow Accumulation (D8)", category: "Hydrology", role: Role::Tool,
            description: "How much area drains through each cell: the number of upstream cells, the catchment area, or the specific contributing area (area per unit of flow width). Streams show up as high values. Use a depression-filled DEM; log scale is easier to look at (WhiteboxTools: D8FlowAccumulation).",
            inputs: vec![rin("in", "Filled elevation raster")], output: Some(PortKind::Raster),
            params: vec![
                Param::new("out_type", "Result", ParamKind::Enum).default("cells").options(&[("cells", "Number of upstream cells"), ("catchment area", "Catchment area"), ("specific contributing area", "Specific contributing area")]),
                Param::new("log", "Log-transform the result", ParamKind::Bool).default(false),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let ty = match p.str("out_type").as_deref() { Some("catchment area") => "catchment area", Some("specific contributing area") => "specific contributing area", _ => "cells" };
        let mut extra = vec![("out_type", ty.to_string())];
        if p.bool("log") {
            extra.push(("log", "true".to_string()));
        }
        rout(wbt_raster("D8FlowAccumulation", i, "input", &extra, "flowacc")?, i)
    }
}

pub struct Basins;
impl Tool for Basins {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "basins", name: "Drainage Basins", category: "Hydrology", role: Role::Tool,
            description: "Label every drainage basin that reaches the edge of the raster with its own number, from a Flow Direction raster (WhiteboxTools: Basins).",
            inputs: vec![rin("in", "Flow direction raster")], output: Some(PortKind::Raster), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        rout(wbt_raster("Basins", i, "d8_pntr", &[("esri_pntr", "true".to_string())], "basins")?, i)
    }
}

pub struct ExtractStreams;
impl Tool for ExtractStreams {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "extract_streams", name: "Extract Streams", category: "Hydrology", role: Role::Tool,
            description: "Mark cells as stream (value 1) where the Flow Accumulation value is at least the threshold; every other cell is no data. A smaller threshold gives a denser network. The threshold is in the units of the accumulation raster (cells, area, or log values) (WhiteboxTools: ExtractStreams).",
            inputs: vec![rin("in", "Flow accumulation raster")], output: Some(PortKind::Raster),
            params: vec![Param::new("threshold", "Threshold", ParamKind::Number).default(1000)] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let t = p.f64("threshold").unwrap_or(f64::NAN);
        if !(t.is_finite() && t > 0.0) {
            bail!("The threshold must be a number greater than 0");
        }
        rout(wbt_raster("ExtractStreams", i, "flow_accum", &[("threshold", t.to_string())], "streams")?, i)
    }
}

pub struct Watersheds;
impl Tool for Watersheds {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "watersheds", name: "Watersheds (from Outlets)", category: "Hydrology", role: Role::Tool,
            description: "The area that drains to each outlet point, from a Flow Direction raster. Each outlet gets its own number (1, 2, 3, in the order of the points). Place outlets exactly on a stream cell of a Flow Accumulation raster; a point off the stream gives a tiny watershed (WhiteboxTools: Watershed). Outlets are projected into the raster's coordinate system first.",
            inputs: vec![rin("pntr", "Flow direction raster"), Port { id: "pour", label: "Outlet points", kind: PortKind::Vector }], output: Some(PortKind::Raster), params: vec![] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (r, pts) = (&inputs["pntr"], &inputs["pour"]);
        let rcrs = need_raster_crs(r)?;
        let pcrs = need_crs(pts)?;
        let rres = crate::crs::resolve(rcrs);
        // Outlets are reprojected into the raster's CRS when it differs (never guessed).
        let geom = if crate::crs::same(pcrs, rcrs) { "geom".to_string() } else { format!("ST_Transform(geom, {}, {}, always_xy := true)", lit(&crate::crs::resolve(pcrs)), lit(&rres)) };
        let shp = raster::new_store_path("outlets", "shp");
        ctx.conn.execute_batch(&format!(
            "COPY (SELECT {geom} AS geom FROM {} WHERE geom IS NOT NULL AND ST_GeometryType(geom) = 'POINT') TO {} (FORMAT GDAL, DRIVER 'ESRI Shapefile', SRS {}, USE_TMP_FILE false)",
            ident(&pts.table), lit(shp.to_str().unwrap()), lit(&rres)
        ))?;
        let out = wbt_raster("Watershed", r, "d8_pntr", &[("pour_pts", shp.display().to_string()), ("esri_pntr", "true".to_string())], "watersheds");
        for ext in ["shp", "shx", "dbf", "prj", "cpg"] {
            let _ = std::fs::remove_file(shp.with_extension(ext));
        }
        rout(out?, r)
    }
}
