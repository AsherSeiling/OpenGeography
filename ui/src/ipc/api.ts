// Typed wrappers around the Rust engine's Tauri commands.
import type { RasterStyle } from "../store/store";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { track } from "../progress/progress";
import type { Layout, LayoutPreview, RenderRequest } from "../layout/types";
import type { AdjustMethod, AdjustReport, AdjustRequest, AdjustResult, ControlSet, ErrorReport, ImageInfo, ParsedControlFile, Resampling, WarpMethod, WarpPreview, WarpRequest } from "../georef/types";

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

/** Go To Coordinates (RB4): what one panel box set holds, as text, and what the engine reads from it. */
export interface LocationRequest {
  kind: "latlon" | "xy" | "utm" | "usng" | "crs";
  lat?: string;
  lon?: string;
  x?: string;
  y?: string;
  zone?: string;
  hemisphere?: string;
  easting?: string;
  northing?: string;
  text?: string;
  datum?: string;
  crs?: string;
}
export interface LocationResult {
  lon: number;
  lat: number;
  mapX: number | null;
  mapY: number | null;
  mapCrs: string | null;
  /** Why the map can't show the place (no coordinate system, or outside what the CRS can draw). */
  mapMessage: string | null;
  reading: string;
  formats: { id: string; label: string; text: string }[];
}
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
export interface GridRefSpec {
  style: "usng" | "mgrs";
  /** Digits per axis: 0 (100 km square) to 5 (1 m). */
  precision: number;
  datum: "auto" | "nad83" | "wgs84";
}
export interface TableExportSpec {
  path: string;
  format: "csv" | "parquet";
  /** The coordinate system of the X/Y columns; empty when only a grid reference is written. */
  crs: string;
  gridRef?: GridRefSpec | null;
  fids?: number[] | null;
}
/** The UTM system USNG/MGRS use at a place. */
export interface UsngCrs {
  id: string;
  name: string;
  zone: number;
  north: boolean;
}
export interface TableExportResult {
  path: string;
  sidecar: string;
  rows: number;
  /** Rows beyond one per feature (a MultiPoint is written one row per point). */
  extraRows: number;
  xColumn: string;
  yColumn: string;
  crsName: string;
  unit: string;
  gridColumn?: string | null;
}

export interface ScriptTrust {
  hash: string;
  approved: boolean;
  warning: string;
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
/** The user's program settings (UR-16): stored once per user by the engine, never in a project, never a secret. */
export interface ProgramSettings {
  theme?: "light" | "dark" | null;
  lengthUnit?: string | null;
  areaUnit?: string | null;
  defaultCrs?: { id: string; name: string } | null;
  /** The dock arrangement new projects start with (UR-17). */
  startupLayout?: unknown;
  scriptingLanguage?: string | null;
  /** The one format the status bar shows the pointer in: dd, dms, projected or mgrs (UX14). */
  coordinateFormat?: string | null;
  /** Show the Developer group on the View tab (Frame Rate, Benchmark, Generate Test Data). Off by default. */
  showDeveloperTools?: boolean | null;
}

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
  /** A sentence about the unit when it is easy to confuse (the two kinds of foot). */
  unitNote: string | null;
}

/** How accurate moving coordinates between two coordinate systems is (CRS accuracy plan). */
export interface TransformReport {
  /** exact (projection only), precise (grid/published operation, up to 2 m), approximate (rougher), ballpark (no accurate shift exists). */
  level: "exact" | "precise" | "approximate" | "ballpark";
  accuracyM: number | null;
  warn: boolean;
  /** The precise shift for this area needs a datum grid that is not installed. */
  gridMissing: boolean;
  text: string;
}

/** One text slot of a layout element (engine `layout/slots.rs`). */
export interface TopoSpec {
  /** Longitude, latitude of the sheet's centre. */
  center: [number, number];
  scale: number;
  paper: "letter" | "tabloid" | "a3" | "a4" | "a2" | "a1";
  landscape: boolean;
  units: "imperial" | "metric";
  datum: "nad83" | "wgs84";
  title: string;
  /** YYYY-MM-DD for the declination; empty = today. */
  date: string;
  /** "landnav" (default), "usgs" (classic quadrangle) or "military" (installation map). */
  style?: "landnav" | "usgs" | "military";
  /** "classic" (default) or "modern": the look of the sheet's symbols. */
  era?: "classic" | "modern";
}

