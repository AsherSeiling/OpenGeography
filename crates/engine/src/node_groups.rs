//! Reusable node groups (UX8): part of a node tree saved by name as the user's own building block, kept in the user's data
//! folder (`node-groups/`, so every project sees it) and shared as a single `.gisgroup` file.
//!
//! A group is plain data: the nodes (tool, settings, position), the connections between them, and which inputs are left open.
//! Inserting one copies those nodes into a tree as ordinary, editable nodes, so nothing new runs and nothing hides. Files from other
//! people are never trusted: the format, sizes and every tool id are checked, project layer references are removed (a layer id means
//! nothing in another project), and a Python script inside still needs this computer's approval before it runs, like any script.

use crate::nodes::{self, ParamKind};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "gis-studio-node-group";
pub const VERSION: u32 = 1;
const MAX_NODES: usize = 500;
const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub id: String,
    pub tool: String,
    #[serde(default)]
    pub params: Map<String, Value>,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupEdge {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub source_handle: Option<String>,
    pub target_handle: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub nodes: Vec<GroupNode>,
    pub edges: Vec<GroupEdge>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub name: String,
    pub description: String,
    pub nodes: usize,
}

/// Check a group and return the safe copy: right format, sane sizes, only tools this version has, connections between its own nodes,
/// and no project layer references (Layer settings are emptied: a layer id from another project means nothing here, and must not leak).
pub fn validated(value: &Value) -> Result<Group> {
    let mut g: Group = serde_json::from_value(value.clone()).map_err(|e| anyhow!("This is not a node group file ({e})."))?;
    if g.format != FORMAT {
        bail!("This is not a GIS Studio node group file.");
    }
    if g.version > VERSION {
        bail!("This node group was made by a newer GIS Studio (format {}). Update GIS Studio to use it.", g.version);
    }
    g.name = g.name.trim().to_string();
    if g.name.is_empty() || g.name.chars().count() > 80 {
        bail!("A node group needs a name of 1 to 80 characters.");
    }
    if g.description.chars().count() > 1000 {
        g.description = g.description.chars().take(1000).collect();
    }
    if g.nodes.is_empty() {
        bail!("A node group needs at least one node.");
    }
    if g.nodes.len() > MAX_NODES {
        bail!("A node group can have at most {MAX_NODES} nodes.");
    }
    let tools = nodes::registry();
    let mut ids = HashSet::new();
    for n in &mut g.nodes {
        if !ids.insert(n.id.clone()) {
            bail!("Two nodes in this group share the id '{}'.", n.id);
        }
        let tool = tools.iter().find(|t| t.spec().id == n.tool).ok_or_else(|| anyhow!("This group uses a tool this version does not have: '{}'.", n.tool))?;
        let spec = tool.spec();
        if matches!(spec.role, nodes::Role::Output) {
            bail!("A node group cannot contain a layer's Output.");
        }
        if !n.x.is_finite() || !n.y.is_finite() {
            bail!("A node in this group has no valid position.");
        }
        for p in spec.params.iter().filter(|p| matches!(p.kind, ParamKind::Layer)) {
            n.params.remove(p.id);
        }
        if n.tool == "input_layer" {
            n.params.remove("layer");
        }
    }
    for e in &g.edges {
        if !ids.contains(&e.source) || !ids.contains(&e.target) {
            bail!("This group has a connection to a node that is not in it.");
        }
    }
    Ok(g)
}

fn group_dir(base: &Path) -> PathBuf {
    base.join("node-groups")
}

/// A file name that is safe on every system, from a group's name.
fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "group".into() } else { s.chars().take(60).collect() }
}

fn read_group(path: &Path) -> Result<Group> {
    if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        bail!("That file is too large to be a node group.");
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("Could not read {}", path.display()))?;
    validated(&serde_json::from_str::<Value>(&text).map_err(|e| anyhow!("This is not a node group file ({e})."))?)
}

fn file_for(dir: &Path, name: &str) -> PathBuf {
    // Same name keeps the same file; another name that makes the same slug gets -2, -3 ...
    let base = slug(name);
    for i in 1.. {
        let p = dir.join(if i == 1 { format!("{base}.gisgroup") } else { format!("{base}-{i}.gisgroup") });
        match read_group(&p) {
            Ok(g) if g.name != name => continue,
            _ => return p,
        }
    }
    unreachable!()
}

/// Save (or replace, when the name already exists) a group in the library at `base`.
pub fn save_in(base: &Path, value: &Value) -> Result<Group> {
    let g = validated(value)?;
    let dir = group_dir(base);
    std::fs::create_dir_all(&dir)?;
    let path = file_for(&dir, &g.name);
    // Written beside, then moved into place, so a crash never leaves half a file.
    let tmp = path.with_extension("gisgroup.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&g)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(g)
}

/// The library's groups, by name. A file that is damaged or from a newer version is skipped, never an error.
pub fn list_in(base: &Path) -> Vec<Group> {
    let mut out: Vec<Group> = std::fs::read_dir(group_dir(base))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "gisgroup"))
        .filter_map(|e| read_group(&e.path()).ok())
        .collect();
    out.sort_by_key(|g| g.name.to_lowercase());
    out
}

pub fn delete_in(base: &Path, name: &str) -> Result<()> {
    let dir = group_dir(base);
    let path = file_for(&dir, name);
    match read_group(&path) {
        Ok(g) if g.name == name => std::fs::remove_file(path).map_err(Into::into),
        _ => bail!("There is no saved node group called '{name}'."),
    }
}

