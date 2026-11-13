//! LAS / LAZ point clouds (package T8): the file's header, point counts, bounds and coordinate system, read with the `las` crate (MIT; LAZ through
//! `laz`, Apache-2.0, pure Rust). Nothing is guessed: the CRS comes from the file's own WKT record or GeoTIFF keys, and a file with neither is
//! reported as having none. The footprint of a cloud can be imported as a one-polygon layer (`Engine::import` does this for .las and .laz).
//!
//! Method and format: ASPRS LAS Specification 1.4 (R15, 2019), https://www.asprs.org/divisions-committees/lidar-division/laser-las-file-format-exchange-activities.
//! 3DEP point clouds (USGS National Map) are LAZ files in a projected CRS (usually UTM) with an NAVD88 compound WKT.

use anyhow::{anyhow, bail, Result};
use las::Reader;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// True for a path that looks like a point cloud (LAS or LAZ).
pub fn is_pointcloud_path(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "las" | "laz"))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClassCount {
    pub code: u8,
    pub name: String,
    pub count: u64,
}

/// Everything the header says about a point cloud, plus (when scanned) how many points each class has.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PointCloudInfo {
    pub path: String,
    pub name: String,
    pub compressed: bool,
    pub version: String,
    pub point_format: u8,
    pub point_count: u64,
    /// xmin, ymin, zmin, xmax, ymax, zmax as stored in the header.
    pub bounds: [f64; 6],
    pub scale: [f64; 3],
    pub offset: [f64; 3],
    /// Points per return number (1 to 5, as the header counts them).
    pub returns: Vec<u64>,
    pub has_rgb: bool,
    pub has_gps_time: bool,
    pub has_nir: bool,
    pub system_identifier: String,
    pub generating_software: String,
    pub creation_date: Option<String>,
    /// The horizontal CRS as "EPSG:code", or the file's own WKT when it is not an EPSG code. None when the file has none.
    pub crs: Option<String>,
    pub crs_name: Option<String>,
    /// "WKT record" or "GeoTIFF keys"; None when the file has no CRS.
    pub crs_source: Option<String>,
    /// The vertical reference named by the file (e.g. "NAVD88 height"), if any.
    pub vertical: Option<String>,
    /// The value `vertical.rs` knows (NAVD88, NGVD29, EGM96, EGM2008, ...), when the name matches one.
    pub vertical_datum: Option<String>,
    /// Unit of the horizontal coordinates according to the CRS ("metre", "US survey foot", "degree"), if known.
    pub unit: Option<String>,
    /// Points per classification, when the file was scanned.
    pub classes: Option<Vec<ClassCount>>,
    pub vlrs: Vec<String>,
    /// Things the reader should know: a missing CRS, an unreadable CRS record, an unrecognised vertical datum.
    pub notes: Vec<String>,
}

pub fn class_name(code: u8) -> &'static str {
    match code {
        0 => "Never classified",
        1 => "Unassigned",
        2 => "Ground",
        3 => "Low vegetation",
        4 => "Medium vegetation",
        5 => "High vegetation",
        6 => "Building",
        7 => "Low point (noise)",
        8 => "Reserved (key point in LAS 1.0 to 1.3)",
        9 => "Water",
        10 => "Rail",
        11 => "Road surface",
        12 => "Reserved (overlap in LAS 1.0 to 1.3)",
        13 => "Wire, guard",
        14 => "Wire, conductor",
        15 => "Transmission tower",
        16 => "Wire-structure connector",
        17 => "Bridge deck",
        18 => "High noise",
        19..=63 => "Reserved",
        _ => "User defined",
    }
}