export interface TopoCheck {
  level: "ok" | "note" | "warning";
  text: string;
}

export interface TopoBuild {
  layout: Layout;
  checks: TopoCheck[];
  crs: string;
  zone: number;
  groundM: [number, number];
  contourInterval: string;
  /** The same interval in metres (the unit of 3DEP elevation). */
  contourIntervalM: number;
  /** West, south, east, north of the map frame plus a margin: the area to download elevation for. */
  bboxLonlat: [number, number, number, number];
}

export interface TopoData {
  contours: LayerInfo;
  hillshade: LayerInfo;
}

export interface TextSlot {
  kind: string;
  id: string;
  label: string;
  /** Default size in points; 0 = automatic (from the element's box). */
  defaultPt: number;
  bold: boolean;
  /** The element's own size field sets this slot's base size (`size`, `textSize`). */
  legacySize: boolean;
}

/** One datum grid of the PROJ-data catalog (Datum Grids menu). */
export interface DatumGrid {
  name: string;
  country: string;
  continent: string;
  source: string;
  sourceId: string;
  /** horizontal, vertical, 3d, deformation or other. */
  kind: "horizontal" | "vertical" | "3d" | "deformation" | "other";
  area: string;
  from: string;
  to: string;
  description: string;
  size: number;
  sha256: string;
  url: string;
  /** Shipped in the installer. */
  bundled: boolean;
  installed: boolean;
  /** "app", "downloaded" or "". */
  location: string;
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

/** One edit in a batch (shape editing): shapes are hex WKB in the MAP's CRS. */
export interface EditOp {
  op: "insert" | "update" | "delete";
  fid?: number;
  geometry?: string;
  attributes?: Record<string, unknown>;
  fix?: boolean;
}
export interface EditStatus {
  layer: string;
  editing: boolean;
  mapCrs: string | null;
  zValue: number | null;
  /** The elevation raster new vertices take Z from (wins over zValue), or null. */
  zDem: string | null;
  added: number;
  changed: number;
  deleted: number;
  steps: number;
  canUndo: boolean;
  canRedo: boolean;
  invalid: number[];
  canUndoSave: boolean;
  layerHasZ: boolean;
  layerHasM: boolean;
}
export interface EditFeature {
  fid: number;
  geometry: string | null;
  attributes: Record<string, unknown>;
  pending: "inserted" | "changed" | null;
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
  /** A Python definition query; the layer has no features until it is approved on this computer. */
  definitionPython?: string | null;
  /** Why the Python definition query gave nothing (failed rows, cancelled). */
  definitionError?: string | null;
  /** A shape-edit session is open on this layer. */
  editing?: boolean;
  /** A Python label expression; labels use the field `__label`. */
  labelPython?: string | null;
  labelError?: string | null;
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
  /** Plain-language search words for the Add menu (engine: nodes::keywords). */
  keywords?: string[];
  /** How a raster this tool makes looks by default: "derived" (new values: colour ramp over the range), "source" (same kind of values: keeps the source's renderer) or "gray" (a shade). */
  resultLook?: "derived" | "source" | "gray";
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
  /** Is this exact script approved on this computer? `warning` is the text to show when asking. */
  scriptTrustInfo: (script: string) => invoke<ScriptTrust>("script_trust_info", { script }),
  /** Record the user's approval of this exact script (app data on this computer, never the project). */
  approveScript: (script: string) => invoke<ScriptTrust>("approve_script", { script }),
  pythonStatus: () => invoke<{ available: boolean; path: string | null }>("python_status"),
  getSettings: () => invoke<ProgramSettings>("get_settings"),
  setSettings: (settings: ProgramSettings) => invoke<ProgramSettings>("set_settings", { settings }),
  updateSource: (citation: Citation) => invoke<void>("update_source", { citation }),
  /** Give a layer a citation the user wrote (a form, or pasted text with kind "Pasted citation"); `accessed` is YYYY-MM-DD (default today). */
  addLayerSource: (layer: string, citation: Citation, accessed?: string) => invoke<Citation>("add_layer_source", { layer, citation, accessed: accessed ?? null }),
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
  rasterClassify: (id: string, band: number, method: "equal" | "quantile" | "jenks" | "stddev", classes: number) => invoke<number[]>("raster_classify", { id, band, method, classes }),
  rasterUniqueValues: (id: string, band: number, max: number) => invoke<RasterUniqueValues>("raster_unique_values", { id, band, max }),
  saveRasterStyle: (path: string, style: RasterStyle) => invoke<void>("save_raster_style", { path, style }),
  loadRasterStyle: (path: string) => invoke<RasterStyle>("load_raster_style", { path }),
  /** The topographic symbol catalogue (USGS sheet + military overlay); `era` = "classic" | "modern". */
  topoSymbols: (era: "classic" | "modern") => invoke<TopoSymbol[]>("topo_symbols", { era }),
  /** A small SVG sample of one symbol, drawn by the layout renderer. */
  topoSymbolSvg: (id: string, era: "classic" | "modern") => invoke<string>("topo_symbol_svg", { id, era }),
  layoutThemes: () => invoke<LayoutTheme[]>("layout_themes"),
  applyLayoutTheme: (layout: Layout, theme: string) => invoke<Layout>("apply_layout_theme", { layout, theme }),
  /** A PNG/JPEG/SVG file as the data URI a layout Picture stores. */
  readPicture: (path: string) => invoke<string>("read_picture", { path }),
  rasterHistogram: (id: string, band: number, bins: number) => invoke<RasterHistogram>("raster_histogram", { id, band, bins }),
  rasterValue: (id: string, lon: number, lat: number) => invoke<number | null>("raster_value", { id, lon, lat }),
  copyLayer: (id: string, name: string, fids: number[] | null = null) => invoke<LayerInfo>("copy_layer", { id, name, fids }),
  listFonts: () => invoke<FontFamily[]>("list_fonts"),
  exportTable: (id: string, spec: TableExportSpec) => invoke<TableExportResult>("export_table", { id, spec }),
  /** The Web Mercator rectangle covering `extent` of the map CRS `key` (null = nothing there can be placed in Web Mercator). */
  basemapMercatorExtent: (key: string, extent: [number, number, number, number]) => invoke<[number, number, number, number] | null>("basemap_mercator_extent", { key, extent }),
  /** A picture drawn in Web Mercator over `merc` (base64 PNG, RGBA) warped to `extent` of the map CRS `key`, `w` × `h` px; base64 PNG back. */
  warpBasemapPicture: (png: string, merc: [number, number, number, number], key: string, extent: [number, number, number, number], w: number, h: number) =>
    invoke<string>("warp_basemap_picture", { png, merc, key, extent, w, h }),
  /** Ground per millimetre of a map frame, in its CRS units: [east, north]. */
  layoutFrameUnitsPerMm: (body: string, crs: string, center: [number, number], scale: number, w: number, h: number) => invoke<[number, number]>("layout_frame_units_per_mm", { body, crs, center, scale, w, h }),
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
  /** A Python definition query (`def calculate(row): return True`): runs once, not at draw time; the layer shows nothing until the script is approved. */
  setDefinitionPython: (id: string, script: string | null) => invoke<LayerInfo>("set_definition_python", { id, script }),
  /** A Python label expression (`def calculate(row): return "text"`): run once, labels read it as the field `__label`. */
  setLabelPython: (id: string, script: string | null) => invoke<LayerInfo>("set_label_python", { id, script }),
  calculateFieldPython: (id: string, field: string, script: string, ty: FieldType | null, fids: number[] | null, skipFailed: boolean) =>
    invoke<{ layer: LayerInfo; count: number; note: string | null }>("calculate_field_python", { id, field, script, ty, fids, skipFailed }),
  selectPython: (id: string, script: string) => invoke<number[]>("select_python", { id, script }),
  addField: (id: string, name: string, ty: FieldType) => invoke<LayerInfo>("add_field", { id, name, ty }),
  deleteField: (id: string, name: string) => invoke<LayerInfo>("delete_field", { id, name }),
  renameField: (id: string, old: string, newName: string) => invoke<LayerInfo>("rename_field", { id, old, new: newName }),
  updateCell: (id: string, fid: number, field: string, value: string | null) => invoke<LayerInfo>("update_cell", { id, fid, field, value }),
  calculateField: (id: string, field: string, expr: string, ty: FieldType | null, fids: number[] | null) =>
    invoke<{ layer: LayerInfo; count: number }>("calculate_field", { id, field, expr, ty, fids }),
  calculateGeometry: (id: string, field: string, property: string, areaUnits: string, lengthUnits: string, fids: number[] | null) =>
    invoke<{ layer: LayerInfo; count: number }>("calculate_geometry", { id, field, property, areaUnits, lengthUnits, fids }),
  createTable: (name: string, fields: [string, string][]) => invoke<LayerInfo>("create_table", { name, fields }),
  addRow: (id: string) => invoke<{ layer: LayerInfo; fid: number }>("add_row", { id }),
  // ---- shape editing (the map's CRS goes with every call that carries shapes) ----
  editSetZ: (layer: string, zValue: number | null, zDem: string | null) => invoke<EditStatus>("edit_set_z", { layer, zValue, zDem }),
  editBegin: (layer: string, mapCrs: string, zValue: number | null) => invoke<EditStatus>("edit_begin", { layer, mapCrs, zValue }),
  editApply: (layer: string, mapCrs: string, ops: EditOp[]) => invoke<{ status: EditStatus; fids: number[]; invalid: number[]; zNote: string | null }>("edit_apply", { layer, mapCrs, ops }),
  snapFeaturesIn: (layer: string, mapCrs: string, extent: [number, number, number, number], limit?: number) => invoke<string[]>("snap_features_in", { layer, mapCrs, extent, limit }),
  editUndo: (layer: string) => invoke<EditStatus>("edit_undo", { layer }),
  editRedo: (layer: string) => invoke<EditStatus>("edit_redo", { layer }),
  editSave: (layer: string) => invoke<{ layer: LayerInfo; added: number; changed: number; deleted: number; invalid: number; note: string | null }>("edit_save", { layer }),
  editDiscard: (layer: string) => invoke<LayerInfo>("edit_discard", { layer }),
  editPending: (layer: string) => invoke<EditStatus>("edit_pending", { layer }),
  editUndoSave: (layer: string) => invoke<LayerInfo>("edit_undo_save", { layer }),
  editFeaturesIn: (layer: string, mapCrs: string, extent: [number, number, number, number], limit?: number) =>
    invoke<{ features: EditFeature[]; truncated: boolean; mapCrs: string }>("edit_features_in", { layer, mapCrs, extent, limit }),
  editReshape: (layer: string, mapCrs: string, op: Record<string, unknown>) => invoke<{ status: EditStatus; fids: number[]; invalid: number[]; zNote: string | null }>("edit_reshape", { layer, mapCrs, op }),
  editCircle: (mapCrs: string, center: [number, number], radiusM: number, vertices: number) => invoke<{ geometry: string; note: string }>("edit_circle", { mapCrs, center, radiusM, vertices }),
  editRectangle: (cornerA: [number, number], cornerB: [number, number]) => invoke<{ geometry: string; note: string }>("edit_rectangle", { cornerA, cornerB }),
  editDistance: (mapCrs: string, a: [number, number], b: [number, number]) => invoke<number>("edit_distance", { mapCrs, a, b }),
  editDestination: (mapCrs: string, from: [number, number], distanceM: number, bearingDeg: number) => invoke<[number, number]>("edit_destination", { mapCrs, from, distanceM, bearingDeg }),
  editParseCoords: (layer: string, mapCrs: string, kind: "xy" | "lonlat" | "usng", text: string) => invoke<[number, number][]>("edit_parse_coords", { layer, mapCrs, kind, text }),
  /** Go To Coordinates: read a typed position; the map CRS is null while the map has none. */
  parseLocation: (mapCrs: string | null, request: LocationRequest) => invoke<LocationResult>("parse_location", { mapCrs, request }),
  editPaste: (fromLayer: string, fids: number[], toLayer: string) =>
    invoke<{ pasted: number; unmatchedFields: string[]; failedValues: number; apply: { status: EditStatus } }>("edit_paste", { fromLayer, fids, toLayer }),
  createLayer: (name: string, geomType: string, crs: string, fields: [string, string][]) => invoke<LayerInfo>("create_layer", { name, geomType, crs, fields }),
  /** The rows with these ids as text, fields in the layer's order (to put back after a delete). */
  rowsByFid: (id: string, fids: number[]) => invoke<{ fid: number; values: (string | null)[] }[]>("rows_by_fid", { id, fids }),
  restoreRows: (id: string, rows: { fid: number; values: (string | null)[] }[]) => invoke<{ layer: LayerInfo; count: number }>("restore_rows", { id, rows }),
  deleteRows: (id: string, fids: number[]) => invoke<{ layer: LayerInfo; count: number }>("delete_rows", { id, fids }),
  defineProjection: (id: string, crs: string, crsName: string | null) =>
    invoke<LayerInfo>("define_projection", { id, crs, crsName }),
  suggestCrs: (id: string) => invoke<{ crs: string; reason: string }[]>("suggest_crs", { id }),
  crsInfo: (crs: string) => invoke<CrsInfo>("crs_info", { crs }),
  /** Accuracy of moving coordinates from `from` to `to` over a layer's extent (minx, miny, maxx, maxy in `from`). */
  transformReport: (from: string, to: string, extent: [number, number, number, number] | null) =>
    invoke<TransformReport>("transform_report", { from, to, extent }),
  /** True when approximate datum shifts are refused (Strict); false warns and continues. */
  datumPolicy: () => invoke<boolean>("datum_policy"),
  setDatumPolicy: (strict: boolean) => invoke<null>("set_datum_policy", { strict }),
  /** How well this installation moves coordinates in each checked region. */
  datumCoverage: () => invoke<[string, TransformReport][]>("datum_coverage"),
  /** Every text slot of every layout element (id, label, default size): the Properties Text section is built from this. */
  layoutTextSlots: () => invoke<TextSlot[]>("layout_text_slots"),
  topoPrepareData: (dem: string, interval: number, indexEvery: number, label: string, units: "imperial" | "metric") => track(invoke<TopoData>("topo_prepare_data", { dem, interval, indexEvery, label, units })),
  usngGridLayer: (bbox: Extent, interval: number, style: "usng" | "mgrs", datum: "auto" | "nad83" | "wgs84", name: string) => track(invoke<LayerInfo>("usng_grid_layer", { bbox, interval, style, datum, name })),
  usngCrsAt: (lon: number, lat: number, datum: "auto" | "nad83" | "wgs84" = "auto") => invoke<UsngCrs>("usng_crs_at", { lon, lat, datum }),
  buildTopoLayout: (spec: TopoSpec, today: string) => invoke<TopoBuild>("build_topo_layout", { spec, today }),
  /** Every datum grid PROJ-data offers, with where each is installed. */
  listDatumGrids: () => invoke<DatumGrid[]>("list_datum_grids"),
  /** Download one grid (checksum-verified). Grids above 50 MB need `confirmed`. */
  downloadDatumGrid: (name: string, confirmed: boolean) => invoke<DatumGrid>("download_datum_grid", { name, confirmed }),
  removeDatumGrid: (name: string) => invoke<null>("remove_datum_grid", { name }),
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
  /** The 0-based row of a feature in the table as getRows lists it for this query (offset and limit ignored), or null when it isn't in that view. */
  rowPosition: (id: string, query: RowsQuery, fid: number) => invoke<number | null>("row_position", { id, query, fid }),
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
  /** Georeferencing control point sets saved in the project (GR1). */
  listControlSets: () => invoke<ControlSet[]>("list_control_sets"),
  saveControlSet: (set: ControlSet) => invoke<void>("save_control_set", { set }),
  deleteControlSet: (id: string) => invoke<void>("delete_control_set", { id }),
  /** CSV whose header states the set's CRS. */
  exportControlSet: (set: ControlSet, path: string) => invoke<void>("export_control_set", { set, path }),
  /** GIS Studio CSV or QGIS `.points`; `crs` is null when the file names none (ask the user). */
  readControlFile: (path: string, imageSize?: [number, number] | null) => invoke<ParsedControlFile>("read_control_file", { path, imageSize: imageSize ?? null }),
  /** The same points expressed in another CRS through an explicit PROJ transform. */
  reprojectControlSet: (set: ControlSet, toCrs: string) => invoke<ControlSet>("reproject_control_set", { set, toCrs }),
  /** Per-point errors and RMS from GDAL's own transform (GR2). Rejects, in words, when the points can't make the transform. */
  georefErrors: (set: ControlSet, method: WarpMethod) => invoke<ErrorReport>("georef_errors", { set, method }),
  georefImageInfo: (path: string) => invoke<ImageInfo>("georef_image_info", { path }),
  /** PNG (base64) of an image window, for the image pane. */
  georefImageRegion: (path: string, x0: number, y0: number, w: number, h: number, outW: number, outH: number) =>
    invoke<string>("georef_image_region", { path, x0, y0, w, h, outW, outH }),
  /** A low-resolution in-memory picture of the warped image; a newer request cancels the older one (it rejects with "Cancelled"). */
  georefPreview: (image: string, set: ControlSet, method: WarpMethod, resampling: Resampling, maxSide: number) =>
    invoke<WarpPreview>("georef_preview", { image, set, method, resampling, maxSide }),
  /** Warp into a new GeoTIFF layer. Cancellable (progress task). */
  /** Error table for adjusting a vector layer with control points (GR3); nothing is changed. */
  adjustErrors: (layer: string, set: ControlSet, method: AdjustMethod) => invoke<AdjustReport>("adjust_errors", { layer, set, method }),
  /** Adjust a layer's shapes into a new layer in the layer's own CRS; features that can't be adjusted are left out and listed. Cancellable. */
  adjustLayer: (req: AdjustRequest) => track(invoke<AdjustResult>("adjust_layer", { req })),
  georefWarp: (req: WarpRequest) => track(invoke<LayerInfo>("georef_warp", { req })),
  /** A PNG of the page (base64) at `dpi`, plus the render's warnings. */
  renderLayoutPreview: (req: RenderRequest, dpi: number) => invoke<LayoutPreview>("render_layout_preview", { req, dpi }),
  /** Write the page to `path` as "svg" | "pdf" | "png"; returns the render's warnings. Cancellable (progress task). */
  exportLayout: (req: RenderRequest, format: "svg" | "pdf" | "png", dpi: number, path: string) =>
    track(invoke<string[]>("export_layout", { req, format, dpi, path })),
};

export interface TopoSymbol {
  id: string;
  /** The heading on the USGS sheet, or "Military overlay". */
  category: string;
  name: string;
  geom: "line" | "area" | "point";
  /** Merged over a layer's style to give it this symbol. */
  style: Partial<import("../store/store").LayerStyle>;
}

export interface LayoutTheme {
  id: string;
  name: string;
  description: string;
  /** Single-symbol layer colours, cycled in draw order. */
  palette: { fill: string; stroke: string; strokeWidth: number; opacity: number }[];
}

export interface RampInfo {
  name: string;
  group: "Scientific" | "Terrain" | "Sequential" | "Diverging" | "Qualitative" | "Gray";
  colors: string[];
  discrete: boolean;
  /** Readable with red–green colour blindness. */
  cbSafe: boolean;
}

export interface RasterUniqueValues {
  values: { value: number; count: number; color: string | null }[];
  /** Counts come from a downsampled read. */
  sampled: boolean;
  /** More distinct values than were asked for. */
  truncated: boolean;
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

/** Tile URL template of an on-screen raster basemap (`usgsTopo`, `osm` …) on a map CRS's grid: the engine fetches the provider's Web Mercator tiles and warps them with PROJ, so it works in every CRS. */
export function basemapTileUrl(providerId: string, crsKey: string): string {
  const key = crsKey && crsKey !== "3857" ? `${crsKey}/` : "";
  return `${TILE_BASE}/tiles/__bm_${providerId}/${key}{z}/{x}/{y}.png`;
}

/** Tile URL template of the bundled Natural Earth basemap on a map CRS's grid (the engine warps it; works in every CRS, offline). */
export function naturalEarthTileUrl(crsKey: string): string {
  const key = crsKey && crsKey !== "3857" ? `${crsKey}/` : "";
  return `${TILE_BASE}/tiles/__naturalEarth/${key}{z}/{x}/{y}.png`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
