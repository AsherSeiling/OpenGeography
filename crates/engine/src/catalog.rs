//! File manager (Catalog pane): list folders with GIS-aware item types, the
//! usual starting places on each OS, and a quick look inside a dataset.

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    /// folder, project (.gproj), gdb (File Geodatabase), vector, raster,
    /// table, archive, or file (not GIS data).
    pub kind: &'static str,
    /// Size in bytes (a shapefile counts its sidecar files too).
    pub size: Option<u64>,
    /// Last modified, seconds since 1970.
    pub modified: Option<i64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub name: String,
    pub path: String,
    /// home, desktop, documents, downloads, drive, volume
    pub kind: &'static str,
}

const VECTOR: &[&str] = &["shp", "gpkg", "geojson", "json", "kml", "kmz", "fgb", "gml", "gpx", "parquet", "geoparquet", "tab", "mif", "sqlite"];
const TABLE: &[&str] = &["csv", "tsv", "txt", "dbf", "xlsx", "ods"];
const ARCHIVE: &[&str] = &["zip"];
/// Files that belong to a shapefile (shown as one item).
const SHP_PARTS: &[&str] = &["dbf", "shx", "prj", "cpg", "sbn", "sbx", "qix", "fix", "atx", "ain", "aih", "ixs", "mxs", "xml"];

fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// What kind of GIS item a path is.
pub fn kind_of(path: &Path, is_dir: bool) -> &'static str {
    let ext = ext_of(path);
    if is_dir {
        return match ext.as_str() {
            "gdb" => "gdb",
            "gproj" => "project",
            _ => "folder",
        };
    }
    if crate::raster::EXTENSIONS.contains(&ext.as_str()) {
        "raster"
    } else if VECTOR.contains(&ext.as_str()) {
        "vector"
    } else if TABLE.contains(&ext.as_str()) {
        "table"
    } else if ARCHIVE.contains(&ext.as_str()) {
        "archive"
    } else {
        "file"
    }
}

/// Sidecar files hidden behind their main dataset: a shapefile's parts, and
/// GDAL's .aux.xml / .ovr next to rasters.
fn is_sidecar(path: &Path, siblings: &std::collections::HashSet<String>) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name.ends_with(".aux.xml") || name.ends_with(".ovr") || name.ends_with(".shp.xml") {
        return true;
    }
    let ext = ext_of(path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    SHP_PARTS.contains(&ext.as_str()) && siblings.contains(&format!("{stem}.shp"))
}

fn modified(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// List a folder: folders and datasets first (by name), sidecars hidden,
/// dot-files skipped. A File Geodatabase or project folder is one item.
pub fn list(path: &str) -> Result<Listing> {
    let dir = PathBuf::from(path);
    let read = std::fs::read_dir(&dir).with_context(|| format!("Can't open {}", dir.display()))?;
    let items: Vec<(PathBuf, std::fs::Metadata)> = read
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?)))
        .collect();
    let names: std::collections::HashSet<String> = items.iter().filter_map(|(p, _)| p.file_name()?.to_str().map(|s| s.to_lowercase())).collect();
    let mut entries: Vec<Entry> = items
        .iter()
        .filter(|(p, m)| m.is_dir() || !is_sidecar(p, &names))
        .map(|(p, m)| {
            let kind = kind_of(p, m.is_dir());
            let mut size = (!m.is_dir()).then_some(m.len());
            if kind == "vector" && ext_of(p) == "shp" {
                // A shapefile's size includes its parts.
                let stem = p.with_extension("");
                size = Some(SHP_PARTS.iter().chain(&["shp"]).filter_map(|e| std::fs::metadata(stem.with_extension(e)).ok()).map(|m| m.len()).sum());
            }
            Entry { name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), path: p.display().to_string(), kind, size, modified: modified(m) }
        })
        .collect();
    let rank = |k: &str| match k {
        "folder" => 0,
        "project" | "gdb" => 1,
        "file" => 3,
        _ => 2,
    };
    entries.sort_by(|a, b| rank(a.kind).cmp(&rank(b.kind)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing { path: dir.display().to_string(), parent: dir.parent().map(|p| p.display().to_string()), entries })
}

/// One path's current state, for listing a project's files (some may have
/// been moved or deleted since they were added).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PathState {
    pub path: String,
    /// Present on disk, with its item details.
    pub entry: Option<Entry>,
}

/// Look up several paths at once; missing ones come back with no entry.
pub fn stat(paths: &[String]) -> Vec<PathState> {
    paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            let entry = std::fs::metadata(path).ok().map(|m| Entry {
                name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone()),
                path: p.clone(),
                kind: kind_of(path, m.is_dir()),
                size: (!m.is_dir()).then_some(m.len()),
                modified: modified(&m),
            });
            PathState { path: p.clone(), entry }
        })
        .collect()
}

