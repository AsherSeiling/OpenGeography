// Typed wrappers around the Rust engine's Tauri commands.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { track } from "../progress/progress";

/** True inside the desktop app; false when the UI runs in a browser against `cargo run -p devserver`. */
export const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const DEV_SERVER = "http://127.0.0.1:7878";

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (IN_TAURI) return tauriInvoke<T>(cmd, args);
  const res = await fetch(`${DEV_SERVER}/invoke/${cmd}`, { method: "POST", body: JSON.stringify(args ?? {}) });
  const body = await res.json();
  if (!res.ok) throw body;
  return body as T;
}

export type Extent = [number, number, number, number];
export type GeomType = "Point" | "Line" | "Polygon" | "Mixed" | "Table" | "Raster";

export interface RasterInfo {
  path: string;
  width: number;
  height: number;
  bands: number;
  dataType: string;
  nodata: number | null;
  min: number;
  max: number;
  kind: "elevation" | "rgb";
  pixelSize: number;
}

export interface TigerDataset {
  id: string;
  label: string;
  needsCounty: boolean;
  /** From the chosen year (boundaries); false = TIGERweb's current data only (roads, water). */
  byYear: boolean;
}

/** A TIGERweb vintage: the year of the geography. */
export interface TigerVintage {
  id: string;
  year: number;
  label: string;
  decennial: boolean;
}

export interface TnmDataset {
  id: string;
  group: string;
  label: string;
}

/** A running engine task (download, import, model run…). */
export interface TaskInfo {
  id: number;
  label: string;
  /** 0..1, or null while the total is unknown. */
  fraction: number | null;
  detail: string;
  /** The task stops when asked (Cancel button). */
  cancellable: boolean;
}

/** A data source citation (see crates/engine/src/sources.rs). */
export interface Citation {
  key: string;
  author: string;
  year: string;
  title: string;
  version: string;
  publisher: string;
  url: string;
  kind: string;
  license: string;
  annotation: string;
  complete: boolean;
}

export interface SourceEntry {
  citation: Citation;
  /** ISO date first used in this project (the "accessed" date). */
  firstUsed: string;
  uses: { layerId: string; layerName: string; at: string }[];
}

/** File manager (Catalog pane). */
export interface FsEntry {
  name: string;
  path: string;
  kind: "folder" | "project" | "gdb" | "vector" | "raster" | "table" | "archive" | "file";
  size: number | null;
  modified: number | null;
}
export interface FsListing {
  path: string;
  parent: string | null;
  entries: FsEntry[];
}
export interface FsRoot {
  name: string;
  path: string;
  kind: "home" | "desktop" | "documents" | "downloads" | "drive" | "volume";
}
export interface FsDetails {
  kind: string;
  driver: string;
  layers: { name: string; geometry: string; features: number; crs: string | null }[];
  raster: [number, number, number] | null;
  crs: string | null;
}

export type FieldType = "text" | "integer" | "double" | "date" | "boolean";

export interface Field {
  name: string;
  type: string;
}

export interface LayerInfo {
  id: string;
  name: string;
  table: string;
  geomType: GeomType;
  crs: string | null;
  crsName: string | null;
  featureCount: number;
  extent: Extent | null;
  nativeExtent: Extent | null;
  fields: Field[];
  source: string | null;
  version: number;
  temporary: boolean;
  warnings: string[];
  /** Set on a layer's node-tree result: the layer it is drawn in place of. */
  resultOf?: string | null;
  definitionQuery?: string | null;
  overviews: number[];
  raster?: RasterInfo | null;
  queryCount?: number | null;
}

export type PortKind = "vector" | "table" | "raster" | "any";
export type ParamKind = "number" | "text" | "expression" | "enum" | "field" | "layer" | "crs" | "bool" | "savePath" | "query";

export interface ToolParam {
  id: string;
  label: string;
  kind: ParamKind;
  default: unknown;
  options: [string, string][];
  fieldOf: string | null;
  optional: boolean;
  help: string;
  hidden: boolean;
}

export interface ToolSpec {
  id: string;
  name: string;
  category: string;
  role: "data" | "tool" | "output";
  description: string;
  inputs: { id: string; label: string; kind: PortKind }[];
  output: PortKind | null;
  params: ToolParam[];
  /** How the output's fields follow from the inputs and settings (engine: nodes::OutFields). */
  fields: OutFieldsRule;
}

