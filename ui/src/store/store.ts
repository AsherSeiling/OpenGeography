// App state: layers, draw order, symbology, selection, map settings.
import { create } from "zustand";
import type { LayerInfo } from "../ipc/api";

export type SelectMode = "new" | "add" | "toggle" | "remove" | "subset";

export interface StyleClass {
  value: string;
  color: string;
  label: string;
}

/** Feature labels of one layer (L1). One definition, used by the map view and, later, by layouts. */
export interface LabelStyle {
  enabled: boolean;
  field: string;
  /** Text size, pt. */
  size: number;
  color: string;
  haloColor: string;
  /** Halo width, px; 0 = none. */
  haloWidth: number;
  /** Zoomed-in limit: labels hide when the map is larger than 1:minScale. Unset = no limit. */
  minScale?: number;
  /** Zoomed-out limit: labels hide when the map is smaller than 1:maxScale. Unset = no limit. */
  maxScale?: number;
  /** Only features with INDEX = 1 (index contours) are labelled. */
  indexOnly?: boolean;
  /** An installed font family; unset = the app font (Noto Sans). A missing font falls back to it. */
  fontFamily?: string;
  bold?: boolean;
  italic?: boolean;
  /** Point labels, after QGIS "Placement": where the label sits around the symbol. Default "right". */
  position?: LabelPosition;
  /** Extra gap between the symbol (or the line) and the label, px. */
  distance?: number;
  /** Line labels, after QGIS "Above line / On line / Below line". Default "on". */
  linePos?: "above" | "on" | "below";
  /** After QGIS "Priority" 0–10: higher keeps its spot when labels collide. Default 5. */
  priority?: number;
  /** After QGIS "Allow overlaps": draw even where labels collide. */
  allowOverlap?: boolean;
  /** Wrap on spaces at this many characters. */
  wrapAt?: number;
  /** Multi-line alignment. */
  align?: "left" | "center" | "right";
  background?: { fill: string; stroke?: string; padding: number };
  shadow?: { color: string; dx: number; dy: number };
  /** Data-defined overrides read from fields: rotation (degrees clockwise), size (pt), colour ("#rrggbb"). */
  rotationField?: string;
  sizeField?: string;
  colorField?: string;
  /** Contour labels on a line layer: repeated along each line, upright, formatted. */
  contour?: ContourLabels;
}

/** Labels repeated along contour lines (the Contour node's ELEV field). */
export interface ContourLabels {
  /** Distance between labels along a line, px. */
  spacing: number;
  /** Break the line under the text (the label sits in the line) instead of above it. */
  inLine: boolean;
  /** Decimals of the elevation; unset = as stored. */
  decimals?: number;
  /** Text after the value, e.g. " m". */
  unit?: string;
}

export const defaultContourLabels = (): ContourLabels => ({ spacing: 250, inLine: true, decimals: 0, unit: "" });

export type LabelPosition = "right" | "left" | "above" | "below" | "center" | "aboveRight" | "aboveLeft" | "belowRight" | "belowLeft";

export function defaultLabel(field: string): LabelStyle {
  return { enabled: true, field, size: 9, color: "#222222", haloColor: "#ffffff", haloWidth: 1.5 };
}

/** Raster symbology (`raster_style.rs`). Every field is optional; the engine fills defaults. "default" = elevation ramp + hillshade / RGB as before. */
export interface RasterStyle {
  renderer: "default" | "gray" | "pseudocolor" | "paletted" | "multiband";
  /** 1-based band for gray and pseudocolour. */
  band?: number;
  stretch?: { mode: "minmax" | "percentile" | "stddev" | "manual"; pctLow?: number; pctHigh?: number; sigma?: number; min?: number; max?: number };
  /** A name from `api.rasterRamps()`, or "custom" to use `stops`. */
  ramp?: string;
  stops?: { pos: number; color: string; alpha?: number }[];
  invert?: boolean;
  mode?: "linear" | "discrete";
  /** Class upper bounds; with mode "discrete" the ramp is sampled once per class. */
  breaks?: number[];
  /** 0–1, on top of the layer opacity. */
  /** Relief shading under the colours (gray and pseudocolour). */
  shading?: { mode: "hillshade" | "multidirectional" | "slope"; azimuth?: number; altitude?: number; zFactor?: number; strength?: number };
  /** Display resampling; default bilinear. */
  resampling?: "nearest" | "bilinear" | "cubic";
  /** Multiband: bands drawn as red, green, blue (1-based), and optionally a stretch per channel. */
  rgbBands?: [number, number, number];
  channelStretch?: NonNullable<RasterStyle["stretch"]>[];
  /** Paletted: one entry per cell value. */
  entries?: { value: number; color: string; label?: string; hidden?: boolean }[];
  opacity?: number;
  nodataTransparent?: boolean;
  transparentRanges?: [number, number][];
  brightness?: number;
  contrast?: number;
  gamma?: number;
  saturation?: number;
}

