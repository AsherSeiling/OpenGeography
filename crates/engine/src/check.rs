//! Check before running (UX7): coordinate-system problems found on a node tree without running it.
//!
//! The rules live in ONE place, [`crs_problems`], a pure function from "the coordinate systems on a tool's feature inputs" to
//! problems and fixes. Everything else here only works out what those coordinate systems are before anything runs, and only
//! states what it can know:
//!
//! * a Layer Input knows its layer's CRS (or knows it has none);
//! * Project's output is its setting; the vetted tools in [`crs_rule`] keep the CRS of their first input (the same rule the tools
//!   use when they run: other inputs are reprojected to the first input's CRS);
//! * everything else is `Unknown`, and an unknown CRS produces no claim at all.
//!
//! Never guessed: a layer without a CRS is an error with no one-click fix (the user must say what the data really uses, with
//! Define Projection). The only fix offered is an explicit Project step to a CRS the tree already uses (the first input's), which
//! is exactly what the tool would do silently when it runs; the user sees and can undo it.

use crate::crs;
use crate::graph::{plan, Graph};
use crate::layers::LayerInfo;
use crate::nodes::{PortKind, Tool};
use serde::Serialize;
use std::collections::HashMap;

/// What a tool's output CRS follows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrsRule {
    /// Not declared: nothing is claimed about it.
    Unknown,
    /// The CRS of the first input port.
    FirstInput,
    /// The coordinate system named by this setting (Project).
    Param(&'static str),
}

/// Output-CRS rule per tool. Only tools whose code was checked to take the first input's CRS (or Project's setting) are listed.
pub fn crs_rule(tool: &str) -> CrsRule {
    match tool {
        "project" => CrsRule::Param("crs"),
        "buffer" | "clip" | "union" | "erase" | "intersect" | "symmetrical_difference" | "select_by_location" | "spatial_join" | "near" | "simplify" | "filter"
        | "dissolve" | "explode" | "feature_to_point" | "calculate_field" | "bounding_geometry" | "repair" | "collect" | "layer_output" => CrsRule::FirstInput,
        _ => CrsRule::Unknown,
    }
}

/// What is known about one value's coordinate system before running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Known {
    Crs(String),
    /// The data is known to have no coordinate system.
    Missing,
    Unknown,
}

