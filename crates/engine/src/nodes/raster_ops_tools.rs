//! Raster foundation tools (phases E1, E2): calculator, reclassify, NoData, band select, resample, align, mosaic, rasterize.
//! Each is a thin wrapper over `raster_ops`; GDAL does the heavy lifting and the calculator uses the MIT `fasteval` crate.
//! QGIS counterparts (named, not copied): Raster calculator (native:rastercalc / gdal:rastercalculator), Reclassify by table
//! (native:reclassifybytable), Fill NoData cells (native:fillnodata / gdal:fillnodata), Translate/Warp, Merge (gdal:merge), Rasterize (gdal:rasterize).

use super::*;
use crate::raster;
use crate::raster_ops::{self as ro, NoDataMode};
use std::path::Path;

fn rin(label: &'static str) -> Port {
    Port { id: "in", label, kind: PortKind::Raster }
}

fn rout(path: std::path::PathBuf, crs: Option<String>) -> Result<Output> {
    Ok(Output::Raster { path: path.display().to_string(), crs })
}

const RESAMPLING: [(&str, &str); 5] = [("near", "Nearest (categories)"), ("bilinear", "Bilinear"), ("cubic", "Cubic"), ("average", "Average"), ("mode", "Most common value")];

/// Raster calculator on one raster (A).
pub struct Calc1;
impl Tool for Calc1 {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "raster_calc", name: "Raster Calculator", category: "Raster › Map Algebra", role: Role::Tool,
            description: "A new raster from an expression of the input raster, called A: A * 0.3048, (A > 1000) * 1, sqrt(A), if(A < 0, 0, A). No-data cells stay no data; results that are not numbers become no data.",
            inputs: vec![rin("Raster (A)")], output: Some(PortKind::Raster),
            params: vec![Param::new("expr", "Expression", ParamKind::Expression).default("A")] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src = Path::new(need_raster(i)?);
        let out = ro::calc(&[src], &p.req_str("expr", "Expression")?)?;
        crate::vertical::inherit(&out, &[src]);
        rout(out, i.crs.clone())
    }
}

/// Raster calculator on two rasters on the same grid (A and B).
pub struct Calc2;
impl Tool for Calc2 {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "raster_calc2", name: "Raster Calculator (A and B)", category: "Raster › Map Algebra", role: Role::Tool,
            description: "A new raster from an expression of two rasters on the same grid, called A and B: (A - B) / (A + B). Use Align Raster first when the grids differ.",
            inputs: vec![Port { id: "a", label: "Raster A", kind: PortKind::Raster }, Port { id: "b", label: "Raster B", kind: PortKind::Raster }],
            output: Some(PortKind::Raster),
            params: vec![Param::new("expr", "Expression", ParamKind::Expression).default("A - B")] }
    }
    fn build(&self, c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["a"], &inputs["b"]);
        let (pa, pb) = (Path::new(need_raster(a)?), Path::new(need_raster(b)?));
        if let Some(n) = crate::vertical::check("the Raster Calculator", &[pa, pb])? {
            c.add_note(n);
        }
        let out = ro::calc(&[pa, pb], &p.req_str("expr", "Expression")?)?;
        crate::vertical::inherit(&out, &[pa, pb]);
        rout(out, a.crs.clone())
    }
}

pub struct Reclassify;
impl Tool for Reclassify {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "reclassify", name: "Reclassify Raster", category: "Raster › Map Algebra", role: Role::Tool,
            description: "Replace cell values by ranges. One rule per line: 0 to 10 = 1 (from 0 up to but not including 10), 5 = 2 (exactly 5), 20 to inf = 3, nodata = 0 (no-data cells get 0), else = nodata (cells no rule matches).",
            inputs: vec![rin("Raster")], output: Some(PortKind::Raster),
            params: vec![
                Param::new("band", "Band", ParamKind::Number).default(1),
                Param::new("rules", "Rules", ParamKind::Expression).default("0 to 10 = 1\n10 to 20 = 2\nelse = nodata").help("0 to 10 = 1"),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let rules = ro::parse_rules(&p.req_str("rules", "Rules")?)?;
        rout(ro::reclassify(Path::new(need_raster(i)?), p.f64("band").unwrap_or(1.0).max(1.0) as usize, &rules)?, i.crs.clone())
    }
}

