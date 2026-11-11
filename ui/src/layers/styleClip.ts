// UX9: copy and paste a layer's whole style (symbology and labels), and apply a style from the personal library. Pasting uses the same
// rules as "Apply Symbology / Labels To Others" (`applyStyleParts`): class-based symbology needs the same field in the target, and the note
// says what was not copied. Rasters and tables take no vector style.
import { create } from "zustand";
import type { LayerInfo } from "../ipc/api";
import { useApp, type LayerStyle } from "../store/store";
import { applyStyleParts } from "./multiselect";

interface Clip {
  /** Layer the style was copied from (or the library entry's name). */
  name: string;
  style: LayerStyle;
}

export const useStyleClip = create<{ clip: Clip | null; set: (c: Clip | null) => void }>((set) => ({ clip: null, set: (clip) => set({ clip }) }));

export const isVector = (i: Pick<LayerInfo, "geomType"> | undefined) => !!i && (i.geomType === "Point" || i.geomType === "Line" || i.geomType === "Polygon" || i.geomType === "Mixed");

/** `src`'s symbology then labels laid over `target`; null when the target takes no vector style. The notes say what was left out. */
export function applyWholeStyle(src: LayerStyle, target: LayerStyle, info: Pick<LayerInfo, "geomType" | "fields">): { style: LayerStyle; notes: string[] } | null {
  const a = applyStyleParts(src, target, info, "symbology");
  if (!a) return null;
  const b = applyStyleParts(src, a.style, info, "labels");
  return { style: b?.style ?? a.style, notes: [a.note, b?.note].filter((n): n is string => !!n) };
}

/** Remember a layer's style so Paste Style can put it on others. A private copy: later edits to the layer do not change it. */
export function copyStyle(id: string) {
  const app = useApp.getState();
  const style = app.styles[id];
  const info = app.layers[id];
  if (!style || !isVector(info)) {
    app.setStatus("Only point, line and polygon layers have a style to copy.");
    return;
  }
  useStyleClip.getState().set({ name: info.name, style: structuredClone(style) });
  app.setStatus(`Copied the style of ${info.name}. Right-click another layer › Paste Style.`);
}

/** Lay a style over the given layers; says which could not take all of it. Returns how many changed. */
export function applyStyleTo(name: string, style: LayerStyle, ids: string[]): number {
  const app = useApp.getState();
  let done = 0;
  const notes: string[] = [];
  for (const id of ids) {
    const info = app.layers[id];
    const to = app.styles[id];
    if (!info || !to) continue;
    const r = applyWholeStyle(style, to, info);
    if (!r) {
      notes.push(`${info.name}: not a vector layer`);
      continue;
    }
    app.setStyle(id, r.style);
    done++;
    for (const n of r.notes) notes.push(`${info.name}: ${n}`);
  }
  app.setStatus(`Applied the style “${name}” to ${done} layer${done === 1 ? "" : "s"}${notes.length ? `. ${notes.join("; ")}` : ""}`);
  return done;
}

export function pasteStyle(ids: string[]) {
  const clip = useStyleClip.getState().clip;
  if (!clip) {
    useApp.getState().setStatus("Nothing copied yet: right-click a layer › Copy Style first.");
    return;
  }
  applyStyleTo(clip.name, clip.style, ids);
}
