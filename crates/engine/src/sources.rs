//! Data sources and citations. Every import records which source it came
//! from; the project's source log (saved with the project) lists each source
//! once, with the layers that used it, as an annotated bibliography.
//!
//! Built-in sources carry publisher-recommended citations (USGS, Census
//! Bureau). Local files get a citation stub the user completes.

use anyhow::Result;
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Citation {
    /// Stable id, e.g. "usgs-nhd" or "file:/path/to/data.shp".
    pub key: String,
    /// Author or organization.
    pub author: String,
    /// Year or vintage; "n.d." for continuously updated services.
    pub year: String,
    pub title: String,
    /// Edition, version or product series, if any.
    pub version: String,
    pub publisher: String,
    pub url: String,
    /// "Web service", "Dataset", "Local file", "Basemap".
    pub kind: String,
    pub license: String,
    /// What the data is and how it was used.
    pub annotation: String,
    /// True once the user has edited the citation (local files start incomplete).
    pub complete: bool,
}

const USGS: &str = "U.S. Geological Survey";
const PUBLIC_DOMAIN_USGS: &str = "Public domain (U.S. Government work). USGS asks that it be credited as the source.";

fn usgs(key: &str, title: &str, version: &str, url: &str, annotation: &str) -> Citation {
    Citation {
        key: key.into(),
        author: USGS.into(),
        year: "n.d.".into(),
        title: title.into(),
        version: version.into(),
        publisher: "U.S. Geological Survey, The National Map".into(),
        url: url.into(),
        kind: "Web service".into(),
        license: PUBLIC_DOMAIN_USGS.into(),
        annotation: annotation.into(),
        complete: true,
    }
}

/// Citation for a National Map vector dataset id (see `tnm_vector::DATASETS`).
pub fn for_tnm(dataset_id: &str) -> Citation {
    let (key, title, version, url, note) = match dataset_id {
        id if id.starts_with("nhd_") => (
            "usgs-nhd",
            "National Hydrography Dataset (NHD)",
            "NHD high resolution, nhd MapServer",
            "https://hydro.nationalmap.gov/arcgis/rest/services/nhd/MapServer",
            "Surface water of the United States: streams, rivers, canals, lakes, ponds, reservoirs and water points, mapped at 1:24,000 or better. Used for streams, waterbodies and other hydrography.",
        ),
        id if id.starts_with("wbd_") => (
            "usgs-wbd",
            "Watershed Boundary Dataset (WBD)",
            "wbd MapServer",
            "https://hydro.nationalmap.gov/arcgis/rest/services/wbd/MapServer",
            "Nested hydrologic unit (HUC) drainage-area boundaries, co-developed by USGS and USDA NRCS. Used for watershed polygons.",
        ),
        "roads_major" | "roads_local" | "roads_4wd" | "railroads" | "airports" => (
            "usgs-ntd",
            "National Transportation Dataset (NTD)",
            "transportation MapServer",
            "https://carto.nationalmap.gov/arcgis/rest/services/transportation/MapServer",
            "Roads, railroads and airports compiled by USGS from federal, state and commercial sources (roads largely from U.S. Census Bureau TIGER). Used for transportation layers.",
        ),
        "trails" => (
            "usgs-trails",
            "National Digital Trails",
            "transportation MapServer, Trails layer",
            "https://carto.nationalmap.gov/arcgis/rest/services/transportation/MapServer",
            "Trails compiled by USGS with the U.S. Forest Service, National Park Service, Bureau of Land Management and others. Used for the trails layer.",
        ),
        id if id.starts_with("st_") => (
            "usgs-nsd",
            "National Structures Dataset (NSD)",
            "structures MapServer",
            "https://carto.nationalmap.gov/arcgis/rest/services/structures/MapServer",
            "Points for schools, emergency services, government buildings, cemeteries and recreation facilities, maintained by USGS with volunteer (crowdsourced) edits. Used for structure points.",
        ),
        id if id.starts_with("gn_") => (
            "usgs-gnis",
            "Geographic Names Information System (GNIS)",
            "geonames MapServer",
            "https://carto.nationalmap.gov/arcgis/rest/services/geonames/MapServer",
            "Official federal names for places and physical features, maintained by USGS for the U.S. Board on Geographic Names. Used for place and feature names.",
        ),
        _ => (
            "usgs-nbd",
            "National Boundary Dataset (NBD) — governmental unit and federal land boundaries",
            "govunits MapServer",
            "https://carto.nationalmap.gov/arcgis/rest/services/govunits/MapServer",
            "Boundaries of states, counties, places and federal lands (national parks, forests, wilderness, BLM, military and wildlife refuges). Used for federal land polygons.",
        ),
    };
    usgs(key, title, version, url, note)
}

