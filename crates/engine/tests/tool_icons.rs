//! Every tool has its own icon in `ui/src/nodes/toolIcon.tsx` (Add menu, Geoprocessing pane, node cards, ribbon).
//! Adding a tool without a line there, leaving a line for a removed tool, or giving two tools one icon fails here.

use engine::Engine;
use std::collections::{BTreeMap, BTreeSet};

fn icon_lines() -> BTreeMap<String, String> {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/nodes/toolIcon.tsx")).expect("ui/src/nodes/toolIcon.tsx");
    let block = src.split("TOOL_ICONS: Record<string, FluentIcon> = {").nth(1).expect("the TOOL_ICONS table").split("};").next().unwrap();
    block
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().trim_end_matches(',').split_once(':')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

#[test]
fn every_tool_has_its_own_icon() {
    let e = Engine::new().unwrap();
    let ids: BTreeSet<String> = e.tool_specs().iter().map(|s| s.id.to_string()).collect();
    let icons = icon_lines();
    let missing: Vec<_> = ids.iter().filter(|i| !icons.contains_key(*i)).collect();
    assert!(missing.is_empty(), "tools without an icon in toolIcon.tsx: {missing:?}");
    let stale: Vec<_> = icons.keys().filter(|k| !ids.contains(*k)).collect();
    assert!(stale.is_empty(), "icons for tools that no longer exist: {stale:?}");
    let mut by_icon: BTreeMap<&String, Vec<&String>> = BTreeMap::new();
    for (tool, icon) in &icons {
        by_icon.entry(icon).or_default().push(tool);
    }
    let shared: Vec<_> = by_icon.iter().filter(|(_, t)| t.len() > 1).collect();
    assert!(shared.is_empty(), "tools sharing one icon: {shared:?}");
}