/// Home folder, the usual user folders, and drives/volumes for this OS.
pub fn roots() -> Vec<Root> {
    let mut out = Vec::new();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    if let Some(h) = &home {
        out.push(Root { name: "Home".into(), path: h.display().to_string(), kind: "home" });
        for (name, kind) in [("Desktop", "desktop"), ("Documents", "documents"), ("Downloads", "downloads")] {
            let p = h.join(name);
            if p.is_dir() {
                out.push(Root { name: name.into(), path: p.display().to_string(), kind });
            }
        }
    }
    if cfg!(windows) {
        for letter in b'A'..=b'Z' {
            let p = format!("{}:\\", letter as char);
            if Path::new(&p).exists() {
                out.push(Root { name: format!("{}: drive", letter as char), path: p, kind: "drive" });
            }
        }
    } else {
        let vols: &[&str] = if cfg!(target_os = "macos") { &["/Volumes"] } else { &["/media", "/mnt", "/run/media"] };
        for base in vols {
            let Ok(rd) = std::fs::read_dir(base) else { continue };
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                // Linux mounts usually sit one level down (/media/<user>/<volume>).
                let mounts: Vec<PathBuf> = if !cfg!(target_os = "macos") && *base != "/mnt" {
                    std::fs::read_dir(&p).map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
                } else {
                    vec![p]
                };
                for m in mounts.into_iter().filter(|m| m.is_dir()) {
                    out.push(Root { name: m.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), path: m.display().to_string(), kind: "volume" });
                }
            }
        }
        out.push(Root { name: "Computer".into(), path: "/".into(), kind: "drive" });
    }
    out
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub kind: String,
    pub driver: String,
    pub layers: Vec<LayerSummary>,
    /// Raster: (width, height, bands)
    pub raster: Option<(usize, usize, usize)>,
    pub crs: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LayerSummary {
    pub name: String,
    pub geometry: String,
    pub features: u64,
    pub crs: Option<String>,
}

fn crs_name(sr: &gdal::spatial_ref::SpatialRef) -> Option<String> {
    let code = match (sr.auth_name(), sr.auth_code().ok()) {
        (Some(a), Some(c)) => Some(format!("{a}:{c}")),
        _ => None,
    };
    match (sr.name(), code) {
        (Some(n), Some(c)) => Some(format!("{n} ({c})")),
        (n, c) => n.or(c),
    }
}

/// What's inside a dataset (via GDAL): layers with geometry type, feature
/// count and CRS, or a raster's size and CRS.
pub fn inspect(path: &str) -> Result<Details> {
    use gdal::vector::LayerAccess;
    let p = Path::new(path);
    if !p.exists() {
        bail!("{path} doesn't exist");
    }
    let kind = kind_of(p, p.is_dir());
    if matches!(kind, "folder" | "project" | "file") {
        bail!("Not a dataset GDAL can read");
    }
    let open = if kind == "archive" { format!("/vsizip/{path}") } else { path.to_string() };
    let ds = gdal::Dataset::open(&open).map_err(|e| anyhow!("GDAL can't read it: {e}"))?;
    let mut d = Details { kind: kind.into(), driver: ds.driver().long_name(), ..Default::default() };
    if ds.raster_count() > 0 {
        let (w, h) = ds.raster_size();
        d.raster = Some((w, h, ds.raster_count()));
        d.crs = ds.spatial_ref().ok().as_ref().and_then(crs_name);
    }
    for mut l in ds.layers() {
        let declared = l.defn().geom_fields().next().map(|g| g.field_type());
        let geometry = match declared {
            None => "None (table)".into(),
            // A generic GEOMETRY column: report what the first feature actually is.
            Some(t) if t == gdal::vector::OGRwkbGeometryType::wkbUnknown => l
                .features()
                .find_map(|f| f.geometry().map(|g| gdal::vector::geometry_type_to_name(g.geometry_type())))
                .unwrap_or_else(|| "Geometry".into()),
            Some(t) => gdal::vector::geometry_type_to_name(t),
        };
        d.layers.push(LayerSummary { name: l.name(), geometry, features: l.feature_count(), crs: l.spatial_ref().as_ref().and_then(crs_name) });
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_gis_items_and_hides_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for f in ["roads.shp", "roads.dbf", "roads.shx", "roads.prj", "wells.csv", "dem.tif", "dem.tif.aux.xml", "notes.pdf", ".hidden", "parcels.gpkg", "lone.dbf"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        for f in ["Data.gdb", "Study.gproj", "sub"] {
            std::fs::create_dir(d.join(f)).unwrap();
        }
        let l = list(d.to_str().unwrap()).unwrap();
        let got: Vec<(&str, &str)> = l.entries.iter().map(|e| (e.name.as_str(), e.kind)).collect();
        assert_eq!(
            got,
            vec![
                ("sub", "folder"),
                ("Data.gdb", "gdb"),
                ("Study.gproj", "project"),
                ("dem.tif", "raster"),
                ("lone.dbf", "table"),
                ("parcels.gpkg", "vector"),
                ("roads.shp", "vector"),
                ("wells.csv", "table"),
                ("notes.pdf", "file"),
            ]
        );
        let shp = l.entries.iter().find(|e| e.name == "roads.shp").unwrap();
        assert_eq!(shp.size, Some(4), "a shapefile's size includes its parts");
        assert!(l.parent.is_some());
        assert!(list(d.join("missing").to_str().unwrap()).is_err());
    }

    #[test]
    fn stats_present_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("wells.gpkg");
        std::fs::write(&f, b"xy").unwrap();
        let s = stat(&[f.display().to_string(), dir.path().join("gone.shp").display().to_string()]);
        assert_eq!(s[0].entry.as_ref().map(|e| (e.kind, e.size)), Some(("vector", Some(2))));
        assert!(s[1].entry.is_none());
    }

    #[test]
    fn has_a_home_root() {
        assert!(roots().iter().any(|r| r.kind == "home"));
    }
}