/// Citation for USGS 3DEP elevation.
pub fn for_3dep() -> Citation {
    usgs(
        "usgs-3dep",
        "3D Elevation Program (3DEP) — best available elevation",
        "3DEPElevation ImageServer (dynamic, resampled from the best available 3DEP DEMs)",
        "https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer",
        "Bare-earth elevation in meters (NAVD88) from lidar and other sources, nationwide at 1/3 arc-second (~10 m) and 1 m where lidar exists. Downloaded clipped to the area and projected to UTM; used for elevation rasters and derived contours, hillshade and slope.",
    )
}

/// Citations for a TIGER dataset id: TIGER geography, plus the 2020 Census
/// counts carried by tract, block group and block layers.
pub fn for_tiger(dataset_id: &str, label: &str) -> Vec<Citation> {
    let census = |key: &str, year: &str, title: &str, version: &str, url: &str, annotation: String| Citation {
        key: key.into(),
        author: "U.S. Census Bureau".into(),
        year: year.into(),
        title: title.into(),
        version: version.into(),
        publisher: "U.S. Department of Commerce, U.S. Census Bureau".into(),
        url: url.into(),
        kind: "Web service".into(),
        license: "Public domain (U.S. Government work).".into(),
        annotation,
        complete: true,
    };
    let census2020 = matches!(dataset_id, "tracts" | "block_groups" | "blocks");
    let mut out = vec![census(
        if census2020 { "census-tigerweb-2020" } else { "census-tigerweb-current" },
        if census2020 { "2020" } else { "n.d." },
        "TIGERweb: Topologically Integrated Geographic Encoding and Referencing (TIGER) geography",
        if census2020 { "tigerWMS_Census2020 REST service" } else { "tigerWMS_Current, Transportation and Hydro REST services" },
        "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb",
        format!("Census Bureau boundaries, roads and water features from the TIGER database. Used for: {label}."),
    )];
    if census2020 {
        out.push(census(
            "census-2020-pl",
            "2021",
            "2020 Census Redistricting Data (Public Law 94-171) Summary File",
            "POP100 and HU100 counts as published on TIGERweb 2020 Census layers",
            "https://www.census.gov/programs-surveys/decennial-census/about/rdo/summary-files.html",
            "Total population (POP100) and housing units (HU100) from the 2020 Census, attached to tract, block group and block geography.".into(),
        ));
    }
    out
}

/// Citation stub for a local file; the user fills in author, year and title.
pub fn for_file(path: &Path, format: &str, features: i64, crs: Option<&str>) -> Citation {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("data").to_string();
    Citation {
        key: format!("file:{}", path.display()),
        author: String::new(),
        year: String::new(),
        title: name.clone(),
        version: String::new(),
        publisher: String::new(),
        url: String::new(),
        kind: "Local file".into(),
        license: String::new(),
        annotation: format!(
            "{format} file{}{} — {}. Add the author, year and where it came from.",
            if features > 0 { format!(", {features} features") } else { String::new() },
            crs.map(|c| format!(", {c}")).unwrap_or_default(),
            path.display()
        ),
        complete: false,
    }
}

/// One entry in the project's bibliography.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceEntry {
    pub citation: Citation,
    /// ISO date the source was first used in this project ("accessed").
    pub first_used: String,
    pub uses: Vec<SourceUse>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceUse {
    pub layer_id: String,
    pub layer_name: String,
    pub at: String,
}

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS __sources (key VARCHAR PRIMARY KEY, citation JSON, first_used VARCHAR);
         CREATE TABLE IF NOT EXISTS __source_uses (key VARCHAR, layer_id VARCHAR, layer_name VARCHAR, used_on VARCHAR);",
    )?;
    Ok(())
}

fn today(conn: &Connection) -> Result<String> {
    Ok(conn.query_row("SELECT strftime(current_date, '%Y-%m-%d')", [], |r| r.get(0))?)
}

/// Record that `layer` came from `citation`. A citation the user already
/// edited is kept as they wrote it.
pub fn record(conn: &Connection, citation: &Citation, layer_id: &str, layer_name: &str) -> Result<()> {
    let day = today(conn)?;
    let existing: Option<String> = conn
        .query_row("SELECT citation::VARCHAR FROM __sources WHERE key = ?", [&citation.key], |r| r.get(0))
        .ok();
    match existing.and_then(|j| serde_json::from_str::<Citation>(&j).ok()) {
        None => {
            conn.execute("INSERT INTO __sources VALUES (?, ?, ?)", [&citation.key, &serde_json::to_string(citation)?, &day])?;
        }
        // Built-in citations refresh (wording may improve between versions);
        // a local-file citation the user completed is theirs.
        Some(old) if !(old.kind == "Local file" && old.complete) => {
            conn.execute("UPDATE __sources SET citation = ? WHERE key = ?", [&serde_json::to_string(citation)?, &citation.key])?;
        }
        Some(_) => {}
    }
    conn.execute("INSERT INTO __source_uses VALUES (?, ?, ?, ?)", [&citation.key, layer_id, layer_name, &day])?;
    Ok(())
}

