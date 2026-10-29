//! Model (node graph) executor.
//!
//! Nodes run in topological order. Each tool's output is materialized into a
//! cache table named by a hash of (tool, params, input hashes), so editing one
//! node only re-runs that node and the nodes downstream of it.

use crate::db::ident;
use crate::layers::LayerInfo;
use crate::nodes::{Ctx, Inputs, Output, Params, Rel, Role, Tool};
use anyhow::{anyhow, bail, Result};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

#[derive(Deserialize, Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<GNode>,
    pub edges: Vec<GEdge>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GNode {
    pub id: String,
    pub tool: String,
    #[serde(default)]
    pub params: Map<String, Value>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GEdge {
    pub source: String,
    pub target: String,
    pub target_handle: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Done,
    Cached,
    Error,
    Skipped,
    /// The user cancelled the run before this step finished.
    Cancelled,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeResult {
    pub id: String,
    pub status: Status,
    pub error: Option<String>,
    pub rows: Option<i64>,
    pub crs: Option<String>,
    /// Cache table holding this node's output (for previews).
    pub output: Option<String>,
    pub message: Option<String>,
    pub layer: Option<LayerInfo>,
    pub reprojected: bool,
    pub millis: u64,
}

impl NodeResult {
    fn new(id: &str, status: Status) -> Self {
        Self { id: id.into(), status, error: None, rows: None, crs: None, output: None, message: None, layer: None, reprojected: false, millis: 0 }
    }
}

/// What the executor needs from the engine besides SQL.
pub trait Host {
    fn layers(&self) -> HashMap<String, LayerInfo>;
    // `inputs`: every layer feeding the step (for provenance/citations).
    /// Create or replace the layer owned by `node_id`.
    fn upsert_layer(&self, conn: &Connection, node_id: &str, name: &str, sql: &str, crs: Option<String>, inputs: &[String]) -> Result<LayerInfo>;
    /// Create or replace the node-tree result of layer `owner`.
    fn set_result(&self, conn: &Connection, owner: &str, sql: &str, crs: Option<String>, inputs: &[String]) -> Result<LayerInfo>;
    /// Create or replace a raster node-tree result of layer `owner`.
    fn set_result_raster(&self, owner: &str, path: &str, inputs: &[String]) -> Result<LayerInfo>;
}

/// Bump whenever a tool's SQL changes meaning: persisted snapshots from an
/// older format are dropped when a project opens.
pub const CACHE_FORMAT: u32 = 1;

fn engine_version() -> String {
    format!("{}+{}", env!("CARGO_PKG_VERSION"), CACHE_FORMAT)
}

/// Default budget for persisted node snapshots (2 GB per project).
pub const DEFAULT_CACHE_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

struct Meta {
    table: String,
    bytes: u64,
    last_used: i64,
}

pub struct Cache {
    /// hash -> materialized output
    entries: HashMap<String, Rel>,
    /// hash -> index data for persisted (non-raster) entries
    meta: HashMap<String, Meta>,
    /// Tables the UI said were live at the last prune.
    live: Option<HashSet<String>>,
    pub budget: u64,
    tick: i64,
}

impl Default for Cache {
    fn default() -> Self {
        Self { entries: HashMap::new(), meta: HashMap::new(), live: None, budget: DEFAULT_CACHE_BUDGET, tick: 0 }
    }
}

fn drop_table(conn: &Connection, table: &str) {
    // DuckDB refuses DROP TABLE on a view (and vice versa), so try both.
    let _ = conn.execute_batch(&format!("DROP VIEW IF EXISTS {}", ident(table)));
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", ident(table)));
}

impl Cache {
    pub fn clear(&mut self, conn: &Connection) {
        for rel in self.entries.values().filter(|r| r.raster.is_none()) {
            drop_table(conn, &rel.table);
        }
        let _ = conn.execute_batch("DELETE FROM __node_cache");
        self.entries.clear();
        self.meta.clear();
    }

    /// Load the persisted index when a project opens. Drops entries from
    /// another engine version, entries whose table or view is gone or broken,
    /// and orphan `__node_` tables that have no index row.
    pub fn load(&mut self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS __node_cache (
               hash VARCHAR PRIMARY KEY, table_name VARCHAR, kind VARCHAR, crs VARCHAR, has_geom BOOLEAN,
               rows BIGINT, bytes BIGINT, engine_version VARCHAR, created TIMESTAMP, last_used TIMESTAMP)",
        )?;
        *self = Cache { budget: self.budget, ..Cache::default() };
        let mut stmt = conn.prepare("SELECT table_name, table_type FROM information_schema.tables WHERE table_name LIKE '\\_\\_node\\_%' ESCAPE '\\' AND table_name <> '__node_cache'")?;
        let present: HashMap<String, String> = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<_, _>>()?;
        let mut stmt = conn.prepare("SELECT hash, table_name, kind, crs, has_geom, bytes, engine_version FROM __node_cache ORDER BY last_used, created")?;
        type Row = (String, String, String, Option<String>, bool, i64, String);
        let rows: Vec<Row> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<bool>>(4)?.unwrap_or(false), r.get::<_, Option<i64>>(5)?.unwrap_or(0), r.get(6)?)))?
            .collect::<Result<_, _>>()?;
        let version = engine_version();
        let mut keep = HashSet::new();
        for (hash, table, kind, crs, has_geom, bytes, ver) in rows {
            let is_view = kind == "view";
            let usable = ver == version && present.contains_key(&table) && (!is_view || conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(&table))).is_ok());
            if !usable {
                drop_table(conn, &table);
                conn.execute("DELETE FROM __node_cache WHERE hash = ?", [&hash])?;
                continue;
            }
            keep.insert(table.clone());
            self.tick += 1;
            self.meta.insert(hash.clone(), Meta { table: table.clone(), bytes: bytes.max(0) as u64, last_used: self.tick });
            self.entries.insert(hash, Rel { table, crs, has_geom, raster: None });
        }
        for t in present.keys().filter(|t| !keep.contains(*t)) {
            drop_table(conn, t);
        }
        Ok(())
    }

    fn touch(&mut self, conn: &Connection, hash: &str) {
        if let Some(m) = self.meta.get_mut(hash) {
            self.tick += 1;
            m.last_used = self.tick;
            let _ = conn.execute("UPDATE __node_cache SET last_used = now() WHERE hash = ?", [hash]);
        }
    }

    fn evict(&mut self, conn: &Connection, hash: &str) {
        if let Some(m) = self.meta.remove(hash) {
            drop_table(conn, &m.table);
            let _ = conn.execute("DELETE FROM __node_cache WHERE hash = ?", [hash]);
        }
        self.entries.remove(hash);
    }

    /// Evict until the total fits the budget: entries outside the last live
    /// list first, then least recently used. `protect` and `pinned` stay.
    fn enforce_budget(&mut self, conn: &Connection, protect: &HashSet<String>, pinned: Option<&str>) -> usize {
        let mut total: u64 = self.meta.values().map(|m| m.bytes).sum();
        if total <= self.budget {
            return 0;
        }
        let mut order: Vec<(bool, i64, String)> = self
            .meta
            .iter()
            .filter(|(h, m)| m.bytes > 0 && !protect.contains(*h) && Some(m.table.as_str()) != pinned)
            .map(|(h, m)| (self.live.as_ref().map_or(false, |l| l.contains(&m.table)), m.last_used, h.clone()))
            .collect();
        order.sort();
        let mut dropped = 0;
        for (_, _, h) in order {
            if total <= self.budget {
                break;
            }
            total -= self.meta[&h].bytes;
            self.evict(conn, &h);
            dropped += 1;
        }
        dropped
    }

    /// Keep only snapshots the UI still shows (`live` = output table names),
    /// then apply the size budget. Returns how many were dropped.
    pub fn prune(&mut self, conn: &Connection, live: &[String], pinned: Option<&str>) -> usize {
        let live: HashSet<String> = live.iter().cloned().collect();
        let stale: Vec<String> = self.meta.iter().filter(|(_, m)| !live.contains(&m.table) && Some(m.table.as_str()) != pinned).map(|(h, _)| h.clone()).collect();
        let mut dropped = stale.len();
        for h in stale {
            self.evict(conn, &h);
        }
        self.live = Some(live);
        dropped += self.enforce_budget(conn, &HashSet::new(), pinned);
        dropped
    }
}

