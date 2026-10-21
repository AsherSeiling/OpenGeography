// User-level actions shared by the ribbon, context menus and shortcuts.
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Edge } from "@xyflow/react";
import { create } from "zustand";
import { api, errorText, type LayerInfo } from "./ipc/api";
import { getMap, zoomToExtent } from "./map/mapController";
import { graphPayload, useModel, useRun, type ModelNode } from "./store/model";
import { defaultStyle, useApp, type LayerStyle } from "./store/store";

// ---------- dialogs ----------

export type DialogState =
  | null
  | { kind: "crs"; title: string; initial?: string | null; onPick: (id: string, name: string) => void }
  | { kind: "defineProjection"; layer: string }
  | { kind: "selectByAttributes"; layer: string }
  | { kind: "testData" }
  | { kind: "benchmark" }
  | { kind: "rename"; layer: string };

export const useDialog = create<{ dialog: DialogState; show: (d: DialogState) => void; close: () => void }>((set) => ({
  dialog: null,
  show: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
}));

const status = (s: string) => useApp.getState().setStatus(s);

async function busy<T>(label: string, f: () => Promise<T>): Promise<T | undefined> {
  status(`${label}…`);
  try {
    const r = await f();
    return r;
  } catch (e) {
    status(`${label} failed: ${errorText(e)}`);
    return undefined;
  }
}

// ---------- data ----------

const DATA_FILTERS = [
  { name: "GIS data", extensions: ["shp", "gpkg", "geojson", "json", "kml", "kmz", "fgb", "gml", "gpx", "csv", "tsv", "txt", "parquet", "zip"] },
  { name: "All files", extensions: ["*"] },
];

export async function addData(paths?: string[]) {
  if (!paths) {
    const picked = await open({ multiple: true, title: "Add Data", filters: DATA_FILTERS });
    if (!picked) return;
    paths = Array.isArray(picked) ? picked : [picked];
  }
  const infos = await busy(`Adding ${paths.length} file(s)`, () => api.importFiles(paths!));
  if (!infos) return;
  const hadLayers = useApp.getState().order.length > 0;
  useApp.getState().addLayers(infos);
  const noCrs = infos.filter((i) => i.geomType !== "Table" && !i.crs);
  status(
    noCrs.length
      ? `Added ${infos.length} layer(s). ${noCrs.length} without a coordinate system — use Define Projection.`
      : `Added ${infos.length} layer(s) · ${infos.reduce((n, i) => n + i.featureCount, 0).toLocaleString()} features`,
  );
  if (!hadLayers) zoomToExtent(infos.find((i) => i.extent)?.extent ?? null);
  if (noCrs.length) useDialog.getState().show({ kind: "defineProjection", layer: noCrs[0].id });
}

export async function zoomToLayer(id: string) {
  const info = useApp.getState().layers[id];
  zoomToExtent(info?.extent ?? null);
}

export async function zoomToSelection() {
  const sel = useApp.getState().selection;
  let ext: [number, number, number, number] | null = null;
  for (const [id, fids] of Object.entries(sel)) {
    if (!fids.length) continue;
    const e = await api.layerExtent(id, fids);
    if (!e) continue;
    ext = ext ? [Math.min(ext[0], e[0]), Math.min(ext[1], e[1]), Math.max(ext[2], e[2]), Math.max(ext[3], e[3])] : e;
  }
  if (ext) zoomToExtent(ext);
  else status("Nothing selected");
}

export async function removeLayer(id: string) {
  const ok = await busy("Removing layer", () => api.removeLayer(id));
  if (ok === undefined) return;
  useApp.getState().removeLayer(id);
  status("Layer removed");
}

export async function defineProjection(layer: string, crs: string, name: string) {
  const info = await busy("Defining projection", () => api.defineProjection(layer, crs, name));
  if (!info) return;
  useApp.getState().updateLayer(info);
  const s = useApp.getState();
  if (!s.mapCrs) s.setMapCrs({ id: crs, name });
  status(`${info.name}: coordinate system set to ${name}`);
  zoomToExtent(info.extent);
}

