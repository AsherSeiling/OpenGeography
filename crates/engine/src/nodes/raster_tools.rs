//! Raster tools. Each mirrors a QGIS/GDAL processing algorithm (cited per
//! tool) and runs the corresponding GDAL utility through raster.rs.

use super::{NewField, OutFields};
use super::*;
use crate::raster;
use std::path::Path;

/// Contour lines from an elevation raster.
/// Equivalent of QGIS "Contour" (gdal:contour) / `gdal_contour -a ELEV -i <interval>`.
/// https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/gdal/rasterextraction.html#contour
pub struct Contours;

impl Tool for Contours {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "contours",
            name: "Contour",
            category: "Raster › Terrain and Contours",
            role: Role::Tool,
            description: "Contour lines from an elevation raster, with an ELEV field (in the raster's units, e.g. meters).",
            inputs: vec![Port { id: "in", label: "Elevation raster", kind: PortKind::Raster }],
            output: Some(PortKind::Vector),
            params: vec![
                Param::new("interval", "Interval", ParamKind::Number).default(10).help("Elevation difference between lines"),
                Param::new("base", "Base (offset)", ParamKind::Number).default(0).help("Contours are drawn at base + n × interval"),
                Param::new("index_every", "Index contour every", ParamKind::Number).default(5).help("Adds INDEX = 1 on every nth line (for heavier symbols/labels); 0 for none"),
            ],
        }
    }

    fn output_fields(&self) -> OutFields {
        OutFields::from(&[])
            .add(NewField::new("ID", "INTEGER"))
            .add(NewField::new("ELEV", "DOUBLE"))
            .add(NewField::new("INDEX", "INTEGER").when_positive("index_every"))
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let src = need_raster(i)?;
        let interval = p.f64("interval").unwrap_or(10.0);
        let base = p.f64("base").unwrap_or(0.0);
        let gpkg = raster::contours(Path::new(src), interval, base)?;
        let every = p.f64("index_every").unwrap_or(0.0);
        let index = if every > 0.0 {
            format!(", CASE WHEN abs(round((ELEV - {base}) / {interval}) % {every}) < 1e-9 THEN 1 ELSE 0 END AS INDEX", every = every.round())
        } else {
            String::new()
        };
        Ok(Output::Select {
            sql: format!(
                "SELECT ID, ELEV{index}, geom::GEOMETRY AS geom FROM (SELECT * EXCLUDE (geom), geom FROM ST_Read({}))",
                lit(gpkg.to_str().unwrap())
            ),
            crs: i.crs.clone(),
            view: false,
        })
    }
}

/// Shaded relief.
/// Equivalent of QGIS "Hillshade" (gdal:hillshade) / `gdaldem hillshade`.
/// https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/gdal/rasteranalysis.html#hillshade
pub struct Hillshade;

impl Tool for Hillshade {
    fn result_look(&self) -> ResultLook {
        ResultLook::Gray
    }
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "hillshade",
            name: "Hillshade",
            category: "Raster › Terrain and Contours",
            role: Role::Tool,
            description: "Shaded relief from an elevation raster.",
            inputs: vec![Port { id: "in", label: "Elevation raster", kind: PortKind::Raster }],
            output: Some(PortKind::Raster),
            params: vec![
                Param::new("azimuth", "Sun azimuth (°)", ParamKind::Number).default(315),
                Param::new("altitude", "Sun altitude (°)", ParamKind::Number).default(45),
                Param::new("z", "Z factor (vertical exaggeration)", ParamKind::Number).default(1),
                Param::new("multi", "Multidirectional", ParamKind::Bool).default(false),
            ],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let out = raster::hillshade(Path::new(need_raster(i)?), p.f64("azimuth").unwrap_or(315.0), p.f64("altitude").unwrap_or(45.0), p.f64("z").unwrap_or(1.0), p.bool("multi"))?;
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

/// Slope steepness.
/// Equivalent of QGIS "Slope" (gdal:slope) / `gdaldem slope`.
/// https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/gdal/rasteranalysis.html#slope
pub struct Slope;

impl Tool for Slope {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "slope",
            name: "Slope",
            category: "Raster › Terrain and Contours",
            role: Role::Tool,
            description: "Steepness of an elevation raster, in degrees or percent.",
            inputs: vec![Port { id: "in", label: "Elevation raster", kind: PortKind::Raster }],
            output: Some(PortKind::Raster),
            params: vec![Param::new("units", "Units", ParamKind::Enum).default("degrees").options(&[("degrees", "Degrees"), ("percent", "Percent rise")])],
        }
    }

    fn build(&self, _ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let out = raster::slope(Path::new(need_raster(i)?), p.str("units").as_deref() == Some("percent"))?;
        Ok(Output::Raster { path: out.display().to_string(), crs: i.crs.clone() })
    }
}

