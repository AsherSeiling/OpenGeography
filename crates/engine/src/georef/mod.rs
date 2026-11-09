//! Georeferencing (GR1): control points that tie positions in an image to positions on the ground.
//!
//! Conventions, fixed for the whole feature:
//! * `pixel`, `line` follow GDAL: pixel-is-area, so (0, 0) is the top-left *corner* of the top-left pixel and the centre of that
//!   pixel is (0.5, 0.5). `line` grows downwards.
//! * `x`, `y` are in the set's target CRS in traditional GIS order: x is easting or longitude, y is northing or latitude.
//! * The target CRS is chosen by the user and stored with the set. It is never guessed: a file that names none is refused until the
//!   caller supplies one.
//!
//! Sets are saved in the project database (`__control_sets`), so old projects (which lack the table) open unchanged.

pub mod adjust;
pub mod api;
pub mod tools;
pub mod warp;

use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControlPoint {
    pub id: u32,
    /// Image column, GDAL convention (pixel-is-area).
    pub pixel: f64,
    /// Image row, GDAL convention; grows downwards.
    pub line: f64,
    /// Target easting or longitude.
    pub x: f64,
    /// Target northing or latitude.
    pub y: f64,
    /// A point turned off is left out of the fit; its error is still reported as a check point.
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ControlSet {
    pub id: String,
    /// Unique (ignoring case) within the project: processing nodes choose a set by name.
    pub name: String,
    /// The CRS the `x`, `y` values are in. Chosen by the user.
    pub target_crs: String,
    /// The image the points were picked on (a file path), for display and reopening. Not read when the set is used.
    #[serde(default)]
    pub image: Option<String>,
    pub points: Vec<ControlPoint>,
}

impl ControlSet {
    /// Check a set before it is stored or used; errors are sentences for the user.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("A control point set needs an id.");
        }
        if self.name.trim().is_empty() {
            bail!("Give the control point set a name.");
        }
        let crs = self.target_crs.trim();
        if crs.is_empty() {
            bail!("Choose the coordinate system the control points are in before saving the set. It is never guessed.");
        }
        crate::crs::info(crs).map_err(|e| anyhow!("The control point set's coordinate system '{}' can't be read: {e}", short_crs(crs)))?;
        let mut seen = std::collections::HashSet::new();
        for p in &self.points {
            if !seen.insert(p.id) {
                bail!("Two control points share the id {}. Each point needs its own id.", p.id);
            }
            for (what, v) in [("pixel", p.pixel), ("line", p.line), ("x", p.x), ("y", p.y)] {
                if !v.is_finite() {
                    bail!("Control point {} has a {what} value that is not a number.", p.id);
                }
            }
        }
        Ok(())
    }
}

/// Warning for a fit with no spare points: the unknowns are exactly determined, so every residual is about zero whatever the picking.
pub(crate) fn exact_fit_warning(enabled: usize, min: usize) -> Option<String> {
    (enabled == min).then(|| format!("This fit has no spare points: {enabled} enabled points is exactly what the method needs, so it passes through all of them and shows about zero error however carelessly they were picked. That zero says nothing about accuracy. Add more points, or turn some off and read them as check points."))
}

/// A geographic target whose enabled points span more than 180° of longitude (or straddle the antimeridian) can't be fitted: refuse.
pub(crate) fn longitude_span_ok(points: &[&ControlPoint]) -> Result<()> {
    let (lo, hi) = points.iter().fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.x), h.max(p.x)));
    if hi - lo > 180.0 {
        bail!("The enabled control points span {:.0}° of longitude ({lo:.1}° to {hi:.1}°). A fit in degrees is meaningless that far apart or across the 180° meridian. Use a projected coordinate system as the target (for example UTM or a local projection) and enter the points in it.", hi - lo);
    }
    Ok(())
}