/// Read the header (and, with `scan`, every point once for the class counts). `scan` reports progress and can be cancelled.
pub fn inspect(path: &Path, scan: bool) -> Result<PointCloudInfo> {
    if !path.exists() {
        bail!("File not found: {}", path.display());
    }
    let mut reader = Reader::from_path(path).map_err(|e| anyhow!("{} is not a readable LAS or LAZ file: {e}", path.display()))?;
    let h = reader.header().clone();
    let b = h.bounds();
    let t = h.transforms();
    let fmt = h.point_format();
    let mut info = PointCloudInfo {
        path: path.display().to_string(),
        name: path.file_name().and_then(|s| s.to_str()).unwrap_or("points").to_string(),
        compressed: fmt.is_compressed || path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("laz")),
        version: format!("{}.{}", h.version().major, h.version().minor),
        point_format: fmt.to_u8().unwrap_or(0),
        point_count: h.number_of_points(),
        bounds: [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z],
        scale: [t.x.scale, t.y.scale, t.z.scale],
        offset: [t.x.offset, t.y.offset, t.z.offset],
        returns: (1..=5u8).map(|n| h.number_of_points_by_return(n).unwrap_or(0)).collect(),
        has_rgb: fmt.has_color,
        has_gps_time: fmt.has_gps_time,
        has_nir: fmt.has_nir,
        system_identifier: h.system_identifier().to_string(),
        generating_software: h.generating_software().to_string(),
        creation_date: h.date().map(|d| d.to_string()),
        crs: None,
        crs_name: None,
        crs_source: None,
        vertical: None,
        vertical_datum: None,
        unit: None,
        classes: None,
        vlrs: h.all_vlrs().map(|v| format!("{} / {}: {}", v.user_id, v.record_id, v.description)).collect(),
        notes: vec![],
    };
    read_crs(&h, &mut info);
    if info.crs.is_none() && info.notes.is_empty() {
        info.notes.push("This file has no coordinate system (no WKT record and no GeoTIFF keys). It is not guessed: state the coordinate system when you use the file.".into());
    }
    if scan {
        let task = crate::progress::Task::start(format!("Reading {}", info.name)).cancellable();
        let total = info.point_count.max(1);
        let mut counts = [0u64; 256];
        let mut done = 0u64;
        let mut pd = las::PointDataBuilder::new().for_header(&h).build();
        while done < info.point_count {
            let n = reader.fill_points(1_000_000, &mut pd).map_err(|e| anyhow!("Could not read the points of {}: {e}", info.name))?;
            if n == 0 {
                break;
            }
            for c in pd.classification() {
                counts[c as usize] += 1;
            }
            done += n;
            task.set(done as f64 / total as f64, Some(format!("{done} of {total} points")));
            task.check()?;
        }
        let mut v: Vec<ClassCount> = (0..=255u8).filter(|c| counts[*c as usize] > 0).map(|c| ClassCount { code: c, name: class_name(c).to_string(), count: counts[c as usize] }).collect();
        v.sort_by_key(|c| c.code);
        info.classes = Some(v);
    }
    Ok(info)
}

/// The CRS of the file. WKT (LAS 1.4, or the optional record of older files) is preferred over GeoTIFF keys, as the specification says.
fn read_crs(h: &las::Header, info: &mut PointCloudInfo) {
    if let Some(bytes) = h.get_wkt_crs_bytes() {
        let text = String::from_utf8_lossy(bytes).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
        if !text.is_empty() {
            match from_wkt(&text) {
                Ok(c) => {
                    info.crs = Some(c.crs);
                    info.crs_name = c.name;
                    info.vertical = c.vertical;
                    info.unit = c.unit;
                    info.crs_source = Some("WKT record".into());
                }
                Err(e) => info.notes.push(format!("The file's WKT coordinate system could not be read ({e}); no coordinate system is assumed.")),
            }
        }
    }
    if info.crs.is_none() {
        match h.get_geotiff_crs() {
            Ok(Some(g)) => {
                let (projected, geodetic, vertical) = (g.get_projected_crs_geo_key_value(), g.get_geodetic_crs_geo_key_value(), g.get_vertical_crs_geo_key_value());
                // 32767 means "user defined" in GeoTIFF: the parameters are in other keys that are not read here.
                let code = projected.filter(|c| *c != 32767).or(geodetic.filter(|c| *c != 32767));
                match code {
                    Some(c) => match gdal::spatial_ref::SpatialRef::from_epsg(c as u32) {
                        Ok(s) => {
                            info.crs = Some(format!("EPSG:{c}"));
                            info.crs_name = s.name();
                            info.unit = unit_of(&s);
                            info.crs_source = Some("GeoTIFF keys".into());
                        }
                        Err(_) => info.notes.push(format!("The file names EPSG:{c}, which this copy of PROJ does not know; no coordinate system is assumed.")),
                    },
                    None => info.notes.push("The file's GeoTIFF keys do not name an EPSG code (a user-defined coordinate system); it is not reconstructed or guessed.".into()),
                }
                if let Some(v) = vertical.filter(|c| *c != 32767) {
                    info.vertical = gdal::spatial_ref::SpatialRef::from_epsg(v as u32).ok().and_then(|s| s.name()).or(Some(format!("EPSG:{v}")));
                }
            }
            Ok(None) => {}
            Err(e) => info.notes.push(format!("The file's GeoTIFF keys could not be read ({e}); no coordinate system is assumed.")),
        }
    }
    if let Some(v) = &info.vertical {
        info.vertical_datum = vertical_label(v);
        if info.vertical_datum.is_none() {
            info.notes.push(format!("The vertical reference \"{v}\" is not one GIS Studio tracks, so the heights of results are not labelled with a datum."));
        }
    } else if info.crs.is_some() {
        info.notes.push("The file names no vertical reference, so heights carry no datum label. Use Define Vertical Datum on the result if you know it.".into());
    }
}

