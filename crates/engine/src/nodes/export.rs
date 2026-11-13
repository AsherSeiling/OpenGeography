use super::*;
use serde_json::Value;

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
                    ("TSV", "Tab-separated text (.tsv)"),
                    ("XLSX", "Excel workbook (.xlsx)"),
                    ("ODS", "OpenDocument spreadsheet (.ods)"),
                    ("JSON", "JSON records (.json)"),
                    ("DBF", "dBASE table (.dbf)"),
                    ("KML", "Google Earth KML (.kml)"),
                    ("GPX", "GPS exchange GPX (.gpx, points and lines only)"),
                    ("DXF", "AutoCAD DXF (.dxf)"),
                ]),
                Param::new("crs", "Output coordinate system", ParamKind::Crs).optional().help("Blank = the layer's own. KML and GPX are always WGS 84; standard GeoJSON is too (see below)."),
                Param::new("fields", "Fields to write", ParamKind::Text).optional().help("Blank = every field. Otherwise names separated by commas."),
                Param::new("clip_layer", "Clip to the shapes of", ParamKind::Layer).optional().help("Keep only the parts of the features inside this polygon layer."),
                Param::new("clip_extent", "Clip to an area", ParamKind::Text).optional().help("west,south,east,north in degrees; the map view fills this in from the Export dialog."),
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
        let mut notes: Vec<String> = Vec::new();

        // ---- which fields, which shapes, in which coordinate system ----
        let all_cols = ctx.columns(i)?;
        let keep: Vec<String> = match field_list(p) {
            Some(want) => {
                for w in &want {
                    if !all_cols.iter().any(|c| c == w) {
                        bail!("Field '{w}' isn't in this layer, so it can't be exported. Fields: {}.", all_cols.join(", "));
                    }
                }
                want
            }
            None => all_cols.clone(),
        };
        let kept_note = (keep.len() != all_cols.len()).then(|| format!("{} of {} fields written", keep.len(), all_cols.len()));
        let cols = keep.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
        let comma = if cols.is_empty() { "" } else { ", " };
        let spatial_fmt = !matches!(fmt.as_str(), "XLSX" | "ODS" | "DBF" | "TSV" | "JSON");
        let mut geom = "geom".to_string();
        let mut target: Option<String> = i.crs.clone();
        let mut clipped = false;
        let mut clip_parts: Option<(String, i32)> = None; // the mask SQL and ST_CollectionExtract kind, to count what the clip removes
        if i.has_geom {
            let from = i.crs.clone();
            // Clip first, in the layer's own coordinate system.
            let extent = p.str("clip_extent").filter(|e| !e.trim().is_empty());
            let clip_layer = p.str("clip_layer").filter(|l| !l.trim().is_empty());
            if extent.is_some() || clip_layer.is_some() {
                let src = from.as_deref().ok_or_else(|| anyhow!("This layer has no coordinate system, so it can't be clipped. Use Define Projection first."))?;
                let mask = match (&extent, &clip_layer) {
                    (Some(e), _) => {
                        let v: Vec<f64> = e.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        if v.len() != 4 || v[0] >= v[2] || v[1] >= v[3] {
                            bail!("The clip area should be west,south,east,north in degrees, for example -98,35,-97,36.");
                        }
                        format!("ST_Transform(ST_MakeEnvelope({}, {}, {}, {}), {}, {}, true)", v[0], v[1], v[2], v[3], lit(crate::crs::WGS84), lit(&crate::crs::resolve(src)))
                    }
                    (None, Some(l)) => {
                        let cl = ctx.layers.get(l).ok_or_else(|| anyhow!("The layer to clip to isn't in the project any more."))?;
                        let ccrs = cl.crs.as_deref().ok_or_else(|| anyhow!("'{}' has no coordinate system, so its shapes can't be used to clip. Use Define Projection on it first.", cl.name))?;
                        if cl.geom_type != "Polygon" && cl.geom_type != "Mixed" {
                            bail!("Clip to a layer needs polygons; '{}' holds {} features.", cl.name, cl.geom_type.to_lowercase());
                        }
                        format!(
                            "(SELECT ST_Union_Agg(ST_Transform(geom, {}, {}, true)) FROM {} WHERE geom IS NOT NULL{})",
                            lit(&crate::crs::resolve(ccrs)),
                            lit(&crate::crs::resolve(src)),
                            ident(&cl.table),
                            cl.def_and()
                        )
                    }
                    _ => unreachable!(),
                };
                let kind = ctx.dimension(i)? + 1; // ST_CollectionExtract: 1 points, 2 lines, 3 polygons
                geom = format!("ST_CollectionExtract(ST_Intersection(geom, {mask}), {kind})");
                clip_parts = Some((mask.clone(), kind));
                clipped = true;
            }
            // Then the output coordinate system.
            let chosen = p.str("crs").filter(|c| !c.trim().is_empty());
            if let Some(c) = &chosen {
                crate::crs::validate(ctx.conn, c).map_err(|e| anyhow!("Can't use the coordinate system '{c}': {e}"))?;
                if from.is_none() {
                    bail!("This layer has no coordinate system, so it can't be moved into another. Use Define Projection first.");
                }
                target = Some(c.clone());
            }
            // KML and GPX are always longitude/latitude on WGS 84 (their standards say so).
            if matches!(fmt.as_str(), "KML" | "GPX") && target.as_deref().is_some_and(|t| !crate::crs::same(t, crate::crs::WGS84)) {
                notes.push(format!("{fmt} is always WGS 84 longitude/latitude, so the shapes were moved into it."));
                target = Some(crate::crs::WGS84.to_string());
            }
            if let (Some(f), Some(t)) = (&from, &target) {
                if !crate::crs::same(f, t) {
                    geom = format!("ST_Transform({geom}, {}, {}, true)", lit(&crate::crs::resolve(f)), lit(&crate::crs::resolve(t)));
                    if spatial_fmt && chosen.is_some() && !matches!(fmt.as_str(), "KML" | "GPX") {
                        notes.push(format!("Shapes written in {t}."));
                    }
                }
            }
            if fmt == "GPX" && ctx.dimension(i)? == 2 {
                bail!("GPX holds points (waypoints) and lines (routes and tracks) only; this layer has polygons. Choose KML or GeoPackage, or turn the polygons into lines first (Polygons to Lines).");
            }
        }
        let t = ident(&i.table);
        let base = if !i.has_geom {
            format!("SELECT {} FROM {t}", if cols.is_empty() { "*".to_string() } else { cols.clone() })
        } else if clipped {
            format!("SELECT {cols}{comma}g AS geom FROM (SELECT {cols}{comma}{geom} AS g FROM {t}) WHERE g IS NOT NULL AND NOT ST_IsEmpty(g)")
        } else if geom == "geom" {
            format!("SELECT {cols}{comma}geom FROM {t}")
        } else {
            format!("SELECT {cols}{comma}{geom} AS geom FROM {t}")
        };
        let src = format!("SELECT * FROM ({base})");
        // Report what the clip took out: features wholly outside, features with no shape, and features whose part inside was a different kind of shape.
        if let Some((mask, kind)) = &clip_parts {
            let (outside, wrong, noshape): (i64, i64, i64) = ctx.conn.query_row(
                &format!(
                    "SELECT count(*) FILTER (WHERE geom IS NOT NULL AND (x IS NULL OR ST_IsEmpty(x))),
                            count(*) FILTER (WHERE x IS NOT NULL AND NOT ST_IsEmpty(x) AND (e IS NULL OR ST_IsEmpty(e))),
                            count(*) FILTER (WHERE geom IS NULL)
                     FROM (SELECT geom, x, ST_CollectionExtract(x, {kind}) AS e FROM (SELECT geom, ST_Intersection(geom, {mask}) AS x FROM {t}))"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let s = |n: i64| if n == 1 { "" } else { "s" };
            if outside > 0 {
                notes.push(format!("{outside} feature{} lay outside the clip area and {} not written", s(outside), if outside == 1 { "was" } else { "were" }));
            }
            if wrong > 0 {
                notes.push(format!("{wrong} feature{} had only a different kind of shape (a point or line, not the layer's own kind) inside the clip area and {} not written", s(wrong), if wrong == 1 { "was" } else { "were" }));
            }
            if noshape > 0 {
                notes.push(format!("{noshape} feature{} had no shape to clip and {} not written", s(noshape), if noshape == 1 { "was" } else { "were" }));
            }
        }

        if fmt == "GPX" {
            if !i.has_geom {
                bail!("GPX holds points and lines; this table has no shapes.");
            }
            // GPX goes through GDAL directly so a column called name, desc, ele … fills GPX's own <name>, <desc>, <ele>.
            let extra = write_gpx(ctx, &path, &keep, &base, ctx.dimension(i)?)?;
            notes.extend(extra);
            return Ok(Output::Done(done_text(&path, "", &notes, &kept_note)));
        }
        let sql = match fmt.as_str() {
            // Tables as plain files: shapes (if the layer has any) are dropped, except in CSV/TSV/JSON where they are written as WKT text.
            "XLSX" | "ODS" | "DBF" => {
                let driver = if fmt == "DBF" { "ESRI Shapefile" } else { fmt.as_str() };
                let drop_geom = if i.has_geom { "SELECT * EXCLUDE (geom) FROM" } else { "SELECT * FROM" };
                let renamed = rename_reserved(&keep, &format!("{drop_geom} ({base})"));
                ctx.conn.execute_batch(&format!("COPY ({renamed}) TO {} (FORMAT GDAL, DRIVER {}, USE_TMP_FILE false)", lit(&path), lit(driver)))?;
                let dropped = if i.has_geom { " The shapes were not written (this format holds a table only); use CSV for them as WKT, or a spatial format." } else { "" };
                return Ok(Output::Done(done_text(&path, dropped, &notes, &kept_note)));
            }
            "TSV" | "JSON" | "CSV" => {
                let src = if i.has_geom { format!("SELECT * REPLACE (ST_AsText(geom) AS geom) FROM ({base})") } else { src };
                match fmt.as_str() {
                    "TSV" => format!("COPY ({src}) TO {} (FORMAT CSV, HEADER, DELIMITER {})", lit(&path), lit("\t")),
                    "JSON" => format!("COPY ({src}) TO {} (FORMAT JSON, ARRAY true)", lit(&path)),
                    _ => format!("COPY ({src}) TO {} (FORMAT CSV, HEADER)", lit(&path)),
                }
            }
            "PARQUET" => {
                let src = match &target {
                    Some(c) if i.has_geom => format!("SELECT * REPLACE (ST_SetCRS(geom, {}) AS geom) FROM ({base})", lit(c)),
                    _ => src,
                };
                format!("COPY ({src}) TO {} (FORMAT PARQUET)", lit(&path))
            }
            "GeoJSON" if i.has_geom && target.as_deref().is_some_and(|c| !crate::crs::same(c, crate::crs::WGS84)) && p.str("geojson_crs").as_deref() != Some("layer") => {
                // RFC 7946: GeoJSON is longitude/latitude on WGS 84. Say so rather than write UTM numbers a reader will take as degrees (RD-7).
                let from = target.as_deref().unwrap_or("");
                let reprojected = format!("SELECT * REPLACE (ST_Transform(geom, {}, {}, true) AS geom) FROM ({base})", lit(&crate::crs::resolve(from)), lit(crate::crs::WGS84));
                ctx.conn.execute_batch(&format!("COPY ({reprojected}) TO {} (FORMAT GDAL, DRIVER 'GeoJSON', SRS {}, USE_TMP_FILE false)", lit(&path), lit(crate::crs::WGS84)))?;
                return Ok(Output::Done(format!("Wrote {path}. Reprojected to WGS 84 longitude/latitude: standard GeoJSON can't hold another coordinate system. Choose \"Keep the layer's coordinate system\" to write {from} instead.{}", tail(&notes, &kept_note))));
            }
            driver => {
                let srs = match &target {
                    Some(c) if i.has_geom => format!(", SRS {}", lit(c)),
                    _ => String::new(),
                };
                let driver_name = match driver {
                    "KML" | "GPX" | "DXF" | "ESRI Shapefile" | "GPKG" | "GeoJSON" | "FlatGeobuf" => driver,
                    other => bail!("Unknown export format '{other}'."),
                };
                // GDAL reserves "fid"/"OGC_FID" for its own feature ids; keep the values under a new name instead of failing.
                let mut src = rename_reserved(&keep, &if i.has_geom { format!("SELECT * FROM ({base})") } else { src });
                if driver_name == "DXF" {
                    // A DXF drawing has layers and entities, not attribute tables: only the shapes are written.
                    src = format!("SELECT geom FROM ({base})");
                    if !keep.is_empty() {
                        notes.push("DXF holds shapes only, so no attributes were written".to_string());
                    }
                }
                // USE_TMP_FILE false: write in place. DuckDB otherwise writes a temp
                // file and renames it, which Windows refuses while GDAL holds it open.
                format!("COPY ({src}) TO {} (FORMAT GDAL, DRIVER {}{srs}, USE_TMP_FILE false)", lit(&path), lit(driver_name))
            }
        };
        ctx.conn.execute_batch(&sql).map_err(|e| {
            let m = e.to_string();
            if m.contains("header.dxf") {
                anyhow!("DXF files need GDAL's template file header.dxf, which this installation doesn't have (GDAL_DATA is not set to a full GDAL data folder). Reinstall the app, or choose another format.")
            } else {
                anyhow::Error::from(e)
            }
        })?;
        if fmt == "ESRI Shapefile" {
            notes.extend(shapefile_report(ctx, &path, &keep, &base));
        }
        Ok(Output::Done(done_text(&path, "", &notes, &kept_note)))
    }
}

