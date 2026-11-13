// User-level actions shared by the ribbon, context menus and shortcuts.
import { exportGraph, gridField, type GridChoice } from "./dialogs/gridRef";
import { exportParams, formatById, formatsFor, type ExportChoices } from "./dialogs/exportFormats";
import { gridLabel, gridSizeFor, groundWidthM, usngGridStyle } from "./map/usngGrid";
import { bibliographyFiles } from "./sources/cite";
import { useBibPrefs } from "./sources/prefs";
import { track } from "./progress/progress";
import { applySettings, useSettings } from "./store/settings";
import { confirmDownloads } from "./downloads/background";
import { open, save } from "./ipc/dialogs";
import { create } from "zustand";
import { approveAuthored } from "./scripts/script";
import { reportError } from "./errors/store";
import { api, errorText, type TableExportSpec, type SplitTarget, type LayerInfo } from "./ipc/api";
import { getViewBounds, restoreCamera, type Camera, snapshotCamera, zoomToExtent } from "./map/mapController";
import { readBookmarks, useBookmarks, type Bookmark } from "./map/bookmarks";
import { loadMaps, PROJECT_VERSION, serializeMaps, type LoadedMaps, type SavedMap } from "./store/maps";
import { openLayerProperties } from "./dialogs/LayerPropertiesWindow";
import { closeLayoutPanels, getDock, showPane } from "./panes/panes";
import { followActivePanel } from "./layout/actions";
import { defaultCompareInput, pruneNodeCache, useTableTarget } from "./panes/table/history";
import { graphPayload, nodeSignature, useModel, useRun, type Tree } from "./store/model";
import { defaultStyle, displayInfo, restoredResultStyles, savedResultStyles, setFrameHook, useApp, type LayerStyle, type RasterStyle, type SelectMode } from "./store/store";
import { styleFromSymbologyNode } from "./layers/nodeStyle";
import { applyStyleParts, moveSelected, safeFileName, unionExtent, type Move, type StyleParts } from "./layers/multiselect";
import { nextProjectEpoch } from "./store/projectEpoch";
import { flushLayoutSaves, useLayouts } from "./store/layout";
import { flushStatSaves, useStats } from "./stats/store";
import { loadStatSheets } from "./stats/actions";
import { pushRecent, type StartTemplate } from "./start/recent";

// ---------- dialogs ----------

export type DialogState =
  | null
  | { kind: "crs"; title: string; initial?: string | null; onPick: (id: string, name: string) => void }
  | { kind: "defineProjection"; layer: string }
  | { kind: "selectByAttributes"; layer: string }
  | { kind: "selectByLocation"; layer: string }
  | { kind: "testData" }
  | { kind: "newTable" }
  | { kind: "settings" }
  | { kind: "benchmark" }
  | { kind: "rename"; layer: string }
  | { kind: "addField"; layer: string }
  | { kind: "calcField"; layer: string; owner: string; field?: string }
  | { kind: "calcGeometry"; layer: string; owner: string; field?: string }
  | { kind: "renameField"; layer: string; field: string }
  | { kind: "tiger" }
  | { kind: "export"; owner: string }
  | { kind: "splitByAttribute"; owner: string }
  | { kind: "exportTable"; owner: string }
  | { kind: "exportLayers"; ids: string[] }
  | { kind: "layoutSeries" }
  | { kind: "layoutPage" }
  | { kind: "nodeQuery"; nodeId: string; param: string }
  | { kind: "elevation" }
  | { kind: "nationalMap" }
  | { kind: "arcgis" }
  | { kind: "hazardsSoils" }
  | { kind: "ogc" }
  | { kind: "datumGrids" }
  | { kind: "topoMap"; preset?: string }
  | { kind: "responsibleUse" }
  | { kind: "overture" }
  | { kind: "stac" }
  | { kind: "census"; layer: string }
  | { kind: "projectionBuilder" }
  | { kind: "nodeShortcuts" }
  | { kind: "start" }
  | { kind: "styleLibrary"; layer: string };

export const useDialog = create<{ dialog: DialogState; show: (d: DialogState) => void; close: () => void }>((set) => ({
  dialog: null,
  show: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
}));

