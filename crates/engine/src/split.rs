//! Split a layer by the values of one field (B6): one new layer per value, or one file per value.
//! Reimplements the behaviour of QGIS `native:splitvectorlayer` (cited in the output's sources note); no QGIS code is used.

use crate::db::{ident, lit};
use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Above this many outputs the user must confirm.
pub const CONFIRM_ABOVE: usize = 200;
/// Hard limit of outputs from one split.
pub const MAX_OUTPUTS: usize = 1000;
/// What the group of rows with no value is called.
pub const NO_VALUE: &str = "(no value)";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SplitTarget {
    /// One new layer in the project per value.
    Layers,
    /// One file per value in `dir`; `format` is "GPKG" or "ESRI Shapefile".
    Files { dir: PathBuf, format: String },
}

/// One value of the field and how many rows have it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SplitGroup {
    /// The value as text; `NO_VALUE` for rows where it is NULL.
    pub value: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitOutput {
    pub value: String,
    pub rows: i64,
    /// Layer name, or file path.
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitResult {
    /// New layers (target `layers`), in value order.
    pub layers: Vec<LayerInfo>,
    pub outputs: Vec<SplitOutput>,
    /// Rows left out of every output (none today: rows with no value get their own output).
    pub skipped_rows: i64,
    pub notes: Vec<String>,
}

/// A value made safe to use as a file name on every platform: letters, digits, space, `-`, `_`, `.`; the rest becomes `_`.
pub fn safe_name(value: &str) -> String {
    let mut s: String = value.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { '_' }).collect();
    s = s.trim().trim_matches('.').trim().to_string();
    if s.is_empty() {
        s = "empty".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
    if reserved.contains(&stem.as_str()) {
        s = format!("_{s}");
    }
    s.chars().take(100).collect()
}

/// `name`, or `name_2`, `name_3`… so no two outputs share a name (compared without case, as most file systems do) and,
/// for files, none lands on an existing file.
fn unique(name: &str, taken: &mut HashSet<String>, exists: &dyn Fn(&str) -> bool) -> String {
    let mut n = 1;
    loop {
        let cand = if n == 1 { name.to_string() } else { format!("{name}_{n}") };
        let key = cand.to_lowercase();
        if !taken.contains(&key) && !exists(&cand) {
            taken.insert(key);
            return cand;
        }
        n += 1;
    }
}

fn canonical(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    match (p.parent().filter(|d| !d.as_os_str().is_empty()).and_then(|d| d.canonicalize().ok()), p.file_name()) {
        (Some(d), Some(f)) => d.join(f),
        _ => p.to_path_buf(),
    }
}

impl Engine {
    /// The values of `field` in a vector layer (honouring its definition query), with row counts, NULL as "(no value)".
    /// This is what the dialog shows before anything is written.
    pub fn split_groups(&self, id: &str, field: &str) -> Result<Vec<SplitGroup>> {
        let info = self.layer(id)?;
        check_splittable(&info, field)?;
        self.with_conn(|c| groups(c, &info, field))
    }

    /// Split `id` by `field`. More than `CONFIRM_ABOVE` outputs need `confirmed`; more than `MAX_OUTPUTS` are refused.
    /// A cancel or an error removes whatever this call already made.
    pub fn split_by_attribute(&self, id: &str, field: &str, target: &SplitTarget, confirmed: bool) -> Result<SplitResult> {
        let info = self.layer(id)?;
        check_splittable(&info, field)?;
        let groups = self.with_conn(|c| groups(c, &info, field))?;
        if groups.is_empty() {
            bail!("'{}' has no rows to split", info.name);
        }
        if groups.len() > MAX_OUTPUTS {
            bail!("Splitting '{}' by {field} would make {} outputs; the limit is {MAX_OUTPUTS}. Pick a field with fewer distinct values.", info.name, groups.len());
        }
        if groups.len() > CONFIRM_ABOVE && !confirmed {
            bail!("Splitting '{}' by {field} would make {} outputs. Confirm to continue.", info.name, groups.len());
        }
        let task = Task::start(format!("Splitting {} by {field}", info.name)).cancellable();
        let mut taken = HashSet::new();
        let mut result = SplitResult { layers: vec![], outputs: vec![], skipped_rows: 0, notes: vec!["Method: split by unique values, after QGIS native:splitvectorlayer (reimplemented).".into()] };
        let mut written: Vec<PathBuf> = Vec::new();
        let run = (|| -> Result<()> {
            // Files: never write onto any layer's source file or an existing file.
            let dir = match target {
                SplitTarget::Files { dir, format } => {
                    if !matches!(format.as_str(), "GPKG" | "ESRI Shapefile") {
                        bail!("Choose GeoPackage or Shapefile as the format");
                    }
                    std::fs::create_dir_all(dir).map_err(|e| anyhow!("Could not create the folder {}: {e}", dir.display()))?;
                    Some(canonical(dir))
                }
                SplitTarget::Layers => None,
            };
            let sources: Vec<PathBuf> = self.list_layers().iter().flat_map(|l| l.source.iter().cloned().chain(l.raster.iter().map(|r| r.path.clone())).map(|s| canonical(Path::new(&s)))).collect();
            let total = groups.len() as f64;
            for (i, g) in groups.iter().enumerate() {
                task.check()?;
                task.set(i as f64 / total, Some(format!("{} of {}: {}", i + 1, groups.len(), g.value)));
                let cond = condition(field, &g.value);
                match target {
                    SplitTarget::Layers => {
                        let name = unique(&format!("{} – {}", info.name, g.value), &mut taken, &|_| false);
                        let layer = self.copy_layer_where(&info, &name, &cond)?;
                        result.outputs.push(SplitOutput { value: g.value.clone(), rows: g.rows, name: layer.name.clone() });
                        result.layers.push(layer);
                    }
                    SplitTarget::Files { format, .. } => {
                        let dir = dir.as_ref().unwrap();
                        let ext = if format == "GPKG" { "gpkg" } else { "shp" };
                        let stem = unique(&safe_name(&g.value), &mut taken, &|n| dir.join(format!("{n}.{ext}")).exists());
                        let path = dir.join(format!("{stem}.{ext}"));
                        if sources.contains(&canonical(&path)) {
                            bail!("{} is the source file of a layer. Choose another folder so the original data is not overwritten.", path.display());
                        }
                        self.with_conn(|c| write_file(c, &info, &cond, format, &path))?;
                        written.push(path.clone());
                        result.outputs.push(SplitOutput { value: g.value.clone(), rows: g.rows, name: path.display().to_string() });
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = run {
            for l in &result.layers {
                let _ = self.remove_layer(&l.id);
            }
            for p in &written {
                remove_vector_file(p);
            }
            return Err(e);
        }
        let n = result.outputs.len();
        result.notes.push(format!("{n} output{} from {} rows.", if n == 1 { "" } else { "s" }, result.outputs.iter().map(|o| o.rows).sum::<i64>()));
        Ok(result)
    }
}

fn check_splittable(info: &LayerInfo, field: &str) -> Result<()> {
    if info.is_raster() {
        bail!("'{}' is a raster; only vector layers and tables can be split by an attribute", info.name);
    }
    if !info.fields.iter().any(|f| f.name == field) {
        bail!("'{}' has no field called '{field}'", info.name);
    }
    Ok(())
}

/// SQL (starting with " AND ") selecting one group's rows.
fn condition(field: &str, value: &str) -> String {
    if value == NO_VALUE {
        // A real value of exactly "(no value)" is also grouped here only when no NULLs exist; see `groups`.
        format!(" AND ({f} IS NULL OR CAST({f} AS VARCHAR) = {v})", f = ident(field), v = lit(value))
    } else {
        format!(" AND CAST({} AS VARCHAR) = {}", ident(field), lit(value))
    }
}

fn groups(conn: &duckdb::Connection, info: &LayerInfo, field: &str) -> Result<Vec<SplitGroup>> {
    let sql = format!(
        "SELECT CAST({f} AS VARCHAR) AS v, count(*) FROM {t} WHERE TRUE{def} GROUP BY 1 ORDER BY v NULLS LAST LIMIT {limit}",
        f = ident(field),
        t = ident(&info.table),
        def = info.def_and(),
        limit = MAX_OUTPUTS + 2,
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)))?;
    let mut out: Vec<SplitGroup> = Vec::new();
    for r in rows {
        let (v, n) = r?;
        out.push(SplitGroup { value: v.unwrap_or_else(|| NO_VALUE.to_string()), rows: n });
    }
    // A real value spelled "(no value)" would collide with the NULL group: merge them into one output.
    let mut merged: Vec<SplitGroup> = Vec::new();
    for g in out {
        match merged.iter_mut().find(|m| m.value == g.value) {
            Some(m) => m.rows += g.rows,
            None => merged.push(g),
        }
    }
    Ok(merged)
}

fn write_file(conn: &duckdb::Connection, info: &LayerInfo, cond: &str, format: &str, path: &Path) -> Result<()> {
    let hidden = if info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
    // GDAL reserves "fid"/"OGC_FID" for its own feature ids; keep the values under a new name instead of failing.
    let reserved: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).filter(|c| c.eq_ignore_ascii_case("fid") || c.eq_ignore_ascii_case("ogc_fid")).collect();
    let excl = std::iter::once(hidden.to_string()).chain(reserved.iter().map(|c| ident(c))).collect::<Vec<_>>().join(", ");
    let renames: String = reserved.iter().map(|c| format!(", {} AS {}", ident(c), ident(&format!("{c}_src")))).collect();
    let srs = match &info.crs {
        Some(c) if info.has_geometry() => format!(", SRS {}", lit(c)),
        _ => String::new(),
    };
    // USE_TMP_FILE false: write in place (DuckDB's temp-file rename fails on Windows while GDAL holds the file).
    let sql = format!(
        "COPY (SELECT * EXCLUDE ({excl}){renames} FROM {} WHERE TRUE{}{cond}) TO {} (FORMAT GDAL, DRIVER {}{srs}, USE_TMP_FILE false)",
        ident(&info.table),
        info.def_and(),
        lit(&path.display().to_string()),
        lit(format)
    );
    conn.execute_batch(&sql).map_err(|e| anyhow!("Could not write {}: {e}", path.display()))
}

/// Delete a file this split wrote, with the sidecar files of a shapefile.
fn remove_vector_file(p: &Path) {
    let _ = std::fs::remove_file(p);
    for ext in ["shx", "dbf", "prj", "cpg"] {
        let _ = std::fs::remove_file(p.with_extension(ext));
    }
}
