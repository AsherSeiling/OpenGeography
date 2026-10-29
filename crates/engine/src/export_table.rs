//! Export Table: a point layer's attributes plus its coordinates, written in a coordinate system the user picks.
//! The target CRS is never guessed or defaulted; its name and units go into a sidecar `<file>.crs.txt`.

use crate::db::{ident, lit};
use crate::progress::Task;
use crate::{crs, Engine};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableExportSpec {
    pub path: String,
    /// "csv" or "parquet".
    pub format: String,
    /// The coordinate system the X/Y (or lon/lat) columns are written in. Required.
    pub crs: String,
    /// Selected features only (None = all features passing the definition query).
    #[serde(default)]
    pub fids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableExportResult {
    pub path: String,
    pub sidecar: String,
    pub rows: i64,
    pub x_column: String,
    pub y_column: String,
    pub crs_name: String,
    pub unit: String,
}

/// Where the CRS description for `path` goes.
pub fn sidecar_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".crs.txt");
    PathBuf::from(s)
}

impl Engine {
    pub fn export_table(&self, id: &str, spec: &TableExportSpec) -> Result<TableExportResult> {
        let info = self.layer(id)?;
        if info.geom_type != "Point" {
            bail!("Export Table needs a point layer; '{}' is a {} layer. Use Feature to Point first, or Export Features for other geometry.", info.name, info.geom_type.to_lowercase());
        }
        let from = info.crs.clone().ok_or_else(|| anyhow!("'{}' has no coordinate system, so its coordinates can't be reprojected. Define its projection first.", info.name))?;
        if spec.crs.trim().is_empty() {
            bail!("Choose the coordinate system for the exported coordinates.");
        }
        let format = spec.format.to_lowercase();
        if format != "csv" && format != "parquet" {
            bail!("Unknown table format '{}'. Choose CSV or Parquet.", spec.format);
        }
        let target = crs::info(&spec.crs).map_err(|e| anyhow!("Can't use the coordinate system '{}': {e}", spec.crs))?;
        let path = PathBuf::from(&spec.path);
        if spec.path.trim().is_empty() {
            bail!("Choose an output file.");
        }
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        for l in self.list_layers() {
            let sources = l.source.iter().cloned().chain(l.raster.iter().map(|r| r.path.clone()));
            for s in sources {
                if canon(Path::new(&s)) == canon(&path) || canon(Path::new(&s)) == canon(&sidecar_path(&path)) {
                    bail!("{} is the source file of the layer '{}'. Export to a different file so the original data is not overwritten.", spec.path, l.name);
                }
            }
        }
        let (xn, yn) = if target.geographic { ("lon", "lat") } else { ("x", "y") };
        let taken: Vec<String> = info.fields.iter().map(|f| f.name.to_lowercase()).collect();
        let unique = |n: &str| {
            let mut c = n.to_string();
            while taken.contains(&c.to_lowercase()) {
                c.push('_');
            }
            c
        };
        let (xc, yc) = (unique(xn), unique(yn));
        let attrs = info.fields.iter().map(|f| ident(&f.name)).collect::<Vec<_>>().join(", ");
        let only = match &spec.fids {
            Some(ids) if ids.is_empty() => " AND FALSE".to_string(),
            Some(ids) => format!(" AND __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        let attr_sql = if attrs.is_empty() { String::new() } else { format!("{attrs}, ") };
        let query = format!(
            "SELECT {attr_sql}ST_X(g) AS {}, ST_Y(g) AS {} FROM (SELECT *, ST_Transform(geom, {}, {}, true) AS g FROM {} WHERE TRUE{}{only} ORDER BY __fid)",
            ident(&xc),
            ident(&yc),
            lit(&crs::resolve(&from)),
            lit(&crs::resolve(&spec.crs)),
            ident(&info.table),
            info.def_and(),
        );
        let task = Task::start(format!("Export table: {}", info.name)).cancellable();
        task.check()?;
        let copy = match format.as_str() {
            "csv" => format!("COPY ({query}) TO {} (FORMAT CSV, HEADER)", lit(&spec.path)),
            _ => format!("COPY ({query}) TO {} (FORMAT PARQUET)", lit(&spec.path)),
        };
        self.with_conn(|c| Ok(c.execute_batch(&copy)?))?;
        task.set(0.9, Some("Writing the coordinate system note"));
        let rows = self.with_conn(|c| Ok(c.query_row(&format!("SELECT count(*) FROM ({query})"), [], |r| r.get::<_, i64>(0))?))?;
        let code = target.code.clone().unwrap_or_else(|| "custom definition".into());
        let note = format!(
            "Coordinate system of the {xc}/{yc} columns in {}\nName: {}\nCode: {code}\nUnits: {}\nDatum: {}\nSource layer: {} (coordinates reprojected from {from})\nColumn order: {xc} is {}, {yc} is {}.\nRows: {rows}\n",
            Path::new(&spec.path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            target.name,
            target.unit,
            target.datum.clone().unwrap_or_else(|| "not stated".into()),
            info.name,
            if target.geographic { "longitude (east positive)" } else { "easting" },
            if target.geographic { "latitude (north positive)" } else { "northing" },
        );
        let side = sidecar_path(&path);
        std::fs::write(&side, note)?;
        Ok(TableExportResult { path: spec.path.clone(), sidecar: side.to_string_lossy().into_owned(), rows, x_column: xc, y_column: yc, crs_name: target.name, unit: target.unit })
    }
}
