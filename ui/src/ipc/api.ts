// Typed wrappers around the Rust engine's Tauri commands.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { track } from "../progress/progress";
import type { Layout, LayoutPreview, RenderRequest } from "../layout/types";

/** True inside the desktop app; false when the UI runs in a browser against `cargo run -p devserver`. */
export const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const DEV_SERVER: string = import.meta.env.VITE_GIS_DEVSERVER ?? "http://127.0.0.1:7878";

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
  /** Every cell is NoData (R5). */
  empty?: boolean;
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
export interface FontFamily {
  name: string;
  bold: boolean;
  italic: boolean;
  monospace: boolean;
}
export interface TableExportSpec {
  path: string;
  format: "csv" | "parquet";
  crs: string;
  fids?: number[] | null;
}
export interface TableExportResult {
  path: string;
  sidecar: string;
  rows: number;
  xColumn: string;
  yColumn: string;
  crsName: string;
  unit: string;
}

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

/** Tile grid of a map CRS (engine `crs_display_info`). Resolutions are CRS units per pixel for z = 0..22. */
export interface CrsGrid {
  extent: [number, number, number, number];
  origin: [number, number];
  resolutions: number[];
  tileSize: number;
}

/** What a 3DEP download of a box will be (tnm::ElevationPlan). */
export interface ElevationPlan {
  requestedRes: number;
  usedRes: number;
  cols: number;
  rows: number;
  bytes: number;
  coarsened: boolean;
  widthM: number;
  heightM: number;
  crs: string;
  tilesX: number;
  tilesY: number;
  needsConfirm: boolean;
  notes: string[];
}

export interface CensusDataset {
  id: string;
  label: string;
  path: string;
  title: string;
}
export interface CensusSpec {
  dataset: string;
  vintage: number;
  level?: "state" | "county" | "tract" | "block_group";
  variables: string[];
}
export interface CensusJoinResult {
  layer: LayerInfo;
  matched: number;
  unmatched: number;
  suppressed: number;
  notes: string[];
}

export interface StacCollection {
  id: string;
  label: string;
  cloud: boolean;
  assets: [string, string][];
  author: string;
  license: string;
  annotation: string;
}
export interface StacApi {
  id: string;
  label: string;
  url: string;
  collections: StacCollection[];
}
export interface StacAsset {
  key: string;
  title: string;
  href: string;
}
export interface StacItem {
  id: string;
  collection: string;
  datetime: string | null;
  cloudCover: number | null;
  assets: StacAsset[];
}

export interface OvertureTheme {
  id: string;
  label: string;
  theme: string;
  kind: string;
}

export interface OgcEntry {
  id: string;
  group: string;
  label: string;
  api: string;
  collection: string;
  author: string;
  license: string;
  annotation: string;
}
export interface OgcCollection {
  id: string;
  title: string;
}

export interface ArcgisEntry {
  id: string;
  group: string;
  label: string;
  service: string;
  layers: string[];
  author: string;
  year: string;
  title: string;
  publisher: string;
  url: string;
  license: string;
  annotation: string;
}
export interface ArcgisServiceLayer {
  id: number;
  name: string;
  geometryType: string | null;
}
export interface ArcgisSpec {
  entryId?: string;
  url?: string;
  layer?: string;
  label?: string;
}

export interface CrsInfo {
  name: string;
  code: string | null;
  geographic: boolean;
  unit: string;
  datum: string | null;
  /** [west, south, east, north] degrees, null when PROJ has no area of use. */
  areaOfUse: [number, number, number, number] | null;
}

export interface CrsDisplayInfo {
  /** "3857" for Web Mercator, else a 12-hex-digit key of the CRS. */
  crsKey: string;
  proj4: string;
  geographic: boolean;
  /** Metres per CRS unit (per degree at the equator when geographic). */
  unitM: number;
  grid: CrsGrid;
  /** [west, south, east, north] degrees. */
  areaOfUse: [number, number, number, number];
  /** False when PROJ has no area of use and the grid came from the fallback extent. */
  areaOfUseKnown: boolean;
  /** Accuracy (m) of PROJ's best transform to WGS 84: how far an online basemap can sit from data in this CRS. Null when unknown. */
  basemapOffsetM: number | null;
  /** Present in world view (crs id prefixed "world:"): the whole Earth cut at lon0 ± 180°. */
  world?: { lon0: number; latSouth: number; latNorth: number; note: string | null };
}

export interface DisplayStatus {
  /** Tiles in this CRS can be served now. */
  ready: boolean;
  building: boolean;
  /** Features wholly outside the CRS's area of use that aren't drawn. */
  dropped: number;
  /** Why the last build failed. */
  error: string | null;
}

