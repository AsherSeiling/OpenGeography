// Arrange (C3.0): the stacking order of layout elements (array order, last = on top) and alignment. Pure functions; the store
// applies them as one undo step.
import type { LayoutElement, Page } from "./types";

export type OrderOp = "front" | "back" | "forward" | "backward";
export type AlignOp = "left" | "center" | "right" | "top" | "middle" | "bottom";

/** The elements with the `ids` moved in the stacking order; the moved ones keep their order among themselves. */
export function reorder(elements: LayoutElement[], ids: string[], op: OrderOp): LayoutElement[] {
  const sel = new Set(ids);
  const picked = elements.filter((e) => sel.has(e.id));
  if (!picked.length || picked.length === elements.length) return elements;
  const rest = elements.filter((e) => !sel.has(e.id));
  if (op === "front") return [...rest, ...picked];
  if (op === "back") return [...picked, ...rest];
  const out = elements.slice();
  if (op === "forward") {
    for (let i = out.length - 2; i >= 0; i--) if (sel.has(out[i].id) && !sel.has(out[i + 1].id)) [out[i], out[i + 1]] = [out[i + 1], out[i]];
  } else {
    for (let i = 1; i < out.length; i++) if (sel.has(out[i].id) && !sel.has(out[i - 1].id)) [out[i], out[i - 1]] = [out[i - 1], out[i]];
  }
  return out;
}

const r1 = (v: number) => Math.round(v * 10) / 10;

/**
 * New x or y of each selected, unlocked element so they line up. Two or more elements align to their common box; one
 * element aligns to the page inside its margins. Rotation is ignored (the unrotated box is used).
 */
export function alignPatches(elements: LayoutElement[], ids: string[], op: AlignOp, page: Pick<Page, "width" | "height" | "margin">): Record<string, { x?: number; y?: number }> {
  const sel = new Set(ids);
  const t = elements.filter((e) => sel.has(e.id) && !e.locked);
  if (!t.length) return {};
  const ref =
    t.length >= 2
      ? { x0: Math.min(...t.map((e) => e.x)), y0: Math.min(...t.map((e) => e.y)), x1: Math.max(...t.map((e) => e.x + e.w)), y1: Math.max(...t.map((e) => e.y + e.h)) }
      : { x0: page.margin, y0: page.margin, x1: page.width - page.margin, y1: page.height - page.margin };
  const out: Record<string, { x?: number; y?: number }> = {};
  for (const e of t) {
    const p =
      op === "left" ? { x: ref.x0 } : op === "center" ? { x: (ref.x0 + ref.x1) / 2 - e.w / 2 } : op === "right" ? { x: ref.x1 - e.w } : op === "top" ? { y: ref.y0 } : op === "middle" ? { y: (ref.y0 + ref.y1) / 2 - e.h / 2 } : { y: ref.y1 - e.h };
    const q = { ...(p.x !== undefined ? { x: r1(p.x) } : {}), ...(p.y !== undefined ? { y: r1(p.y) } : {}) };
    if ((q.x !== undefined && q.x !== e.x) || (q.y !== undefined && q.y !== e.y)) out[e.id] = q;
  }
  return out;
}

export type DistributeAxis = "horizontal" | "vertical";

/** Spread three or more selected, unlocked elements so the gaps between them are equal; the outermost two stay put. */
export function distributePatches(elements: LayoutElement[], ids: string[], axis: DistributeAxis): Record<string, { x?: number; y?: number }> {
  const sel = new Set(ids);
  const t = elements.filter((e) => sel.has(e.id) && !e.locked);
  if (t.length < 3) return {};
  const h = axis === "horizontal";
  const pos = (e: LayoutElement) => (h ? e.x : e.y);
  const len = (e: LayoutElement) => (h ? e.w : e.h);
  const sorted = [...t].sort((a, b) => pos(a) - pos(b));
  const start = pos(sorted[0]);
  const end = Math.max(...sorted.map((e) => pos(e) + len(e)));
  const gap = (end - start - sorted.reduce((n, e) => n + len(e), 0)) / (sorted.length - 1);
  const out: Record<string, { x?: number; y?: number }> = {};
  let at = start;
  for (const e of sorted) {
    const v = r1(at);
    if (v !== pos(e)) out[e.id] = h ? { x: v } : { y: v };
    at += len(e) + gap;
  }
  return out;
}

/** Make the selected elements as wide / tall as the first-selected one (or the largest if none is first). */
export function sizePatches(elements: LayoutElement[], ids: string[], dim: "w" | "h"): Record<string, { w?: number; h?: number }> {
  const sel = new Set(ids);
  const t = ids.map((id) => elements.find((e) => e.id === id)).filter((e): e is LayoutElement => !!e && sel.has(e.id));
  if (t.length < 2) return {};
  const v = t[0][dim];
  const out: Record<string, { w?: number; h?: number }> = {};
  for (const e of t.slice(1)) if (!e.locked && e[dim] !== v) out[e.id] = { [dim]: v };
  return out;
}

/** `ids` plus everyone who shares a group with any of them. */
export function withGroupMates(elements: LayoutElement[], ids: string[]): string[] {
  const groups = new Set(elements.filter((e) => ids.includes(e.id) && e.group).map((e) => e.group));
  return elements.filter((e) => ids.includes(e.id) || (e.group && groups.has(e.group))).map((e) => e.id);
}
