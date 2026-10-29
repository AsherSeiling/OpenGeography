// Selecting several layers in the Contents pane and the actions on that selection. Pure functions, so they are easy to test.
import type { LayerInfo } from "../ipc/api";
import type { LayerStyle } from "../store/store";

export interface Pick {
  selected: string[];
  /** Where a Shift-range starts. */
  anchor: string | null;
}

/** The selection after clicking `id`: plain click = just it; Ctrl/Cmd = toggle it; Shift = the range from the anchor (added to the selection with Ctrl/Cmd). `order` is the drawing order shown in the pane. */
export function nextSelection(order: string[], cur: Pick, id: string, mods: { shift?: boolean; toggle?: boolean }): Pick {
  if (mods.shift && cur.anchor && order.includes(cur.anchor)) {
    const [a, b] = [order.indexOf(cur.anchor), order.indexOf(id)].sort((x, y) => x - y);
    const range = order.slice(a, b + 1);
    const selected = mods.toggle ? order.filter((x) => cur.selected.includes(x) || range.includes(x)) : range;
    return { selected, anchor: cur.anchor };
  }
  if (mods.toggle) {
    const on = cur.selected.includes(id);
    const selected = order.filter((x) => (x === id ? !on : cur.selected.includes(x)));
    return { selected, anchor: id };
  }
  return { selected: [id], anchor: id };
}

export type Move = "up" | "down" | "top" | "bottom" | "gather";

/** New drawing order (top first) after moving the selected layers together, keeping their order among themselves.
 * "gather" makes them contiguous at the position of the highest selected one (the closest this app has to a group). */
export function moveSelected(order: string[], selected: string[], how: Move): string[] {
  const sel = new Set(selected);
  const picked = order.filter((x) => sel.has(x));
  if (!picked.length) return order;
  const rest = order.filter((x) => !sel.has(x));
  if (how === "top") return [...picked, ...rest];
  if (how === "bottom") return [...rest, ...picked];
  if (how === "gather") {
    const at = order.findIndex((x) => sel.has(x));
    const before = order.slice(0, at).filter((x) => !sel.has(x));
    return [...before, ...picked, ...rest.slice(before.length)];
  }
  // One step: each selected layer swaps with its unselected neighbour, from the leading edge.
  const out = [...order];
  if (how === "up") {
    for (let i = 1; i < out.length; i++) if (sel.has(out[i]) && !sel.has(out[i - 1])) [out[i - 1], out[i]] = [out[i], out[i - 1]];
  } else {
    for (let i = out.length - 2; i >= 0; i--) if (sel.has(out[i]) && !sel.has(out[i + 1])) [out[i + 1], out[i]] = [out[i], out[i + 1]];
  }
  return out;
}

/** Union of lon/lat extents ([west, south, east, north]) of the given layers; null when none has one. */
export function unionExtent(infos: Pick2<LayerInfo, "extent">[]): [number, number, number, number] | null {
  let u: [number, number, number, number] | null = null;
  for (const i of infos) {
    const e = i.extent;
    if (!e) continue;
    u = u ? [Math.min(u[0], e[0]), Math.min(u[1], e[1]), Math.max(u[2], e[2]), Math.max(u[3], e[3])] : [...e];
  }
  return u;
}
type Pick2<T, K extends keyof T> = { [P in K]: T[P] };

export type StyleParts = "symbology" | "labels";

export interface StyleResult {
  style: LayerStyle;
  /** Why part of the style was not copied (shown to the user), or null. */
  note: string | null;
}

const hasGeometry = (g: string) => g === "Point" || g === "Line" || g === "Polygon" || g === "Mixed";

/** `target`'s style with the source's symbology or labels applied. Class-based symbology needs the same field in the target, else the colour of the source's single symbol is used and the note says so. Rasters and tables take nothing. */
export function applyStyleParts(src: LayerStyle, target: LayerStyle, targetInfo: Pick2<LayerInfo, "geomType" | "fields">, parts: StyleParts): StyleResult | null {
  if (!hasGeometry(targetInfo.geomType)) return null;
  const fieldNames = new Set(targetInfo.fields.map((f) => f.name));
  if (parts === "labels") {
    if (!src.label) return { style: { ...target, label: undefined }, note: null };
    if (!fieldNames.has(src.label.field)) return { style: target, note: `labels not copied: no field "${src.label.field}"` };
    return { style: { ...target, label: { ...src.label } }, note: null };
  }
  let note: string | null = null;
  let kind = src.kind;
  if (kind !== "single" && !(src.field && fieldNames.has(src.field))) {
    kind = "single";
    note = `classes not copied: no field "${src.field}"; used the single symbol colour`;
  }
  const style: LayerStyle = { ...target, kind, fill: src.fill, stroke: src.stroke, strokeWidth: src.strokeWidth, opacity: src.opacity, radius: src.radius };
  if (kind === "single") {
    delete style.field; delete style.classes; delete style.breaks; delete style.colors;
  } else {
    style.field = src.field;
    style.classes = src.classes?.map((c) => ({ ...c }));
    style.breaks = src.breaks ? [...src.breaks] : undefined;
    style.colors = src.colors ? [...src.colors] : undefined;
  }
  return { style, note };
}

/** A file name safe on every system, unique among `taken` (compared ignoring case). */
export function safeFileName(name: string, ext: string, taken: Set<string>): string {
  const base = name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, "_").replace(/[. ]+$/, "").slice(0, 80) || "layer";
  let n = 1;
  let f = `${base}.${ext}`;
  while (taken.has(f.toLowerCase())) f = `${base}_${++n}.${ext}`;
  taken.add(f.toLowerCase());
  return f;
}
