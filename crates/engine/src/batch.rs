//! Batch run (UX5): one node tree run over many inputs (a folder of files, or a list of project layers), one result file per input,
//! and a report saying what happened to each.
//!
//! Rules (MASTER_PLAN Phase UX): Output never modifies source data, so results are written only as NEW files in the chosen output
//! folder and an existing file is never replaced unless the user said so; one bad input never stops the rest; Cancel stops between
//! inputs and interrupts the one running; every failure is a readable sentence.
//!
//! Each input runs the tree with the chosen Layer Input node pointed at that input, up to the chosen result node, then a temporary
//! Export Features step writes the result (the same tool as the Export node, so the coordinate system is written and a source file is
//! refused). Inputs that were read from files are removed from the project afterwards; the files are untouched.

use crate::graph::{Graph, GEdge, GNode, Status};
use crate::layers::LayerInfo;
use crate::progress::Task;
use crate::Engine;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Extensions the folder scan offers as inputs (the same family as Add Data, without sidecar and archive types).
const DATA_EXTENSIONS: &[&str] = &["shp", "gpkg", "geojson", "json", "fgb", "gml", "kml", "parquet", "csv", "xlsx", "ods", "tif", "tiff", "img", "asc", "vrt"];

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BatchSource {
    /// A data file; every layer in it is one input.
    File { path: String },
    /// A layer already in the project (its original data).
    Layer { id: String },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub graph: Graph,
    /// The Layer Input node that receives each input.
    pub input_node: String,
    /// The node whose result is written (usually what feeds the tree's Output).
    pub result_node: String,
    pub sources: Vec<BatchSource>,
    pub out_dir: String,
    /// An Export Features format id ("GPKG", "ESRI Shapefile", "GeoJSON", "FlatGeobuf", "PARQUET", "CSV"). Raster results are always GeoTIFF.
    #[serde(default = "gpkg")]
    pub format: String,
    /// Write over files that already exist in the output folder. Off by default.
    #[serde(default)]
    pub overwrite: bool,
    /// Add each written result to the project as a layer.
    #[serde(default)]
    pub add_to_project: bool,
}