/// Keep the log's layer names current after a rename.
pub fn rename_layer(conn: &Connection, layer_id: &str, name: &str) -> Result<()> {
    conn.execute("UPDATE __source_uses SET layer_name = ? WHERE layer_id = ?", [name, layer_id])?;
    Ok(())
}

/// Replace a citation (user edits). The key can't change.
pub fn update(conn: &Connection, citation: &Citation) -> Result<()> {
    let mut c = citation.clone();
    c.complete = true;
    conn.execute("UPDATE __sources SET citation = ? WHERE key = ?", [&serde_json::to_string(&c)?, &c.key])?;
    Ok(())
}

pub fn remove(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM __sources WHERE key = ?", [key])?;
    conn.execute("DELETE FROM __source_uses WHERE key = ?", [key])?;
    Ok(())
}

/// All sources, in order of first use.
pub fn list(conn: &Connection) -> Result<Vec<SourceEntry>> {
    let mut stmt = conn.prepare("SELECT key, citation::VARCHAR, first_used FROM __sources ORDER BY first_used, key")?;
    let rows: Vec<(String, String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut uses = conn.prepare("SELECT layer_id, layer_name, min(used_on) FROM __source_uses WHERE key = ? GROUP BY layer_id, layer_name ORDER BY min(used_on), layer_name")?;
    let mut out = Vec::new();
    for (key, json, first_used) in rows {
        let citation: Citation = serde_json::from_str(&json).unwrap_or_default();
        let u: Vec<SourceUse> = uses
            .query_map([&key], |r| Ok(SourceUse { layer_id: r.get(0)?, layer_name: r.get(1)?, at: r.get(2)? }))?
            .collect::<Result<_, _>>()?;
        out.push(SourceEntry { citation, first_used, uses: u });
    }
    Ok(out)
}

/// A layer made from other layers (a copy, a selection, a saved model result)
/// uses their sources too.
pub fn inherit(conn: &Connection, from_ids: &[&str], layer_id: &str, layer_name: &str) -> Result<()> {
    let day = today(conn)?;
    for from in from_ids {
        conn.execute(
            "INSERT INTO __source_uses SELECT DISTINCT key, ?, ?, ? FROM __source_uses WHERE layer_id = ?",
            [layer_id, layer_name, &day, from],
        )?;
    }
    Ok(())
}

/// Set a derived layer's sources to those of `from_ids` (replacing any it had).
pub fn replace_derived(conn: &Connection, from_ids: &[&str], layer_id: &str, layer_name: &str) -> Result<()> {
    conn.execute("DELETE FROM __source_uses WHERE layer_id = ?", [layer_id])?;
    inherit(conn, from_ids, layer_id, layer_name)?;
    // One row per source, however many inputs shared it.
    conn.execute(
        "DELETE FROM __source_uses WHERE layer_id = ? AND rowid NOT IN (SELECT min(rowid) FROM __source_uses WHERE layer_id = ? GROUP BY key)",
        [layer_id, layer_id],
    )?;
    Ok(())
}

/// Citation for a basemap id (see `BASEMAPS` in the UI's map/style.ts).
pub fn for_basemap(id: &str) -> Option<Citation> {
    let basemap = |key: &str, title: &str, url: &str, annotation: &str| {
        let mut c = usgs(key, title, "The National Map base map tile service", url, annotation);
        c.kind = "Basemap".into();
        c
    };
    Some(match id {
        "usgsTopo" => basemap(
            "usgs-basemap-topo",
            "USGS Topo base map",
            "https://basemap.nationalmap.gov/arcgis/rest/services/USGSTopo/MapServer",
            "Topographic base map combining contours, shaded relief, hydrography, boundaries, names and transportation from The National Map. Used as the map background.",
        ),
        "usgsImagery" => basemap(
            "usgs-basemap-imagery",
            "USGS Imagery Only base map",
            "https://basemap.nationalmap.gov/arcgis/rest/services/USGSImageryOnly/MapServer",
            "Orthoimagery from The National Map, mostly USDA Farm Service Agency NAIP imagery at large scales. Used as the map background.",
        ),
        "usgsRelief" => basemap(
            "usgs-basemap-relief",
            "USGS Shaded Relief Only base map",
            "https://basemap.nationalmap.gov/arcgis/rest/services/USGSShadedReliefOnly/MapServer",
            "Shaded relief derived from 3DEP elevation. Used as the map background.",
        ),
        "osm" => Citation {
            key: "osm".into(),
            author: "OpenStreetMap contributors".into(),
            year: "n.d.".into(),
            title: "OpenStreetMap".into(),
            version: "Standard tile layer (tile.openstreetmap.org)".into(),
            publisher: "OpenStreetMap Foundation".into(),
            url: "https://www.openstreetmap.org/copyright".into(),
            kind: "Basemap".into(),
            license: "Open Database License (ODbL) 1.0; maps must show “© OpenStreetMap contributors”.".into(),
            annotation: "Crowdsourced world map of roads, buildings, land use and places. Used as the map background.".into(),
            complete: true,
        },
        _ => return None,
    })
}