pub struct NoData;
impl Tool for NoData {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "nodata", name: "NoData Cells", category: "Raster › Map Algebra", role: Role::Tool,
            description: "Set the value that means no data, give no-data cells a value, or fill them by interpolating from the cells around them.",
            inputs: vec![rin("Raster")], output: Some(PortKind::Raster),
            params: vec![
                Param::new("mode", "What to do", ParamKind::Enum).default("declare").options(&[("declare", "Treat a value as no data"), ("replace", "Give no-data cells a value"), ("fill", "Fill no-data cells by interpolation")]),
                Param::new("value", "Value", ParamKind::Number).default(-9999).help("The no-data value (treat) or the new value (give)"),
                Param::new("distance", "Search distance (cells)", ParamKind::Number).default(10).help("Fill: how far to look for cells to interpolate from"),
                Param::new("smoothing", "Smoothing passes", ParamKind::Number).default(0).help("Fill: passes of 3 × 3 averaging on the filled cells"),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let v = p.f64("value").unwrap_or(-9999.0);
        let mode = match p.str("mode").as_deref() {
            Some("replace") => NoDataMode::Replace(v),
            Some("fill") => NoDataMode::Fill { max_distance: p.f64("distance").unwrap_or(10.0), smoothing: p.f64("smoothing").unwrap_or(0.0).max(0.0) as u32 },
            _ => NoDataMode::Declare(v),
        };
        let src = Path::new(need_raster(i)?);
        let out = ro::nodata_tool(src, mode)?;
        crate::vertical::inherit(&out, &[src]);
        rout(out, i.crs.clone())
    }
}

pub struct SelectBands;
impl Tool for SelectBands {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "select_bands", name: "Select Bands", category: "Raster › Management", role: Role::Tool,
            description: "Keep some bands of a multi-band raster, in the order given (for example 3,2,1).",
            inputs: vec![rin("Raster")], output: Some(PortKind::Raster),
            params: vec![Param::new("bands", "Bands to keep", ParamKind::Text).default("1")] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        rout(ro::select_bands(Path::new(need_raster(i)?), &p.req_str("bands", "Bands to keep")?)?, i.crs.clone())
    }
}

pub struct ResampleRaster;
impl Tool for ResampleRaster {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "resample_raster", name: "Resample Raster", category: "Raster › Management", role: Role::Tool,
            description: "Change the cell size (in the raster's own units: metres, or degrees for a geographic raster).",
            inputs: vec![rin("Raster")], output: Some(PortKind::Raster),
            params: vec![
                Param::new("cell", "New cell size", ParamKind::Number).default(30),
                Param::new("resampling", "Resampling", ParamKind::Enum).default("bilinear").options(&RESAMPLING),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src = Path::new(need_raster(i)?);
        let out = ro::resample(src, p.f64("cell").unwrap_or(0.0), &p.str("resampling").unwrap_or_else(|| "bilinear".into()))?;
        crate::vertical::inherit(&out, &[src]);
        rout(out, i.crs.clone())
    }
}

pub struct AlignRaster;
impl Tool for AlignRaster {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "align_raster", name: "Align Raster", category: "Raster › Management", role: Role::Tool,
            description: "Put a raster on another raster's grid: its coordinate system, extent and cell size, so the two can be combined cell by cell.",
            inputs: vec![rin("Raster to align"), Port { id: "reference", label: "Reference raster", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![Param::new("resampling", "Resampling", ParamKind::Enum).default("bilinear").options(&RESAMPLING)] }
    }
    fn build(&self, c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (i, r) = (&inputs["in"], &inputs["reference"]);
        need_raster_crs(i)?;
        let (src, refp) = (Path::new(need_raster(i)?), Path::new(need_raster(r)?));
        if let Some(n) = crate::vertical::check("Align Raster", &[src, refp])? {
            c.add_note(n);
        }
        let out = ro::align(src, refp, &p.str("resampling").unwrap_or_else(|| "bilinear".into()))?;
        crate::vertical::inherit(&out, &[src]); // the values are the aligned raster's own heights
        rout(out, r.crs.clone())
    }
}

pub struct MosaicRasters;
impl Tool for MosaicRasters {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "mosaic_rasters", name: "Mosaic Rasters", category: "Raster › Management", role: Role::Tool,
            description: "Join two rasters into one covering both (the second wins where they overlap). Join more by chaining Mosaic nodes. The result is in the first raster's coordinate system.",
            inputs: vec![Port { id: "a", label: "First raster", kind: PortKind::Raster }, Port { id: "b", label: "Second raster", kind: PortKind::Raster }], output: Some(PortKind::Raster),
            params: vec![Param::new("resampling", "Resampling", ParamKind::Enum).default("near").options(&RESAMPLING)] }
    }
    fn build(&self, c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (a, b) = (&inputs["a"], &inputs["b"]);
        need_raster_crs(a)?;
        need_raster_crs(b)?;
        let (pa, pb) = (Path::new(need_raster(a)?), Path::new(need_raster(b)?));
        if let Some(n) = crate::vertical::check("Mosaic Rasters", &[pa, pb])? {
            c.add_note(n);
        }
        let out = ro::mosaic(&[pa, pb], &p.str("resampling").unwrap_or_else(|| "near".into()))?;
        crate::vertical::inherit(&out, &[pa, pb]);
        rout(out, a.crs.clone())
    }
}