/// Write a points or lines layer as GPX: points as waypoints, lines as routes. A column whose name matches (any case) one of the
/// layer's predefined GPX fields (name, desc, ele, time, …) fills that field, so it comes out as the standard element; the other
/// columns are written as extensions. If two columns match one field the one spelled exactly like it wins, else the first, and the
/// others are written as extensions (and listed in the result).
fn write_gpx(ctx: &Ctx, path: &str, keep: &[String], base: &str, dimension: i32) -> Result<Vec<String>> {
    use gdal::spatial_ref::SpatialRef;
    use gdal::vector::{Feature, FieldDefn, Geometry, LayerAccess, LayerOptions};
    use gdal_sys::OGRFieldType::{OFTInteger, OFTInteger64, OFTReal, OFTString};
    let mut notes = Vec::new();
    let reserved = |c: &str| c.eq_ignore_ascii_case("fid") || c.eq_ignore_ascii_case("ogc_fid");
    // Column types, to type the extension fields.
    let types: std::collections::HashMap<String, String> = {
        let mut st = ctx.conn.prepare(&format!("DESCRIBE SELECT * FROM ({base})"))?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let (layer_name, geom_type) = if dimension == 0 { ("waypoints", gdal_sys::OGRwkbGeometryType::wkbPoint) } else { ("routes", gdal_sys::OGRwkbGeometryType::wkbLineString) };
    let _ = std::fs::remove_file(path);
    // Asking the driver manager registers GDAL's drivers (once); calling GDAL directly first would find none.
    let gpx = gdal::DriverManager::get_driver_by_name("GPX").map_err(|_| anyhow!("This GDAL has no GPX writer."))?;
    let ds = unsafe {
        let opts = crate::raster::argv_pub(&["GPX_USE_EXTENSIONS=YES".to_string()])?;
        let driver = gpx.c_driver();
        let p = std::ffi::CString::new(path)?;
        let h = gdal_sys::GDALCreate(driver, p.as_ptr(), 0, 0, 0, gdal_sys::GDALDataType::GDT_Unknown, opts.as_ptr());
        if h.is_null() {
            bail!("Could not create the GPX file {path}.");
        }
        gdal::Dataset::from_c_dataset(h)
    };
    let mut ds = ds;
    let mut srs = SpatialRef::from_epsg(4326)?;
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let layer = ds.create_layer(LayerOptions { name: layer_name, srs: Some(&srs), ty: geom_type, options: None })?;
    let predefined: Vec<String> = layer.defn().fields().map(|f| f.name()).collect();
    // Which column fills which predefined field.
    let mut fills: Vec<(String, String)> = Vec::new(); // (column, GPX field)
    for field in &predefined {
        let matches: Vec<&String> = keep.iter().filter(|c| !reserved(c) && c.eq_ignore_ascii_case(field)).collect();
        if let Some(win) = matches.iter().find(|c| c.as_str() == field).or(matches.first()) {
            fills.push(((*win).clone(), field.clone()));
            for other in matches.iter().filter(|c| *c != win) {
                notes.push(format!("'{win}' and '{other}' both match GPX's {field} field: '{win}' was written there and '{other}' as an extension"));
            }
        }
    }
    // Extension fields: every other column (fid-like names are renamed, GDAL keeps them for itself).
    let ext_name = |c: &str| if reserved(c) { format!("{c}_src") } else { c.to_string() };
    let ext: Vec<&String> = keep.iter().filter(|c| !fills.iter().any(|(col, _)| col == *c)).collect();
    for c in &ext {
        let ty = match types.get(c.as_str()).map(|t| t.to_uppercase()) {
            Some(t) if t.contains("INT") => OFTInteger64,
            Some(t) if t.contains("DOUBLE") || t.contains("FLOAT") || t.contains("DECIMAL") || t == "REAL" => OFTReal,
            _ => OFTString,
        };
        FieldDefn::new(&ext_name(c), ty)?.add_to_layer(&layer)?;
    }
    // One waypoint or route per point or line (a multi-part shape becomes several), attributes repeated.
    let cols = keep.iter().map(|c| format!("CAST({} AS VARCHAR)", ident(c))).collect::<Vec<_>>();
    let sql = format!(
        "SELECT ST_AsWKB(__g){}{} FROM (SELECT *, UNNEST(list_transform(ST_Dump(geom), __d -> __d.geom)) AS __g FROM ({base}) WHERE geom IS NOT NULL)",
        if cols.is_empty() { "" } else { ", " },
        cols.join(", ")
    );
    let mut st = ctx.conn.prepare(&sql)?;
    let mut rows = st.query([])?;
    let defn_fields: Vec<(String, u32)> = layer.defn().fields().map(|f| (f.name(), f.field_type())).collect();
    let ftype = |n: &str| defn_fields.iter().find(|(x, _)| x == n).map(|(_, t)| *t);
    let mut written = 0;
    while let Some(r) = rows.next()? {
        let wkb: Vec<u8> = r.get(0)?;
        let mut f = Feature::new(layer.defn())?;
        f.set_geometry(Geometry::from_wkb(&wkb)?)?;
        for (k, c) in keep.iter().enumerate() {
            let Some(v) = r.get::<_, Option<String>>(k + 1)? else { continue };
            let target = fills.iter().find(|(col, _)| col == c).map(|(_, field)| field.clone()).unwrap_or_else(|| ext_name(c));
            let idx = f.field_index(&target)?;
            match ftype(&target) {
                Some(t) if t == OFTReal => {
                    if let Ok(x) = v.trim().parse::<f64>() {
                        f.set_field_double(idx, x)?;
                    }
                }
                Some(t) if t == OFTInteger64 || t == OFTInteger => {
                    if let Ok(x) = v.trim().parse::<i64>() {
                        f.set_field_integer64(idx, x)?;
                    }
                }
                _ => f.set_field_string(idx, &v)?,
            }
        }
        f.create(&layer)?;
        written += 1;
    }
    drop(rows);
    drop(st);
    drop(ds);
    if written == 0 {
        notes.push("No points or lines were written".to_string());
    }
    if !fills.is_empty() {
        notes.push(format!("Written as standard GPX elements: {}", fills.iter().map(|(c, f)| if c.as_str() == f.as_str() { f.clone() } else { format!("{c} → {f}") }).collect::<Vec<_>>().join(", ")));
    }
    if !ext.is_empty() {
        notes.push(format!("{} other field(s) written as extensions", ext.len()));
    }
    Ok(notes)
}

fn done_text(path: &str, first: &str, notes: &[String], kept: &Option<String>) -> String {
    format!("Wrote {path}.{first}{}", tail(notes, kept))
}

fn tail(notes: &[String], kept: &Option<String>) -> String {
    let all: Vec<&str> = kept.iter().map(String::as_str).chain(notes.iter().map(String::as_str)).map(|n| n.trim_end_matches('.')).collect();
    if all.is_empty() { String::new() } else { format!(" {}", all.join(". ")) }
}

/// The fields to write: a JSON list or comma-separated names; None = all.
fn field_list(p: &Params) -> Option<Vec<String>> {
    match p.0.get("fields") {
        Some(Value::Array(a)) => {
            let v: Vec<String> = a.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            (!a.is_empty()).then_some(v)
        }
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()),
        _ => None,
    }
}

