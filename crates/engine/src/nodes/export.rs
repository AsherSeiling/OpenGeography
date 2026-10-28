use super::*;

pub struct Export;

impl Tool for Export {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            id: "export",
            name: "Export Features",
            category: "Output",
            role: Role::Output,
            description: "Write the result to a file. The coordinate system is written with it.",
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Any }],
            output: None,
            params: vec![
                Param::new("path", "Output file", ParamKind::SavePath),
                Param::new("format", "Format", ParamKind::Enum).default("GPKG").options(&[
                    ("GPKG", "GeoPackage (.gpkg)"),
                    ("ESRI Shapefile", "Shapefile (.shp)"),
                    ("GeoJSON", "GeoJSON (.geojson)"),
                    ("FlatGeobuf", "FlatGeobuf (.fgb)"),
                    ("PARQUET", "GeoParquet (.parquet)"),
                    ("CSV", "CSV (attributes + WKT)"),
                ]),
                Param::new("geojson_crs", "GeoJSON coordinates", ParamKind::Enum).default("wgs84").options(&[
                    ("wgs84", "WGS 84 longitude/latitude (standard GeoJSON)"),
                    ("layer", "Keep the layer's coordinate system (older style, with a crs member)"),
                ]),
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let path = p.req_str("path", "Output file")?;
        refuse_source_file(ctx, &path)?;
        if let Some(src) = &i.raster {
            let ds = gdal::Dataset::open(src)?;
            crate::raster::translate(&ds, std::path::Path::new(&path), &["-of", "GTiff", "-co", "COMPRESS=DEFLATE", "-co", "TILED=YES"].map(String::from))?;
            return Ok(Output::Done(format!("Wrote {path} (GeoTIFF)")));
        }
        let fmt = p.str("format").unwrap_or_else(|| "GPKG".into());
        let src = format!("SELECT * FROM {}", ident(&i.table));
        let sql = match fmt.as_str() {
            "PARQUET" => {
                let src = match &i.crs {
                    Some(c) if i.has_geom => format!("SELECT * REPLACE (ST_SetCRS(geom, {}) AS geom) FROM {}", lit(c), ident(&i.table)),
                    _ => src,
                };
                format!("COPY ({src}) TO {} (FORMAT PARQUET)", lit(&path))
            }
            "CSV" => {
                let src = if i.has_geom { format!("SELECT * REPLACE (ST_AsText(geom) AS geom) FROM {}", ident(&i.table)) } else { src };
                format!("COPY ({src}) TO {} (FORMAT CSV, HEADER)", lit(&path))
            }
            "GeoJSON" if i.has_geom && i.crs.as_deref().is_some_and(|c| !crate::crs::same(c, crate::crs::WGS84)) && p.str("geojson_crs").as_deref() != Some("layer") => {
                // RFC 7946: GeoJSON is longitude/latitude on WGS 84. Say so rather than write UTM numbers a reader will take as degrees (RD-7).
                let from = i.crs.as_deref().unwrap_or("");
                let reprojected = format!("SELECT * REPLACE (ST_Transform(geom, {}, {}, true) AS geom) FROM {}", lit(&crate::crs::resolve(from)), lit(crate::crs::WGS84), ident(&i.table));
                ctx.conn.execute_batch(&format!("COPY ({reprojected}) TO {} (FORMAT GDAL, DRIVER 'GeoJSON', SRS {}, USE_TMP_FILE false)", lit(&path), lit(crate::crs::WGS84)))?;
                return Ok(Output::Done(format!("Wrote {path}. Reprojected to WGS 84 longitude/latitude: standard GeoJSON can't hold another coordinate system. Choose \"Keep the layer's coordinate system\" for the older style.")));
            }
            driver => {
                let srs = match &i.crs {
                    Some(c) if i.has_geom => format!(", SRS {}", lit(c)),
                    _ => String::new(),
                };
                // GDAL reserves "fid"/"OGC_FID" for its own feature ids; keep
                // the values under a new name instead of failing.
                let reserved: Vec<String> = ctx
                    .columns(i)?
                    .into_iter()
                    .filter(|c| c.eq_ignore_ascii_case("fid") || c.eq_ignore_ascii_case("ogc_fid"))
                    .collect();
                let src = if reserved.is_empty() {
                    src
                } else {
                    let renames = reserved.iter().map(|c| format!("{} AS {}", ident(c), ident(&format!("{c}_src")))).collect::<Vec<_>>();
                    let excl = reserved.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
                    format!("SELECT * EXCLUDE ({excl}), {} FROM {}", renames.join(", "), ident(&i.table))
                };
                // USE_TMP_FILE false: write in place. DuckDB otherwise writes a temp
                // file and renames it, which Windows refuses while GDAL holds it open.
                format!("COPY ({src}) TO {} (FORMAT GDAL, DRIVER {}{srs}, USE_TMP_FILE false)", lit(&path), lit(driver))
            }
        };
        ctx.conn.execute_batch(&sql)?;
        Ok(Output::Done(format!("Wrote {path}")))
    }
}

/// Canonical form of a path that may not exist yet: the parent is resolved, the file name kept.
fn canonical(p: &std::path::Path) -> std::path::PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let parent = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.canonicalize().ok(),
        _ => std::path::Path::new(".").canonicalize().ok(),
    };
    match (parent, p.file_name()) {
        (Some(d), Some(f)) => d.join(f),
        _ => p.to_path_buf(),
    }
}

/// Never overwrite source data: refuse an output path that is any layer's source file.
fn refuse_source_file(ctx: &Ctx, path: &str) -> Result<()> {
    let target = canonical(std::path::Path::new(path));
    for l in ctx.layers.values() {
        let sources = l.source.iter().cloned().chain(l.raster.iter().map(|r| r.path.clone()));
        for s in sources {
            if canonical(std::path::Path::new(&s)) == target {
                bail!("{path} is the source file of the layer '{}'. Export to a different file so the original data is not overwritten.", l.name);
            }
        }
    }
    Ok(())
}
