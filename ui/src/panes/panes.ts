// Dockable pane layout. Panes can be dragged, tabbed and floated; View → Reset Panes restores the default.
//
// Default (docs/UI_REDESIGN_PLAN.md §1):
//   Map Layers |        Map         | Properties
//   Catalog    |                    | Sources
// "Map" is one panel per map of the project (panel id = map id, title = map name); drag a tab to put two side by side.
// The bottom panel (Node Editor, Attribute Table) does not exist until one of them is opened; closing the last one removes it and the
// map fills the centre again.
import type { DockviewApi, IDockviewPanel } from "dockview-react";
import { create } from "zustand";
import { activeMapOf, isMapPanelId, mapOf } from "../store/maps";
import { useApp } from "../store/store";

export type PaneId = "contents" | "catalog" | "map" | "table" | "model" | "properties" | "sources" | "items";

export const PANE_TITLES: Record<PaneId, string> = {
  contents: "Map Layers",
  catalog: "Catalog",
  map: "Map",
  table: "Attribute Table",
  model: "Processing nodes",
  properties: "Properties",
  sources: "Sources",
  items: "Items",
};

/**
 * Version of the saved dock layout. A project saved with another version (or none) opens with the default layout.
 * 3: one map panel per map (id `map` or `map-…`, title = map name). Version 2 (a single panel `map`) is still read: it is a valid 3.
 */
export const DOCK_VERSION = 3;
const READABLE_VERSIONS = [2, 3];
/** Panes that go in the bottom panel. */
const BOTTOM: PaneId[] = ["model", "table"];
/** Pane ids a saved layout may name. Anything else (the removed Geoprocessing pane, say) makes the layout unusable. */
const KNOWN = new Set<string>(Object.keys(PANE_TITLES));

interface PaneState {
  /** Ids of the panes that are open. */
  open: string[];
}
export const usePanes = create<PaneState>(() => ({ open: [] }));
export const isPaneOpen = (id: PaneId) => usePanes.getState().open.includes(id);

let dock: DockviewApi | null = null;

export function setDock(api: DockviewApi) {
  dock = api;
  const refresh = () => {
    const open = api.panels.map((p) => p.id);
    if (open.join() !== usePanes.getState().open.join()) usePanes.setState({ open });
  };
  api.onDidAddPanel(refresh);
  api.onDidRemovePanel(refresh);
  refresh();
}

export function getDock() {
  return dock;
}

type Position = Parameters<DockviewApi["addPanel"]>[0]["position"];

function add(id: PaneId, position?: Position) {
  return dock!.addPanel({ id, component: id, title: PANE_TITLES[id], position });
}

// ---------- map panels ----------

/** Panels of the dock that show a map. */
export function mapPanels(): IDockviewPanel[] {
  return (dock?.panels ?? []).filter((p) => isMapPanelId(p.id));
}

/** The panel of the active map, else any map panel (what other panes anchor beside). */
export function anchorMapPanel(): IDockviewPanel | undefined {
  const s = useApp.getState();
  return dock?.getPanel(s.activeMapId) ?? mapPanels()[0];
}

function addMapPanel(mapId: string, position?: Position) {
  const m = mapOf(useApp.getState(), mapId);
  return dock!.addPanel({ id: mapId, component: "map", title: m?.name ?? "Map", position });
}

/** Show a map's panel and make it the active map. `side` opens it to the right of the current map instead of as a tab beside it. */
export function showMap(mapId: string, opts: { side?: boolean; group?: IDockviewPanel["group"] } = {}) {
  useApp.getState().setActiveMap(mapId);
  if (!dock) return;
  const existing = dock.getPanel(mapId);
  if (existing) {
    existing.api.setActive();
    return;
  }
  const near = anchorMapPanel();
  let position: Position;
  if (opts.group) position = { referenceGroup: opts.group, direction: "within" };
  else if (near) position = { referencePanel: near.id, direction: opts.side ? "right" : "within" };
  addMapPanel(mapId, position).api.setActive();
}

/** Make the map panels match the project's maps: close those of maps that are gone, retitle renamed ones, make sure one is open. */
export function syncMapPanels() {
  if (!dock || rebuilding > 0) return;
  const s = useApp.getState();
  for (const p of mapPanels()) {
    const m = mapOf(s, p.id);
    if (!m) p.api.close();
    else if (p.title !== m.name) p.api.setTitle(m.name);
  }
  if (!mapPanels().length) {
    const m = activeMapOf(s);
    const anchor = dock.getPanel("catalog") ?? dock.panels[0];
    const p = addMapPanel(m.id, anchor ? { referencePanel: anchor.id, direction: "right" } : undefined);
    p.api.setActive();
  }
}

/** Default layout: Map Layers over Catalog on the left, the map in the middle, Properties on the right. No bottom panel; Sources is left out and opens from View. */
export function defaultLayout() {
  if (!dock) return;
  rebuilding++;
  try {
    dock.clear();
    addMapPanel(activeMapOf(useApp.getState()).id);
    add("contents", { direction: "left" }); // whole-height column on the far left
    add("catalog", { referencePanel: "contents", direction: "below" });
    add("properties", { direction: "right" }); // whole-height column on the far right
    anchorMapPanel()?.api.setActive();
  } finally {
    rebuilding--;
  }
  applyDefaultSizes();
}

