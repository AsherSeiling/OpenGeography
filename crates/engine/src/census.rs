//! Census data join (phase D2): American Community Survey and decennial tables from the Census Bureau Data API, joined to a
//! layer by GEOID. The API key (optional, free) is passed in by the caller from the OS keychain; it is never stored, logged or
//! put in an error message. The join is a command, not a model node: a node would put a network call inside every model run.

use crate::db::{ident, lit};
use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::sources::{self, Citation};
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Read;

pub const BASE_URL: &str = "https://api.census.gov/data";
/// The API takes at most 50 variables per request.
pub const MAX_VARIABLES: usize = 40;
/// Where to get a free key.
pub const KEY_SIGNUP: &str = "https://api.census.gov/data/key_signup.html";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dataset {
    pub id: &'static str,
    pub label: &'static str,
    /// Path after the vintage, e.g. `acs/acs5`.
    pub path: &'static str,
    pub title: &'static str,
}

pub const DATASETS: &[Dataset] = &[
    Dataset { id: "acs5", label: "American Community Survey, 5-year estimates", path: "acs/acs5", title: "American Community Survey 5-Year Estimates" },
    Dataset { id: "acs1", label: "American Community Survey, 1-year estimates (areas of 65,000+ people)", path: "acs/acs1", title: "American Community Survey 1-Year Estimates" },
    Dataset { id: "dec_dhc", label: "Decennial Census, Demographic and Housing Characteristics (2020)", path: "dec/dhc", title: "Decennial Census Demographic and Housing Characteristics" },
    Dataset { id: "dec_pl", label: "Decennial Census, redistricting counts (2020)", path: "dec/pl", title: "Decennial Census Redistricting Data (P.L. 94-171)" },
];

pub fn dataset(id: &str) -> Result<&'static Dataset> {
    DATASETS.iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown Census dataset '{id}'"))
}

/// A geography level a GEOID can have, by its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    State,
    County,
    Tract,
    BlockGroup,
}