/// GDAL keeps "fid"/"OGC_FID" for itself: carry such columns under another name.
fn rename_reserved(keep: &[String], select: &str) -> String {
    let reserved: Vec<&String> = keep.iter().filter(|c| c.eq_ignore_ascii_case("fid") || c.eq_ignore_ascii_case("ogc_fid")).collect();
    if reserved.is_empty() {
        return select.to_string();
    }
    let excl = reserved.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let renames = reserved.iter().map(|c| format!("{} AS {}", ident(c), ident(&format!("{c}_src")))).collect::<Vec<_>>().join(", ");
    format!("SELECT * EXCLUDE ({excl}), {renames} FROM ({select})")
}

/// What a Shapefile could not keep: field names cut to 10 characters, text cut at 254, and the like. Read from the file written.
fn shapefile_report(ctx: &Ctx, path: &str, keep: &[String], base: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(ds) = gdal::Dataset::open(path) {
        if let Ok(layer) = ds.layer(0) {
            use gdal::vector::LayerAccess;
            let written: Vec<String> = layer.defn().fields().map(|f| f.name()).collect();
            // Fields called fid / ogc_fid go last under a new name (see rename_reserved).
            let is_reserved = |c: &String| c.eq_ignore_ascii_case("fid") || c.eq_ignore_ascii_case("ogc_fid");
            let expected: Vec<String> = keep.iter().filter(|c| !is_reserved(c)).cloned().chain(keep.iter().filter(|c| is_reserved(c)).map(|c| format!("{c}_src"))).collect();
            let expected_orig: Vec<String> = keep.iter().filter(|c| !is_reserved(c)).cloned().chain(keep.iter().filter(|c| is_reserved(c)).cloned()).collect();
            let renamed: Vec<String> = expected.iter().zip(expected_orig.iter()).zip(written.iter()).filter(|((exp, _), got)| exp != got).map(|((_, orig), got)| format!("'{orig}' became '{got}'")).collect();
            if !renamed.is_empty() {
                out.push(format!("Shapefile field names are limited to 10 characters, so {}", renamed.join(", ")));
            }
            if written.len() < keep.len() {
                out.push(format!("{} field(s) were not written", keep.len() - written.len()));
            }
        }
    }
    // Text longer than a dBASE field holds (254 characters) is cut.
    for c in keep {
        let q = format!("SELECT max(length(CAST({} AS VARCHAR))) FROM ({base})", ident(c));
        if let Ok(Some(n)) = ctx.conn.query_row(&q, [], |r| r.get::<_, Option<i64>>(0)) {
            if n > 254 {
                out.push(format!("Text in '{c}' is up to {n} characters; a Shapefile keeps 254, so longer values are cut"));
            }
        }
    }
    out
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
pub(super) fn refuse_source_file(ctx: &Ctx, path: &str) -> Result<()> {
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