fn gpkg() -> String {
    "GPKG".into()
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BatchStatus {
    Done,
    Failed,
    /// Left alone on purpose (the output file already exists).
    Skipped,
    Cancelled,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem {
    /// What was processed: the layer or file name.
    pub name: String,
    /// The file the input came from, or the project layer id.
    pub source: String,
    pub status: BatchStatus,
    /// What went wrong, or notes from the run (reprojected, features dropped ...).
    pub message: Option<String>,
    pub rows: Option<i64>,
    /// The file written.
    pub output: Option<String>,
    pub layer: Option<LayerInfo>,
    pub millis: u64,
}

impl BatchItem {
    fn new(name: &str, source: &str, status: BatchStatus, message: Option<String>) -> Self {
        Self { name: name.into(), source: source.into(), status, message, rows: None, output: None, layer: None, millis: 0 }
    }
}

/// Data files in `folder` (not its sub-folders), sorted by name. Hidden files and sidecar files (.dbf, .shx, .prj next to a .shp) are left out.
pub fn list_folder(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.is_dir() {
        bail!("{} is not a folder.", folder.display());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.retain(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')) && DATA_EXTENSIONS.contains(&ext(p).as_str()));
    files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    Ok(files)
}

fn ext(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn format_ext(fmt: &str) -> &'static str {
    match fmt {
        "ESRI Shapefile" => "shp",
        "GeoJSON" => "geojson",
        "FlatGeobuf" => "fgb",
        "PARQUET" => "parquet",
        "CSV" => "csv",
        _ => "gpkg",
    }
}

/// A file name stem that is safe on every system.
pub fn safe_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') { c } else { '_' }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "result".into() } else { s.chars().take(100).collect() }
}

/// The first free name in `used` for `stem` + `.ext` (a second input with the same name becomes `name_2`).
pub fn unique_name(used: &mut HashSet<String>, stem: &str, ext: &str) -> String {
    for i in 1.. {
        let name = if i == 1 { format!("{stem}.{ext}") } else { format!("{stem}_{i}.{ext}") };
        if used.insert(name.to_lowercase()) {
            return name;
        }
    }
    unreachable!()
}

/// Check the request before anything runs, with sentences the user can act on.
pub fn check_request(req: &BatchRequest) -> Result<()> {
    let tool_of = |id: &str| req.graph.nodes.iter().find(|n| n.id == id).map(|n| n.tool.as_str());
    match tool_of(&req.input_node) {
        Some("input_layer") => {}
        Some(_) => bail!("The input step must be a Layer Input node."),
        None => bail!("The input step is not in this node tree."),
    }
    match tool_of(&req.result_node) {
        None => bail!("The step to write results from is not in this node tree."),
        Some("layer_output") | Some("export") => bail!("Pick the step whose result you want written, not an Output step."),
        Some(_) => {}
    }
    if req.sources.is_empty() {
        bail!("Add at least one file or layer to run over.");
    }
    if req.out_dir.trim().is_empty() {
        bail!("Choose a folder for the results.");
    }
    Ok(())
}

impl Engine {
    /// Run the tree over every input. Never fails for one bad input: the report has a row per input.
    pub fn run_batch(&self, req: &BatchRequest) -> Result<Vec<BatchItem>> {
        check_request(req)?;
        let out_dir = Path::new(&req.out_dir);
        std::fs::create_dir_all(out_dir).map_err(|e| anyhow!("Could not make the results folder {}: {e}", out_dir.display()))?;
        let task = Task::start(format!("Batch run — {}", out_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| req.out_dir.clone()))).cancellable();
        let handle = task.handle();
        let total = req.sources.len().max(1);
        let mut items: Vec<BatchItem> = Vec::new();
        let mut used: HashSet<String> = HashSet::new();
        let mut cancelled = false;
        for (si, src) in req.sources.iter().enumerate() {
            if cancelled || handle.check().is_err() {
                cancelled = true;
                items.push(BatchItem::new(&source_name(src), &source_ref(src), BatchStatus::Cancelled, Some("Cancelled before this input ran.".into())));
                continue;
            }
            task.set(si as f64 / total as f64, Some(format!("Input {} of {total}: {}", si + 1, source_name(src))));
            // Reading a file may give several layers (a GeoPackage): each is one input.
            let (layers, imported): (Vec<LayerInfo>, bool) = match src {
                BatchSource::Layer { id } => match self.layer(id) {
                    Ok(l) => (vec![l], false),
                    Err(_) => {
                        items.push(BatchItem::new(id, id, BatchStatus::Failed, Some("This layer is not in the project any more.".into())));
                        continue;
                    }
                },
                BatchSource::File { path } => match self.import(Path::new(path)) {
                    Ok(l) if !l.is_empty() => (l, true),
                    Ok(_) => {
                        items.push(BatchItem::new(&source_name(src), path, BatchStatus::Failed, Some("No data was found in this file.".into())));
                        continue;
                    }
                    Err(e) => {
                        items.push(BatchItem::new(&source_name(src), path, BatchStatus::Failed, Some(format!("Could not read this file: {e:#}"))));
                        continue;
                    }
                },
            };
            for layer in layers {
                if handle.check().is_err() {
                    cancelled = true;
                    items.push(BatchItem::new(&layer.name, &source_ref(src), BatchStatus::Cancelled, Some("Cancelled before this input ran.".into())));
                } else {
                    let started = Instant::now();
                    let mut item = self.batch_one(req, &layer, src, out_dir, &mut used, handle);
                    item.millis = started.elapsed().as_millis() as u64;
                    if item.status == BatchStatus::Cancelled {
                        cancelled = true;
                    }
                    items.push(item);
                }
                if imported {
                    let _ = self.remove_layer(&layer.id);
                }
            }
        }
        task.set(1.0, Some("Done"));
        Ok(items)
    }

    fn batch_one(&self, req: &BatchRequest, layer: &LayerInfo, src: &BatchSource, out_dir: &Path, used: &mut HashSet<String>, parent: crate::progress::TaskHandle) -> BatchItem {
        let source = source_ref(src);
        let fail = |msg: String| BatchItem::new(&layer.name, &source, BatchStatus::Failed, Some(msg));
        // The tree with this input in place of the chosen Layer Input.
        let mut g = req.graph.clone();
        if let Some(n) = g.nodes.iter_mut().find(|n| n.id == req.input_node) {
            n.params.insert("layer".into(), json!(layer.id));
        }
        let results = match self.run_graph_in(&g, Some(&req.result_node), &mut |_| {}, Some(parent)) {
            Ok(r) => r,
            Err(e) => return fail(format!("{e:#}")),
        };
        if results.iter().any(|r| r.status == Status::Cancelled) || parent.check().is_err() {
            return BatchItem::new(&layer.name, &source, BatchStatus::Cancelled, Some("Cancelled while this input was running.".into()));
        }
        let Some(res) = results.iter().find(|r| r.id == req.result_node) else { return fail("The step did not run.".into()) };
        if !matches!(res.status, Status::Done | Status::Cached) {
            // Say which step went wrong: the first one that failed, not "an upstream step failed".
            let why = results.iter().find(|r| r.status == Status::Error).or(Some(res)).and_then(|r| r.error.clone()).unwrap_or_else(|| "The step did not finish.".into());
            return fail(why);
        }
        let raster = res.output.as_deref().is_some_and(|o| !o.starts_with("__node_"));
        let name = unique_name(used, &safe_stem(&layer.name), if raster { "tif" } else { format_ext(&req.format) });
        let dest = out_dir.join(&name);
        if dest.exists() && !req.overwrite {
            used.remove(&name.to_lowercase());
            let mut it = BatchItem::new(&layer.name, &source, BatchStatus::Skipped, Some(format!("{name} already exists in the results folder, so it was left as it is. Choose another folder or allow replacing.")));
            it.rows = res.rows;
            return it;
        }
        if dest.exists() {
            // Replacing was asked for, but never an input file, and a shapefile goes with its sidecars.
            let same = |a: &Path, b: &Path| matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y);
            if layer.source.as_deref().is_some_and(|s| same(Path::new(s), &dest)) {
                return fail(format!("{name} is the input file itself, and a result never replaces its input. Choose another results folder."));
            }
            for e in ["", "shx", "dbf", "prj", "cpg"] {
                let p = if e.is_empty() { dest.clone() } else { dest.with_extension(e) };
                if p.is_file() {
                    if let Err(err) = std::fs::remove_file(&p) {
                        return fail(format!("Could not replace {}: {err}", p.display()));
                    }
                }
            }
        }
        // Write it with the Export Features tool, as a step after the result step.
        const EXPORT: &str = "__batch_export";
        g.nodes.push(GNode { id: EXPORT.into(), tool: "export".into(), params: json!({"path": dest.display().to_string(), "format": req.format}).as_object().cloned().unwrap_or_default() });
        g.edges.push(GEdge { source: req.result_node.clone(), target: EXPORT.into(), target_handle: "in".into() });
        let wrote = match self.run_graph_in(&g, Some(EXPORT), &mut |_| {}, Some(parent)) {
            Ok(r) => r,
            Err(e) => return fail(format!("{e:#}")),
        };
        let Some(ex) = wrote.iter().find(|r| r.id == EXPORT) else { return fail("The result was not written.".into()) };
        if ex.status == Status::Cancelled {
            return BatchItem::new(&layer.name, &source, BatchStatus::Cancelled, Some("Cancelled while this input was running.".into()));
        }
        if ex.status != Status::Done {
            return fail(ex.error.clone().unwrap_or_else(|| "The result was not written.".into()));
        }
        let mut notes: Vec<String> = Vec::new();
        for r in results.iter().chain(wrote.iter()) {
            if let Some(m) = r.message.as_ref().filter(|m| !m.is_empty() && !m.starts_with("Wrote ")) {
                if !notes.contains(m) {
                    notes.push(m.clone());
                }
            }
        }
        let mut item = BatchItem::new(&layer.name, &source, BatchStatus::Done, (!notes.is_empty()).then(|| notes.join(" ")));
        item.rows = res.rows;
        item.output = Some(dest.display().to_string());
        if req.add_to_project {
            match self.import(&dest) {
                Ok(mut l) if !l.is_empty() => item.layer = Some(l.remove(0)),
                Ok(_) => {}
                Err(e) => item.message = Some(format!("Written, but could not be added to the project: {e:#}")),
            }
        }
        item
    }
}

fn source_name(s: &BatchSource) -> String {
    match s {
        BatchSource::File { path } => Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.clone()),
        BatchSource::Layer { id } => id.clone(),
    }
}