export interface LayerStyle {
  kind: "single" | "categorized" | "graduated";
  fill: string;
  stroke: string;
  strokeWidth: number;
  opacity: number;
  radius: number;
  field?: string;
  /** categorized: one class per value */
  classes?: StyleClass[];
  /** graduated: upper bounds of each class except the last; colors.length = breaks.length + 1 */
  breaks?: number[];
  colors?: string[];
  /** Feature labels; projects saved before L1 have none (off). */
  label?: LabelStyle;
  /** Raster layers only; projects saved before raster symbology have none (the default look). */
  raster?: RasterStyle;
}

export type MapTool = "pan" | "identify" | "select" | "selectBox" | "measure";
export type Basemap = "usgsTopo" | "usgsImagery" | "usgsRelief" | "osm" | "none";

export interface MapCrs {
  id: string;
  name: string;
}

export type PropertiesTarget = { kind: "layer"; id: string } | { kind: "node"; id: string } | { kind: "layoutElement"; layout: string; id: string } | { kind: "layoutLabel"; layout: string; frame: string; layer: string; fid: number } | null;

// Default fills, in the spirit of ArcGIS Pro's random-but-pleasant symbols.
const PALETTE = ["#7aa8d8", "#e6a35c", "#8cc084", "#d77a7a", "#b09ad6", "#d6c36a", "#6fc3c0", "#c98fb4"];
let paletteIndex = 0;

export function defaultStyle(info: LayerInfo): LayerStyle {
  const fill = PALETTE[paletteIndex++ % PALETTE.length];
  return {
    kind: "single",
    fill,
    stroke: info.geomType === "Polygon" ? "#4d4d4d" : fill,
    strokeWidth: info.geomType === "Line" ? 1.5 : 0.7,
    opacity: info.geomType === "Polygon" ? 0.75 : 1, // rasters draw fully opaque
    radius: 3.5,
  };
}

interface AppState {
  layers: Record<string, LayerInfo>;
  /** Node-tree result per layer id; drawn in place of the layer. */
  results: Record<string, LayerInfo>;
  /** Layers whose original data is shown instead of their node-tree result. */
  showOriginal: Record<string, boolean>;
  /** Draw order, top layer first (as in the Contents pane). */
  order: string[];
  visible: Record<string, boolean>;
  styles: Record<string, LayerStyle>;
  selection: Record<string, number[]>;
  activeLayer: string | null;
  /** Layers picked in the Contents pane with Shift/Ctrl/Cmd-click (always includes the active one when non-empty). */
  selectedLayers: string[];
  layerAnchor: string | null;
  properties: PropertiesTarget;
  /** Tab to show in layer Properties next time (e.g. "query"). */
  propsTab: string | null;
  mapCrs: MapCrs | null;
  /** Show the whole Earth in the map CRS, cut at its central meridian + 180° (R8). Session-only. */
  worldView: boolean;
  tool: MapTool;
  basemap: Basemap;
  showFps: boolean;
  dark: boolean;
  projectDir: string | null;
  dirty: boolean;
  status: string;

  addLayers: (infos: LayerInfo[]) => void;
  updateLayer: (info: LayerInfo) => void;
  removeLayer: (id: string) => void;
  setVisible: (id: string, v: boolean) => void;
  setShowOriginal: (id: string, v: boolean) => void;
  moveLayer: (id: string, toIndex: number) => void;
  setStyle: (id: string, style: LayerStyle) => void;
  /** new / add / toggle; remove = take these out; subset = keep only these from the current selection. */
  setSelection: (id: string, fids: number[], mode?: SelectMode) => void;
  clearSelection: () => void;
  setActiveLayer: (id: string | null) => void;
  setLayerSelection: (ids: string[], anchor: string | null) => void;
  setProperties: (t: PropertiesTarget) => void;
  setMapCrs: (c: MapCrs | null) => void;
  setWorldView: (on: boolean) => void;
  setTool: (t: MapTool) => void;
  setBasemap: (b: Basemap) => void;
  setStatus: (s: string) => void;
  set: (p: Partial<AppState>) => void;
}

