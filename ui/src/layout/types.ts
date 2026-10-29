// Mirror of the engine's layout model (crates/engine/src/layout/model.rs).
// Page units are millimetres, origin top-left, y down; font sizes are points.
import type { LayerStyle } from "../store/store";

export const LAYOUT_VERSION = 1;

export interface Page {
  width: number;
  height: number;
  units: "mm";
  margin: number;
  background: string;
}

export interface Placement {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  rotation: number;
  locked: boolean;
  visible: boolean;
  /** Background, frame and drop shadow under any element. */
  decor?: Decor;
}

export interface Decor {
  background?: string | null;
  frameColor?: string | null;
  /** mm; 0 = no frame. */
  frameWidth?: number;
  cornerRadius?: number;
  shadow?: boolean;
  /** mm, right and down. */
  shadowOffset?: number;
}

export interface Border {
  width: number;
  color: string;
}

export type FrameLayers = "map" | { id: string; visible: boolean }[];

/** Graticule (interval in degrees) or projected grid (interval in CRS units). */
export interface GridSpec {
  interval: "auto" | number;
  labels: boolean;
}

export interface MapFrame extends Placement {
  kind: "map_frame";
  /** Explicit frame CRS; "" = not set (the engine refuses to render it). */
  crs: string;
  /** Frame centre in frame-CRS units. */
  center: [number, number];
  /** Nominal scale denominator. */
  scale: number;
  layers: FrameLayers;
  /** OpenStreetMap is not offered: its tile policy forbids bulk downloads (the engine refuses it). */
  basemap: "none" | "usgsTopo" | "usgsImagery" | "usgsRelief";
  graticule: GridSpec | null;
  grid: GridSpec | null;
  border: Border | null;
  /** Label edits made in this layout only, by layer id (L3). */
  labelOverrides?: Record<string, LabelOverrides>;
  /** The layout's map series moves this frame to each index feature (C3.2). */
  followsSeries?: boolean;
  /** Overview/inset: outline the extent of another map frame on this one. */
  indicator?: ExtentIndicator | null;
}

export interface ExtentIndicator {
  /** Id of the map frame whose extent is drawn. */
  frame: string;
  color?: string;
  /** mm */
  width?: number;
  /** 0–1; 0 = outline only. */
  fillOpacity?: number;
}

/** This layout's label style for one layer: unset fields fall back to the map's `LayerStyle.label`. */
export interface LabelPatch {
  enabled?: boolean;
  field?: string;
  size?: number;
  color?: string;
  haloColor?: string;
  haloWidth?: number;
}

/** One label's edits. Any of dxMm, dyMm, rotation, text or sizePt pins it (placed first, never dropped). */
export interface FeatureLabel {
  /** Move from the automatic position, page mm (right and down are positive). */
  dxMm?: number;
  dyMm?: number;
  /** Degrees on the page, clockwise. */
  rotation?: number;
  text?: string;
  sizePt?: number;
  hidden?: boolean;
}

export interface LabelOverrides {
  style?: LabelPatch;
  features?: Record<string, FeatureLabel>;
}

/** A label as the engine placed it; the Layout view draws a hit box over each. */
export interface PlacedLabel {
  frame: string;
  layer: string;
  fid: number;
  text: string;
  /** [x0, y0, x1, y1], page mm. */
  boxMm: [number, number, number, number];
  rotation: number;
  pinned: boolean;
  hidden: boolean;
}

export interface DroppedLabelEdit {
  frame: string;
  layer: string;
  fid: number;
}

export interface TextElement extends Placement {
  kind: "text";
  text: string;
  font: "sans" | "sans-bold" | "sans-italic";
  /** An installed font family used in place of the bundled face. */
  family?: string;
  size: number;
  align: "left" | "center" | "right";
  color: string;
}

export interface LegendElement extends Placement {
  kind: "legend";
  frame: string;
  title: string;
  columns: number;
  /** Renamed or hidden entries, by generated key (`LegendRow.key`). */
  overrides?: Record<string, { label?: string; hidden?: boolean }>;
  /** Hand-made entries at the end. */
  custom?: { label: string; color: string }[];
  patch?: "square" | "rounded" | "circle";
  /** Entry text size, pt; 0/unset = 8. */
  textSize?: number;
}

/** A legend entry as the engine generated it (for the editing list). */
export interface LegendRow {
  legend: string;
  key: string;
  layer: string;
  label: string;
  /** A layer heading or single entry: hiding it hides the layer's whole block. */
  layerRow: boolean;
  hidden: boolean;
}