fn source_ref(s: &BatchSource) -> String {
    match s {
        BatchSource::File { path } => path.clone(),
        BatchSource::Layer { id } => id.clone(),
    }
}

/// Make a request from JSON (the dev server passes it as a value).
pub fn request_from(v: &Value) -> Result<BatchRequest> {
    serde_json::from_value(v.clone()).map_err(|e| anyhow!("The batch settings are not valid: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(input: &str, result: &str) -> BatchRequest {
        let graph: Graph = serde_json::from_value(json!({
            "nodes": [{"id": "in", "tool": "input_layer", "params": {}}, {"id": "b", "tool": "buffer", "params": {}}, {"id": "o", "tool": "layer_output", "params": {}}],
            "edges": [{"source": "in", "target": "b", "targetHandle": "in"}, {"source": "b", "target": "o", "targetHandle": "in"}]
        }))
        .unwrap();
        BatchRequest { graph, input_node: input.into(), result_node: result.into(), sources: vec![BatchSource::Layer { id: "L1".into() }], out_dir: "/tmp/x".into(), format: "GPKG".into(), overwrite: false, add_to_project: false }
    }

    #[test]
    fn the_request_is_checked_with_sentences() {
        assert!(check_request(&req("in", "b")).is_ok());
        assert!(check_request(&req("zz", "b")).unwrap_err().to_string().contains("not in this node tree"));
        assert!(check_request(&req("b", "b")).unwrap_err().to_string().contains("Layer Input"));
        assert!(check_request(&req("in", "o")).unwrap_err().to_string().contains("not an Output"));
        let mut r = req("in", "b");
        r.sources.clear();
        assert!(check_request(&r).unwrap_err().to_string().contains("at least one"));
        let mut r = req("in", "b");
        r.out_dir = " ".into();
        assert!(check_request(&r).unwrap_err().to_string().contains("folder"));
    }

    #[test]
    fn file_names_are_safe_and_unique() {
        assert_eq!(safe_stem("a/b:c*d"), "a_b_c_d");
        assert_eq!(safe_stem("../.."), "_");
        assert_eq!(safe_stem("..."), "result");
        let mut used = HashSet::new();
        assert_eq!(unique_name(&mut used, "roads", "gpkg"), "roads.gpkg");
        assert_eq!(unique_name(&mut used, "Roads", "gpkg"), "Roads_2.gpkg");
        assert_eq!(unique_name(&mut used, "roads", "shp"), "roads.shp");
    }

    #[test]
    fn the_folder_scan_lists_data_files_only() {
        let d = tempfile::tempdir().unwrap();
        for f in ["b.shp", "b.dbf", "b.shx", "b.prj", "a.gpkg", "notes.txt", ".hidden.gpkg", "c.TIF", "d.dbf"] {
            std::fs::write(d.path().join(f), b"x").unwrap();
        }
        std::fs::create_dir(d.path().join("sub")).unwrap();
        std::fs::write(d.path().join("sub/e.gpkg"), b"x").unwrap();
        let names: Vec<String> = list_folder(d.path()).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["a.gpkg", "b.shp", "c.TIF"]);
        assert!(list_folder(&d.path().join("nope")).is_err());
    }
}