/// Write one library group to a file anyone can open with Import.
pub fn export_in(base: &Path, name: &str, dest: &Path) -> Result<()> {
    let g = list_in(base).into_iter().find(|g| g.name == name).ok_or_else(|| anyhow!("There is no saved node group called '{name}'."))?;
    std::fs::write(dest, serde_json::to_vec_pretty(&g)?).with_context(|| format!("Could not write {}", dest.display()))
}

/// Read a shared file, check it, and add it to the library (replacing a group of the same name).
pub fn import_in(base: &Path, src: &Path) -> Result<Group> {
    let g = read_group(src)?;
    save_in(base, &serde_json::to_value(&g)?)
}

fn base() -> Result<PathBuf> {
    crate::resources::data_dir().ok_or_else(|| anyhow!("GIS Studio could not find a folder to keep your node groups in."))
}

pub fn list() -> Vec<Summary> {
    base().map(|b| list_in(&b)).unwrap_or_default().into_iter().map(|g| Summary { nodes: g.nodes.len(), name: g.name, description: g.description }).collect()
}
pub fn get(name: &str) -> Result<Group> {
    list_in(&base()?).into_iter().find(|g| g.name == name).ok_or_else(|| anyhow!("There is no saved node group called '{name}'."))
}
pub fn save(value: &Value) -> Result<Group> {
    save_in(&base()?, value)
}
pub fn delete(name: &str) -> Result<()> {
    delete_in(&base()?, name)
}
pub fn export(name: &str, dest: &str) -> Result<()> {
    export_in(&base()?, name, Path::new(dest))
}
pub fn import(src: &str) -> Result<Group> {
    import_in(&base()?, Path::new(src))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn group(name: &str) -> Value {
        json!({
            "format": FORMAT, "version": 1, "name": name, "description": "Buffer then dissolve",
            "nodes": [
                {"id": "a", "tool": "buffer", "params": {"distance": 100}, "x": 0.0, "y": 0.0},
                {"id": "b", "tool": "dissolve", "params": {}, "x": 300.0, "y": 0.0}
            ],
            "edges": [{"source": "a", "target": "b", "sourceHandle": "out", "targetHandle": "in"}]
        })
    }

    #[test]
    fn save_list_replace_delete_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), &group("Buffer and merge")).unwrap();
        save_in(dir.path(), &group("Second")).unwrap();
        assert_eq!(list_in(dir.path()).iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Buffer and merge", "Second"]);
        // The same name replaces, not duplicates.
        let mut again = group("Buffer and merge");
        again["description"] = json!("changed");
        save_in(dir.path(), &again).unwrap();
        let l = list_in(dir.path());
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].description, "changed");
        delete_in(dir.path(), "Second").unwrap();
        assert_eq!(list_in(dir.path()).len(), 1);
        assert!(delete_in(dir.path(), "Second").unwrap_err().to_string().contains("no saved node group"));
    }

    #[test]
    fn names_that_make_the_same_file_name_do_not_overwrite_each_other() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), &group("Clean up")).unwrap();
        save_in(dir.path(), &group("clean-up")).unwrap();
        assert_eq!(list_in(dir.path()).len(), 2);
    }

    #[test]
    fn a_name_cannot_escape_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), &group("../../etc/passwd")).unwrap();
        let files: Vec<_> = std::fs::read_dir(dir.path().join("node-groups")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(files, ["etc-passwd.gisgroup"]);
    }

    #[test]
    fn export_then_import_into_another_library() {
        let (a, b, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        save_in(a.path(), &group("Shared")).unwrap();
        let file = out.path().join("shared.gisgroup");
        export_in(a.path(), "Shared", &file).unwrap();
        let g = import_in(b.path(), &file).unwrap();
        assert_eq!(g.name, "Shared");
        assert_eq!(list_in(b.path()), list_in(a.path()));
    }

    #[test]
    fn rejects_files_that_are_not_safe_groups() {
        let mut v = group("x");
        v["format"] = json!("something-else");
        assert!(validated(&v).unwrap_err().to_string().contains("not a GIS Studio node group"));
        let mut v = group("x");
        v["version"] = json!(99);
        assert!(validated(&v).unwrap_err().to_string().contains("newer GIS Studio"));
        let mut v = group("x");
        v["nodes"][0]["tool"] = json!("rm_rf");
        assert!(validated(&v).unwrap_err().to_string().contains("'rm_rf'"));
        let mut v = group("x");
        v["edges"][0]["target"] = json!("zzz");
        assert!(validated(&v).unwrap_err().to_string().contains("not in it"));
        let mut v = group("x");
        v["nodes"][1]["id"] = json!("a");
        assert!(validated(&v).unwrap_err().to_string().contains("share the id"));
        let mut v = group("x");
        v["nodes"] = json!([{"id": "o", "tool": "layer_output", "params": {}, "x": 0, "y": 0}]);
        v["edges"] = json!([]);
        assert!(validated(&v).unwrap_err().to_string().contains("Output"));
        assert!(validated(&json!({"hello": 1})).unwrap_err().to_string().contains("not a node group file"));
        let mut v = group("   ");
        v["name"] = json!("   ");
        assert!(validated(&v).is_err());
    }

    #[test]
    fn project_layer_references_are_removed() {
        let mut v = group("x");
        v["nodes"][0] = json!({"id": "a", "tool": "input_layer", "params": {"layer": "L-from-another-project"}, "x": 0, "y": 0});
        v["edges"] = json!([]);
        let g = validated(&v).unwrap();
        assert!(g.nodes[0].params.get("layer").is_none(), "{:?}", g.nodes[0].params);
    }

    #[test]
    fn a_damaged_file_in_the_library_is_skipped_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        save_in(dir.path(), &group("Good")).unwrap();
        std::fs::write(dir.path().join("node-groups/bad.gisgroup"), "{not json").unwrap();
        assert_eq!(list_in(dir.path()).len(), 1);
    }
}