/// Rough size of a snapshot: rows × (8 bytes per plain column + average WKB
/// size), from a 1,000-row sample. Views hold no data.
fn estimate_bytes(conn: &Connection, table: &str, rows: i64, is_view: bool) -> u64 {
    if is_view || rows <= 0 {
        return 0;
    }
    let Ok(cols) = describe(conn, table) else { return 0 };
    let plain = cols.iter().filter(|(n, t)| !(n == "geom" && t.to_uppercase().starts_with("GEOMETRY"))).count() as f64;
    let geom = if cols.len() as f64 > plain {
        conn.query_row(&format!("SELECT coalesce(avg(octet_length(ST_AsWKB(geom))), 0) FROM (SELECT geom FROM {} LIMIT 1000)", ident(table)), [], |r| r.get::<_, f64>(0)).unwrap_or(0.0)
    } else {
        0.0
    };
    (rows as f64 * (8.0 * plain + geom)) as u64
}

/// Order nodes so each runs after its inputs. If `target` is set, only the
/// target and its upstream nodes are included.
pub fn plan(graph: &Graph, target: Option<&str>) -> Result<Vec<String>> {
    let ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut include: HashSet<String> = ids.iter().map(|s| s.to_string()).collect();
    if let Some(t) = target {
        if !ids.contains(t) {
            bail!("Unknown node {t}");
        }
        include.clear();
        let mut stack = vec![t.to_string()];
        while let Some(n) = stack.pop() {
            if include.insert(n.clone()) {
                for e in graph.edges.iter().filter(|e| e.target == n) {
                    stack.push(e.source.clone());
                }
            }
        }
    }
    let mut indeg: HashMap<&str, usize> = HashMap::new();
    for n in &graph.nodes {
        if include.contains(&n.id) {
            indeg.insert(&n.id, 0);
        }
    }
    for e in &graph.edges {
        if include.contains(&e.source) && include.contains(&e.target) {
            *indeg.get_mut(e.target.as_str()).ok_or_else(|| anyhow!("Edge to unknown node"))? += 1;
        }
    }
    // Stable order: follow the node list order for ties.
    let mut queue: VecDeque<&str> = graph.nodes.iter().map(|n| n.id.as_str()).filter(|id| indeg.get(id) == Some(&0)).collect();
    let mut out = Vec::new();
    while let Some(n) = queue.pop_front() {
        out.push(n.to_string());
        for e in graph.edges.iter().filter(|e| e.source == n && include.contains(&e.target)) {
            let d = indeg.get_mut(e.target.as_str()).unwrap();
            *d -= 1;
            if *d == 0 {
                queue.push_back(&e.target);
            }
        }
    }
    if out.len() != indeg.len() {
        bail!("The model has a cycle. Remove a connection so data flows one way.");
    }
    Ok(out)
}

