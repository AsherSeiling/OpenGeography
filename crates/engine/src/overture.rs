//! Overture Maps GeoParquet (phase D4): buildings, places, roads, water, land use and boundaries for a lon/lat box, read straight
//! from Overture's public S3 files with DuckDB (httpfs). Only the row groups that overlap the box are fetched, using the files' own
//! `bbox` column. Overture's nested columns are flattened to the few plain fields that are useful in a GIS.

use crate::db::{ident, lit};
use crate::layers::{self, ImportSource, LayerInfo};
use crate::progress::Task;
use crate::sources::{self, Citation};
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;

/// The release read. Overture publishes monthly and removes old ones after a while, so this is pinned and updated with the app.
pub const RELEASE: &str = "2026-10-21.0";
/// Most features one download may hold.
pub const MAX_FEATURES: i64 = 500_000;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: &'static str,
    pub label: &'static str,
    pub theme: &'static str,
    pub kind: &'static str,
    /// (SQL expression over the file's columns, field name, the column it needs)
    #[serde(skip)]
    pub fields: &'static [(&'static str, &'static str, &'static str)],
}

pub const THEMES: &[Theme] = &[
    Theme { id: "buildings", label: "Buildings", theme: "buildings", kind: "building", fields: &[("class", "class", "class"), ("subtype", "subtype", "subtype"), ("height", "height", "height"), ("num_floors", "num_floors", "num_floors"), ("names.primary", "name", "names")] },
    Theme { id: "places", label: "Places of interest (shops, services, attractions)", theme: "places", kind: "place", fields: &[("names.primary", "name", "names"), ("categories.primary", "category", "categories"), ("confidence", "confidence", "confidence")] },
    Theme { id: "roads", label: "Roads, paths and railways", theme: "transportation", kind: "segment", fields: &[("class", "class", "class"), ("subtype", "subtype", "subtype"), ("names.primary", "name", "names")] },
    Theme { id: "water", label: "Water", theme: "base", kind: "water", fields: &[("class", "class", "class"), ("subtype", "subtype", "subtype"), ("names.primary", "name", "names")] },
    Theme { id: "land_use", label: "Land use", theme: "base", kind: "land_use", fields: &[("class", "class", "class"), ("subtype", "subtype", "subtype"), ("names.primary", "name", "names")] },
    Theme { id: "boundaries", label: "Administrative areas (countries, states, counties, places)", theme: "divisions", kind: "division_area", fields: &[("subtype", "subtype", "subtype"), ("class", "class", "class"), ("names.primary", "name", "names"), ("country", "country", "country")] },
];

pub fn theme(id: &str) -> Result<&'static Theme> {
    THEMES.iter().find(|t| t.id == id).ok_or_else(|| anyhow!("Unknown Overture theme '{id}'"))
}

/// The files of one theme and type in a release.
pub fn glob(t: &Theme) -> String {
    format!("s3://overturemaps-us-west-2/release/{RELEASE}/theme={}/type={}/*", t.theme, t.kind)
}

pub fn citation(t: &Theme) -> Citation {
    Citation {
        key: format!("overture:{}", t.id),
        author: "Overture Maps Foundation".into(),
        year: RELEASE[..4].into(),
        title: format!("Overture Maps — {}", t.label),
        version: format!("Release {RELEASE}, theme {} / {}", t.theme, t.kind),
        publisher: "Overture Maps Foundation".into(),
        url: "https://docs.overturemaps.org/".into(),
        kind: "Dataset".into(),
        license: match t.id {
            "places" => "CDLA Permissive 2.0 (credit Overture Maps Foundation).".into(),
            "buildings" | "roads" | "land_use" | "boundaries" => "ODbL 1.0 where the data derives from OpenStreetMap, otherwise CDLA Permissive 2.0 or the source's own open licence; credit Overture Maps Foundation and OpenStreetMap contributors. Check the 'sources' of a feature before redistributing.".into(),
            _ => "ODbL 1.0 where the data derives from OpenStreetMap, otherwise the source's own open licence; credit Overture Maps Foundation and OpenStreetMap contributors.".into(),
        },
        annotation: "Open map data read from Overture's public GeoParquet files for the area; nested fields flattened to name, class and a few others.".into(),
        complete: true,
    }
}

impl Engine {
    /// Download an Overture theme for a lon/lat box as a layer, cited.
    pub fn overture_import(&self, theme_id: &str, bbox: [f64; 4], area_label: &str) -> Result<LayerInfo> {
        let t = theme(theme_id)?;
        self.overture_import_from(t, &glob(t), bbox, area_label)
    }