/** Side columns of about 15 % and 21 % of the window, Map Layers 26 % of the left column, Sources 19 % of the right. Waits until the dock has a real size. */
function applyDefaultSizes(tries = 0) {
  if (!dock) return;
  if ((dock.width < 200 || dock.height < 200) && tries < 60) {
    requestAnimationFrame(() => applyDefaultSizes(tries + 1));
    return;
  }
  const W = dock.width;
  const H = dock.height;
  const left = Math.min(260, Math.max(190, Math.round(W * 0.15)));
  const right = Math.min(340, Math.max(240, Math.round(W * 0.21)));
  dock.getPanel("contents")?.group.api.setSize({ width: left, height: Math.round(H * 0.26) });
  dock.getPanel("properties")?.group.api.setSize({ width: right });
  dock.getPanel("sources")?.group.api.setSize({ height: Math.round(H * 0.19) });
}

/** Share of the centre column the bottom panel takes when it opens. */
const BOTTOM_SHARE = 0.47;

/** Show a pane, re-opening it in a sensible place if it was closed. */
export function showPane(id: PaneId) {
  if (!dock) return;
  if (id === "map") return showMap(useApp.getState().activeMapId);
  const existing = dock.getPanel(id);
  if (existing) {
    existing.api.setActive();
    return;
  }
  const other = dock.getPanel(id === "model" ? "table" : "model");
  const map = anchorMapPanel();
  const anchor: Record<PaneId, Position> = {
    map: undefined,
    contents: dock.getPanel("catalog") ? { referencePanel: "catalog", direction: "above" } : { direction: "left" },
    catalog: dock.getPanel("contents") ? { referencePanel: "contents", direction: "below" } : { direction: "left" },
    properties: dock.getPanel("sources") ? { referencePanel: "sources", direction: "above" } : { direction: "right" },
    // Items shares the Map Layers pane, which is the layout's list of what is on the page.
    items: dock.getPanel("contents") ? { referencePanel: "contents", direction: "within" } : { direction: "left" },
    sources: dock.getPanel("properties") ? { referencePanel: "properties", direction: "below" } : { direction: "right" },
    table: other ? { referencePanel: other.id, direction: "within" } : map ? { referencePanel: map.id, direction: "below" } : undefined,
    model: other ? { referencePanel: other.id, direction: "within" } : map ? { referencePanel: map.id, direction: "below" } : undefined,
  };
  const mapHeight = map?.group.api.height ?? 0;
  const p = add(id, anchor[id]);
  if (BOTTOM.includes(id) && !other && mapHeight > 0) p.group.api.setSize({ height: Math.round(mapHeight * BOTTOM_SHARE) });
}

/** Close every layout tab (the project changed). */
export function closeLayoutPanels() {
  for (const p of [...(dock?.panels ?? [])]) if (p.id.startsWith("layout:")) p.api.close();
}

export function togglePane(id: PaneId) {
  if (id === "map") return showMap(useApp.getState().activeMapId);
  const p = dock?.getPanel(id);
  if (p) p.api.close();
  else showPane(id);
}

/** What the project file stores for the dock. */
export function serializeDock(): unknown {
  return dock ? { version: DOCK_VERSION, layout: dock.toJSON() } : null;
}

/** Is `saved` a dock layout this version can restore (a readable version, only known panes, has a map)? */
export function usableDock(saved: unknown): saved is { version: number; layout: { panels?: Record<string, unknown> } } {
  const d = saved as { version?: unknown; layout?: { panels?: Record<string, unknown> } } | null;
  if (!d || typeof d !== "object" || typeof d.version !== "number" || !READABLE_VERSIONS.includes(d.version) || !d.layout || typeof d.layout !== "object") return false;
  const ids = Object.keys(d.layout.panels ?? {});
  return ids.some(isMapPanelId) && ids.every((id) => KNOWN.has(id) || id.startsWith("layout:") || isMapPanelId(id));
}

/** Restore a saved layout, or fall back to the default when it is old, unknown, names a removed pane or cannot be read. */
export function restoreDock(saved: unknown) {
  if (!dock) return;
  if (!usableDock(saved)) return defaultLayout();
  rebuilding++;
  try {
    dock.fromJSON(saved.layout as never);
    // A project saved before a pane was renamed still holds the old tab title.
    for (const p of dock.panels) {
      const t = PANE_TITLES[p.id as PaneId];
      if (t && KNOWN.has(p.id) && p.title !== t) p.api.setTitle(t);
    }
  } catch {
    defaultLayout();
  } finally {
    rebuilding--;
  }
  syncMapPanels();
}

/** True while the whole dock is rebuilt (open project, reset): panels closing then are not the user closing a map tab. */
let rebuilding = 0;
export const dockIsRebuilding = () => rebuilding > 0;