/// Geodesic metres between two (x = longitude, y = latitude) positions. A latitude beyond ±90° (a wild fit) is clamped to the pole;
/// the flag says it was, and the figure is then a lower bound. Never NaN.
pub(crate) fn geodesic_m(g: &geographiclib_rs::Geodesic, stated: (f64, f64), placed: (f64, f64)) -> (f64, bool) {
    use geographiclib_rs::InverseGeodesic;
    let off = placed.1.abs() > 90.0 || stated.1.abs() > 90.0;
    let m: f64 = g.inverse(stated.1.clamp(-90.0, 90.0), stated.0, placed.1.clamp(-90.0, 90.0), placed.0);
    (if m.is_finite() { m } else { 0.0 }, off)
}

pub(crate) fn short_crs(crs: &str) -> String {
    let one: String = crs.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 60 {
        format!("{}…", one.chars().take(60).collect::<String>())
    } else {
        one
    }
}

/// What a control point file held. `crs` is `None` when the file names none; the caller must then ask the user (never guess).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ParsedControlFile {
    /// "csv" (GIS Studio) or "qgis" (QGIS `.points`).
    pub format: String,
    pub crs: Option<String>,
    pub points: Vec<ControlPoint>,
    /// Things the user should know about how the file was read.
    pub notes: Vec<String>,
}

const CSV_HEADER: &str = "id,pixel,line,x,y,enabled";

/// The CSV text for a set: a header that states the CRS and the conventions, then one row per point.
pub fn to_csv(set: &ControlSet) -> String {
    let crs: String = set.target_crs.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s = String::new();
    s.push_str("# GIS Studio control points\n");
    s.push_str(&format!("# crs: {crs}\n"));
    s.push_str("# x = easting or longitude, y = northing or latitude (traditional GIS order), in the crs above.\n");
    s.push_str("# pixel, line = GDAL image coordinates: pixel-is-area, (0,0) is the top-left corner of the top-left pixel, line grows downwards.\n");
    s.push_str(CSV_HEADER);
    s.push('\n');
    for p in &set.points {
        s.push_str(&format!("{},{},{},{},{},{}\n", p.id, p.pixel, p.line, p.x, p.y, if p.enabled { 1 } else { 0 }));
    }
    s
}

fn parse_bool(v: &str, line_no: usize) -> Result<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Ok(true),
        "0" | "false" | "no" | "n" | "off" => Ok(false),
        other => bail!("Line {line_no}: '{other}' is not an enabled flag (use 1 or 0)."),
    }
}

fn parse_num(v: &str, what: &str, line_no: usize) -> Result<f64> {
    let n: f64 = v.trim().parse().map_err(|_| anyhow!("Line {line_no}: '{}' is not a number for {what}.", v.trim()))?;
    if !n.is_finite() {
        bail!("Line {line_no}: {what} is not a finite number.");
    }
    Ok(n)
}

/// Read a control point file: GIS Studio CSV or QGIS `.points`. The CRS is returned exactly as the file states it (or `None`);
/// nothing is converted or assumed here.
pub fn parse_control_file(text: &str) -> Result<ParsedControlFile> {
    parse_control_file_for(text, None)
}