/** Known-answer check of PROJ's datum files (R3): `ok` is false when NAD83 / NAD27 / WGS 84 shifts can't be trusted. */
export interface DatumReport {
  ok: boolean;
  problems: string[];
  projData: string | null;
}

/** One page of a layout's map series (C3.2). */
export interface SeriesPageInfo {
  index: number;
  fid: number;
  name: string;
}
export interface SeriesExportResult {
  pages: number;
  files: string[];
  warnings: string[];
}

/** A bundled or user layout template (C3.1). */
export interface LayoutTemplate {
  id: string;
  name: string;
  builtin: boolean;
  width: number;
  height: number;
}

/** One value of a field and its row count; NULL is "(no value)" (B6). */
export interface SplitGroup {
  value: string;
  rows: number;
}
export type SplitTarget = { kind: "layers" } | { kind: "files"; dir: string; format: "GPKG" | "ESRI Shapefile" };
export interface SplitResult {
  layers: LayerInfo[];
  outputs: { value: string; rows: number; name: string }[];
  skippedRows: number;
  notes: string[];
}
/** Splitting asks for a confirmation above this many outputs and refuses more than the limit. */
export const SPLIT_CONFIRM_ABOVE = 200;
export const SPLIT_MAX_OUTPUTS = 1000;

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
export type ParamKind = "number" | "text" | "expression" | "enum" | "field" | "layer" | "crs" | "bool" | "color" | "savePath" | "query";

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

export interface NodeTableInfo {
  fields: { name: string; type: string }[];
  rows: number;
  crs: string | null;
  hasGeom: boolean;
  /** Layer Input snapshots are views. */
  kind: "table" | "view";
}

export type DiffKind = "added" | "removed" | "changed";

export interface DiffSummary {
  diffId: string;
  keyUsed: string | null;
  keySource: "user" | "auto" | "none";
  note: string | null;
  /** Field order of the rows `nodeDiffRows` returns (a trailing "geom" when geometry is compared). */
  fields: string[];
  fieldsAdded: string[];
  fieldsRemoved: string[];
  fieldsRetyped: { name: string; from: string; to: string }[];
  rowsA: number;
  rowsB: number;
  added: number;
  removed: number;
  changed: number;
  unchanged: number;
  geometryCompared: boolean;
}

