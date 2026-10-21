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
    /// Create or replace the layer owned by `node_id`.
    fn upsert_layer(&self, conn: &Connection, node_id: &str, name: &str, sql: &str, crs: Option<String>) -> Result<LayerInfo>;
}

#[derive(Default)]
pub struct Cache {
    /// hash -> materialized output
    entries: HashMap<String, Rel>,
}

impl Cache {
    pub fn clear(&mut self, conn: &Connection) {
        for rel in self.entries.values() {
            let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {0}; DROP VIEW IF EXISTS {0};", ident(&rel.table)));
        }
        self.entries.clear();
    }
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
                        if port.kind == crate::nodes::PortKind::Vector && !rel.has_geom {
                            problem = Some((Status::Error, format!("'{}' needs features with geometry, but got a table", port.label)));
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
            if let Some(rel) = cache.entries.get(&hash) {
                let mut r = NodeResult::new(&id, Status::Cached);
                r.output = Some(rel.table.clone());
                r.crs = rel.crs.clone();
                r.rows = count(conn, &rel.table).ok();
                outputs.insert(id.clone(), (hash, rel.clone()));
                finish(r, on_status, &mut results);
                continue;
            }
        }

        on_status(&NodeResult::new(&id, Status::Running));
        let ctx = Ctx::new(conn, &layers);
        let built = tool.build(&ctx, &inputs, &Params(&node.params));
        let res: Result<NodeResult> = (|| {
            let mut r = NodeResult::new(&id, Status::Done);
            match built? {
                Output::Select { sql, crs, view } => {
                    let table = format!("__node_{hash}");
                    let kind = if view { "VIEW" } else { "TABLE" };
                    conn.execute_batch(&format!("CREATE OR REPLACE {kind} {} AS {sql}", ident(&table)))?;
                    let has_geom = has_geom_column(conn, &table)?;
                    let rel = Rel { table: table.clone(), crs: crs.clone(), has_geom };
                    cache.entries.insert(hash.clone(), rel.clone());
                    outputs.insert(id.clone(), (hash.clone(), rel));
                    r.rows = count(conn, &table).ok();
                    r.output = Some(table);
                    r.crs = crs;
                }
                Output::Layer { name, sql, crs } => {
                    let info = host.upsert_layer(conn, &id, &name, &sql, crs)?;
                    r.rows = Some(info.feature_count);
                    r.message = Some(format!("Added '{}' to the map", info.name));
                    r.layer = Some(info);
                }
                Output::Done(msg) => r.message = Some(msg),
            }
            r.reprojected = ctx.was_reprojected();
            Ok(r)
        })();
        match res {
            Ok(r) => finish(r, on_status, &mut results),
            Err(e) => {
                let mut r = NodeResult::new(&id, Status::Error);
                r.error = Some(clean_error(&e.to_string()));
                failed.insert(id.clone());
                finish(r, on_status, &mut results);
            }
        }
    }
    Ok(results)
}

fn count(conn: &Connection, table: &str) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT count(*) FROM {}", ident(table)), [], |r| r.get(0))?)
}

fn has_geom_column(conn: &Connection, table: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("DESCRIBE SELECT * FROM {}", ident(table)))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (name, ty) = row?;
        if name == "geom" && ty.to_uppercase().starts_with("GEOMETRY") {
            return Ok(true);
        }
    }
    Ok(false)
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