/// As [`parse_control_file`], with the image's (width, height) in pixels when known: a QGIS file whose image positions fall outside
/// the image is refused, because it then holds map coordinates (made on an already georeferenced raster), not pixels.
pub fn parse_control_file_for(text: &str, image: Option<(f64, f64)>) -> Result<ParsedControlFile> {
    let text = text.trim_start_matches('\u{feff}');
    let mut crs: Option<String> = None;
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            let rest = rest.trim();
            if rest.len() >= 4 && rest.is_char_boundary(4) && rest[..4].to_ascii_lowercase() == "crs:" {
                let v = rest[4..].trim();
                if !v.is_empty() {
                    crs = Some(v.to_string());
                }
            }
            continue;
        }
        let cells: Vec<String> = line.split(',').map(|c| c.trim().to_string()).collect();
        if header.is_none() {
            header = Some(cells.iter().map(|c| c.to_ascii_lowercase()).collect());
            continue;
        }
        rows.push((i + 1, cells));
    }
    let header = header.ok_or_else(|| anyhow!("This file has no control points. Expected a header line such as '{CSV_HEADER}' (GIS Studio) or 'mapX,mapY,sourceX,sourceY,enable,dX,dY,residual' (QGIS)."))?;
    let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
    let mut points = Vec::new();
    if let (Some(mx), Some(my), Some(sx), Some(sy)) = (col(&["mapx"]), col(&["mapy"]), col(&["sourcex", "pixelx"]), col(&["sourcey", "pixely"])) {
        // QGIS: sourceX is the pixel column; sourceY is the row counted downwards but written with a minus sign. Both are measured
        // from the top-left corner of the image, the same origin as GDAL.
        let en = col(&["enable", "enabled"]);
        let mut any_positive_y = false;
        for (n, (ln, r)) in rows.iter().enumerate() {
            let get = |c: usize| r.get(c).map(String::as_str).ok_or_else(|| anyhow!("Line {ln}: too few columns."));
            let sy_v = parse_num(get(sy)?, "sourceY", *ln)?;
            if sy_v > 0.0 {
                any_positive_y = true;
            }
            points.push(ControlPoint {
                id: n as u32 + 1,
                pixel: parse_num(get(sx)?, "sourceX", *ln)?,
                line: -sy_v,
                x: parse_num(get(mx)?, "mapX", *ln)?,
                y: parse_num(get(my)?, "mapY", *ln)?,
                enabled: match en {
                    Some(c) => parse_bool(get(c)?, *ln)?,
                    None => true,
                },
            });
        }
        if any_positive_y {
            bail!("This QGIS file has positive sourceY values. QGIS writes the image row as a negative number (sourceY = -line), so positive values mean the points are not image positions measured from the top-left corner. Nothing was imported.");
        }
        if let Some((w, h)) = image {
            if let Some(p) = points.iter().find(|p| p.pixel < -0.5 || p.pixel > w + 0.5 || p.line < -0.5 || p.line > h + 0.5) {
                bail!("QGIS point {} is at pixel {}, line {}, outside this {} by {} pixel image. In a .points file made on an already georeferenced raster, sourceX and sourceY hold map coordinates, not image pixels, and can't be imported here. Nothing was imported.", p.id, p.pixel, p.line, w, h);
            }
        }
        let notes = vec!["QGIS file: sourceX and sourceY are read as image pixels (line = -sourceY, measured from the top-left corner like GDAL). A .points file made on an already georeferenced raster holds map coordinates there and must not be imported. The residual columns are ignored; errors are recomputed.".to_string()];
        Ok(ParsedControlFile { format: "qgis".into(), crs, points, notes })
    } else if let (Some(ci), Some(cp), Some(cl), Some(cx), Some(cy)) = (col(&["id"]), col(&["pixel"]), col(&["line"]), col(&["x"]), col(&["y"])) {
        let en = col(&["enabled"]);
        for (ln, r) in &rows {
            let get = |c: usize| r.get(c).map(String::as_str).ok_or_else(|| anyhow!("Line {ln}: too few columns."));
            let id: u32 = get(ci)?.parse().map_err(|_| anyhow!("Line {ln}: '{}' is not a whole-number id.", get(ci).unwrap_or("")))?;
            points.push(ControlPoint {
                id,
                pixel: parse_num(get(cp)?, "pixel", *ln)?,
                line: parse_num(get(cl)?, "line", *ln)?,
                x: parse_num(get(cx)?, "x", *ln)?,
                y: parse_num(get(cy)?, "y", *ln)?,
                enabled: match en {
                    Some(c) => parse_bool(get(c)?, *ln)?,
                    None => true,
                },
            });
        }
        let mut seen = std::collections::HashSet::new();
        if let Some(p) = points.iter().find(|p| !seen.insert(p.id)) {
            bail!("The file gives the id {} to more than one point.", p.id);
        }
        Ok(ParsedControlFile { format: "csv".into(), crs, points, notes: vec![] })
    } else {
        bail!("This file's header ('{}') is not one this version reads. Use '{CSV_HEADER}' or a QGIS .points file ('mapX,mapY,sourceX,sourceY,enable,…').", header.join(","))
    }
}

