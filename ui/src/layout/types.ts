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
}

export interface TextElement extends Placement {
  kind: "text";
  text: string;
  font: "sans" | "sans-bold" | "sans-italic";
  size: number;
  align: "left" | "center" | "right";
  color: string;
}

export interface LegendElement extends Placement {
  kind: "legend";
  frame: string;
  title: string;
  columns: number;
}

export interface ScaleBarElement extends Placement {
  kind: "scale_bar";
  frame: string;
  units: "km" | "mi" | "m" | "ft";
  segments: number;
}

export interface NorthArrowElement extends Placement {
  kind: "north_arrow";
  frame: string;
  style: "simple" | "compass";
}

export interface SourcesElement extends Placement {
  kind: "sources";
  frame: string;
  /** Text size in points. */
  size: number;
}

export type SurroundElement = LegendElement | ScaleBarElement | NorthArrowElement | SourcesElement;

export type LayoutElement = MapFrame | TextElement | SurroundElement;

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
  elements: LayoutElement[];
  series: unknown | null;
}

export interface RenderRequest {
  layout: Layout;
  styles: Record<string, LayerStyle>;
  order: string[];
  visible: Record<string, boolean>;
  project: string;
  date: string;
}

export interface LayoutPreview {
  /** PNG, base64. */
  png: string;
  pngBytes: number;
  warnings: string[];
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
  return STANDARD_SCALES.find((s) => s >= scale * 0.999) ?? Math.ceil(scale / 1e6) * 1e6;
}

/** Ground metres a MapLibre screen pixel spans at `lat` and `zoom` (512 px tiles). */
export function metersPerPixel(lat: number, zoom: number): number {
  return (40075016.686 * Math.cos((lat * Math.PI) / 180)) / (512 * 2 ** zoom);
}

export function frames(l: Layout): MapFrame[] {
  return l.elements.filter((e): e is MapFrame => e.kind === "map_frame");
}

export function nextId(l: Layout, prefix: string): string {
  let n = 1;
  while (l.elements.some((e) => e.id === `${prefix}${n}`)) n++;
  return `${prefix}${n}`;
}