pub fn run(
    conn: &Connection,
    tools: &[Box<dyn Tool>],
    host: &dyn Host,
    cache: &mut Cache,
    graph: &Graph,
    target: Option<&str>,
    on_status: &mut dyn FnMut(&NodeResult),
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<NodeResult>> {
    let order = plan(graph, target)?;
    let layers = host.layers();
    let tool_by_id: HashMap<&str, &dyn Tool> = tools.iter().map(|t| (t.spec().id, t.as_ref())).collect();
    let node_by_id: HashMap<&str, &GNode> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut outputs: HashMap<String, (String, Rel)> = HashMap::new(); // node -> (hash, rel)
    let mut failed: HashSet<String> = HashSet::new();
    let mut results = Vec::new();

    for id in order {
        let node = node_by_id[id.as_str()];
        let started = Instant::now();
        // Cancelled: this step and everything after it don't run. Nothing is
        // half-written: outputs only enter the cache (or replace a layer's
        // result) after their SQL has finished.
        if cancelled() {
            let mut r = NodeResult::new(&id, Status::Cancelled);
            r.error = Some("Cancelled".into());
            failed.insert(id.clone());
            on_status(&r);
            results.push(r);
            continue;
        }
        let finish = |mut r: NodeResult, on_status: &mut dyn FnMut(&NodeResult), results: &mut Vec<NodeResult>| {
            r.millis = started.elapsed().as_millis() as u64;
            on_status(&r);
            results.push(r);
        };

        let Some(tool) = tool_by_id.get(node.tool.as_str()) else {
            let mut r = NodeResult::new(&id, Status::Error);
            r.error = Some(format!("Unknown tool '{}'", node.tool));
            failed.insert(id.clone());
            finish(r, on_status, &mut results);
            continue;
        };
        let spec = tool.spec();

        // Gather inputs.
        let mut inputs: Inputs = HashMap::new();
        let mut input_hashes = Vec::new();
        let mut problem: Option<(Status, String)> = None;
        for port in &spec.inputs {
            match graph.edges.iter().find(|e| e.target == id && e.target_handle == port.id) {
                None => problem = Some((Status::Error, format!("Connect an input to '{}'", port.label))),
                Some(e) if failed.contains(&e.source) => problem = Some((Status::Skipped, "An upstream step failed".into())),
                Some(e) => match outputs.get(&e.source) {
                    Some((h, rel)) => {
                        use crate::nodes::PortKind;
                        match port.kind {
                            PortKind::Vector if rel.raster.is_some() => problem = Some((Status::Error, format!("'{}' needs features, but got a raster", port.label))),
                            PortKind::Vector if !rel.has_geom => problem = Some((Status::Error, format!("'{}' needs features with geometry, but got a table", port.label))),
                            PortKind::Table if rel.raster.is_some() => problem = Some((Status::Error, format!("'{}' needs a table or features, but got a raster", port.label))),
                            PortKind::Raster if rel.raster.is_none() => problem = Some((Status::Error, format!("'{}' needs a raster", port.label))),
                            _ => {}
                        }
                        input_hashes.push(format!("{}={h}", port.id));
                        inputs.insert(port.id, rel.clone());
                    }
                    None => problem = Some((Status::Skipped, "Upstream step has no output".into())),
                },
            }
        }
        if let Some((status, msg)) = problem {
            let mut r = NodeResult::new(&id, status);
            r.error = Some(msg);
            failed.insert(id.clone());
            finish(r, on_status, &mut results);
            continue;
        }

        let mut hasher = Sha256::new();
        hasher.update(node.tool.as_bytes());
        hasher.update(serde_json::to_string(&node.params)?.as_bytes());
        // Strict datum mode can refuse a step that Warn mode ran, so the two must not share cached results (CRS accuracy plan A3).
        hasher.update([crate::transform_report::is_strict() as u8]);
        for h in &input_hashes {
            hasher.update(h.as_bytes());
        }
        if spec.id == "input_layer" {
            if let Some(info) = node.params.get("layer").and_then(|v| v.as_str()).and_then(|l| layers.get(l)) {
                hasher.update(format!("{}:{}:{:?}", info.id, info.version, info.crs).as_bytes());
            }
        }
        let hash = hex::encode(&hasher.finalize()[..8]);

        if spec.role != Role::Output {
            if let Some(rel) = cache.entries.get(&hash).cloned() {
                cache.touch(conn, &hash);
                let mut r = NodeResult::new(&id, Status::Cached);
                r.output = Some(rel.raster.clone().unwrap_or_else(|| rel.table.clone()));
                r.crs = rel.crs.clone();
                r.rows = if rel.raster.is_some() { None } else { count(conn, &rel.table).ok() };
                outputs.insert(id.clone(), (hash, rel));
                finish(r, on_status, &mut results);
                continue;
            }
        }

        on_status(&NodeResult::new(&id, Status::Running));
        let ctx = Ctx::new(conn, &layers);
        let built = crate::nodes::validate_params(&spec, &node.params).and_then(|_| tool.build(&ctx, &inputs, &Params(&node.params)));
        let res: Result<NodeResult> = (|| {
            let mut r = NodeResult::new(&id, Status::Done);
            match built? {
                built @ (Output::Select { .. } | Output::Script { .. }) => {
                    let table = format!("__node_{hash}");
                    let (statements, crs, view) = match built {
                        Output::Select { sql, crs, view } => {
                            let kind = if view { "VIEW" } else { "TABLE" };
                            (vec![format!("CREATE OR REPLACE {kind} {} AS {sql}", ident(&table))], crs, view)
                        }
                        Output::Script { statements, crs } => (statements.iter().map(|s| s.replace("{out}", &table)).collect(), crs, false),
                        _ => unreachable!(),
                    };
                    // The table and its index row commit together, so a
                    // cancelled step leaves neither.
                    let (has_geom, rows, bytes) = crate::atomically(conn, || {
                        for (n, stmt) in statements.iter().enumerate() {
                            if n > 0 && cancelled() {
                                bail!("Cancelled");
                            }
                            conn.execute_batch(stmt)?;
                        }
                        let has_geom = has_geom_column(conn, &table)?;
                        let rows = count(conn, &table)?;
                        let bytes = estimate_bytes(conn, &table, rows, view);
                        conn.execute("DELETE FROM __node_cache WHERE hash = ?", [&hash])?;
                        conn.execute(
                            "INSERT INTO __node_cache VALUES (?, ?, ?, ?, ?, ?, ?, ?, now(), now())",
                            duckdb::params![hash, table, if view { "view" } else { "table" }, crs, has_geom, rows, bytes as i64, engine_version()],
                        )?;
                        Ok((has_geom, rows, bytes))
                    })?;
                    let rel = Rel { table: table.clone(), crs: crs.clone(), has_geom, raster: None };
                    cache.tick += 1;
                    cache.meta.insert(hash.clone(), Meta { table: table.clone(), bytes, last_used: cache.tick });
                    cache.entries.insert(hash.clone(), rel.clone());
                    outputs.insert(id.clone(), (hash.clone(), rel));
                    r.rows = Some(rows);
                    r.output = Some(table);
                    r.crs = crs;
                }
                Output::Layer { name, sql, crs } => {
                    let info = host.upsert_layer(conn, &id, &name, &sql, crs, &upstream_layers(graph, &id))?;
                    r.rows = Some(info.feature_count);
                    r.message = Some(format!("Added '{}' to the map", info.name));
                    r.layer = Some(info);
                }
                Output::Result { owner, sql, crs } => {
                    let info = host.set_result(conn, &owner, &sql, crs, &upstream_layers(graph, &id))?;
                    r.rows = Some(info.feature_count);
                    r.message = Some(format!("{} now shows this result", info.name));
                    r.layer = Some(info);
                }
                Output::Raster { path, crs } => {
                    let rel = Rel { table: String::new(), crs: crs.clone(), has_geom: false, raster: Some(path.clone()) };
                    cache.entries.insert(hash.clone(), rel.clone());
                    outputs.insert(id.clone(), (hash.clone(), rel));
                    r.output = Some(path);
                    r.crs = crs;
                }
                Output::ResultRaster { owner, path } => {
                    let info = host.set_result_raster(&owner, &path, &upstream_layers(graph, &id))?;
                    r.message = Some(format!("{} now shows this raster", info.name));
                    r.layer = Some(info);
                }
                Output::Done(msg) => r.message = Some(msg),
            }
            r.reprojected = ctx.was_reprojected();
            let notes = ctx.datum_notes();
            if !notes.is_empty() {
                let n = notes.join(" ");
                r.message = Some(match r.message.take() {
                    Some(m) => format!("{m} {n}"),
                    None => n,
                });
            }
            Ok(r)
        })();
        match res {
            Ok(r) => finish(r, on_status, &mut results),
            Err(e) => {
                // A query stopped by Cancel fails with DuckDB's "Interrupted" error.
                let stopped = cancelled();
                let mut r = NodeResult::new(&id, if stopped { Status::Cancelled } else { Status::Error });
                r.error = Some(if stopped { "Cancelled".into() } else { clean_error(&e.to_string()) });
                failed.insert(id.clone());
                finish(r, on_status, &mut results);
            }
        }
    }
    // Keep the project within its snapshot budget (0 = decide on save only).
    if cache.budget > 0 {
        let protect: HashSet<String> = outputs.values().map(|(h, _)| h.clone()).collect();
        cache.enforce_budget(conn, &protect, None);
    }
    Ok(results)
}

/// Layers read by Layer Input nodes upstream of `node` (each once, in order).
pub fn upstream_layers(graph: &Graph, node: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![node.to_string()];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        if let Some(gn) = graph.nodes.iter().find(|x| x.id == n) {
            if gn.tool == "input_layer" {
                if let Some(l) = gn.params.get("layer").and_then(|v| v.as_str()) {
                    if !out.iter().any(|x| x == l) {
                        out.push(l.to_string());
                    }
                }
            }
        }
        stack.extend(graph.edges.iter().filter(|e| e.target == n).map(|e| e.source.clone()));
    }
    out
}

fn count(conn: &Connection, table: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT count(*) FROM {}", ident(table)), [], |r| r.get(0))?)
}

fn describe(conn: &Connection, table: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(table)))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn has_geom_column(conn: &Connection, table: &str) -> Result<bool> {
    Ok(describe(conn, table)?.iter().any(|(name, ty)| name == "geom" && ty.to_uppercase().starts_with("GEOMETRY")))
}

/// Strip internal table names from DuckDB errors so messages read well.
fn clean_error(e: &str) -> String {
    let e = e.split("\n\nLINE").next().unwrap_or(e);
    let mut out = String::new();
    let mut rest = e;
    while let Some(i) = rest.find("__node_") {
        out.push_str(&rest[..i]);
        out.push_str("<input>");
        rest = &rest[i + 7..];
        rest = rest.trim_start_matches(|c: char| c.is_ascii_hexdigit());
    }
    out.push_str(rest);
    out
}