export interface ScaleBarElement extends Placement {
  kind: "scale_bar";
  frame: string;
  units: "km" | "mi" | "m" | "ft";
  segments: number;
  /** Also print "1:N" under the bar (C3.0). */
  ratio?: boolean;
  /** Default "alternating". "double" adds a second bar in `units2`. */
  style?: "alternating" | "hollow" | "line" | "ticks" | "double";
  units2?: "km" | "mi" | "m" | "ft";
  /** Also print "1 cm on the map = N units". */
  verbal?: boolean;
  color?: string;
}

export interface NorthArrowElement extends Placement {
  kind: "north_arrow";
  frame: string;
  style: "simple" | "compass" | "arrow" | "triangle" | "half" | "rose" | "circle" | "custom";
  /** "true" (default; follows meridian convergence) or "grid". */
  north?: "true" | "grid";
  color?: string;
  /** Style "custom": picture data URI whose top points north. */
  source?: string;
}

export interface SourcesElement extends Placement {
  kind: "sources";
  frame: string;
  /** Text size in points. */
  size: number;
}

export interface ShapeElement extends Placement {
  kind: "shape";
  shape: "rectangle" | "ellipse" | "triangle" | "line" | "arrow";
  fill: string;
  fillOpacity?: number;
  stroke: string;
  /** Line width, mm. */
  strokeWidth: number;
  dash?: "solid" | "dashed" | "dotted";
  cornerRadius?: number;
}

export interface PictureElement extends Placement {
  kind: "picture";
  /** `data:image/png|jpeg|svg+xml;base64,…`; empty until a file is chosen. */
  source: string;
  fit?: "contain" | "cover" | "stretch";
  opacity?: number;
}

export interface TableElement extends Placement {
  kind: "table";
  /** Layer id. */
  layer: string;
  /** Fields in order; empty = the first five. */
  fields: string[];
  sortField?: string;
  descending?: boolean;
  maxRows?: number;
  /** pt; 0/unset = 7 */
  size?: number;
  zebra?: boolean;
}

export interface ChartElement extends Placement {
  kind: "chart";
  layer: string;
  chartType: "bar" | "pie";
  category: string;
  /** Numeric field to add up; empty = count features. */
  value?: string;
  agg?: "sum" | "avg" | "count";
  topN?: number;
  title?: string;
  size?: number;
}

export type SurroundElement = LegendElement | ScaleBarElement | NorthArrowElement | SourcesElement;

export type LayoutElement = MapFrame | TextElement | SurroundElement | ShapeElement | PictureElement | TableElement | ChartElement;

/** Value rounded for display (stored values stay as they are). */
export function rounded(v: number, decimals: number): number {
  const k = 10 ** decimals;
  return Math.round(v * k) / k;
}

export interface Layout {
  id: string;
  name: string;
  version: number;
  body: "earth";
  page: Page;
  /** Text token {author}. */
  author?: string;
  elements: LayoutElement[];
  series: SeriesSpec | null;
}

/** Map series (C3.2): one page per feature of the index layer; a map frame with `followsSeries` moves to each feature. */
export interface SeriesSpec {
  indexLayer: string;
  nameField: string;
  /** Pages are ordered by this field (default: the name field). */
  sortField?: string;
  extent: "fit" | "fixed_scale";
  /** Space around the feature, % of its size on each side (fit only). */
  marginPct: number;
  scale?: number;
}

export interface RenderRequest {
  layout: Layout;
  styles: Record<string, LayerStyle>;
  order: string[];
  visible: Record<string, boolean>;
  project: string;
  date: string;
  /** Draw this page (0-based) of the layout's map series. */
  seriesPage?: number;
}

export interface LayoutPreview {
  /** PNG, base64. */
  png: string;
  pngBytes: number;
  warnings: string[];
  labels: PlacedLabel[];
  droppedLabelEdits: DroppedLabelEdit[];
  legendRows?: LegendRow[];
}

/** Page sizes in millimetres, portrait (width < height). */
export const PAGE_SIZES: Record<string, [number, number]> = {
  Letter: [215.9, 279.4],
  Legal: [215.9, 355.6],
  Tabloid: [279.4, 431.8],
  A4: [210, 297],
  A3: [297, 420],
};

export type Orientation = "portrait" | "landscape";

export function pageFor(size: string, orientation: Orientation, margin = 12.7): Page {
  const [w, h] = PAGE_SIZES[size] ?? PAGE_SIZES.Letter;
  const [width, height] = orientation === "landscape" ? [h, w] : [w, h];
  return { width, height, units: "mm", margin, background: "#ffffff" };
}

