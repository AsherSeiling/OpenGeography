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
 * Default layout from the sketch:
 *   Contents | Map | Attribute table | Properties/Geoprocessing
 *            |  Model (spans map + table)  |
 */
export function defaultLayout() {
  if (!dock) return;
  dock.clear();
  add("map");
  add("model", { referencePanel: "map", direction: "below" });
  add("table", { referencePanel: "map", direction: "right" });
  add("contents", { direction: "left" }); // whole-height column on the far left
  add("properties", { direction: "right" }); // whole-height column on the far right
  add("tools", { referencePanel: "properties", direction: "within" });
  dock.getPanel("properties")?.api.setActive();
  dock.getPanel("map")?.api.setActive();
  applyDefaultSizes();
}

/**
 * Narrow side columns, map gets most of the top row, Model gets ~40% of the
 * height. Waits until the dock has a real size (sizes applied while it is
 * 0×0 are distributed wrongly).
 */
function applyDefaultSizes(tries = 0) {
  if (!dock) return;
  if ((dock.width < 200 || dock.height < 200) && tries < 60) {
    requestAnimationFrame(() => applyDefaultSizes(tries + 1));
    return;
  }
  const W = dock.width;
  const H = dock.height;
  const contents = 230;
  const properties = 300;
  const center = W - contents - properties;
  dock.getPanel("contents")?.group.api.setSize({ width: contents });
  dock.getPanel("properties")?.group.api.setSize({ width: properties });
  dock.getPanel("table")?.group.api.setSize({ width: Math.round(Math.max(300, center * 0.34)) });
  dock.getPanel("model")?.group.api.setSize({ height: Math.round(Math.max(220, H * 0.4)) });
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
    contents: { direction: "left" },
    properties: { direction: "right" },
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