export const useApp = create<AppState>((set, get) => ({
  layers: {},
  results: {},
  showOriginal: {},
  order: [],
  visible: {},
  styles: {},
  selection: {},
  activeLayer: null,
  selectedLayers: [],
  layerAnchor: null,
  properties: null,
  propsTab: null,
  mapCrs: null,
  worldView: false,
  tool: "pan",
  basemap: "usgsTopo",
  showFps: false,
  dark: false,
  projectDir: null,
  dirty: false,
  status: "Ready",

  addLayers: (all) =>
    set((s) => {
      const results = { ...s.results };
      for (const r of all.filter((i) => i.resultOf)) results[r.resultOf!] = r;
      const infos = all.filter((i) => !i.resultOf);
      const layers = { ...s.layers };
      const visible = { ...s.visible };
      const styles = { ...s.styles };
      const order = [...s.order];
      for (const info of infos) {
        if (!layers[info.id]) {
          // New layers go on top, except tables which have nothing to draw.
          order.unshift(info.id);
          visible[info.id] = true;
          styles[info.id] = defaultStyle(info);
        }
        layers[info.id] = info;
      }
      // Map CRS defaults to the first layer that has one (like ArcGIS Pro).
      const first = infos.find((i) => i.crs && i.geomType !== "Table");
      const mapCrs = s.mapCrs ?? (first ? { id: first.crs!, name: first.crsName ?? first.crs! } : null);
      const activeLayer = s.activeLayer ?? infos[0]?.id ?? null;
      return { layers, results, order, visible, styles, mapCrs, activeLayer, dirty: true };
    }),

  updateLayer: (info) => {
    if (info.resultOf) {
      const owner = info.resultOf;
      // The layer now shows a different feature set: old selection ids no longer apply.
      set((s) => ({ results: { ...s.results, [owner]: info }, selection: { ...s.selection, [owner]: [] }, dirty: true }));
      return;
    }
    if (!get().layers[info.id]) return get().addLayers([info]);
    set((s) => ({ layers: { ...s.layers, [info.id]: info }, dirty: true }));
  },

  removeLayer: (id) =>
    set((s) => {
      const { [id]: _l, ...layers } = s.layers;
      const { [id]: _sel, ...selection } = s.selection;
      const { [id]: _res, ...results } = s.results;
      return {
        layers,
        results,
        selection,
        order: s.order.filter((x) => x !== id),
        selectedLayers: s.selectedLayers.filter((x) => x !== id),
        layerAnchor: s.layerAnchor === id ? null : s.layerAnchor,
        activeLayer: s.activeLayer === id ? (s.order.find((x) => x !== id) ?? null) : s.activeLayer,
        properties: s.properties?.kind === "layer" && s.properties.id === id ? null : s.properties,
        dirty: true,
      };
    }),

  setVisible: (id, v) => set((s) => ({ visible: { ...s.visible, [id]: v }, dirty: true })),
  setShowOriginal: (id, v) =>
    set((s) => ({ showOriginal: { ...s.showOriginal, [id]: v }, selection: { ...s.selection, [id]: [] }, dirty: true })),

  moveLayer: (id, toIndex) =>
    set((s) => {
      const order = s.order.filter((x) => x !== id);
      order.splice(Math.max(0, Math.min(toIndex, order.length)), 0, id);
      return { order, dirty: true };
    }),

  setStyle: (id, style) => set((s) => ({ styles: { ...s.styles, [id]: style }, dirty: true })),

  setSelection: (id, fids, mode = "new") =>
    set((s) => {
      let next = fids;
      const cur = s.selection[id] ?? [];
      if (mode === "add") next = Array.from(new Set([...cur, ...fids]));
      if (mode === "remove") {
        const drop = new Set(fids);
        next = cur.filter((f) => !drop.has(f));
      }
      if (mode === "subset") {
        const keep = new Set(fids);
        next = cur.filter((f) => keep.has(f));
      }
      if (mode === "toggle") {
        const set1 = new Set(cur);
        for (const f of fids) set1.has(f) ? set1.delete(f) : set1.add(f);
        next = Array.from(set1);
      }
      return { selection: { ...s.selection, [id]: next } };
    }),

  clearSelection: () => set({ selection: {} }),
  setActiveLayer: (id) => set({ activeLayer: id }),
  setLayerSelection: (ids, anchor) => set({ selectedLayers: ids, layerAnchor: anchor }),
  setProperties: (t) => set({ properties: t }),
  setMapCrs: (c) => set({ mapCrs: c, dirty: true }),
  setWorldView: (on) => set({ worldView: on }),
  setTool: (t) => set({ tool: t }),
  setBasemap: (b) => set({ basemap: b, dirty: true }),
  setStatus: (status) => set({ status }),
  set: (p) => set(p),
}));

export function layerList(): LayerInfo[] {
  const s = useApp.getState();
  return s.order.map((id) => s.layers[id]).filter(Boolean);
}

type Displayable = Pick<AppState, "layers" | "results" | "showOriginal">;

/** What a layer currently shows: its node-tree result, or its original data. */
export function displayInfo(s: Displayable, id: string): LayerInfo | undefined {
  const r = s.results[id];
  return r && !s.showOriginal[id] ? r : s.layers[id];
}

/** The Contents layer a displayed layer (original or result) belongs to. */
export function ownerOf(displayId: string): string {
  const s = useApp.getState();
  if (s.layers[displayId]) return displayId;
  return Object.values(s.results).find((r) => r.id === displayId)?.resultOf ?? displayId;
}

/** Ids of what is drawn on the map right now (results where applicable). */
export function drawableLayerIds(): string[] {
  const s = useApp.getState();
  return s.order
    .filter((id) => s.visible[id])
    .map((id) => displayInfo(s, id))
    .filter((l): l is LayerInfo => !!l && l.geomType !== "Table" && !!l.crs)
    .map((l) => l.id);
}