pub struct Rasterize;
impl Tool for Rasterize {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "rasterize", name: "Rasterize (Vector to Raster)", category: "Raster › Conversion", role: Role::Tool,
            description: "Burn features into a raster covering them: each cell takes the value of a number field, or 1 where there is a feature. The cell size is in the layer's units.",
            inputs: vec![Port { id: "in", label: "Features", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("field", "Value field (empty: burn 1)", ParamKind::Field).optional(),
                Param::new("cell", "Cell size", ParamKind::Number).default(30),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = crate::crs::resolve(need_crs(i)?);
        let field = p.str("field").filter(|f| !f.is_empty());
        let sel = match &field {
            Some(f) => format!("{}::DOUBLE AS v, ", ident(f)),
            None => String::new(),
        };
        let gpkg = raster::new_store_path("rasterize_in", "gpkg");
        ctx.conn.execute_batch(&format!(
            "COPY (SELECT {sel}geom FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)) TO {} (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'f', SRS {}, USE_TMP_FILE false)",
            ident(&i.table), lit(gpkg.to_str().unwrap()), lit(&crs)
        ))?;
        let out = ro::rasterize(&gpkg, field.as_ref().map(|_| "v"), p.f64("cell").unwrap_or(0.0));
        let _ = std::fs::remove_file(&gpkg);
        rout(out?, i.crs.clone())
    }
}

/// Surface from points (QGIS: IDW interpolation `qgis:idwinterpolation`, TIN interpolation `qgis:tininterpolation`, via GDAL grid algorithms).
pub struct Interpolate;
impl Tool for Interpolate {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "interpolate", name: "Interpolate Points to Raster", category: "Raster › Interpolation", role: Role::Tool,
            description: "Make a surface from points that have a number field. IDW: weighted by distance (the power sets how fast influence falls off); Linear: triangulated, no data outside the points' outline; Nearest: value of the closest point; Average: mean of points within the radius. Cell size and radius are in the layer's units. Points without a number are ignored.",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }], output: Some(PortKind::Raster),
            params: vec![
                Param::new("field", "Value field", ParamKind::Field),
                Param::new("method", "Method", ParamKind::Enum).default("idw").options(&[("idw", "IDW (inverse distance)"), ("linear", "Linear (TIN)"), ("nearest", "Nearest point"), ("average", "Moving average")]),
                Param::new("cell", "Cell size", ParamKind::Number).default(100),
                Param::new("power", "IDW power", ParamKind::Number).default(2),
                Param::new("radius", "Search radius (0: none)", ParamKind::Number).default(0),
            ] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let crs = crate::crs::resolve(need_crs(i)?);
        let field = p.req_str("field", "Value field")?;
        // Cell size and radius are in map units: say so when the projection stretches lengths over the points (CRS accuracy plan C3).
        if let Ok(ext) = ctx.conn.query_row(
            &format!("SELECT min(ST_XMin(geom)), min(ST_YMin(geom)), max(ST_XMax(geom)), max(ST_YMax(geom)) FROM {} WHERE geom IS NOT NULL", ident(&i.table)),
            [],
            |r| Ok([r.get::<_, Option<f64>>(0)?, r.get(1)?, r.get(2)?, r.get(3)?]),
        ) {
            if let [Some(a), Some(b), Some(c), Some(d)] = ext {
                if let Some(n) = crate::crs_warnings::planar_note("Interpolate Points to Raster", need_crs(i)?, [a, b, c, d]) {
                    ctx.add_note(n);
                }
            }
        }
        let gpkg = raster::new_store_path("interpolate_in", "gpkg");
        ctx.conn.execute_batch(&format!(
            "COPY (SELECT TRY_CAST({}::VARCHAR AS DOUBLE) AS v, geom FROM {} WHERE geom IS NOT NULL AND ST_GeometryType(geom) = 'POINT' AND TRY_CAST({}::VARCHAR AS DOUBLE) IS NOT NULL) TO {} (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'f', SRS {}, USE_TMP_FILE false)",
            ident(&field), ident(&i.table), ident(&field), lit(gpkg.to_str().unwrap()), lit(&crs)
        ))?;
        let out = ro::interpolate(&gpkg, p.str("method").as_deref().unwrap_or("idw"), p.f64("cell").unwrap_or(0.0), p.f64("power").unwrap_or(2.0), p.f64("radius").unwrap_or(0.0));
        let _ = std::fs::remove_file(&gpkg);
        rout(out?, i.crs.clone())
    }
}

/// Record which height reference a raster's values use (CRS accuracy plan C1).
pub struct DefineVerticalDatum;
impl Tool for DefineVerticalDatum {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "define_vertical_datum", name: "Define Vertical Datum", category: "Raster › Management", role: Role::Tool,
            description: "Record which height reference an elevation raster uses (NAVD88, a geoid such as EGM2008, ellipsoidal heights from GPS, …). The values do not change. Raster Calculator, Align and Mosaic then warn when you combine rasters on different vertical datums, whose heights can differ by metres. USGS 3DEP downloads are recorded as NAVD88 and Copernicus DEM as EGM2008 automatically. Converting between datums needs a geoid grid and is not done here.",
            inputs: vec![rin("Elevation raster")], output: Some(PortKind::Raster),
            params: vec![Param::new("datum", "Vertical datum", ParamKind::Enum).default("NAVD88").options(&crate::vertical::DATUMS)] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let out = crate::vertical::define(Path::new(need_raster(i)?), &p.req_str("datum", "Vertical datum")?)?;
        rout(out, i.crs.clone())
    }
}