setFrameHook((mapId, ext) => zoomToExtent(ext, 16, mapId));

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
  { name: "GIS data", extensions: ["shp", "gpkg", "geojson", "json", "kml", "kmz", "fgb", "gml", "gpx", "csv", "tsv", "txt", "parquet", "xlsx", "ods", "dbf", "ndjson", "jsonl", "zip", "tif", "tiff", "img", "asc", "vrt", "dem", "hgt", "jp2"] },
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
  useApp.getState().addLayers(infos); // frames a map with no layers and no saved view on the first layer (store.addLayers)
  const noCrs = infos.filter((i) => i.geomType !== "Table" && !i.crs);
  status(
    noCrs.length
      ? `Added ${infos.length} layer(s). ${noCrs.length} without a coordinate system — use Define Projection.`
      : `Added ${infos.length} layer(s) · ${infos.reduce((n, i) => n + i.featureCount, 0).toLocaleString()} features`,
  );
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

// ---------- several layers at once (the Contents pane selection) ----------

/** The layers picked in the Contents pane, in drawing order; the active layer alone when nothing else is picked. */
export function pickedLayers(): string[] {
  const s = useApp.getState();
  const picked = s.order.filter((id) => s.selectedLayers.includes(id));
  return picked.length ? picked : s.activeLayer && s.layers[s.activeLayer] ? [s.activeLayer] : [];
}

export function zoomToLayers(ids: string[]) {
  const infos = ids.map((id) => shown(id)).filter((i): i is LayerInfo => !!i);
  const ext = unionExtent(infos);
  if (!ext) return void status("None of the selected layers has an extent to zoom to.");
  zoomToExtent(ext);
}

export function setLayersVisible(ids: string[], v: boolean) {
  const app = useApp.getState();
  for (const id of ids) if (app.layers[id]?.geomType !== "Table") app.setVisible(id, v);
  status(`${v ? "Showing" : "Hiding"} ${ids.length} layer${ids.length === 1 ? "" : "s"}`);
}

export function moveLayers(ids: string[], how: Move) {
  const app = useApp.getState();
  app.set({ order: moveSelected(app.order, ids, how), dirty: true });
}

export async function removeLayers(ids: string[]) {
  if (ids.length > 1 && !confirm(`Remove ${ids.length} layers from the project? Their source files are not touched.`)) return;
  for (const id of ids) await removeLayer(id);
  useApp.getState().setLayerSelection([], null);
}

/** Copy the symbology or the label settings of `src` onto the other layers; says which could not take them. */
export function applyStyleToLayers(src: string, ids: string[], parts: StyleParts) {
  const app = useApp.getState();
  const from = app.styles[src];
  if (!from) return;
  const notes: string[] = [];
  let done = 0;
  for (const id of ids) {
    if (id === src) continue;
    const info = app.layers[id];
    const to = app.styles[id];
    if (!info || !to) continue;
    const r = applyStyleParts(from, to, info, parts);
    if (!r) { notes.push(`${info.name}: not a vector layer`); continue; }
    app.setStyle(id, r.style);
    done++;
    if (r.note) notes.push(`${info.name}: ${r.note}`);
  }
  status(`Applied ${parts} of ${app.layers[src]?.name} to ${done} layer${done === 1 ? "" : "s"}${notes.length ? `. ${notes.join("; ")}` : ""}`);
}