export interface DiffRow {
  kind: DiffKind;
  key: string | null;
  values: { field: string; a: string | null; b: string | null; changed: boolean }[];
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
  selectByLocation: (target: string, by: string, byFids: number[] | null, relation: string, distance: number, units: string, invert: boolean) =>
    invoke<number[]>("select_by_location", { target, by, byFids, relation, distance, units, invert }),
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
  censusDatasets: () => invoke<CensusDataset[]>("census_datasets"),
  censusKeyStatus: () => invoke<boolean>("census_key_status"),
  /** Store the key in the OS keychain; it is never returned. */
  censusKeySet: (key: string) => invoke<void>("census_key_set", { key }),
  censusKeyDelete: () => invoke<void>("census_key_delete"),
  censusJoin: (layer: string, geoidField: string, spec: CensusSpec) => track(invoke<CensusJoinResult>("census_join", { layer, geoidField, spec })),
  stacCatalog: () => invoke<StacApi[]>("stac_catalog"),
  stacSearch: (spec: { collection: string; bbox: Extent; datetime?: string; maxCloud?: number; limit?: number }) => invoke<StacItem[]>("stac_search", { spec }),
  stacImport: (spec: { href: string; bbox: Extent; label: string; collection: string; itemId: string; asset: string; areaLabel: string }) => track(invoke<LayerInfo>("stac_import", { spec })),
  overtureThemes: () => invoke<OvertureTheme[]>("overture_themes"),
  overtureImport: (theme: string, bbox: Extent, label: string) => track(invoke<LayerInfo>("overture_import", { theme, bbox, label })),
  ogcCatalog: () => invoke<OgcEntry[]>("ogc_catalog"),
  ogcCollections: (url: string) => invoke<OgcCollection[]>("ogc_collections", { url }),
  ogcImport: (spec: { entryId?: string; api?: string; collection?: string; label?: string }, bbox: Extent, label: string) => track(invoke<LayerInfo | null>("ogc_import", { spec, bbox, label })),
  earthquakesImport: (bbox: Extent, start: string | null, end: string | null, minMag: number, label: string) => track(invoke<LayerInfo | null>("earthquakes_import", { bbox, start, end, minMag, label })),
  ssurgoImport: (bbox: Extent, label: string) => track(invoke<LayerInfo | null>("ssurgo_import", { bbox, label })),
  arcgisCatalog: () => invoke<ArcgisEntry[]>("arcgis_catalog"),
  arcgisLayers: (url: string) => invoke<ArcgisServiceLayer[]>("arcgis_layers", { url }),
  arcgisImport: (spec: ArcgisSpec, bbox: Extent, label: string) => track(invoke<LayerInfo[]>("arcgis_import", { spec, bbox, label })),
  planElevation: (bbox: Extent, resolution: number) => invoke<ElevationPlan>("plan_elevation", { bbox, resolution }),
  downloadElevation: (bbox: Extent, resolution: number, label: string, confirmed = false) => track(invoke<LayerInfo>("download_elevation", { bbox, resolution, label, confirmed })),
  rasterRamps: () => invoke<RampInfo[]>("raster_ramps"),
  rasterHistogram: (id: string, band: number, bins: number) => invoke<RasterHistogram>("raster_histogram", { id, band, bins }),
  rasterValue: (id: string, lon: number, lat: number) => invoke<number | null>("raster_value", { id, lon, lat }),
  copyLayer: (id: string, name: string, fids: number[] | null = null) => invoke<LayerInfo>("copy_layer", { id, name, fids }),
  listFonts: () => invoke<FontFamily[]>("list_fonts"),
  exportTable: (id: string, spec: TableExportSpec) => invoke<TableExportResult>("export_table", { id, spec }),
  layoutSeriesPages: (layout: Layout) => invoke<SeriesPageInfo[]>("layout_series_pages", { layout }),
  exportLayoutSeries: (req: RenderRequest, format: string, dpi: number, path: string) => invoke<SeriesExportResult>("export_layout_series", { req, format, dpi, path }),
  datumDataStatus: () => invoke<DatumReport>("datum_data_status"),
  listLayoutTemplates: () => invoke<{ templates: LayoutTemplate[]; skipped: string[] }>("list_layout_templates"),
  getLayoutTemplate: (id: string) => invoke<Layout>("get_layout_template", { id }),
  saveLayoutTemplate: (name: string, layout: Layout) => invoke<LayoutTemplate>("save_layout_template", { name, layout }),
  deleteLayoutTemplate: (id: string) => invoke<void>("delete_layout_template", { id }),
  splitGroups: (id: string, field: string) => invoke<SplitGroup[]>("split_groups", { id, field }),
  splitByAttribute: (id: string, field: string, target: SplitTarget, confirmed: boolean) => invoke<SplitResult>("split_by_attribute", { id, field, target, confirmed }),
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
  crsInfo: (crs: string) => invoke<CrsInfo>("crs_info", { crs }),
  /** [k, convergence in degrees] of `crs` at (x, y) in that CRS. */
  mapFactors: (crs: string, x: number, y: number) => invoke<[number, number]>("map_factors", { crs, x, y }),
  /** Tissot circles for `crs` as a WGS 84 layer; [layer, grid points the CRS cannot represent]. */
  tissotLayer: (crs: string, spacingDeg: number, radiusKm: number) => invoke<[LayerInfo, number]>("tissot_layer", { crs, spacingDeg, radiusKm }),
  sameCrs: (a: string, b: string) => invoke<boolean>("same_crs", { a, b }),
  /** The bundled Natural Earth 110m land polygons as a layer (cited in Sources). */
  worldOutline: () => invoke<LayerInfo>("world_outline"),
  /** [graticule, reference lines] layers for `crs`. */
  worldReference: (crs: string, spacingDeg: number) => invoke<LayerInfo[]>("world_reference", { crs, spacingDeg }),
  validateCrs: (crs: string) => invoke<void>("validate_crs", { crs }),
  transformPoint: (x: number, y: number, from: string, to: string) =>
    invoke<[number, number]>("transform_point", { x, y, from, to }),
  /** Point scale factor k of `crs` at (x, y) in that CRS (ground = grid / k); the status-bar `1:N` uses it. */
  mapScaleFactor: (crs: string, x: number, y: number) => invoke<number>("map_scale_factor", { crs, x, y }),
  /** Grid, proj4 string and key for drawing the map in `crs`. `fallbackExtent` ([w, s, e, n] degrees) is used only when the CRS has no area of use. */
  crsDisplayInfo: (crs: string, fallbackExtent?: [number, number, number, number]) =>
    invoke<CrsDisplayInfo>("crs_display_info", { crs, fallbackExtent: fallbackExtent ?? null }),
  /** Start projecting a layer for display in `crs` (background task); poll `displayStatus`. */
  buildDisplay: (layer: string, crs: string) => invoke<DisplayStatus>("build_display", { layer, crs }),
  displayStatus: (layer: string, crs: string) => invoke<DisplayStatus>("display_status", { layer, crs }),
  getRows: (id: string, query: RowsQuery) => invoke<RowsPage>("get_rows", { id, query }),
  selectWhere: (id: string, filter: string) => invoke<number[]>("select_where", { id, filter }),
  /** Features within `tolerance` of (x, y); both are in `crs` (the map CRS) units, nearest first. */
  identify: (ids: string[], x: number, y: number, crs: string, tolerance: number) =>
    invoke<LayerHits[]>("identify", { ids, x, y, crs, tolerance }),
  /** Features touching a map-CRS box [xmin, ymin, xmax, ymax]; the engine follows the box's curved edges in other CRSs. */
  selectRect: (ids: string[], bbox: Extent, crs: string) => invoke<LayerHits[]>("select_rect", { ids, bbox, crs }),
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
  /** Drop node snapshots the UI no longer shows; returns how many were dropped. */
  pruneNodeCache: (live: string[], pinned?: string | null) =>
    invoke<number>("prune_node_cache", { live, pinned: pinned ?? null }),
  setNodeCacheBudget: (bytes: number) => invoke<void>("set_node_cache_budget", { bytes }),
  nodeTableInfo: (table: string) => invoke<NodeTableInfo>("node_table_info", { table }),
  nodeRows: (table: string, query: RowsQuery) => invoke<RowsPage>("node_rows", { table, query }),
  /** Compare two node snapshots; `key` = match rows by this field (else auto-detected). */
  nodeDiff: (a: string, b: string, key?: string | null) =>
    invoke<DiffSummary>("node_diff", { a, b, key: key ?? null }),
  /** `kind` = added | removed | changed | all. */
  nodeDiffRows: (diffId: string, kind: DiffKind | "all", offset: number, limit: number) =>
    invoke<DiffRow[]>("node_diff_rows", { diffId, kind, offset, limit }),
  dropNodeDiff: (diffId: string) => invoke<void>("drop_node_diff", { diffId }),
  generateTestLayer: (kind: "points" | "polygons", count: number, bbox: Extent) =>
    invoke<LayerInfo>("generate_test_layer", { kind, count, bbox }),
  saveProject: (dir: string, uiState: string) => invoke<void>("save_project", { dir, uiState }),
  openProject: (dir: string) => invoke<{ uiState: string; layers: LayerInfo[] }>("open_project", { dir }),
  newProject: () => invoke<void>("new_project"),
  listLayouts: () => invoke<Layout[]>("list_layouts"),
  saveLayout: (layout: Layout) => invoke<void>("save_layout", { layout }),
  deleteLayout: (id: string) => invoke<void>("delete_layout", { id }),
  /** A PNG of the page (base64) at `dpi`, plus the render's warnings. */
  renderLayoutPreview: (req: RenderRequest, dpi: number) => invoke<LayoutPreview>("render_layout_preview", { req, dpi }),
  /** Write the page to `path` as "svg" | "pdf" | "png"; returns the render's warnings. Cancellable (progress task). */
  exportLayout: (req: RenderRequest, format: "svg" | "pdf" | "png", dpi: number, path: string) =>
    track(invoke<string[]>("export_layout", { req, format, dpi, path })),
};

export interface RampInfo {
  name: string;
  group: "Scientific" | "Terrain" | "Sequential" | "Diverging" | "Qualitative" | "Gray";
  colors: string[];
  discrete: boolean;
}

export interface RasterHistogram {
  min: number;
  max: number;
  mean: number;
  std: number;
  p2: number;
  p98: number;
  counts: number[];
}

// Custom protocol base: Windows WebView2 maps custom schemes to http://<scheme>.localhost.
const TILE_BASE = !IN_TAURI ? DEV_SERVER : navigator.userAgent.includes("Windows") ? "http://gis.localhost" : "gis://localhost";

/** Tile URL template. `crsKey` (from `crsDisplayInfo`) selects a map CRS's grid; without it, or with "3857", tiles are Web Mercator. */
export function tileUrl(layer: LayerInfo, fields: string[], crsKey?: string): string {
  const f = fields.length ? `&fields=${encodeURIComponent(fields.join(","))}` : "";
  const ext = layer.geomType === "Raster" ? "png" : "pbf";
  const key = crsKey && crsKey !== "3857" ? `${crsKey}/` : "";
  return `${TILE_BASE}/tiles/${layer.id}/${key}{z}/{x}/{y}.${ext}?v=${layer.version}${f}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
