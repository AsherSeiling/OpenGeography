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
    /// The coordinate system the X/Y (or lon/lat) columns are written in. Required unless a grid reference is asked for instead.
    #[serde(default)]
    pub crs: String,
    /// Also (or only) write each point's USNG or MGRS reference as a text column.
    #[serde(default)]
    pub grid_ref: Option<GridRefSpec>,
    /// Selected features only (None = all features passing the definition query).
    #[serde(default)]
    pub fids: Option<Vec<i64>>,
}

/// A USNG/MGRS column of an exported table.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRefSpec {
    /// "usng" (spaces) or "mgrs".
    pub style: String,
    /// Digits per axis, 0 (100 km square) to 5 (1 m).
    pub precision: u8,
    /// "auto" (NAD 83 in North America, WGS 84 elsewhere), "nad83" or "wgs84".
    #[serde(default)]
    pub datum: String,
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
    /// The grid reference column, when one was written.
    pub grid_column: Option<String>,
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
        if spec.crs.trim().is_empty() && spec.grid_ref.is_none() {
            bail!("Choose the coordinate system for the exported coordinates, or a USNG/MGRS reference.");
        }
        let format = spec.format.to_lowercase();
        if format != "csv" && format != "parquet" {
            bail!("Unknown table format '{}'. Choose CSV or Parquet.", spec.format);
        }
        let target = if spec.crs.trim().is_empty() {
            None
        } else {
            Some(crs::info(&spec.crs).map_err(|e| anyhow!("Can't use the coordinate system '{}': {e}", spec.crs))?)
        };
        if let Some(g) = &spec.grid_ref {
            if g.precision > 5 {
                bail!("A USNG/MGRS reference has 0 to 5 digits per axis, not {}", g.precision);
            }
        }
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
        let (xn, yn) = match &target {
            Some(t) if t.geographic => ("lon", "lat"),
            _ => ("x", "y"),
        };
        let taken: Vec<String> = info.fields.iter().map(|f| f.name.to_lowercase()).collect();
        let unique = |n: &str| {
            let mut c = n.to_string();
            while taken.contains(&c.to_lowercase()) {
                c.push('_');
            }
            c
        };
        let (xc, yc) = (unique(xn), unique(yn));
        let grid_col = spec.grid_ref.as_ref().map(|g| unique(crate::usng::Style::parse(&g.style).map(|s| s.name()).unwrap_or("USNG")));
        let attrs = info.fields.iter().map(|f| ident(&f.name)).collect::<Vec<_>>().join(", ");
        let only = match &spec.fids {
            Some(ids) if ids.is_empty() => " AND FALSE".to_string(),
            Some(ids) => format!(" AND __fid IN ({})", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")),
            None => String::new(),
        };
        // The grid reference is added to the whole table first (it needs the geometry), then the columns are picked.
        let (source, skipped) = match &spec.grid_ref {
            Some(g) => {
                let style = crate::usng::Style::parse(&g.style)?;
                let datum = crate::usng::Datum::parse(&g.datum)?;
                let (sql, skipped) = self.with_conn(|c| crate::usng::add_field_sql(Some(c), &info.table, &from, "t.geom", grid_col.as_deref().unwrap_or("USNG"), g.precision, style, datum, None))?;
                (format!("({sql})"), skipped)
            }
            None => (ident(&info.table), 0),
        };
        let transformed = if target.is_some() {
            format!("*, ST_Transform(geom, {}, {}, true) AS g", lit(&crs::resolve(&from)), lit(&crs::resolve(&spec.crs)))
        } else {
            "*".to_string()
        };
        let mut cols: Vec<String> = Vec::new();
        if !attrs.is_empty() {
            cols.push(attrs.clone());
        }
        if target.is_some() {
            cols.push(format!("ST_X(g) AS {}, ST_Y(g) AS {}", ident(&xc), ident(&yc)));
        }
        if let Some(c) = &grid_col {
            cols.push(ident(c));
        }
        let query = format!("SELECT {} FROM (SELECT {transformed} FROM {source} WHERE TRUE{}{only} ORDER BY __fid)", cols.join(", "), info.def_and());
        let task = Task::start(format!("Export table: {}", info.name)).cancellable();
        task.check()?;
        let copy = match format.as_str() {
            "csv" => format!("COPY ({query}) TO {} (FORMAT CSV, HEADER)", lit(&spec.path)),
            _ => format!("COPY ({query}) TO {} (FORMAT PARQUET)", lit(&spec.path)),
        };
        self.with_conn(|c| Ok(c.execute_batch(&copy)?))?;
        task.set(0.9, Some("Writing the coordinate system note"));
        let rows = self.with_conn(|c| Ok(c.query_row(&format!("SELECT count(*) FROM ({query})"), [], |r| r.get::<_, i64>(0))?))?;
        let file = Path::new(&spec.path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let mut note = String::new();
        let (crs_name, unit) = match &target {
            Some(t) => {
                let code = t.code.clone().unwrap_or_else(|| "custom definition".into());
                let transform = match crate::transform_report::datum_note(&from, &spec.crs, info.native_extent)? {
                    Some(n) => n,
                    None => "No datum change; only the map projection differs.".into(),
                };
                note.push_str(&format!(
                    "Coordinate system of the {xc}/{yc} columns in {file}\nName: {}\nCode: {code}\nUnits: {}\nDatum: {}\nSource layer: {} (coordinates reprojected from {from})\nTransformation: {transform}\nColumn order: {xc} is {}, {yc} is {}.\nRows: {rows}\n",
                    t.name,
                    t.unit,
                    t.datum.clone().unwrap_or_else(|| "not stated".into()),
                    info.name,
                    if t.geographic { "longitude (east positive)" } else { "easting" },
                    if t.geographic { "latitude (north positive)" } else { "northing" },
                ));
                (t.name.clone(), t.unit.clone())
            }
            None => {
                note.push_str(&format!("{file}: no X/Y columns were written, only the grid reference.\nSource layer: {}\nRows: {rows}\n", info.name));
                (String::new(), String::new())
            }
        };
        if let (Some(g), Some(c)) = (&spec.grid_ref, &grid_col) {
            let style = crate::usng::Style::parse(&g.style)?;
            note.push_str(&format!(
                "\nGrid reference column {c}: {} reference of each point, {} (UTM zone and latitude band chosen for each point; the 100 km square letters follow the modern WGS 84 lettering). Datum: {}. Digits are cut, not rounded, so the reference names the square holding the point. Points beyond 84°N or 80°S are written in the polar (UPS) grid, which is always WGS 84. Points without a location are left empty ({skipped} here).\n",
                style.name(),
                match g.precision {
                    0 => "the 100 km square only".to_string(),
                    5 => "to 1 m (5 digits per axis)".to_string(),
                    n => format!("with {n} digit{} per axis (a square {} m wide)", if n == 1 { "" } else { "s" }, 10u32.pow(5 - n as u32)),
                },
                match g.datum.as_str() {
                    "nad83" => "NAD 83 (UTM zones 1–23 north; WGS 84 elsewhere)",
                    "auto" => "NAD 83 in UTM zones 1–23 north (the United States and Canada), WGS 84 elsewhere",
                    _ => "WGS 84",
                },
            ));
        }
        let side = sidecar_path(&path);
        std::fs::write(&side, note)?;
        Ok(TableExportResult { path: spec.path.clone(), sidecar: side.to_string_lossy().into_owned(), rows, x_column: if target.is_some() { xc } else { String::new() }, y_column: if target.is_some() { yc } else { String::new() }, crs_name, unit, grid_column: grid_col })
    }
}