/// Cut a raster to the shapes of a polygon layer (cells outside become no-data).
/// Equivalent of QGIS "Clip raster by mask layer" (gdal:cliprasterbymasklayer), which runs `gdalwarp -cutline`.
/// https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/gdal/rasterextraction.html#clip-raster-by-mask-layer
/// "Selected features only" is chosen where the layer enters the graph (Layer Input's selection), as for every tool.
pub struct ClipRasterByLayer;

impl Tool for ClipRasterByLayer {
    fn result_look(&self) -> ResultLook {
        ResultLook::Source
    }
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "clip_raster_by_layer",
            name: "Clip Raster by Layer",
            category: "Raster › Management",
            role: Role::Tool,
            description: "Keep only the cells inside the polygons of a layer; cells outside become no-data. The shapes are moved into the raster's coordinate system first. Cell values and the cell grid are not changed.",
            inputs: vec![
                Port { id: "in", label: "Raster", kind: PortKind::Raster },
                Port { id: "mask", label: "Shapes (polygons)", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Raster),
            params: vec![Param::new("crop", "Crop to the shapes' extent", ParamKind::Bool).default(true)],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let (r, m) = (&inputs["in"], &inputs["mask"]);
        let from = m.crs.as_deref().ok_or_else(|| anyhow!("The shapes layer has no coordinate system, so it can't be placed on the raster. Use Define Projection on it first."))?;
        let to = need_raster_crs(r)?;
        if ctx.dimension(m)? != 2 {
            bail!("Clip Raster by Layer needs polygons. Use Buffer to turn points or lines into areas first.");
        }
        let (src, dst) = (lit(&crate::crs::resolve(from)), lit(&crate::crs::resolve(to)));
        let moved = format!("SELECT ST_Transform(geom, {src}, {dst}, true) AS geom FROM {} WHERE geom IS NOT NULL AND NOT ST_IsEmpty(geom)", ident(&m.table));
        let (n, x0, y0, x1, y1): (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>) =
            ctx.conn.query_row(&format!("SELECT count(*), min(ST_XMin(geom)), min(ST_YMin(geom)), max(ST_XMax(geom)), max(ST_YMax(geom)) FROM ({moved})"), [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?;
        let bbox = match (n, x0, y0, x1, y1) {
            (n, Some(a), Some(b), Some(c), Some(d)) if n > 0 => [a, b, c, d],
            _ => bail!("The shapes layer has no features with a shape, so there is nothing to keep."),
        };
        let file = crate::raster::new_store_path("cutline", "gpkg");
        ctx.conn.execute_batch(&format!("COPY ({moved}) TO {} (FORMAT GDAL, DRIVER 'GPKG', SRS {}, USE_TMP_FILE false)", lit(&file.display().to_string()), lit(&crate::crs::resolve(to))))?;
        let out = raster::clip_to_cutline(Path::new(need_raster(r)?), &file, bbox, p.0.get("crop").and_then(|v| v.as_bool()).unwrap_or(true));
        let _ = std::fs::remove_file(&file);
        Ok(Output::Raster { path: out?.display().to_string(), crs: r.crs.clone() })
    }
}

/// Crop a raster to the extent of features.
/// Equivalent of QGIS "Clip raster by extent" (gdal:cliprasterbyextent).
/// https://docs.qgis.org/latest/en/docs/user_manual/processing_algs/gdal/rasterextraction.html#clip-raster-by-extent
pub struct ClipRaster;

impl Tool for ClipRaster {
    fn result_look(&self) -> ResultLook {
        ResultLook::Source
    }
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "clip_raster",
            name: "Clip Raster by Extent",
            category: "Raster › Management",
            role: Role::Tool,
            description: "Crop a raster to the bounding box of other features.",
            inputs: vec![
                Port { id: "in", label: "Raster", kind: PortKind::Raster },
                Port { id: "extent", label: "Extent from", kind: PortKind::Vector },
            ],
            output: Some(PortKind::Raster),
            params: vec![],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, _p: &Params) -> Result<Output> {
        let (r, e) = (&inputs["in"], &inputs["extent"]);
        let crs = need_crs(e)?;
        need_raster_crs(r)?;
        let (w, s, ea, n): (f64, f64, f64, f64) = ctx.conn.query_row(
            &format!(
                "SELECT min(ST_XMin(g)), min(ST_YMin(g)), max(ST_XMax(g)), max(ST_YMax(g)) FROM (SELECT ST_Transform(geom, {}, 'EPSG:4326', true) AS g FROM {})",
                lit(&crate::crs::resolve(crs)),
                ident(&e.table)
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let out = raster::clip_to_extent(Path::new(need_raster(r)?), [w, s, ea, n])?;
        Ok(Output::Raster { path: out.display().to_string(), crs: r.crs.clone() })
    }
}
