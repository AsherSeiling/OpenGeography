// Dockable pane layout (ArcGIS Pro style). Panes can be dragged, tabbed and
// floated; View → Reset Panes restores this default.
import type { DockviewApi } from "dockview-react";

export type PaneId = "contents" | "map" | "table" | "model" | "properties" | "tools";

export const PANE_TITLES: Record<PaneId, string> = {
  contents: "Contents",
  map: "Map",
  table: "Attribute Table",
  model: "Model",
  properties: "Properties",
  tools: "Geoprocessing",
};

let dock: DockviewApi | null = null;

export function setDock(api: DockviewApi) {
  dock = api;
}

export function getDock() {
  return dock;
}

function add(id: PaneId, position?: Parameters<DockviewApi["addPanel"]>[0]["position"]) {
  return dock!.addPanel({ id, component: id, title: PANE_TITLES[id], position });
}

/**
 * Default layout from the sketch: Contents on the left; Map with the
 * Attribute Table beside it; the Model below; Properties/Geoprocessing right.
 */
export function defaultLayout() {
  if (!dock) return;
  dock.clear();
  add("map");
  add("contents", { referencePanel: "map", direction: "left" });
  add("properties", { referencePanel: "map", direction: "right" });
  add("tools", { referencePanel: "properties", direction: "within" });
  add("table", { referencePanel: "map", direction: "right" });
  add("model", { referencePanel: "map", direction: "below" });
  dock.getPanel("properties")?.api.setActive();
  dock.getPanel("map")?.api.setActive();
  // Initial sizes: narrow side columns, map gets most of the width.
  const total = dock.width;
  dock.getPanel("contents")?.group.api.setSize({ width: 240 });
  dock.getPanel("properties")?.group.api.setSize({ width: 300 });
  dock.getPanel("table")?.group.api.setSize({ width: Math.max(320, total * 0.25) });
  dock.getPanel("model")?.group.api.setSize({ height: Math.max(240, dock.height * 0.36) });
}

/** Show a pane, re-opening it in a sensible place if it was closed. */
export function showPane(id: PaneId) {
  if (!dock) return;
  const existing = dock.getPanel(id);
  if (existing) {
    existing.api.setActive();
    return;
  }
  const anchor: Record<PaneId, Parameters<DockviewApi["addPanel"]>[0]["position"]> = {
    map: undefined,
    contents: dock.getPanel("map") ? { referencePanel: "map", direction: "left" } : undefined,
    properties: dock.getPanel("map") ? { referencePanel: "map", direction: "right" } : undefined,
    tools: dock.getPanel("properties") ? { referencePanel: "properties", direction: "within" } : undefined,
    table: dock.getPanel("map") ? { referencePanel: "map", direction: "right" } : undefined,
    model: dock.getPanel("map") ? { referencePanel: "map", direction: "below" } : undefined,
  };
  add(id, anchor[id]);
}

export function togglePane(id: PaneId) {
  const p = dock?.getPanel(id);
  if (p) p.api.close();
  else showPane(id);
}
