// User-level actions shared by the ribbon, context menus and shortcuts.
import { bibliographyFiles } from "./sources/cite";
import { useBibPrefs } from "./sources/prefs";
import { track } from "./progress/progress";
import { open, save } from "@tauri-apps/plugin-dialog";
import { create } from "zustand";
import { reportError } from "./errors/store";
import { api, errorText, type SplitTarget, type LayerInfo } from "./ipc/api";
import { getViewBounds, restoreCamera, type Camera, snapshotCamera, zoomToExtent } from "./map/mapController";
import { openLayerProperties } from "./dialogs/LayerPropertiesWindow";
import { closeLayoutPanels, showPane } from "./panes/panes";
import { defaultCompareInput, pruneNodeCache, useTableTarget } from "./panes/table/history";
import { graphPayload, nodeSignature, useModel, useRun, type Tree } from "./store/model";
import { defaultStyle, displayInfo, useApp, type LayerStyle, type SelectMode } from "./store/store";
import { flushLayoutSaves, useLayouts } from "./store/layout";

// ---------- dialogs ----------

export type DialogState =
  | null
  | { kind: "crs"; title: string; initial?: string | null; onPick: (id: string, name: string) => void }
  | { kind: "defineProjection"; layer: string }
  | { kind: "selectByAttributes"; layer: string }
  | { kind: "selectByLocation"; layer: string }
  | { kind: "testData" }
  | { kind: "benchmark" }
  | { kind: "rename"; layer: string }
  | { kind: "addField"; layer: string }
  | { kind: "calcField"; layer: string; owner: string; field?: string }
  | { kind: "calcGeometry"; layer: string; owner: string; field?: string }
  | { kind: "renameField"; layer: string; field: string }
  | { kind: "tiger" }
  | { kind: "export"; owner: string }
  | { kind: "splitByAttribute"; owner: string }
  | { kind: "layoutSeries" }
  | { kind: "nodeQuery"; nodeId: string; param: string }
  | { kind: "elevation" }
  | { kind: "nationalMap" }
  | { kind: "nodeShortcuts" };

export const useDialog = create<{ dialog: DialogState; show: (d: DialogState) => void; close: () => void }>((set) => ({
  dialog: null,
  show: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
}));

const status = (s: string) => useApp.getState().setStatus(s);

async function busy<T>(label: string, f: () => Promise<T>): Promise<T | undefined> {
  status(`${label}…`);
  try {
    const r = await track(f());
    return r;
  } catch (e) {
    status(`${label} failed: ${errorText(e)}`);
    reportError(e, `action:${label}`, { title: `${label} failed`, context: label });
    return undefined;
  }
}

// ---------- data ----------

const DATA_FILTERS = [
  { name: "GIS data", extensions: ["shp", "gpkg", "geojson", "json", "kml", "kmz", "fgb", "gml", "gpx", "csv", "tsv", "txt", "parquet", "zip", "tif", "tiff", "img", "asc", "vrt", "dem", "hgt", "jp2"] },
  { name: "All files", extensions: ["*"] },
];

/** Add files as layers (asks for them if not given); returns the new layers. */
export async function addData(paths?: string[]): Promise<LayerInfo[]> {
  if (!paths) {
    const picked = await open({ multiple: true, title: "Add Data", filters: DATA_FILTERS });
    if (!picked) return [];
    paths = Array.isArray(picked) ? picked : [picked];
  }
  const infos = await busy(`Adding ${paths.length} file(s)`, () => api.importFiles(paths!));
  if (!infos) return [];
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
  return infos;
}

const shown = (id: string) => displayInfo(useApp.getState(), id);

export async function zoomToLayer(id: string) {
  const info = shown(id);
  // With a definition query, zoom to the matching features only.
  const filtered = useApp.getState().layers[id]?.definitionQuery && info && !info.resultOf;
  zoomToExtent(filtered ? await api.layerExtent(info!.id) : (info?.extent ?? null));
}

export async function zoomToSelection() {
  const sel = useApp.getState().selection;
  let ext: [number, number, number, number] | null = null;
  for (const [id, fids] of Object.entries(sel)) {
    if (!fids.length) continue;
    const e = await api.layerExtent(shown(id)?.id ?? id, fids);
    if (!e) continue;
    ext = ext ? [Math.min(ext[0], e[0]), Math.min(ext[1], e[1]), Math.max(ext[2], e[2]), Math.max(ext[3], e[3])] : e;
  }
  if (ext) zoomToExtent(ext);
  else status("Nothing selected");
}