/// A feature input of a tool with its coordinate system.
#[derive(Debug, Clone)]
pub struct PortCrs {
    pub port: String,
    pub label: String,
    pub crs: Known,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A safe, explicit fix: put a Project step with this CRS in front of this port.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFix {
    pub port: String,
    pub crs: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CrsProblem {
    pub port: String,
    pub severity: Severity,
    /// "crs-missing" or "crs-mismatch".
    pub code: &'static str,
    pub message: String,
    pub fix: Option<ProjectFix>,
}

/// THE coordinate-system rules for a tool's feature inputs, in port order.
///
/// 1. An input known to have no CRS is an error, with no fix: GIS Studio never guesses one.
/// 2. When the first input's CRS is known, every later input known to be in a different CRS gets a warning saying it will be
///    reprojected to the first input's, and the datum change if there is one. The fix puts an explicit Project step to the first
///    input's CRS in front of that input.
/// 3. Anything unknown makes no claim.
pub fn crs_problems(inputs: &[PortCrs]) -> Vec<CrsProblem> {
    let mut out = Vec::new();
    for p in inputs.iter().filter(|p| p.crs == Known::Missing) {
        out.push(CrsProblem {
            port: p.port.clone(),
            severity: Severity::Error,
            code: "crs-missing",
            message: format!(
                "'{}' has no coordinate system, and GIS Studio will not guess one. Use Define Projection on the source layer with the system the data really uses.",
                p.label
            ),
            fix: None,
        });
    }
    let Some(first) = inputs.first() else { return out };
    let Known::Crs(target) = &first.crs else { return out };
    for p in &inputs[1..] {
        let Known::Crs(c) = &p.crs else { continue };
        if crs::same(c, target) {
            continue;
        }
        let name = |s: &str| crs::info(s).map(|i| i.name).unwrap_or_else(|_| crs::resolve(s));
        let mut message = format!(
            "'{}' is in {} but '{}' is in {}. This tool reprojects '{}' to {} when it runs.",
            p.label,
            name(c),
            first.label,
            name(target),
            p.label,
            name(target)
        );
        if let Some(note) = crs::datum_change_note(c, target) {
            message.push(' ');
            message.push_str(&note);
        }
        out.push(CrsProblem { port: p.port.clone(), severity: Severity::Warning, code: "crs-mismatch", message, fix: Some(ProjectFix { port: p.port.clone(), crs: crs::resolve(target) }) });
    }
    out
}

/// Problems on one node of a tree.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeProblem {
    pub node: String,
    #[serde(flatten)]
    pub problem: CrsProblem,
}

/// Check every node's coordinate systems without running anything. `layers` is what the engine knows (as for a run).
pub fn check_graph(graph: &Graph, tools: &[Box<dyn Tool>], layers: &HashMap<String, LayerInfo>) -> anyhow::Result<Vec<NodeProblem>> {
    let order = plan(graph, None)?;
    let tool_by_id: HashMap<&str, &dyn Tool> = tools.iter().map(|t| (t.spec().id, t.as_ref())).collect();
    let mut known: HashMap<String, Known> = HashMap::new();
    let mut out = Vec::new();
    for id in order {
        let Some(node) = graph.nodes.iter().find(|n| n.id == id) else { continue };
        let Some(tool) = tool_by_id.get(node.tool.as_str()) else { continue };
        let spec = tool.spec();
        let source_of = |port: &str| graph.edges.iter().find(|e| e.target == id && e.target_handle == port).map(|e| e.source.as_str());
        let mut ports: Vec<PortCrs> = Vec::new();
        // Raster tools reproject in their own ways; only feature-only tools are checked.
        let features_only = spec.inputs.iter().all(|p| p.kind != PortKind::Raster);
        if features_only {
            for p in &spec.inputs {
                let k = source_of(p.id).and_then(|s| known.get(s)).cloned().unwrap_or(Known::Unknown);
                ports.push(PortCrs { port: p.id.to_string(), label: p.label.to_string(), crs: k });
            }
            for pr in crs_problems(&ports) {
                out.push(NodeProblem { node: id.clone(), problem: pr });
            }
        }
        let value = if spec.id == "input_layer" {
            match node.params.get("layer").and_then(|v| v.as_str()).and_then(|l| layers.get(l)) {
                Some(l) if l.geom_type == "Raster" => Known::Unknown,
                Some(l) => l.crs.clone().map(Known::Crs).unwrap_or(Known::Missing),
                None => Known::Unknown,
            }
        } else {
            match crs_rule(spec.id) {
                CrsRule::Unknown => Known::Unknown,
                CrsRule::FirstInput => ports.first().map(|p| p.crs.clone()).unwrap_or(Known::Unknown),
                CrsRule::Param(k) => match node.params.get(k).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
                    Some(c) => Known::Crs(crs::resolve(c)),
                    None => Known::Unknown,
                },
            }
        };
        known.insert(id, value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(port: &str, label: &str, crs: Known) -> PortCrs {
        PortCrs { port: port.into(), label: label.into(), crs }
    }
    fn c(s: &str) -> Known {
        Known::Crs(s.into())
    }

    #[test]
    fn same_crs_is_quiet_even_when_written_differently() {
        let v = crs_problems(&[p("a", "Input", c("EPSG:32615")), p("b", "Overlay", c("epsg:32615"))]);
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn a_layer_with_no_crs_is_an_error_with_no_fix() {
        let v = crs_problems(&[p("a", "Input", Known::Missing)]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].severity, Severity::Error);
        assert_eq!(v[0].code, "crs-missing");
        assert!(v[0].fix.is_none(), "a missing CRS is never fixed by guessing");
        assert!(v[0].message.contains("will not guess"), "{}", v[0].message);
    }

    #[test]
    fn missing_overlay_is_an_error_even_when_the_first_input_is_fine() {
        let v = crs_problems(&[p("a", "Input", c("EPSG:32615")), p("b", "Overlay", Known::Missing)]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].port, "b");
        assert_eq!(v[0].code, "crs-missing");
    }

    #[test]
    fn mismatch_warns_on_the_later_input_and_offers_project_to_the_first_inputs_crs() {
        let v = crs_problems(&[p("a", "Input", c("EPSG:32615")), p("b", "Overlay", c("EPSG:4326"))]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].port, "b");
        assert_eq!(v[0].severity, Severity::Warning);
        assert_eq!(v[0].code, "crs-mismatch");
        assert_eq!(v[0].fix, Some(ProjectFix { port: "b".into(), crs: "EPSG:32615".into() }));
        assert!(v[0].message.contains("reprojects 'Overlay'"), "{}", v[0].message);
    }

    #[test]
    fn unknown_crs_makes_no_claim() {
        assert!(crs_problems(&[p("a", "Input", Known::Unknown), p("b", "Overlay", c("EPSG:4326"))]).is_empty());
        assert!(crs_problems(&[p("a", "Input", c("EPSG:4326")), p("b", "Overlay", Known::Unknown)]).is_empty());
        // Nothing is compared when the first input's CRS is not known, even if two later inputs differ.
        assert!(crs_problems(&[p("a", "A", Known::Unknown), p("b", "B", c("EPSG:4326")), p("c", "C", c("EPSG:3857"))]).is_empty());
    }

    #[test]
    fn a_datum_change_is_named_in_the_warning() {
        // NAD27 (EPSG:4267) to WGS 84 (EPSG:4326) changes datum.
        let v = crs_problems(&[p("a", "Input", c("EPSG:4326")), p("b", "Overlay", c("EPSG:4267"))]);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.to_lowercase().contains("datum"), "{}", v[0].message);
    }

    #[test]
    fn every_rule_names_a_real_tool() {
        let ids: Vec<&str> = crate::nodes::registry().iter().map(|t| t.spec().id).collect();
        for t in [
            "project", "buffer", "clip", "union", "erase", "intersect", "symmetrical_difference", "select_by_location", "spatial_join", "near", "simplify", "filter", "dissolve",
            "explode", "feature_to_point", "calculate_field", "bounding_geometry", "repair", "collect", "layer_output",
        ] {
            assert!(ids.contains(&t), "{t} is not a registered tool");
            assert_ne!(crs_rule(t), CrsRule::Unknown);
        }
    }
}