struct FromWkt {
    crs: String,
    name: Option<String>,
    vertical: Option<String>,
    unit: Option<String>,
}

fn unit_of(s: &gdal::spatial_ref::SpatialRef) -> Option<String> {
    if s.is_geographic() {
        return Some("degree".into());
    }
    s.linear_units_name()
}

/// Split a (possibly compound) WKT into its horizontal CRS and vertical name. A compound CRS is common for 3DEP (NAD83 / UTM + NAVD88 height).
fn from_wkt(wkt: &str) -> Result<FromWkt> {
    let s = gdal::spatial_ref::SpatialRef::from_wkt(wkt).map_err(|e| anyhow!("{e}"))?;
    let json: serde_json::Value = serde_json::from_str(&s.to_projjson().map_err(|e| anyhow!("{e}"))?)?;
    let (horizontal, vertical): (serde_json::Value, Option<String>) = if json.get("type").and_then(|t| t.as_str()) == Some("CompoundCRS") {
        let comps = json.get("components").and_then(|c| c.as_array()).cloned().unwrap_or_default();
        let h = comps.first().cloned().ok_or_else(|| anyhow!("the compound coordinate system has no horizontal part"))?;
        let v = comps.get(1).and_then(|v| v.get("name")).and_then(|n| n.as_str()).map(String::from);
        (h, v)
    } else {
        (json.clone(), None)
    };
    let hs = if horizontal == json { s.clone() } else { gdal::spatial_ref::SpatialRef::from_definition(&horizontal.to_string()).map_err(|e| anyhow!("{e}"))? };
    // "EPSG:code" only when the definition really is that code (the same rule as for rasters); otherwise the full definition is kept.
    let crs = match crate::raster::crs_string(&hs) {
        Some(c) => c,
        None => bail!("the coordinate system cannot be written out"),
    };
    Ok(FromWkt { crs, name: hs.name(), vertical, unit: unit_of(&hs) })
}

/// Map the name of a vertical reference to the labels `vertical.rs` uses.
pub fn vertical_label(name: &str) -> Option<String> {
    let n = name.to_ascii_uppercase();
    if n.contains("NAVD") || n.contains("NORTH AMERICAN VERTICAL DATUM 1988") {
        Some("NAVD88".into())
    } else if n.contains("NGVD") {
        Some("NGVD29".into())
    } else if n.contains("EGM2008") || n.contains("EGM08") {
        Some("EGM2008".into())
    } else if n.contains("EGM96") {
        Some("EGM96".into())
    } else if n.contains("ELLIPSOID") {
        Some("Ellipsoidal WGS 84".into())
    } else {
        None
    }
}

/// The footprint of the cloud (its header bounds) as a one-feature GeoPackage in the file's own CRS (none when the file has none), with the
/// summary as attributes. Used to bring a LAS/LAZ file into the layer list without reading millions of points into the database.
pub fn footprint_gpkg(conn: &duckdb::Connection, info: &PointCloudInfo, out: &Path) -> Result<()> {
    use crate::db::lit;
    let [x0, y0, z0, x1, y1, z1] = info.bounds;
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        bail!("{} has no usable bounds in its header", info.name);
    }
    let wkt = format!("POLYGON(({x0} {y0}, {x1} {y0}, {x1} {y1}, {x0} {y1}, {x0} {y0}))");
    let srs = info.crs.as_ref().map(|c| format!(", SRS {}", lit(&crate::crs::resolve(c)))).unwrap_or_default();
    let format = format!("{} {} point format {}", if info.compressed { "LAZ" } else { "LAS" }, info.version, info.point_format);
    let sql = format!(
        "COPY (SELECT ST_GeomFromText({}) AS geom, {} AS file, {} AS format, {}::DOUBLE AS points, {}::DOUBLE AS zmin, {}::DOUBLE AS zmax, {} AS crs_source, {} AS vertical) TO {} (FORMAT GDAL, DRIVER 'GPKG', LAYER_NAME 'footprint'{srs}, USE_TMP_FILE false)",
        lit(&wkt), lit(&info.name), lit(&format), info.point_count, z0, z1,
        lit(info.crs_source.as_deref().unwrap_or("none")), lit(info.vertical.as_deref().unwrap_or("")), lit(out.to_str().ok_or_else(|| anyhow!("bad path"))?)
    );
    conn.execute_batch(&sql).map_err(|e| anyhow!("Could not write the footprint of {}: {e}", info.name))?;
    Ok(())
}

/// Points per class as a name-keyed map for messages.
pub fn class_summary(info: &PointCloudInfo) -> String {
    let m: BTreeMap<u8, u64> = info.classes.iter().flatten().map(|c| (c.code, c.count)).collect();
    m.iter().map(|(c, n)| format!("{} ({}): {n}", class_name(*c), c)).collect::<Vec<_>>().join(", ")
}
