// App state: layers, draw order, symbology, selection, map settings.
import { create } from "zustand";
import type { LayerInfo } from "../ipc/api";

export interface StyleClass {
  value: string;
  color: string;
  label: string;
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
}

export type MapTool = "pan" | "identify" | "select" | "selectBox" | "measure";
export type Basemap = "usgsTopo" | "usgsImagery" | "usgsRelief" | "osm" | "none";

export interface MapCrs {
  id: string;
  name: string;
}

export type PropertiesTarget = { kind: "layer"; id: string } | { kind: "node"; id: string } | null;

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
  properties: PropertiesTarget;
  /** Tab to show in layer Properties next time (e.g. "query"). */
  propsTab: string | null;
  mapCrs: MapCrs | null;
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
  setSelection: (id: string, fids: number[], mode?: "new" | "add" | "toggle") => void;
  clearSelection: () => void;
  setActiveLayer: (id: string | null) => void;
  setProperties: (t: PropertiesTarget) => void;
  setMapCrs: (c: MapCrs | null) => void;
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
  properties: null,
  propsTab: null,
  mapCrs: null,
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
      if (mode === "toggle") {
        const set1 = new Set(cur);
        for (const f of fids) set1.has(f) ? set1.delete(f) : set1.add(f);
        next = Array.from(set1);
      }
      return { selection: { ...s.selection, [id]: next } };
    }),

  clearSelection: () => set({ selection: {} }),
  setActiveLayer: (id) => set({ activeLayer: id }),
  setProperties: (t) => set({ properties: t }),
  setMapCrs: (c) => set({ mapCrs: c, dirty: true }),
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