/// Every control point set in the project, by name.
pub(crate) fn sets_in(c: &duckdb::Connection) -> Result<Vec<ControlSet>> {
    let mut stmt = c.prepare("SELECT json::VARCHAR FROM __control_sets")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(serde_json::from_str::<ControlSet>(&r?)?);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    Ok(out)
}

/// The set with this name (ignoring case), or a readable error naming the sets that exist (a Model node reads it this way).
pub(crate) fn set_named(c: &duckdb::Connection, name: &str) -> Result<ControlSet> {
    if name.trim().is_empty() {
        bail!("Choose a control point set: type the name of one saved in this project.");
    }
    let all = sets_in(c)?;
    let want = name.trim().to_lowercase();
    if let Some(s) = all.iter().find(|s| s.name.trim().to_lowercase() == want) {
        return Ok(s.clone());
    }
    if all.is_empty() {
        bail!("There is no control point set named '{}': this project has none yet. Make one in Georeference.", name.trim());
    }
    bail!("There is no control point set named '{}'. Sets in this project: {}.", name.trim(), all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", "))
}

impl Engine {
    /// Express a set's target positions in another CRS through an explicit PROJ transform, and label the set with the new CRS.
    pub fn reproject_control_set(&self, set: &ControlSet, to_crs: &str) -> Result<ControlSet> {
        set.validate()?;
        crate::crs::info(to_crs).map_err(|e| anyhow!("The coordinate system '{}' can't be read: {e}", short_crs(to_crs)))?;
        let mut out = set.clone();
        out.target_crs = to_crs.trim().to_string();
        if crate::crs::same(&set.target_crs, to_crs) {
            return Ok(out);
        }
        self.with_conn(|c| {
            for p in &mut out.points {
                let (x, y) = crate::crs::transform_point(c, p.x, p.y, &set.target_crs, to_crs)?;
                if !x.is_finite() || !y.is_finite() {
                    bail!("Control point {} can't be expressed in the new coordinate system.", p.id);
                }
                p.x = x;
                p.y = y;
            }
            Ok(())
        })?;
        Ok(out)
    }

    /// Saved control point sets, by name.
    pub fn list_control_sets(&self) -> Result<Vec<ControlSet>> {
        self.with_conn(sets_in)
    }

    /// The set with this name (ignoring case), or a readable error naming the sets that exist.
    pub fn control_set_by_name(&self, name: &str) -> Result<ControlSet> {
        self.with_conn(|c| set_named(c, name))
    }

    /// Validate and store a set (insert or replace). A name already used by another set is refused.
    pub fn save_control_set(&self, set: &ControlSet) -> Result<()> {
        set.validate()?;
        let want = set.name.trim().to_lowercase();
        if let Some(other) = self.list_control_sets()?.iter().find(|s| s.id != set.id && s.name.trim().to_lowercase() == want) {
            bail!("A control point set named '{}' already exists. Choose another name.", other.name);
        }
        let json = serde_json::to_string(set)?;
        self.with_conn(|c| {
            c.execute("INSERT OR REPLACE INTO __control_sets VALUES (?, ?::JSON, now()::TIMESTAMP)", duckdb::params![set.id, json])?;
            Ok(())
        })
    }

    pub fn delete_control_set(&self, id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM __control_sets WHERE id = ?", duckdb::params![id])?;
            Ok(())
        })
    }

    /// Write a set to a CSV file (the header states the CRS), through a temporary file and a rename.
    pub fn export_control_set(&self, set: &ControlSet, path: &std::path::Path) -> Result<()> {
        set.validate()?;
        let tmp = path.with_extension("csv.saving");
        std::fs::write(&tmp, to_csv(set)).map_err(|e| anyhow!("Could not write {}: {e}", path.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            anyhow!("Could not save {}: {e}", path.display())
        })
    }

    /// `image` is the (width, height) of the image the points are for, when one is open: used to refuse QGIS positions outside it.
    pub fn read_control_file(&self, path: &std::path::Path, image: Option<(f64, f64)>) -> Result<ParsedControlFile> {
        let text = std::fs::read_to_string(path).map_err(|e| anyhow!("Could not read {}: {e}", path.display()))?;
        parse_control_file_for(&text, image)
    }
}