export async function removeLayer(id: string) {
  const ok = await busy("Removing layer", () => api.removeLayer(id));
  if (ok === undefined) return;
  const name = useApp.getState().layers[id]?.name ?? "Layer";
  useApp.getState().removeLayer(id);
  // Its own node tree goes with it; other trees that used it as an input keep
  // their Layer node, which now says the layer is gone.
  const m = useModel.getState();
  if (m.layer === id) m.openTree(null);
  const { [id]: _gone, ...trees } = useModel.getState().trees;
  const { [id]: _goneOutputs, ...lastOutputs } = useModel.getState().lastOutputs;
  useModel.setState({ trees, lastOutputs });
  void pruneSnapshots();
  const using = Object.entries(m.allTrees()).filter(([owner, t]) => owner !== id && t.nodes.some((n) => n.data.tool === "input_layer" && n.data.params.layer === id));
  const names = using.map(([owner]) => useApp.getState().layers[owner]?.name ?? owner);
  status(names.length ? `${name} removed. ${names.length === 1 ? "The node tree of" : "Node trees of"} ${names.join(", ")} used it — its Layer node there is marked.` : `${name} removed`);
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

/** Set the map CRS only if the engine can use it; an invalid one leaves the current CRS in place. */
export async function setMapCrsChecked(id: string, name: string) {
  try {
    await api.validateCrs(id);
  } catch (e) {
    status(`Coordinate system ${id} can't be used: ${errorText(e)}. The map keeps ${useApp.getState().mapCrs?.name ?? "its current setting"}.`);
    return;
  }
  useApp.getState().setMapCrs({ id, name });
  status(`Map coordinate system: ${name}. The map, coordinates, measurements and layouts use it.`);
}

export function pickMapCrs() {
  const cur = useApp.getState().mapCrs;
  useDialog.getState().show({
    kind: "crs",
    title: "Map Coordinate System",
    initial: cur?.id,
    onPick: (id, name) => void setMapCrsChecked(id, name),
  });
}

/** Add a layer of Tissot circles (R-H11) for the map CRS: each circle is round on the ground, so its shape on the map is the distortion. */
export async function addTissotCircles() {
  const crs = useApp.getState().mapCrs;
  if (!crs) {
    status("Choose a map coordinate system first");
    return;
  }
  const r = await busy("Computing Tissot circles", () => api.tissotLayer(crs.id, 15, 500));
  if (!r) return;
  const [info, skipped] = r;
  useApp.getState().addLayers([info]);
  status(`Tissot circles for ${crs.name}: ${info.featureCount} drawn${skipped ? `, ${skipped} grid points left out (outside what this coordinate system can show)` : ""}`);
}

/** Open the Split by Attribute dialog (B6). */
export function splitByAttribute(owner: string) {
  const info = shown(owner);
  if (!info || info.geomType === "Raster") {
    status("Split by Attribute works on vector layers and tables.");
    return;
  }
  useDialog.getState().show({ kind: "splitByAttribute", owner });
}

/** Run a split from the dialog: new layers join the map; files are reported by folder. Returns false on failure (the error pop-up has the details). */
export async function runSplit(owner: string, field: string, target: SplitTarget, confirmed: boolean): Promise<boolean> {
  const info = shown(owner);
  if (!info) return false;
  const r = await busy("Splitting by attribute", () => api.splitByAttribute(info.id, field, target, confirmed));
  if (!r) return false;
  if (r.layers.length) useApp.getState().addLayers(r.layers);
  const n = r.outputs.length;
  status(target.kind === "files" ? `Wrote ${n} file${n === 1 ? "" : "s"} to ${target.dir}` : `Created ${n} layer${n === 1 ? "" : "s"} from ${info.name} by ${field}`);
  return true;
}

/** Open the Export Features dialog (all or selected features). */
export function exportLayer(owner: string) {
  useDialog.getState().show({ kind: "export", owner });
}

const EXPORT_FORMATS: Record<string, string> = { gpkg: "GPKG", shp: "ESRI Shapefile", geojson: "GeoJSON", json: "GeoJSON", parquet: "PARQUET", csv: "CSV", fgb: "FlatGeobuf" };

/** Write what a layer shows (its node result if any, after its definition
 * query) to a file; `selectedOnly` writes just the selected features. */
export async function runExport(owner: string, selectedOnly: boolean) {
  const info = shown(owner);
  if (!info) return;
  const fids = selectedOnly ? (useApp.getState().selection[owner] ?? []) : null;
  const path = await save({
    title: `Export ${selectedOnly ? `${fids!.length.toLocaleString()} selected features of ` : ""}${info.name}`,
    defaultPath: `${info.name}${selectedOnly ? " selection" : ""}.${info.geomType === "Raster" ? "tif" : "gpkg"}`,
    filters: info.geomType === "Raster" ? [{ name: "GeoTIFF", extensions: ["tif"] }] : [
      { name: "GeoPackage", extensions: ["gpkg"] },
      { name: "Shapefile", extensions: ["shp"] },
      { name: "GeoJSON", extensions: ["geojson"] },
      { name: "FlatGeobuf", extensions: ["fgb"] },
      { name: "GeoParquet", extensions: ["parquet"] },
      { name: "CSV", extensions: ["csv"] },
    ],
  });
  if (!path) return;
  const format = EXPORT_FORMATS[path.split(".").pop()?.toLowerCase() ?? ""] ?? "GPKG";
  const res = await busy(`Exporting ${info.name}`, () =>
    api.runGraph({
      nodes: [
        { id: "in", tool: "input_layer", params: fids ? { layer: info.id, fids } : { layer: info.id } },
        { id: "out", tool: "export", params: { path, format } },
      ],
      edges: [{ source: "in", target: "out", targetHandle: "in" }],
    }),
  );
  const out = res?.find((r) => r.id === "out");
  const n = res?.find((r) => r.id === "in")?.rows;
  if (out) status(out.error ? `Export failed: ${out.error}` : `Exported ${n?.toLocaleString() ?? ""} feature(s) to ${path}`);
}

/** Copy the selected features into a new layer. */
export async function layerFromSelection(owner: string) {
  const s = useApp.getState();
  const info = shown(owner);
  const fids = s.selection[owner] ?? [];
  if (!info || !fids.length) {
    status("Select some features first.");
    return;
  }
  const created = await busy("Creating layer from selection", () => api.copyLayer(info.id, `${s.layers[owner]?.name ?? info.name} selection`, fids));
  if (!created) return;
  useApp.getState().addLayers([created]);
  status(`Created "${created.name}" with ${created.featureCount.toLocaleString()} feature(s)`);
}

/** Open a layer's Properties window at its Definition Query section. */
export function openDefinitionQuery(owner: string) {
  useApp.getState().setActiveLayer(owner);
  openLayerProperties(owner, "query");
}

export async function setDefinitionQuery(owner: string, query: string | null) {
  const info = await api.setDefinitionQuery(owner, query); // throws on a bad query
  useApp.getState().updateLayer(info);
  useApp.getState().setSelection(owner, []);
  status(query ? `Definition query on ${info.name}: ${info.queryCount?.toLocaleString()} of ${info.featureCount.toLocaleString()} features` : `Definition query cleared on ${info.name}`);
  return info;
}

export async function selectByAttributes(layer: string, where: string, mode: "new" | "add") {
  const fids = await busy("Selecting", () => api.selectWhere(shown(layer)?.id ?? layer, where));
  if (!fids) return false;
  useApp.getState().setSelection(layer, fids, mode);
  status(`Selected ${fids.length.toLocaleString()} feature(s)`);
  return true;
}

/**
 * Select By Location: select features of `target` by how they relate to the
 * features of `by` (only its selected features when `onlySelected`).
 */
export async function selectByLocation(
  target: string,
  by: string,
  opts: { relation: string; distance: number; units: string; invert: boolean; onlySelected: boolean; mode: SelectMode },
) {
  const s = useApp.getState();
  const byFids = opts.onlySelected ? (s.selection[by] ?? []) : null;
  const fids = await busy("Selecting by location", () =>
    api.selectByLocation(shown(target)?.id ?? target, shown(by)?.id ?? by, byFids, opts.relation, opts.distance, opts.units, opts.invert),
  );
  if (!fids) return false;
  useApp.getState().setSelection(target, fids, opts.mode);
  const n = useApp.getState().selection[target]?.length ?? 0;
  status(`${fids.length.toLocaleString()} feature(s) matched · ${n.toLocaleString()} selected in ${s.layers[target]?.name ?? "layer"}`);
  return true;
}

export async function generateTestData(kind: "points" | "polygons", count: number) {
  const bbox: [number, number, number, number] = getViewBounds() ?? [-106, 26, -94, 36];
  const info = await busy(`Generating ${count.toLocaleString()} ${kind}`, () => api.generateTestLayer(kind, count, bbox));
  if (!info) return;
  useApp.getState().addLayers([info]);
  status(`Generated ${info.featureCount.toLocaleString()} test ${kind}`);
}

// ---------- model ----------

export async function runModel(target?: string) {
  const run = useRun.getState();
  if (run.running) return;
  const owner = useModel.getState().layer;
  if (!owner) {
    status("Select a layer in Contents to open and run its node tree.");
    return;
  }
  const graph = graphPayload();
  // Each node's state as sent, to mark later edits as "changed since last run".
  const memo = new Map<string, string>();
  const sigs = Object.fromEntries(useModel.getState().nodes.map((n) => [n.id, nodeSignature(n.id, memo)]));
  run.set({ running: true, results: target ? run.results : {} });
  const t0 = performance.now();
  const res = await busy(target ? "Running to selected step" : "Running model", () => api.runGraph(graph, target));
  run.set({ running: false });
  if (!res) return;
  for (const r of res) {
    run.setResult(r);
    if (r.layer) useApp.getState().updateLayer(r.layer);
  }
  useModel.getState().setLastOutputs(Object.fromEntries(res.filter((r) => (r.status === "done" || r.status === "cached") && r.output).map((r) => [r.id, r.output!])));
  const ok = res.filter((r) => r.status === "done" || r.status === "cached").map((r) => [r.id, sigs[r.id]]);
  useRun.setState((s) => ({ ranSig: { ...s.ranSig, ...Object.fromEntries(ok) } }));
  const errors = res.filter((r) => r.status === "error");
  if (res.some((r) => r.status === "cancelled")) {
    status(`Model run cancelled. ${useApp.getState().layers[owner]?.name ?? "The layer"} still shows its last result.`);
    return;
  }
  status(
    errors.length
      ? `Node tree finished with ${errors.length} error(s). Click a red step to see why.`
      : `${useApp.getState().layers[owner]?.name ?? "Layer"} updated in ${((performance.now() - t0) / 1000).toFixed(1)}s (${res.filter((r) => r.status === "cached").length} step(s) reused from cache)`,
  );
}

/** Copy a layer's node-tree result into a new, independent layer. */
export async function saveResultAsLayer(owner: string) {
  const s = useApp.getState();
  const result = s.results[owner];
  if (!result) {
    status("Run the node tree first, then save its result.");
    return;
  }
  const name = prompt("Name for the new layer:", `${s.layers[owner]?.name ?? "Layer"} result`);
  if (!name) return;
  const info = await busy("Saving result as a new layer", () => api.copyLayer(result.id, name.trim()));
  if (!info) return;
  useApp.getState().addLayers([info]);
  status(`Saved "${info.name}" as a new layer`);
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
  camera: Camera | null;
  /** Node tree per layer id. */
  trees: Record<string, Tree>;
  /** Last run's snapshot table per layer then node (B5). Absent in older projects, which kept it in each node's data. */
  lastOutputs?: Record<string, Record<string, string>>;
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
  return {
    version: 1,
    order: s.order,
    visible: s.visible,
    styles: s.styles,
    mapCrs: s.mapCrs,
    basemap: s.basemap,
    camera: snapshotCamera(),
    trees: m.allTrees(),
    lastOutputs: m.lastOutputs,
    dock: dockSerializer?.() ?? null,
  };
}

/** Tell the engine which node snapshots the trees still reference; the table on screen is never evicted. */
export async function pruneSnapshots() {
  const t = useTableTarget.getState().target;
  const m = useModel.getState();
  await pruneNodeCache(m.lastOutputs, t ? (m.lastOutputs[t.tree]?.[t.nodeId] ?? null) : null, api);
}

export async function saveProject(saveAs = false) {
  let dir = useApp.getState().projectDir;
  if (!dir || saveAs) {
    const picked = await save({ title: "Save Project", defaultPath: "Untitled.gproj", filters: [{ name: "GIS Studio project", extensions: ["gproj"] }] });
    if (!picked) return;
    dir = picked.endsWith(".gproj") ? picked : `${picked}.gproj`;
  }
  await pruneSnapshots();
  await flushLayoutSaves();
  const ok = await busy("Saving project", () => api.saveProject(dir!, JSON.stringify(uiState())));
  if (ok === undefined) return;
  useApp.getState().set({ projectDir: dir, dirty: false });
  useRun.getState().set({ results: {} }); // cache tables are not saved
  const bib = useBibPrefs.getState().autoExport ? await exportBibliography(`${dir}/Bibliography`) : 0;
  status(`Saved ${dir}${bib ? ` · annotated bibliography (APA, MLA, Chicago, BibTeX, RIS) in Bibliography/` : ""}`);
}

/**
 * Write the project's annotated bibliography in every format (APA, MLA and
 * Chicago as Markdown + HTML; BibTeX; RIS) into `folder`. Returns how many
 * sources were written (0 if the project has none or writing failed).
 */
export async function exportBibliography(folder: string): Promise<number> {
  try {
    const entries = await api.sources();
    if (!entries.length) return 0;
    const project = folder.split(/[\\/]/).slice(-2, -1)[0]?.replace(/\.gproj$/, "") || "Untitled project";
    for (const [name, text] of Object.entries(bibliographyFiles(entries, project))) await api.saveText(`${folder}/${name}`, text);
    return entries.length;
  } catch (e) {
    status(`Saved, but the bibliography couldn't be written: ${errorText(e)}`);
    return 0;
  }
}

export async function openProject(path?: string) {
  // Only a real path counts: a click event passed in by mistake must not reach the engine.
  const picked = typeof path === "string" && path ? path : await open({ title: "Open Project", directory: true });
  if (!picked || Array.isArray(picked)) return;
  const res = await busy("Opening project", () => api.openProject(picked));
  if (!res) return;
  let ui: Partial<UiStateV1> = {};
  try {
    ui = JSON.parse(res.uiState);
  } catch {
    /* older or hand-edited project: fall back to defaults */
  }
  await loadLayouts();
  restoreState(res.layers, ui, picked);
  status(`Opened ${picked}`);
}

function restoreState(layers: LayerInfo[], ui: Partial<UiStateV1>, dir: string | null) {
  closeLayoutPanels(); // the dock layout below brings back the new project's layout tabs
  const byId = Object.fromEntries(layers.filter((l) => !l.temporary && !l.resultOf).map((l) => [l.id, l]));
  const results = Object.fromEntries(layers.filter((l) => l.resultOf && byId[l.resultOf]).map((l) => [l.resultOf!, l]));
  const order = (ui.order ?? []).filter((id) => byId[id]);
  for (const l of Object.values(byId)) if (!order.includes(l.id)) order.unshift(l.id);
  useApp.setState({
    layers: byId,
    results,
    showOriginal: {},
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
  useModel.getState().load(ui.trees ?? {}, order[0] ?? null, ui.lastOutputs ?? {});
  useRun.getState().set({ results: {} });
  // A damaged camera or dock layout must not stop the project from opening.
  try {
    if (ui.camera) restoreCamera(ui.camera);
  } catch (e) {
    status(`The saved map view could not be restored (${errorText(e)}); showing the default view.`);
  }
  try {
    if (ui.dock) dockRestorer?.(ui.dock);
  } catch (e) {
    status(`The saved window layout could not be restored (${errorText(e)}).`);
  }
}

/** Read the project's layouts from the engine; their tabs come back with the dock layout. */
async function loadLayouts() {
  try {
    useLayouts.getState().loadAll(await api.listLayouts());
  } catch (e) {
    useLayouts.getState().loadAll([]);
    reportError(e, "action:Opening project", { title: "This project's layouts could not be read", context: "reading layouts" });
  }
}

export async function newProject() {
  if (useApp.getState().dirty && !confirm("Discard unsaved changes?")) return;
  const ok = await busy("Creating project", () => api.newProject());
  if (ok === undefined) return;
  useLayouts.getState().loadAll([]);
  restoreState([], {}, null);
  status("New project");
}

/** Open the table pane on a node of the open tree; `compare` also diffs it against its input. */
export function viewNodeTable(nodeId: string, compare = false) {
  const m = useModel.getState();
  if (!m.layer) return;
  const input = compare ? defaultCompareInput({ nodes: m.nodes, edges: m.edges }, nodeId, useRun.getState().specs) : null;
  useTableTarget.getState().set({ tree: m.layer, nodeId, compareId: input });
  showPane("table");
  if (compare && !input) status("This node has no input to compare with.");
}