    /// `overture_import` from another file or glob (tests point it at a local server).
    pub fn overture_import_from(&self, t: &Theme, source: &str, bbox: [f64; 4], area_label: &str) -> Result<LayerInfo> {
        let [w, s, e, n] = bbox;
        if !(w < e && s < n) || bbox.iter().any(|v| !v.is_finite()) {
            bail!("The area is empty");
        }
        let task = Task::start(format!("Downloading Overture {} for {area_label}", t.label)).cancellable();
        task.set(0.05, Some("Connecting to Overture's data…".to_string()));
        let src = format!("read_parquet({}, hive_partitioning = 1)", lit(source));
        let select = self.with_conn(|c| {
            crate::db::load_httpfs(c)?;
            if source.starts_with("s3://") {
                c.execute_batch("SET s3_region = 'us-west-2';")?;
            }
            // The file's columns decide which fields exist and whether geometry arrives as GEOMETRY or WKB.
            let mut cols = std::collections::HashMap::new();
            let mut st = c.prepare(&format!("DESCRIBE SELECT * FROM {src} LIMIT 0")).map_err(|x| anyhow!("Could not read the Overture data ({x})"))?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (name, ty) = r?;
                cols.insert(name, ty);
            }
            let geom_ty = cols.get("geometry").ok_or_else(|| anyhow!("The Overture file has no geometry column"))?;
            let geom = if geom_ty.to_uppercase().contains("GEOMETRY") { "geometry".to_string() } else { "ST_GeomFromWKB(geometry)".to_string() };
            let mut fields = vec![if cols.contains_key("id") { "id".to_string() } else { "NULL::VARCHAR AS id".to_string() }];
            for (expr, name, needs) in t.fields {
                if cols.contains_key(*needs) {
                    fields.push(format!("{expr} AS {}", ident(name)));
                }
            }
            let (has_bbox, bbox_filter) = (cols.contains_key("bbox"), format!("bbox.xmin <= {e} AND bbox.xmax >= {w} AND bbox.ymin <= {n} AND bbox.ymax >= {s}"));
            let filter = if has_bbox { bbox_filter } else { format!("ST_Intersects({geom}, ST_MakeEnvelope({w}, {s}, {e}, {n}))") };
            // One pass over the remote files: the matching rows go into a scratch table (not TEMP: the next step may run on another connection of the pool), which is counted, used as the layer's source, then dropped.
            let tmp = ident(&format!("__overture_{}", self.alloc_id()));
            let created = crate::history::cancellable(c, task.handle(), || {
                c.execute_batch(&format!("CREATE OR REPLACE TABLE {tmp} AS SELECT {}, {geom} AS geom FROM {src} WHERE {filter}", fields.join(", "))).map_err(|x| anyhow!("Could not read the Overture data ({x})"))
            });
            task.check()?;
            created?;
            let count: i64 = c.query_row(&format!("SELECT count(*) FROM {tmp}"), [], |r| r.get(0))?;
            if count == 0 {
                let _ = c.execute_batch(&format!("DROP TABLE {tmp}"));
                bail!("EMPTY");
            }
            if count > MAX_FEATURES {
                let _ = c.execute_batch(&format!("DROP TABLE {tmp}"));
                bail!("{} has {count} features in this area (limit {MAX_FEATURES}). Choose a smaller area.", t.label);
            }
            Ok(tmp)
        });
        let tmp = match select {
            Ok(t) => t,
            Err(e) if e.to_string() == "EMPTY" => bail!("Overture has no {} in this area. Try a larger area.", t.label.to_lowercase()),
            Err(e) => return Err(e),
        };
        task.check()?;
        task.set(0.4, Some("Reading features…".to_string()));
        let name = format!("Overture {} — {area_label}", t.label);
        let import = ImportSource { name: name.clone(), select: format!("SELECT * FROM {tmp}"), crs: Some(crate::crs::WGS84.into()), crs_name: Some("WGS 84".into()), warnings: vec![] };
        let cite = citation(t);
        self.with_conn(|conn| {
            let id = self.alloc_id();
            let info = layers::build_layer(conn, &id, &import)?;
            self.save_meta(conn, &info)?;
            sources::record(conn, &cite, &info.id, &info.name)?;
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {tmp}"));
            Ok(info)
        })
    }
}