export function pickMapCrs() {
  const cur = useApp.getState().mapCrs;
  useDialog.getState().show({
    kind: "crs",
    title: "Map Coordinate System",
    initial: cur?.id,
    onPick: (id, name) => {
      useApp.getState().setMapCrs({ id, name });
      status(`Map coordinate system: ${name}. Coordinates, measurements and layouts use it; the screen draws in Web Mercator.`);
    },
  });
}

export async function exportLayer(id: string) {
  const info = useApp.getState().layers[id];
  if (!info) return;
  const path = await save({
    title: `Export ${info.name}`,
    defaultPath: `${info.name}.gpkg`,
    filters: [
      { name: "GeoPackage", extensions: ["gpkg"] },
      { name: "Shapefile", extensions: ["shp"] },
      { name: "GeoJSON", extensions: ["geojson"] },
      { name: "GeoParquet", extensions: ["parquet"] },
      { name: "CSV", extensions: ["csv"] },
    ],
  });
  if (!path) return;
  const ext = path.split(".").pop()?.toLowerCase();
  const format = { gpkg: "GPKG", shp: "ESRI Shapefile", geojson: "GeoJSON", json: "GeoJSON", parquet: "PARQUET", csv: "CSV", fgb: "FlatGeobuf" }[ext ?? ""] ?? "GPKG";
  const res = await busy(`Exporting ${info.name}`, () =>
    api.runGraph({
      nodes: [
        { id: "in", tool: "input_layer", params: { layer: id } },
        { id: "out", tool: "export", params: { path, format } },
      ],
      edges: [{ source: "in", target: "out", targetHandle: "in" }],
    }),
  );
  const out = res?.find((r) => r.id === "out");
  if (out) status(out.error ? `Export failed: ${out.error}` : (out.message ?? "Exported"));
}

export async function selectByAttributes(layer: string, where: string, mode: "new" | "add") {
  const fids = await busy("Selecting", () => api.selectWhere(layer, where));
  if (!fids) return false;
  useApp.getState().setSelection(layer, fids, mode);
  status(`Selected ${fids.length.toLocaleString()} feature(s)`);
  return true;
}

export async function generateTestData(kind: "points" | "polygons", count: number) {
  const map = getMap();
  const b = map?.getBounds();
  const bbox: [number, number, number, number] = b ? [b.getWest(), b.getSouth(), b.getEast(), b.getNorth()] : [-106, 26, -94, 36];
  const info = await busy(`Generating ${count.toLocaleString()} ${kind}`, () => api.generateTestLayer(kind, count, bbox));
  if (!info) return;
  useApp.getState().addLayers([info]);
  status(`Generated ${info.featureCount.toLocaleString()} test ${kind}`);
}

// ---------- model ----------

export async function runModel(target?: string) {
  const run = useRun.getState();
  if (run.running) return;
  const graph = graphPayload();
  if (!graph.nodes.length) {
    status("The model is empty. Add tools from the Analysis tab or the Geoprocessing pane.");
    return;
  }
  run.set({ running: true, results: target ? run.results : {} });
  const t0 = performance.now();
  const res = await busy(target ? "Running to selected step" : "Running model", () => api.runGraph(graph, target));
  run.set({ running: false });
  if (!res) return;
  for (const r of res) {
    run.setResult(r);
    if (r.layer) useApp.getState().updateLayer(r.layer);
  }
  const errors = res.filter((r) => r.status === "error");
  status(
    errors.length
      ? `Model finished with ${errors.length} error(s). Click a red step to see why.`
      : `Model finished in ${((performance.now() - t0) / 1000).toFixed(1)}s (${res.filter((r) => r.status === "cached").length} step(s) reused from cache)`,
  );
}

export async function previewNode(nodeId: string) {
  const r = useRun.getState().results[nodeId];
  if (!r?.output) {
    status("Run the model first to preview this step.");
    return;
  }
  const node = useModel.getState().nodes.find((n) => n.id === nodeId);
  const name = `Preview: ${useRun.getState().specs[node?.data.tool ?? ""]?.name ?? nodeId}`;
  const info = await busy("Previewing", () => api.previewOutput(r.output!, r.crs, name));
  if (!info) return;
  useApp.getState().updateLayer(info);
  useApp.getState().setVisible(info.id, true);
  status(`${name}: ${info.featureCount.toLocaleString()} features`);
}