/** The named size and orientation of a page, or null for a custom size. */
export function describePage(p: Page): { size: string; orientation: Orientation } | null {
  const near = (a: number, b: number) => Math.abs(a - b) < 0.5;
  for (const [name, [w, h]] of Object.entries(PAGE_SIZES)) {
    if (near(p.width, w) && near(p.height, h)) return { size: name, orientation: "portrait" };
    if (near(p.width, h) && near(p.height, w)) return { size: name, orientation: "landscape" };
  }
  return null;
}

export function newLayoutDoc(id: string, name: string, size = "Letter", orientation: Orientation = "landscape"): Layout {
  return { id, name, version: LAYOUT_VERSION, body: "earth", page: pageFor(size, orientation), elements: [], series: null };
}

/** Scale denominators offered when a frame is set from the map view. */
export const STANDARD_SCALES = [500, 1000, 1200, 2000, 2400, 2500, 4800, 5000, 6000, 10000, 12000, 15000, 20000, 24000, 25000, 30000, 50000, 62500, 100000, 125000, 250000, 500000, 1000000, 2000000, 2500000, 5000000, 10000000, 25000000, 50000000, 100000000];

/** The smallest standard scale at or above `scale` (so the frame covers what the map showed). */
export function roundScaleUp(scale: number): number {
  if (!Number.isFinite(scale) || scale <= 0) return 1_000_000;
  return STANDARD_SCALES.find((s) => s >= scale * 0.999) ?? Math.ceil(scale / 1e6) * 1e6;
}

export function frames(l: Layout): MapFrame[] {
  return l.elements.filter((e): e is MapFrame => e.kind === "map_frame");
}

export function nextId(l: Layout, prefix: string): string {
  let n = 1;
  while (l.elements.some((e) => e.id === `${prefix}${n}`)) n++;
  return `${prefix}${n}`;
}

const num = (v: unknown, fallback: number) => (typeof v === "number" && Number.isFinite(v) ? v : fallback);

/** Smallest size an element may have, mm. */
export const MIN_SIZE_MM = 1;

/**
 * Drop edits that would break the document: non-finite numbers, sizes below
 * 1 mm, scales ≤ 0, sizes/columns/segments ≤ 0, a centre that isn't two finite
 * numbers. Valid parts of the patch are kept.
 */
export function cleanPatch(patch: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(patch)) {
    if (typeof v === "number") {
      if (!Number.isFinite(v)) continue;
      if ((k === "w" || k === "h") && v < MIN_SIZE_MM) continue;
      if ((k === "scale" || k === "size" || k === "columns" || k === "segments") && v <= 0) continue;
    }
    if (k === "center" && !(Array.isArray(v) && v.length === 2 && v.every((c) => typeof c === "number" && Number.isFinite(c)))) continue;
    out[k] = v;
  }
  return out;
}

/** Fill what an older or damaged layout lacks so the editor never reads undefined. */
export function normalizeLayout(raw: Layout): Layout {
  const def = pageFor("Letter", "landscape");
  const page = (raw.page ?? {}) as Partial<Page>;
  const w = num(page.width, def.width);
  const h = num(page.height, def.height);
  const elements = (Array.isArray(raw.elements) ? raw.elements : [])
    .filter((e) => e && typeof e === "object" && typeof e.id === "string")
    .map((e) => {
      const base = { ...e, x: num(e.x, 0), y: num(e.y, 0), w: Math.max(MIN_SIZE_MM, num(e.w, 10)), h: Math.max(MIN_SIZE_MM, num(e.h, 10)), rotation: num(e.rotation, 0), locked: !!e.locked, visible: e.visible !== false };
      if (base.kind === "map_frame") {
        const c = (base as MapFrame).center;
        return { ...base, crs: base.crs ?? "", center: Array.isArray(c) && c.length === 2 && c.every((v) => Number.isFinite(v)) ? c : [0, 0], scale: num(base.scale, 1_000_000) > 0 ? num(base.scale, 1_000_000) : 1_000_000 } as MapFrame;
      }
      return base as LayoutElement;
    });
  return {
    ...raw,
    name: typeof raw.name === "string" && raw.name ? raw.name : raw.id,
    page: { ...def, ...page, width: w > 0 ? w : def.width, height: h > 0 ? h : def.height, margin: Math.max(0, num(page.margin, def.margin)) },
    elements,
  };
}