/** Write each layer to its own GeoPackage (rasters GeoTIFF) in `dir`; names are made safe and never clash, so nothing is overwritten. */
export async function runExportLayers(ids: string[], dir: string): Promise<boolean> {
  const taken = new Set<string>();
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  let n = 0;
  for (const id of ids) {
    const info = shown(id);
    if (!info) continue;
    const raster = info.geomType === "Raster";
    const file = safeFileName(useApp.getState().layers[id]?.name ?? info.name, raster ? "tif" : "gpkg", taken);
    const path = `${dir.replace(/[\\/]+$/, "")}${sep}${file}`;
    const res = await busy(`Exporting ${info.name}`, () =>
      api.runGraph({
        nodes: [{ id: "in", tool: "input_layer", params: { layer: info.id } }, { id: "out", tool: "export", params: { path, format: "GPKG" } }],
        edges: [{ source: "in", target: "out", targetHandle: "in" }],
      }),
    );
    const out = res?.find((r) => r.id === "out");
    if (!out || out.error) { status(`Export of ${info.name} failed${out?.error ? `: ${out.error}` : ""}. ${n} layer(s) written before it.`); return false; }
    n++;
  }
  status(`Exported ${n} layer${n === 1 ? "" : "s"} to ${dir}`);
  return true;
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

/** Show the whole Earth in the map CRS, or go back to its area of use (R8). */
export function toggleWorldView() {
  const s = useApp.getState();
  const on = !s.worldView;
  s.setWorldView(on);
  status(on ? "World view: the whole Earth, cut at the opposite side of the central meridian. Add Graticule and World Outline from the Coordinates tab." : "Back to the area of use of the map coordinate system");
}

/** Add an MGRS grid for the map view as a layer: outlines and a reference in each square, sized for how far in you are zoomed. */
export async function addUsngGrid() {
  const bounds = getViewBounds();
  if (!bounds) return status("Zoom the map to your area first: the grid is made for the map view.");
  const size = gridSizeFor(groundWidthM(bounds));
  const info = await busy("Drawing the USNG grid", () => api.usngGridLayer(bounds, size, "mgrs", "wgs84", `MGRS grid ${gridLabel(size)}`));
  if (!info) return;
  useApp.getState().addLayers([info]);
  useApp.getState().setStyle(info.id, usngGridStyle(useApp.getState().styles[info.id], "MGRS"));
  status(`Added ${info.name}: ${info.featureCount.toLocaleString()} squares of ${gridLabel(size)}, each labelled with its MGRS reference. Zoom in and run it again for finer squares.`);
}

/** Add the graticule and the CRS's reference lines (central meridian, standard parallels, edge of the map). */
export async function addWorldReference() {
  const crs = useApp.getState().mapCrs;
  if (!crs) {
    status("Choose a map coordinate system first");
    return;
  }
  const layers = await busy("Drawing the graticule", () => api.worldReference(crs.id, 15));
  if (!layers) return;
  useApp.getState().addLayers(layers);
  status(`Added ${layers.map((l) => l.name).join(" and ")}`);
}

/** Add the bundled Natural Earth land outline (no download). */
export async function addWorldOutline() {
  const info = await busy("Adding the world outline", () => api.worldOutline());
  if (!info) return;
  useApp.getState().addLayers([info]);
  status("Added the world outline (Natural Earth 1:110m, public domain; see Sources)");
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

/** Open the Export Table dialog (point layers: attributes plus coordinates in a chosen CRS). */
export function exportTable(owner: string) {
  const info = shown(owner);
  if (!info || info.geomType !== "Point") {
    status("Export Table works on point layers. For other layers use Export Features.");
    return;
  }
  useDialog.getState().show({ kind: "exportTable", owner });
}

/** Write the table from the dialog. Returns false on failure (the error pop-up has the details). */
export async function runExportTable(owner: string, spec: TableExportSpec): Promise<boolean> {
  const info = shown(owner);
  if (!info) return false;
  const r = await busy(`Exporting the table of ${info.name}`, () => api.exportTable(info.id, spec));
  if (!r) return false;
  const coords = r.xColumn ? `${r.xColumn}/${r.yColumn} in ${r.crsName} (${r.unit})` : "";
  const grid = r.gridColumn ? `${r.gridColumn} references` : "";
  const extra = r.extraRows > 0 ? ` (${r.extraRows.toLocaleString()} more than the features, because some hold several points: one row per point)` : "";
  status(`Exported ${r.rows.toLocaleString()} row(s)${extra} to ${r.path} with ${[coords, grid].filter(Boolean).join(" and ")}; what they mean is described in ${r.sidecar}`);
  return true;
}

/** Open the Export Features dialog (all or selected features). */
export function exportLayer(owner: string) {
  useDialog.getState().show({ kind: "export", owner });
}

const EXPORT_FORMATS: Record<string, string> = { gpkg: "GPKG", shp: "ESRI Shapefile", geojson: "GeoJSON", json: "GeoJSON", parquet: "PARQUET", csv: "CSV", fgb: "FlatGeobuf", tsv: "TSV", xlsx: "XLSX", ods: "ODS", dbf: "DBF", kml: "KML", gpx: "GPX", dxf: "DXF" };
/** A table has no shapes: .json is a list of records, not GeoJSON. */
const TABLE_EXTENSIONS: Record<string, string> = { ...EXPORT_FORMATS, json: "JSON" };

/** Write what a layer shows (its node result if any, after its definition
 * query) to a file; `selectedOnly` writes just the selected features. `format` is
 * the dialog's choice (else the file name's extension decides); `choices` are its
 * coordinate system, field and clip choices. */
export async function runExport(owner: string, selectedOnly: boolean, grid: GridChoice | null = null, format?: string, choices: ExportChoices = {}) {
  const info = shown(owner);
  if (!info) return;
  const fids = selectedOnly ? (useApp.getState().selection[owner] ?? []) : null;
  const raster = info.geomType === "Raster";
  const chosen = format ? formatById(info.geomType, format) : null;
  const path = await save({
    title: `Export ${selectedOnly ? `${fids!.length.toLocaleString()} selected features of ` : ""}${info.name}`,
    defaultPath: `${info.name}${selectedOnly ? " selection" : ""}.${raster ? "tif" : chosen?.ext ?? (info.geomType === "Table" ? "csv" : "gpkg")}`,
    filters: raster ? [{ name: "GeoTIFF", extensions: ["tif"] }] : chosen ? [{ name: chosen.label.replace(/ \(.*$/, ""), extensions: [chosen.ext] }] : formatsFor(info.geomType).map((f) => ({ name: f.label.replace(/ \(.*$/, ""), extensions: [f.ext] })),
  });
  if (!path) return;
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const resolved = chosen?.id ?? (info.geomType === "Table" ? TABLE_EXTENSIONS : EXPORT_FORMATS)[ext] ?? (info.geomType === "Table" ? "CSV" : "GPKG");
  const res = await busy(`Exporting ${info.name}`, () =>
    api.runGraph(exportGraph(info.id, fids, path, resolved, raster ? null : grid, raster ? {} : exportParams(choices))),
  );
  const out = res?.find((r) => r.id === "out");
  const n = res?.find((r) => r.id === "in")?.rows;
  const gridRes = res?.find((r) => r.id === "grid");
  if (gridRes?.error) return status(`Export failed: ${gridRes.error}`);
  if (out) {
    if (out.error) {
      status(`Export failed: ${out.error}`);
      reportError(out.error, "action:Export", { title: `Export of ${info.name} failed`, context: "exporting features" });
    } else {
      // The engine's own text says what was written and what a format could not keep (renamed fields, dropped shapes, …).
      const wrote = `Wrote ${path}`;
      const note = out.message?.startsWith(wrote) ? out.message.slice(wrote.length).replace(/^\.\s*/, "") : (out.message ?? "");
      status(`Exported ${n?.toLocaleString() ?? ""} feature(s) to ${path}${grid ? ` with a ${gridField(grid.style)} column` : ""}. ${note}`.trim());
    }
  }
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

/** A Python definition query written here: approved on this computer (the user is its author), then run once by the engine. */
export async function setDefinitionPython(owner: string, script: string | null) {
  if (script) await approveAuthored(script);
  const info = await api.setDefinitionPython(owner, script);
  useApp.getState().updateLayer(info);
  useApp.getState().setSelection(owner, []);
  status(
    !script
      ? `Definition query cleared on ${info.name}`
      : info.definitionError
        ? `The script on ${info.name} failed; the layer shows no features`
        : `Definition query on ${info.name}: ${info.queryCount?.toLocaleString()} of ${info.featureCount.toLocaleString()} features`,
  );
  return info;
}

/** Select by a Python script (the user wrote it here, so it is approved first). */
export async function selectByPython(layer: string, script: string, mode: "new" | "add") {
  await approveAuthored(script);
  const fids = await busy("Selecting", () => api.selectPython(shown(layer)?.id ?? layer, script));
  if (!fids) return false;
  useApp.getState().setSelection(layer, fids, mode);
  status(`Selected ${fids.length.toLocaleString()} feature(s)`);
  return true;
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
    status("Select a layer in Map Layers to open and run its node tree.");
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
  applySymbologyNodes(owner, res);
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

/** A Symbology and Labels node that ran in the open tree restyles the layer the tree belongs to (the last one in the tree wins). */
function applySymbologyNodes(owner: string, res: { id: string; status: string }[]) {
  const app = useApp.getState();
  const base = app.styles[owner];
  if (!base) return;
  const ran = new Set(res.filter((r) => r.status === "done" || r.status === "cached").map((r) => r.id));
  const nodes = useModel.getState().nodes.filter((n) => n.data.tool === "symbology" && ran.has(n.id));
  const last = nodes.at(-1);
  if (!last) return;
  app.setStyle(owner, styleFromSymbologyNode(last.data.params, base));
  status(`Symbology and Labels node applied to ${app.layers[owner]?.name ?? "the layer"}`);
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

/** What the project file keeps besides the engine's data. Version 1 had one map in `order` / `visible` / `mapCrs` / `basemap` / `camera`. */
interface UiState {
  version: number;
  /** Version 2. */
  maps?: SavedMap[];
  activeMapId?: string;
  styles: Record<string, LayerStyle>;
  /** Raster styles the user set on node-tree results, by the layer the result belongs to. Absent in older projects and when there are none (TP-11). */
  resultRasterStyles?: Record<string, RasterStyle>;
  /** Node tree per layer id. */
  trees: Record<string, Tree>;
  /** Last run's snapshot table per layer then node (B5). Absent in older projects, which kept it in each node's data. */
  lastOutputs?: Record<string, Record<string, string>>;
  dock: unknown;
  /** Map bookmarks (UX3). Absent in older projects and when there are none. */
  bookmarks?: Bookmark[];
  // Version 1 only (read, never written):
  order?: string[];
  visible?: Record<string, boolean>;
  mapCrs?: { id: string; name: string } | null;
  basemap?: string;
  camera?: Camera | null;
}

let dockSerializer: (() => unknown) | null = null;
let mapPanelSync: (() => void) | null = null;
/** The dock registers how to make its map tabs match the project's maps (after open, new project, delete). */
export function registerMapPanelSync(fn: () => void) {
  mapPanelSync = fn;
}
let dockRestorer: ((d: unknown) => void) | null = null;
export function registerDock(serialize: () => unknown, restore: (d: unknown) => void) {
  dockSerializer = serialize;
  dockRestorer = restore;
}

function uiState(): UiState {
  const s = useApp.getState();
  const m = useModel.getState();
  // A map with a live view saves where it is; a map whose tab is closed keeps the view it had.
  const maps = s.maps.map((mp) => ({ ...mp, camera: snapshotCamera(mp.id) ?? mp.camera }));
  return {
    version: PROJECT_VERSION,
    maps: serializeMaps(maps),
    activeMapId: s.activeMapId,
    styles: s.styles,
    ...(savedResultStyles(s.resultRasterStyles) ? { resultRasterStyles: s.resultRasterStyles } : {}),
    trees: m.allTrees(),
    lastOutputs: m.lastOutputs,
    ...(useBookmarks.getState().items.length ? { bookmarks: useBookmarks.getState().items } : {}),
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
  if (!(await confirmDownloads("save"))) return;
  let dir = useApp.getState().projectDir;
  if (!dir || saveAs) {
    const picked = await save({ title: "Save Project", defaultPath: "Untitled.gproj", filters: [{ name: "GIS Studio project", extensions: ["gproj"] }] });
    if (!picked) return;
    dir = picked.endsWith(".gproj") ? picked : `${picked}.gproj`;
  }
  await pruneSnapshots();
  await flushLayoutSaves();
  await flushStatSaves();
  const ok = await busy("Saving project", () => api.saveProject(dir!, JSON.stringify(uiState())));
  if (ok === undefined) return;
  useApp.getState().set({ projectDir: dir, dirty: false });
  void clearOwnRecovery(); // the project file now holds everything the recovery copy did
  void rememberProject(dir);
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
  if (!(await confirmDownloads("leave"))) return;
  // Only a real path counts: a click event passed in by mistake must not reach the engine.
  const picked = typeof path === "string" && path ? path : await open({ title: "Open Project", directory: true });
  if (!picked || Array.isArray(picked)) return;
  const res = await busy("Opening project", () => api.openProject(picked));
  if (!res) return;
  let ui: Partial<UiState> = {};
  try {
    ui = JSON.parse(res.uiState);
  } catch {
    /* older or hand-edited project: fall back to defaults */
  }
  // Decide before anything on screen changes: a project from a newer app is refused whole, never half-read.
  const byId = projectLayers(res.layers);
  const loaded = loadMaps(ui, Object.keys(byId));
  if (!loaded.ok) {
    status(loaded.error);
    reportError(loaded.error, "action:Opening project", { title: "This project can't be opened", context: "opening a project" });
    return;
  }
  await loadLayouts();
  await loadStatSheets();
  restoreState(res.layers, ui, picked, loaded);
  void rememberProject(picked);
  status(`Opened ${picked}${loaded.notes.length ? `. ${loaded.notes.join(" ")}` : ""}`);
}

/** The project's own layers (not temporary, not node results) by id. */
const projectLayers = (layers: LayerInfo[]) => Object.fromEntries(layers.filter((l) => !l.temporary && !l.resultOf).map((l) => [l.id, l]));

function restoreState(layers: LayerInfo[], ui: Partial<UiState>, dir: string | null, loaded?: Extract<LoadedMaps, { ok: true }>) {
  void clearOwnRecovery(); // the work this run was holding was left on purpose (opened or started another project)
  nextProjectEpoch(); // a new project's layers may reuse ids and versions (U-6)
  closeLayoutPanels(); // the dock layout below brings back the new project's layout tabs
  const byId = projectLayers(layers);
  const results = Object.fromEntries(layers.filter((l) => l.resultOf && byId[l.resultOf]).map((l) => [l.resultOf!, l]));
  const { maps, activeMapId } = loaded ?? (loadMaps(ui, Object.keys(byId)) as Extract<LoadedMaps, { ok: true }>);
  const order = maps.find((m) => m.id === activeMapId)?.order ?? [];
  useApp.setState({
    layers: byId,
    results,
    showOriginal: {},
    resultRasterStyles: restoredResultStyles(ui.resultRasterStyles, Object.keys(byId)),
    selectedLayers: [],
    layerAnchor: null,
    maps,
    activeMapId,
    // Every project layer keeps a style, shown on a map or not.
    styles: Object.fromEntries(Object.keys(byId).map((id) => [id, ui.styles?.[id] ?? defaultStyle(byId[id])])),
    selection: {},
    activeLayer: order[0] ?? Object.keys(byId)[0] ?? null,
    properties: null,
    projectDir: dir,
    dirty: false,
  });
  useModel.getState().load(ui.trees ?? {}, useApp.getState().activeLayer, ui.lastOutputs ?? {});
  useBookmarks.getState().set(readBookmarks(ui.bookmarks));
  useRun.getState().set({ results: {} });
  // A damaged camera or dock layout must not stop the project from opening.
  try {
    for (const m of maps) if (m.camera) restoreCamera(m.camera, m.id);
  } catch (e) {
    status(`The saved map view could not be restored (${errorText(e)}); showing the default view.`);
  }
  try {
    if (ui.dock) dockRestorer?.(ui.dock);
  } catch (e) {
    status(`The saved window layout could not be restored (${errorText(e)}).`);
  }
  mapPanelSync?.();
  // A saved window that was in a layout or statistics workspace comes back with that tab already active: no activation event follows, so enter it now.
  const shown = getDock()?.activePanel;
  if (shown?.id.startsWith("layout:") || shown?.id.startsWith("stats:")) followActivePanel(shown.params as { layoutId?: string; statsId?: string });
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
  if (!(await confirmDownloads("leave"))) return;
  if (useApp.getState().dirty && !confirm("Discard unsaved changes?")) return;
  const ok = await busy("Creating project", () => api.newProject());
  if (ok === undefined) return;
  useLayouts.getState().loadAll([]);
  useStats.getState().loadAll([]);
  restoreState([], {}, null);
  applySettings(useSettings.getState().settings); // the default coordinate system for the new, empty map
  status("New project");
}

// ---------- start page (UX2) ----------

/** Put a project folder at the top of the recent list (kept in the per-user settings, never in a project). */
export async function rememberProject(dir: string) {
  try {
    const cur = useSettings.getState().settings.recentProjects ?? [];
    const next = pushRecent(cur, dir);
    if (next.join("\n") !== cur.join("\n")) await useSettings.getState().update({ recentProjects: next });
  } catch {
    /* the list is a convenience */
  }
}

/** Forget one recent project (its folder is not touched). */
export async function forgetProject(dir: string) {
  const cur = useSettings.getState().settings.recentProjects ?? [];
  await useSettings.getState().update({ recentProjects: cur.filter((p) => p !== dir) });
}

/** Start a new project from a start-page template (see start/templates.ts). */
export async function newProjectFromTemplate(t: StartTemplate) {
  await newProject();
  // newProject asks before discarding unsaved work: only carry on if it really started a new one.
  if (useApp.getState().projectDir !== null || useApp.getState().dirty) return;
  const m = useApp.getState().maps.find((x) => x.id === useApp.getState().activeMapId);
  if (t.crs && m) useApp.getState().setMapCrs(t.crs, m.id);
  useApp.setState({ dirty: false });
  status(`New project: ${t.name}`);
}

// ---------- autosave and crash recovery (UX1) ----------

/** Remove this run's recovery copy. Never throws: a copy that can't be removed is only offered again after a crash. */
export async function clearOwnRecovery() {
  try {
    await api.discardOwnRecovery();
  } catch {
    /* nothing to remove, or the engine isn't there */
  }
}

let autosaving = false;
let lastAutosaveError = "";

/**
 * Write the recovery copy of unsaved work, if there is any. It is a separate copy kept by the app, never the project folder, and it
 * neither marks the project saved nor changes it. Returns "saved", "skipped" (nothing unsaved, or one is already being written) or the
 * error sentence (reported once in the status bar, not on every try).
 */
export async function autosaveProject(): Promise<string> {
  if (autosaving || !useApp.getState().dirty) return "skipped";
  autosaving = true;
  try {
    await flushLayoutSaves();
    await flushStatSaves();
    const dir = useApp.getState().projectDir;
    const name = dir ? (dir.split(/[\\/]/).pop() ?? dir).replace(/\.gproj$/, "") : "Untitled project";
    await api.writeRecovery(JSON.stringify(uiState()), dir, name);
    lastAutosaveError = "";
    // Saved by the user while this was being written: the copy is stale now.
    if (!useApp.getState().dirty) await clearOwnRecovery();
    return "saved";
  } catch (e) {
    const msg = errorText(e);
    if (msg !== lastAutosaveError) status(`Autosave: ${msg}`);
    lastAutosaveError = msg;
    return msg;
  } finally {
    autosaving = false;
  }
}

/**
 * Bring back a recovery copy as unsaved work (the next Save asks where to put it). The old copy is removed only once the restored work
 * has been written to a new one, so a failure in between loses nothing. Returns whether the work was restored.
 */
export async function restoreRecovery(id: string): Promise<boolean> {
  const res = await busy("Restoring recovered work", () => api.restoreRecovery(id));
  if (!res) return false;
  let ui: Partial<UiState> = {};
  try {
    ui = JSON.parse(res.uiState);
  } catch {
    /* fall back to defaults */
  }
  const loaded = loadMaps(ui, Object.keys(projectLayers(res.layers)));
  if (!loaded.ok) {
    status(loaded.error);
    reportError(loaded.error, "action:Restoring recovered work", { title: "This recovery copy can't be used", context: "restoring recovered work" });
    return false;
  }
  await loadLayouts();
  await loadStatSheets();
  // A copy of a project with a folder reopens that project with the recovered changes unsaved; a never-saved one is untitled.
  restoreState(res.layers, ui, res.openedOrigin ? res.origin : null, loaded);
  useApp.setState({ dirty: true });
  status(
    res.openedOrigin
      ? `Restored your unsaved changes to ${res.origin}. The project's saved files are unchanged until you save.`
      : `Restored your unsaved work.${res.origin ? ` It came from ${res.origin}; that project folder was not changed.` : ""} It is not saved yet: use Save to choose where it goes.`,
  );
  if ((await autosaveProject()) === "saved") {
    try {
      await api.discardRecovery(id);
    } catch {
      /* it will be offered again; harmless */
    }
  }
  return true;
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