export interface OutFieldsRule {
  /** Input ports whose fields carry through, in order (null = the first input). */
  from: string[] | null;
  add: { name: string; type: string; nameParam: string | null; prefixParam: [string, string] | null; whenPositive: string | null }[];
  keepParam: string | null;
  noneIf: string | null;
  layerParam: string | null;
}

export type NodeStatus = "running" | "done" | "cached" | "error" | "skipped" | "cancelled";

export interface NodeResult {
  id: string;
  status: NodeStatus;
  error: string | null;
  rows: number | null;
  crs: string | null;
  output: string | null;
  message: string | null;
  layer: LayerInfo | null;
  reprojected: boolean;
  millis: number;
}

export interface RowsQuery {
  offset: number;
  limit: number;
  sort?: string | null;
  desc?: boolean;
  filter?: string | null;
  fids?: number[] | null;
}

export interface RowsPage {
  total: number;
  fids: number[];
  rows: (string | null)[][];
}

export interface LayerHits {
  layer: string;
  fids: number[];
}

export interface GraphPayload {
  nodes: { id: string; tool: string; params: Record<string, unknown> }[];
  edges: { source: string; target: string; targetHandle: string }[];
}

export const api = {
  importFiles: (paths: string[]) => invoke<LayerInfo[]>("import_files", { paths }),
  listLayers: () => invoke<LayerInfo[]>("list_layers"),
  removeLayer: (id: string) => invoke<void>("remove_layer", { id }),
  renameLayer: (id: string, name: string) => invoke<LayerInfo>("rename_layer", { id, name }),
  progress: () => invoke<TaskInfo[]>("progress"),
  fsRoots: () => invoke<FsRoot[]>("fs_roots"),
  fsList: (path: string) => invoke<FsListing>("fs_list", { path }),
  fsInspect: (path: string) => invoke<FsDetails>("fs_inspect", { path }),
  fsStat: (paths: string[]) => invoke<{ path: string; entry: FsEntry | null }[]>("fs_stat", { paths }),
  sources: () => invoke<SourceEntry[]>("sources"),
  updateSource: (citation: Citation) => invoke<void>("update_source", { citation }),
  removeSource: (key: string) => invoke<void>("remove_source", { key }),
  recordBasemap: (id: string) => invoke<void>("record_basemap", { id }),
  saveText: (path: string, text: string) => invoke<void>("save_text", { path, text }),
  cancelTask: (id: number) => invoke<void>("cancel_task", { id }),
  tnmDatasets: () => invoke<TnmDataset[]>("tnm_datasets"),
  tnmImport: (datasets: string[], bbox: Extent, clipLayer: string | null, clip: boolean, label: string) =>
    track(invoke<{ layers: LayerInfo[]; empty: string[]; failed: string[] }>("tnm_import", { datasets, bbox, clipLayer, clip, label })),
  tigerDatasets: () => invoke<TigerDataset[]>("tiger_datasets"),
  tigerVintages: () => invoke<TigerVintage[]>("tiger_vintages"),
  tigerAvailability: (vintage: string) => invoke<[string, boolean][]>("tiger_availability", { vintage }),
  tigerCounties: (state: string, vintage: string) => invoke<[string, string][]>("tiger_counties", { state, vintage }),
  tigerImport: (datasets: string[], vintage: string, state: string, county: string | null, label: string) =>
    track(invoke<LayerInfo[]>("tiger_import", { datasets, vintage, state, county, label })),
  measureFeature: (id: string, fid: number) =>
    invoke<{ area: number | null; perimeter: number | null; length: number | null; centroid: [number, number] | null }>("measure_feature", { id, fid }),
  createFeatures: (name: string, features: { wkt: string; props?: Record<string, unknown> }[]) => invoke<LayerInfo>("create_features", { name, features }),
  downloadElevation: (bbox: Extent, resolution: number, label: string) => track(invoke<LayerInfo>("download_elevation", { bbox, resolution, label })),
  rasterValue: (id: string, lon: number, lat: number) => invoke<number | null>("raster_value", { id, lon, lat }),
  copyLayer: (id: string, name: string, fids: number[] | null = null) => invoke<LayerInfo>("copy_layer", { id, name, fids }),
  setDefinitionQuery: (id: string, query: string | null) => invoke<LayerInfo>("set_definition_query", { id, query }),
  addField: (id: string, name: string, ty: FieldType) => invoke<LayerInfo>("add_field", { id, name, ty }),
  deleteField: (id: string, name: string) => invoke<LayerInfo>("delete_field", { id, name }),
  renameField: (id: string, old: string, newName: string) => invoke<LayerInfo>("rename_field", { id, old, new: newName }),
  updateCell: (id: string, fid: number, field: string, value: string | null) => invoke<LayerInfo>("update_cell", { id, fid, field, value }),
  calculateField: (id: string, field: string, expr: string, ty: FieldType | null, fids: number[] | null) =>
    invoke<{ layer: LayerInfo; count: number }>("calculate_field", { id, field, expr, ty, fids }),
  calculateGeometry: (id: string, field: string, property: string, areaUnits: string, lengthUnits: string, fids: number[] | null) =>
    invoke<{ layer: LayerInfo; count: number }>("calculate_geometry", { id, field, property, areaUnits, lengthUnits, fids }),
  deleteRows: (id: string, fids: number[]) => invoke<{ layer: LayerInfo; count: number }>("delete_rows", { id, fids }),
  defineProjection: (id: string, crs: string, crsName: string | null) =>
    invoke<LayerInfo>("define_projection", { id, crs, crsName }),
  suggestCrs: (id: string) => invoke<{ crs: string; reason: string }[]>("suggest_crs", { id }),
  validateCrs: (crs: string) => invoke<void>("validate_crs", { crs }),
  transformPoint: (x: number, y: number, from: string, to: string) =>
    invoke<[number, number]>("transform_point", { x, y, from, to }),
  getRows: (id: string, query: RowsQuery) => invoke<RowsPage>("get_rows", { id, query }),
  selectWhere: (id: string, filter: string) => invoke<number[]>("select_where", { id, filter }),
  identify: (ids: string[], lon: number, lat: number, zoom: number) =>
    invoke<LayerHits[]>("identify", { ids, lon, lat, zoom }),
  selectBox: (ids: string[], bbox: Extent) => invoke<LayerHits[]>("select_box", { ids, bbox }),
  getFeature: (id: string, fid: number) => invoke<[string, string | null][]>("get_feature", { id, fid }),
  layerExtent: (id: string, fids?: number[] | null) => invoke<Extent | null>("layer_extent", { id, fids: fids ?? null }),
  uniqueValues: (id: string, field: string, limit = 100) =>
    invoke<[string, number][]>("unique_values", { id, field, limit }),
  quantileBreaks: (id: string, field: string, classes: number) =>
    invoke<number[]>("quantile_breaks", { id, field, classes }),
  toolSpecs: () => invoke<ToolSpec[]>("tool_specs"),
  runGraph: (graph: GraphPayload, target?: string | null) =>
    invoke<NodeResult[]>("run_graph", { graph, target: target ?? null }),
  previewOutput: (output: string, crs: string | null, name: string) =>
    invoke<LayerInfo>("preview_output", { output, crs, name }),
  clearModelCache: () => invoke<void>("clear_model_cache"),
  generateTestLayer: (kind: "points" | "polygons", count: number, bbox: Extent) =>
    invoke<LayerInfo>("generate_test_layer", { kind, count, bbox }),
  saveProject: (dir: string, uiState: string) => invoke<void>("save_project", { dir, uiState }),
  openProject: (dir: string) => invoke<{ uiState: string; layers: LayerInfo[] }>("open_project", { dir }),
  newProject: () => invoke<void>("new_project"),
};

// Custom protocol base: Windows WebView2 maps custom schemes to http://<scheme>.localhost.
const TILE_BASE = !IN_TAURI ? DEV_SERVER : navigator.userAgent.includes("Windows") ? "http://gis.localhost" : "gis://localhost";

export function tileUrl(layer: LayerInfo, fields: string[]): string {
  const f = fields.length ? `&fields=${encodeURIComponent(fields.join(","))}` : "";
  const ext = layer.geomType === "Raster" ? "png" : "pbf";
  return `${TILE_BASE}/tiles/${layer.id}/{z}/{x}/{y}.${ext}?v=${layer.version}${f}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
