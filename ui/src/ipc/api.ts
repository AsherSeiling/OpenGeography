// Typed wrappers around the Rust engine's Tauri commands.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

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
export type GeomType = "Point" | "Line" | "Polygon" | "Mixed" | "Table";

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
}

export type PortKind = "vector" | "table";
export type ParamKind = "number" | "text" | "expression" | "enum" | "field" | "layer" | "crs" | "bool" | "savePath";

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
}

export type NodeStatus = "running" | "done" | "cached" | "error" | "skipped";

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
  copyLayer: (id: string, name: string) => invoke<LayerInfo>("copy_layer", { id, name }),
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
  return `${TILE_BASE}/tiles/${layer.id}/{z}/{x}/{y}.pbf?v=${layer.version}${f}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
