//! More raster tools mirroring QGIS Processing / GDAL algorithms (original
//! implementations using GDAL's C API; the matching QGIS algorithm and its
//! documentation are cited per tool).
//! QGIS docs: https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/

use super::*;
use crate::raster;
use std::path::Path;

fn rin() -> Vec<Port> {
    vec![Port { id: "in", label: "Raster", kind: PortKind::Raster }]
}

fn rout(path: std::path::PathBuf, i: &Rel) -> Result<Output> {
    Ok(Output::Raster { path: path.display().to_string(), crs: i.crs.clone() })
}

/// Unique scratch table name for joined results.
fn scratch(prefix: &str) -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("__node_{prefix}_{n:x}")
}

pub struct Aspect;
/// QGIS: Aspect (gdal:aspect) — gdal/rasteranalysis.html#aspect
impl Tool for Aspect {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "aspect", name: "Aspect", category: "Raster", role: Role::Tool,
            description: "Direction each slope faces, in degrees from north (QGIS: Aspect).", inputs: rin(), output: Some(PortKind::Raster), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        rout(raster::dem_mode(Path::new(need_raster(i)?), "aspect", &[])?, i)
    }
}

pub struct Ruggedness;
/// QGIS: Terrain Ruggedness Index (gdal:triterrainruggednessindex), Topographic Position Index (gdal:tpitopographicpositionindex), Roughness (gdal:roughness) — gdal/rasteranalysis.html
impl Tool for Ruggedness {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "ruggedness", name: "Terrain Ruggedness", category: "Raster", role: Role::Tool,
            description: "Terrain Ruggedness Index, Topographic Position Index, or Roughness from an elevation raster (QGIS: TRI / TPI / Roughness).", inputs: rin(), output: Some(PortKind::Raster),
            params: vec![Param::new("mode", "Index", ParamKind::Enum).default("TRI").options(&[("TRI", "Terrain Ruggedness Index (TRI)"), ("TPI", "Topographic Position Index (TPI)"), ("roughness", "Roughness")])] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let mode = match p.str("mode").as_deref() { Some("TPI") => "TPI", Some("roughness") => "roughness", _ => "TRI" };
        rout(raster::dem_mode(Path::new(need_raster(i)?), mode, &[])?, i)
    }
}

pub struct Warp;
/// QGIS: Warp (reproject) (gdal:warpreproject) — gdal/rasterprojections.html#warp-reproject
impl Tool for Warp {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "warp", name: "Warp (Reproject Raster)", category: "Raster", role: Role::Tool,
            description: "Reproject a raster to another coordinate system (QGIS: Warp (reproject)).", inputs: rin(), output: Some(PortKind::Raster),
            params: vec![
                Param::new("crs", "Target coordinate system", ParamKind::Crs),
                Param::new("resampling", "Resampling", ParamKind::Enum).default("bilinear").options(&[("near", "Nearest (categories)"), ("bilinear", "Bilinear"), ("cubic", "Cubic"), ("average", "Average")]),
            ] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let target = crs::resolve(&p.req_str("crs", "Target coordinate system")?);
        let out = raster::reproject(Path::new(need_raster(i)?), &target, &p.str("resampling").unwrap_or_else(|| "bilinear".into()))?;
        Ok(Output::Raster { path: out.display().to_string(), crs: Some(target) })
    }
}

pub struct Polygonize;
/// QGIS: Polygonize (raster to vector) (gdal:polygonize) — gdal/rasterconversion.html#polygonize-raster-to-vector
impl Tool for Polygonize {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "polygonize", name: "Polygonize (Raster to Vector)", category: "Raster", role: Role::Tool,
            description: "Polygons around connected cells that share a value, with that value in DN (QGIS: Polygonize). Best for classified rasters.", inputs: rin(), output: Some(PortKind::Vector), params: vec![] }
    }
    fn build(&self, _c: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let gpkg = raster::polygonize(Path::new(need_raster(i)?))?;
        Ok(Output::Select { sql: format!("SELECT DN, geom::GEOMETRY AS geom FROM ST_Read({})", lit(gpkg.to_str().unwrap())), crs: i.crs.clone(), view: false })
    }
}

