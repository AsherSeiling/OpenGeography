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
            inputs: vec![Port { id: "in", label: "Input", kind: PortKind::Table }],
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
            ],
        }
    }

    fn build(&self, ctx: &Ctx, inputs: &Inputs, p: &Params) -> Result<Output> {
        let i = &inputs["in"];
        let path = p.req_str("path", "Output file")?;
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
                format!("COPY ({src}) TO {} (FORMAT GDAL, DRIVER {}{srs})", lit(&path), lit(driver))
            }
        };
        ctx.conn.execute_batch(&sql)?;
        Ok(Output::Done(format!("Wrote {path}")))
    }
}