// ---------- projects ----------

interface UiStateV1 {
  version: 1;
  order: string[];
  visible: Record<string, boolean>;
  styles: Record<string, LayerStyle>;
  mapCrs: { id: string; name: string } | null;
  basemap: string;
  camera: { center: [number, number]; zoom: number } | null;
  model: { nodes: ModelNode[]; edges: Edge[] };
  dock: unknown;
}

let dockSerializer: (() => unknown) | null = null;
let dockRestorer: ((d: unknown) => void) | null = null;
export function registerDock(serialize: () => unknown, restore: (d: unknown) => void) {
  dockSerializer = serialize;
  dockRestorer = restore;
}

function uiState(): UiStateV1 {
  const s = useApp.getState();
  const m = useModel.getState();
  const map = getMap();
  return {
    version: 1,
    order: s.order,
    visible: s.visible,
    styles: s.styles,
    mapCrs: s.mapCrs,
    basemap: s.basemap,
    camera: map ? { center: map.getCenter().toArray() as [number, number], zoom: map.getZoom() } : null,
    model: {
      nodes: m.nodes.map(({ id, type, position, data }) => ({ id, type, position, data })),
      edges: m.edges.map(({ id, source, target, sourceHandle, targetHandle }) => ({ id, source, target, sourceHandle, targetHandle })),
    },
    dock: dockSerializer?.() ?? null,
  };
}

export async function saveProject(saveAs = false) {
  let dir = useApp.getState().projectDir;
  if (!dir || saveAs) {
    const picked = await save({ title: "Save Project", defaultPath: "Untitled.gproj", filters: [{ name: "GIS Studio project", extensions: ["gproj"] }] });
    if (!picked) return;
    dir = picked.endsWith(".gproj") ? picked : `${picked}.gproj`;
  }
  const ok = await busy("Saving project", () => api.saveProject(dir!, JSON.stringify(uiState())));
  if (ok === undefined) return;
  useApp.getState().set({ projectDir: dir, dirty: false });
  useRun.getState().set({ results: {} }); // cache tables are not saved
  status(`Saved ${dir}`);
}

export async function openProject() {
  const picked = await open({ title: "Open Project", directory: true });
  if (!picked || Array.isArray(picked)) return;
  const res = await busy("Opening project", () => api.openProject(picked));
  if (!res) return;
  let ui: Partial<UiStateV1> = {};
  try {
    ui = JSON.parse(res.uiState);
  } catch {
    /* older or hand-edited project: fall back to defaults */
  }
  restoreState(res.layers, ui, picked);
  status(`Opened ${picked}`);
}

function restoreState(layers: LayerInfo[], ui: Partial<UiStateV1>, dir: string | null) {
  const byId = Object.fromEntries(layers.filter((l) => !l.temporary).map((l) => [l.id, l]));
  const order = (ui.order ?? []).filter((id) => byId[id]);
  for (const l of Object.values(byId)) if (!order.includes(l.id)) order.unshift(l.id);
  useApp.setState({
    layers: byId,
    order,
    visible: { ...Object.fromEntries(order.map((id) => [id, true])), ...(ui.visible ?? {}) },
    styles: Object.fromEntries(order.map((id) => [id, ui.styles?.[id] ?? defaultStyle(byId[id])])),
    selection: {},
    mapCrs: ui.mapCrs ?? null,
    basemap: (ui.basemap as never) ?? "usgsTopo",
    activeLayer: order[0] ?? null,
    properties: null,
    projectDir: dir,
    dirty: false,
  });
  useModel.getState().load(ui.model?.nodes ?? [], ui.model?.edges ?? []);
  useModel.temporal.getState().clear();
  useRun.getState().set({ results: {} });
  if (ui.camera) getMap()?.jumpTo(ui.camera);
  if (ui.dock) dockRestorer?.(ui.dock);
}

export async function newProject() {
  if (useApp.getState().dirty && !confirm("Discard unsaved changes?")) return;
  const ok = await busy("Creating project", () => api.newProject());
  if (ok === undefined) return;
  restoreState([], {}, null);
  status("New project");
}