pub struct SampleRaster;
/// QGIS: Sample raster values (native:rastersampling) — qgis/rasteranalysis.html#sample-raster-values
impl Tool for SampleRaster {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "sample_raster", name: "Sample Raster Values", category: "Raster", role: Role::Tool,
            description: "Add the raster value under each point as a field (QGIS: Sample raster values).",
            inputs: vec![Port { id: "in", label: "Points", kind: PortKind::Vector }, Port { id: "raster", label: "Raster", kind: PortKind::Raster }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("field", "Field name", ParamKind::Text).default("VALUE")] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (pts, r) = (&inputs["in"], &inputs["raster"]);
        let crs = crs::resolve(need_crs(pts)?);
        let rpath = need_raster(r)?;
        let rwkt = raster::wkt_of(Path::new(rpath))?;
        let field = p.str("field").unwrap_or_else(|| "VALUE".into());
        // Number the input once so coordinates and results line up.
        let numbered = scratch("pts");
        ctx.conn.execute_batch(&format!("CREATE TABLE {} AS SELECT row_number() OVER () AS __rid, * FROM {}", ident(&numbered), ident(&pts.table)))?;
        let mut stmt = ctx.conn.prepare(&format!(
            "SELECT ST_X(c), ST_Y(c) FROM (SELECT __rid, ST_Transform(ST_PointOnSurface(geom), {}, {}, true) AS c FROM {}) ORDER BY __rid",
            lit(&crs), lit(&rwkt), ident(&numbered)
        ))?;
        let coords: Vec<(f64, f64)> = stmt.query_map([], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)))?.collect::<Result<_, _>>()?;
        let vals = raster::sample_points(Path::new(rpath), &coords)?;
        let t = scratch("sample");
        ctx.conn.execute_batch(&format!("CREATE TABLE {} (rid BIGINT, v DOUBLE)", ident(&t)))?;
        {
            let mut app = ctx.conn.appender(&t)?;
            for (k, v) in vals.iter().enumerate() {
                app.append_row(duckdb::params![k as i64 + 1, v])?;
            }
        }
        Ok(Output::Select {
            sql: format!(
                "SELECT a.* EXCLUDE (geom, __rid), s.v AS {f}, a.geom FROM {pt} a LEFT JOIN {st} s ON s.rid = a.__rid",
                f = ident(&field), pt = ident(&numbered), st = ident(&t)
            ),
            crs: pts.crs.clone(), view: false,
        })
    }
}

pub struct ZonalStatistics;
/// QGIS: Zonal statistics (native:zonalstatisticsfb) — qgis/rasteranalysis.html#zonal-statistics
impl Tool for ZonalStatistics {
    fn spec(&self) -> ToolSpec {
        ToolSpec { id: "zonal_statistics", name: "Zonal Statistics", category: "Raster", role: Role::Tool,
            description: "Count, mean, min and max of raster cells inside each polygon, added as fields (QGIS: Zonal statistics).",
            inputs: vec![Port { id: "in", label: "Zones (polygons)", kind: PortKind::Vector }, Port { id: "raster", label: "Raster", kind: PortKind::Raster }],
            output: Some(PortKind::Vector),
            params: vec![Param::new("prefix", "Field prefix", ParamKind::Text).default("Z_")] }
    }
    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (zones, r) = (&inputs["in"], &inputs["raster"]);
        let crs = crs::resolve(need_crs(zones)?);
        let rpath = need_raster(r)?;
        let rwkt = raster::wkt_of(Path::new(rpath))?;
        let pre = p.str("prefix").unwrap_or_else(|| "Z_".into());
        // Zones in the raster's CRS, numbered, written for gdal_rasterize.
        let numbered = scratch("zones");
        ctx.conn.execute_batch(&format!("CREATE TABLE {} AS SELECT row_number() OVER () AS __z, * FROM {} WHERE ST_Dimension(geom) = 2", ident(&numbered), ident(&zones.table)))?;
        let gpkg = raster::new_store_path("zones", "gpkg");
        ctx.conn.execute_batch(&format!(
            "COPY (SELECT __z::INTEGER AS ZONE, ST_Transform(geom, {c}, {w}, true) AS geom FROM {t})
             TO {out} (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'zones', SRS {w}, USE_TMP_FILE false)",
            c = lit(&crs), w = lit(&rwkt), t = ident(&numbered), out = lit(gpkg.to_str().unwrap())
        ))?;
        let stats = raster::zonal(Path::new(rpath), &gpkg)?;
        let _ = std::fs::remove_file(&gpkg);
        let t = scratch("zonal");
        ctx.conn.execute_batch(&format!("CREATE TABLE {} (zone BIGINT, n BIGINT, mean DOUBLE, mn DOUBLE, mx DOUBLE)", ident(&t)))?;
        {
            let mut app = ctx.conn.appender(&t)?;
            for (z, (n, sum, mn, mx)) in &stats {
                app.append_row(duckdb::params![*z, *n, sum / *n as f64, *mn, *mx])?;
            }
        }
        let f = |s: &str| ident(&format!("{pre}{s}"));
        Ok(Output::Select {
            sql: format!(
                "SELECT a.* EXCLUDE (geom, __z), coalesce(s.n, 0) AS {c}, s.mean AS {m}, s.mn AS {mi}, s.mx AS {ma}, a.geom
                 FROM {zt} a LEFT JOIN {st} s ON s.zone = a.__z",
                c = f("COUNT"), m = f("MEAN"), mi = f("MIN"), ma = f("MAX"), zt = ident(&numbered), st = ident(&t)
            ),
            crs: zones.crs.clone(), view: false,
        })
    }
}