impl Level {
    pub fn geoid_len(self) -> usize {
        match self {
            Level::State => 2,
            Level::County => 5,
            Level::Tract => 11,
            Level::BlockGroup => 12,
        }
    }
    pub fn from_len(n: usize) -> Option<Level> {
        [Level::State, Level::County, Level::Tract, Level::BlockGroup].into_iter().find(|l| l.geoid_len() == n)
    }
    fn label(self) -> &'static str {
        match self {
            Level::State => "state",
            Level::County => "county",
            Level::Tract => "tract",
            Level::BlockGroup => "block group",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub dataset: String,
    pub vintage: u32,
    /// Geography level; guessed from the GEOID length when absent.
    pub level: Option<Level>,
    /// Variable codes such as `B19013_001E`.
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinResult {
    pub layer: LayerInfo,
    /// Features of the layer that found a row in the Census table.
    pub matched: i64,
    pub unmatched: i64,
    /// Cells the Census Bureau left empty (suppressed or not available), stored as no value.
    pub suppressed: i64,
    pub notes: Vec<String>,
}

/// Check the variable codes before any request; returns them upper-cased.
pub fn check_variables(vars: &[String]) -> Result<Vec<String>> {
    if vars.is_empty() {
        bail!("Choose at least one Census variable");
    }
    if vars.len() > MAX_VARIABLES {
        bail!("Choose at most {MAX_VARIABLES} variables at a time");
    }
    let mut out = Vec::new();
    for v in vars {
        let v = v.trim().to_uppercase();
        if v.is_empty() || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            bail!("'{v}' is not a Census variable code (they look like B19013_001E)");
        }
        if !out.contains(&v) {
            out.push(v);
        }
    }
    Ok(out)
}

/// The Census Bureau's markers for "no value" in numeric columns.
fn is_missing(v: f64) -> bool {
    [-666666666.0, -999999999.0, -888888888.0, -555555555.0, -333333333.0, -222222222.0].contains(&v)
}

/// One request: the header row and the data rows of the Census JSON answer.
fn get_table(url: &str, vintage: u32, dataset: &Dataset, key_given: bool) -> Result<Vec<Vec<Option<String>>>> {
    let resp = ureq::get(url).set("User-Agent", "GIS-Studio (open-source GIS)").timeout(std::time::Duration::from_secs(90)).call();
    let mut body = String::new();
    match resp {
        Ok(r) => {
            if r.status() == 204 {
                bail!("The Census Bureau has no {} {vintage} data for that geography", dataset.title);
            }
            r.into_reader().take(200_000_000).read_to_string(&mut body)?;
        }
        Err(ureq::Error::Status(code, r)) => {
            let mut text = String::new();
            r.into_reader().take(2000).read_to_string(&mut text).ok();
            let t = text.trim();
            if let Some(v) = t.split('\'').nth(1).filter(|_| t.to_lowercase().contains("unknown variable")) {
                bail!("The Census Bureau has no variable '{v}' in {} {vintage}. Check the code at https://api.census.gov/data/{vintage}/{}/variables.html", dataset.title, dataset.path);
            }
            if key_given && (code == 401 || t.to_lowercase().contains("key")) {
                bail!("The Census Bureau did not accept the API key. Enter it again under Data → Census Data → API key (a free key: {KEY_SIGNUP}).");
            }
            if code == 404 {
                bail!("The Census Bureau has no {} for {vintage}. Choose another year.", dataset.title);
            }
            if code == 400 && t.to_lowercase().contains("key") {
                bail!("The Census Bureau needs an API key for this many requests. Get a free key at {KEY_SIGNUP} and enter it under Data → Census Data → API key.");
            }
            bail!("The Census Bureau refused the request (HTTP {code}){}", if t.is_empty() { String::new() } else { format!(": {}", t.chars().take(200).collect::<String>()) });
        }
        Err(e) => bail!("Could not reach the Census Bureau Data API: {e}"),
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| anyhow!("The Census Bureau answered with something that isn't a table: {}", body.chars().take(150).collect::<String>()))?;
    let rows = v.as_array().ok_or_else(|| anyhow!("The Census Bureau answered with something that isn't a table"))?;
    Ok(rows.iter().map(|r| r.as_array().map(|c| c.iter().map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()).collect())
}

/// All rows of `vars` for `level` in `state` (None for the state level, which is one request for all states):
/// `(geoid, one value per variable)`; the Bureau's "no value" markers become `None`. Returns also how many were missing.
pub fn fetch_level(base: &str, dataset: &Dataset, vintage: u32, level: Level, state: Option<&str>, vars: &[String], key: Option<&str>) -> Result<(Vec<(String, Vec<Option<f64>>)>, i64)> {
    let geo = match (level, state) {
        (Level::State, _) => "for=state:*".to_string(),
        (Level::County, Some(s)) => format!("for=county:*&in=state:{s}"),
        (Level::Tract, Some(s)) => format!("for=tract:*&in=state:{s}&in=county:*"),
        (Level::BlockGroup, Some(s)) => format!("for=block%20group:*&in=state:{s}&in=county:*"),
        _ => bail!("A state is needed to fetch {} data", level.label()),
    };
    let mut url = format!("{base}/{vintage}/{}?get={}&{geo}", dataset.path, vars.join(","));
    if let Some(k) = key.filter(|k| !k.trim().is_empty()) {
        url.push_str(&format!("&key={}", k.trim()));
    }
    let rows = get_table(&url, vintage, dataset, key.is_some_and(|k| !k.trim().is_empty())).map_err(|e| anyhow!(redact(&e.to_string(), key)))?;
    let header = rows.first().ok_or_else(|| anyhow!("The Census Bureau answered with an empty table"))?;
    let col = |name: &str| header.iter().position(|h| h.as_deref() == Some(name));
    let geo_cols: Vec<usize> = ["state", "county", "tract", "block group"].iter().take(match level { Level::State => 1, Level::County => 2, Level::Tract => 3, Level::BlockGroup => 4 }).map(|n| col(n).ok_or_else(|| anyhow!("The Census table has no '{n}' column"))).collect::<Result<_>>()?;
    let var_cols: Vec<usize> = vars.iter().map(|v| col(v).ok_or_else(|| anyhow!("The Census Bureau did not return variable {v}"))).collect::<Result<_>>()?;
    let mut missing = 0;
    let mut out = Vec::new();
    for r in rows.iter().skip(1) {
        let geoid: String = geo_cols.iter().map(|&i| r.get(i).cloned().flatten().unwrap_or_default()).collect();
        let vals: Vec<Option<f64>> = var_cols
            .iter()
            .map(|&i| match r.get(i).cloned().flatten().and_then(|s| s.parse::<f64>().ok()) {
                Some(v) if !is_missing(v) => Some(v),
                _ => {
                    missing += 1;
                    None
                }
            })
            .collect();
        out.push((geoid, vals));
    }
    Ok((out, missing))
}

/// A key must never appear in a message.
fn redact(text: &str, key: Option<&str>) -> String {
    match key.filter(|k| !k.trim().is_empty()) {
        Some(k) => text.replace(k.trim(), "(key)"),
        None => text.to_string(),
    }
}

pub fn citation(d: &Dataset, vintage: u32, vars: &[String]) -> Citation {
    Citation {
        key: format!("census-api:{}:{vintage}", d.id),
        author: "U.S. Census Bureau".into(),
        year: vintage.to_string(),
        title: format!("{} {vintage}", d.title),
        version: format!("Census Data API, {vintage}/{}", d.path),
        publisher: "U.S. Census Bureau".into(),
        url: format!("https://api.census.gov/data/{vintage}/{}.html", d.path),
        kind: "Web service".into(),
        license: "Public domain (U.S. Government work). This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.".into(),
        annotation: format!("Estimates joined to features by GEOID: {}.", vars.join(", ")),
        complete: true,
    }
}

impl Engine {
    /// Join Census variables to `layer` by `geoid_field`, as a new layer. `key` comes from the OS keychain (or is None).
    pub fn census_join(&self, layer: &str, geoid_field: &str, spec: &Spec, key: Option<&str>) -> Result<JoinResult> {
        self.census_join_from(BASE_URL, layer, geoid_field, spec, key)
    }

    /// `census_join` against another API address (tests).
    pub fn census_join_from(&self, base: &str, layer: &str, geoid_field: &str, spec: &Spec, key: Option<&str>) -> Result<JoinResult> {
        let info = self.layer(layer)?;
        if info.is_raster() {
            bail!("'{}' is a raster; a Census join needs features or a table with a GEOID field", info.name);
        }
        let field = info.fields.iter().find(|f| f.name == geoid_field).ok_or_else(|| anyhow!("'{}' has no field called '{geoid_field}'", info.name))?;
        let d = dataset(&spec.dataset)?;
        let vars = check_variables(&spec.variables)?;
        if !(2005..=2100).contains(&spec.vintage) {
            bail!("Choose a year between 2005 and the current year");
        }
        let numeric = ["INT", "DOUBLE", "DECIMAL", "FLOAT", "HUGEINT"].iter().any(|t| field.ty.to_uppercase().contains(t));
        let text = if numeric { format!("CAST(CAST({} AS BIGINT) AS VARCHAR)", ident(geoid_field)) } else { format!("trim(CAST({} AS VARCHAR))", ident(geoid_field)) };
        // The GEOID values decide the level (by length, after restoring leading zeros) and the states to ask for.
        let lens: Vec<(i64, i64)> = self.with_conn(|c| {
            let sql = format!("SELECT length({text}) AS n, count(*) FROM {} WHERE {} IS NOT NULL{} GROUP BY 1 ORDER BY 2 DESC", ident(&info.table), ident(geoid_field), info.def_and());
            let mut st = c.prepare(&sql)?;
            let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(r)
        })?;
        if lens.is_empty() {
            bail!("'{geoid_field}' has no values to join on");
        }
        let level = match spec.level {
            Some(l) => l,
            None => {
                let (n, _) = lens[0];
                // A number lost its leading zero: 1001 is county 01001 (5 digits), 4 or 10 or 11 digits mean a restored zero.
                Level::from_len(n as usize).or_else(|| Level::from_len(n as usize + 1)).ok_or_else(|| {
                    anyhow!("Cannot tell the geography from '{geoid_field}' values {n} characters long. A Census GEOID has 2 (state), 5 (county), 11 (tract) or 12 (block group) digits; choose the level.")
                })?
            }
        };
        let len = level.geoid_len();
        let padded = format!("lpad({text}, {len}, '0')");
        let states: Vec<String> = self.with_conn(|c| {
            let sql = format!("SELECT DISTINCT substr({padded}, 1, 2) FROM {} WHERE {} IS NOT NULL{}", ident(&info.table), ident(geoid_field), info.def_and());
            let mut st = c.prepare(&sql)?;
            let r = st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<BTreeSet<_>, _>>()?;
            Ok(r.into_iter().collect())
        })?;

        let task = Task::start(format!("Fetching Census data for {}", info.name)).cancellable();
        let mut rows: Vec<(String, Vec<Option<f64>>)> = Vec::new();
        let mut suppressed = 0;
        let requests: Vec<Option<&str>> = if level == Level::State { vec![None] } else { states.iter().map(|s| Some(s.as_str())).collect() };
        for (i, st) in requests.iter().enumerate() {
            task.check()?;
            task.set(0.8 * i as f64 / requests.len() as f64, Some(format!("{} {} of {}", d.title, i + 1, requests.len())));
            let (r, miss) = fetch_level(base, d, spec.vintage, level, *st, &vars, key)?;
            suppressed += miss;
            rows.extend(r);
        }
        task.set(0.85, Some("Joining…".to_string()));
        // The Census table as a VALUES list.
        let values: Vec<String> = rows
            .iter()
            .map(|(g, v)| format!("({}, {})", lit(g), v.iter().map(|x| x.map_or("NULL::DOUBLE".to_string(), |n| format!("{n}::DOUBLE"))).collect::<Vec<_>>().join(", ")))
            .collect();
        let cols = vars.iter().map(|v| ident(v)).collect::<Vec<_>>().join(", ");
        let existing: Vec<&str> = vars.iter().filter(|v| info.fields.iter().any(|f| f.name.eq_ignore_ascii_case(v))).map(|s| s.as_str()).collect();
        if !existing.is_empty() {
            bail!("'{}' already has a field called {}. Rename or remove it first, so the join cannot overwrite your data.", info.name, existing.join(", "));
        }
        let hidden = if info.has_geometry() { "__fid, __g, __x0, __y0, __sz" } else { "__fid" };
        // The key of a feature, restored to its full length, and the Census table as a VALUES list.
        let key_of = format!("lpad({}, {len}, '0')", if numeric { format!("CAST(CAST(s.{} AS BIGINT) AS VARCHAR)", ident(geoid_field)) } else { format!("trim(CAST(s.{} AS VARCHAR))", ident(geoid_field)) });
        let table = if values.is_empty() { format!("(VALUES (NULL, {})) t(__geoid, {cols})", vars.iter().map(|_| "NULL::DOUBLE").collect::<Vec<_>>().join(", ")) } else { format!("(VALUES {}) t(__geoid, {cols})", values.join(", ")) };
        let select = format!(
            "SELECT s.* EXCLUDE ({hidden}), {} FROM {} s LEFT JOIN {table} ON t.__geoid = {key_of} WHERE TRUE{}",
            vars.iter().map(|v| format!("t.{}", ident(v))).collect::<Vec<_>>().join(", "),
            ident(&info.table),
            info.def_and()
        );
        let matched: i64 = self.with_conn(|c| Ok(c.query_row(&format!("SELECT count(*) FROM {} s JOIN {table} ON t.__geoid = {key_of} WHERE TRUE{}", ident(&info.table), info.def_and()), [], |r| r.get(0))?))?;
        let src = crate::layers::ImportSource { name: format!("{} + Census {}", info.name, d.id.to_uppercase()), select, crs: info.crs.clone(), crs_name: info.crs_name.clone(), warnings: vec![] };
        let from: Vec<String> = std::iter::once(info.id.clone()).chain(info.result_of.clone()).collect();
        let cite = citation(d, spec.vintage, &vars);
        let layer_info = self.with_conn(|conn| {
            let id = self.alloc_id();
            let mut li = crate::layers::build_layer(conn, &id, &src)?;
            li.source = info.source.clone();
            self.save_meta(conn, &li)?;
            let refs: Vec<&str> = from.iter().map(|s| s.as_str()).collect();
            sources::inherit(conn, &refs, &li.id, &li.name)?;
            sources::record(conn, &cite, &li.id, &li.name)?;
            Ok(li)
        })?;
        let total = layer_info.feature_count;
        let mut notes = vec![format!("{} {} by GEOID ({} level, {} state{}).", d.title, spec.vintage, level.label(), states.len().max(1), if states.len() <= 1 { "" } else { "s" })];
        if suppressed > 0 {
            notes.push(format!("{suppressed} value{} the Census Bureau left empty (suppressed or not available) {} stored as no value.", if suppressed == 1 { "" } else { "s" }, if suppressed == 1 { "is" } else { "are" }));
        }
        if key.is_none_or(|k| k.trim().is_empty()) {
            notes.push("No API key was used; the Census Bureau limits requests without a key.".into());
        }
        let unmatched = total - matched.min(total);
        if unmatched > 0 {
            notes.push(format!("{unmatched} feature{} found no row in the Census table (a GEOID from another vintage, or suppressed values).", if unmatched == 1 { "" } else { "s" }));
        }
        Ok(JoinResult { layer: layer_info, matched, unmatched, suppressed, notes })
    }
}
