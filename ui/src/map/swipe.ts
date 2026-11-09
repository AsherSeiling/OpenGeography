// Swipe compare (UX4): the chosen layers are drawn only to the left of a vertical divider, so what lies beneath them shows on
// the right. The clip is in screen pixels on each layer's own canvas, so it works for raster and vector layers in any map CRS
// and needs nothing from the engine. It is a view tool: nothing is saved in the project and no data changes.
import type BaseLayer from "ol/layer/Base";
import type OlMap from "ol/Map";
import type RenderEvent from "ol/render/Event";
import { getRenderPixel } from "ol/render";
import { create } from "zustand";

// The layer base class's types leave out the render events a canvas layer fires.
type Listen = { on(t: string, f: (e: RenderEvent) => void): unknown; un(t: string, f: (e: RenderEvent) => void): unknown };

export interface SwipeState {
  /** Contents layer ids drawn left of the divider. */
  layers: string[];
  /** Divider position as a fraction of the map's width, 0 to 1. */
  pos: number;
}

export const useSwipe = create<{
  byMap: Record<string, SwipeState>;
  setLayers: (mapId: string, layers: string[]) => void;
  setPos: (mapId: string, pos: number) => void;
  off: (mapId: string) => void;
}>((set) => ({
  byMap: {},
  // No layers chosen means swipe is off. The divider keeps its place when the choice changes.
  setLayers: (mapId, layers) =>
    set((s) => {
      const rest = { ...s.byMap };
      if (!layers.length) {
        delete rest[mapId];
        return { byMap: rest };
      }
      return { byMap: { ...rest, [mapId]: { layers, pos: rest[mapId]?.pos ?? 0.5 } } };
    }),
  setPos: (mapId, pos) => set((s) => (s.byMap[mapId] ? { byMap: { ...s.byMap, [mapId]: { ...s.byMap[mapId], pos: clampPos(pos) } } } : s)),
  off: (mapId) =>
    set((s) => {
      const rest = { ...s.byMap };
      delete rest[mapId];
      return { byMap: rest };
    }),
}));

export const clampPos = (p: number) => (Number.isFinite(p) ? Math.min(0.98, Math.max(0.02, p)) : 0.5);

/** Layer ids still in the map, in the given order (a deleted or removed layer drops out of the comparison). */
export const chosenPresent = (chosen: string[], present: string[]) => chosen.filter((id) => present.includes(id));

/**
 * Clip each layer to the left of the divider. `getPos` is read at every draw, so dragging the divider only needs `map.render()`.
 * Returns a function that removes the clip and repaints.
 */
export function attachSwipe(map: OlMap, layers: BaseLayer[], getPos: () => number): () => void {
  const pre = (ev: RenderEvent) => {
    const ctx = ev.context as CanvasRenderingContext2D | null;
    const size = map.getSize();
    if (!ctx || !size) return;
    const w = size[0] * getPos();
    const tl = getRenderPixel(ev, [0, 0]);
    const tr = getRenderPixel(ev, [w, 0]);
    const bl = getRenderPixel(ev, [0, size[1]]);
    const br = getRenderPixel(ev, [w, size[1]]);
    ctx.save();
    ctx.beginPath();
    ctx.moveTo(tl[0], tl[1]);
    ctx.lineTo(bl[0], bl[1]);
    ctx.lineTo(br[0], br[1]);
    ctx.lineTo(tr[0], tr[1]);
    ctx.closePath();
    ctx.clip();
  };
  const post = (ev: RenderEvent) => (ev.context as CanvasRenderingContext2D | null)?.restore();
  for (const l of layers) {
    (l as unknown as Listen).on("prerender", pre);
    (l as unknown as Listen).on("postrender", post);
  }
  map.render();
  return () => {
    for (const l of layers) {
      (l as unknown as Listen).un("prerender", pre);
      (l as unknown as Listen).un("postrender", post);
    }
    map.render();
  };
}
