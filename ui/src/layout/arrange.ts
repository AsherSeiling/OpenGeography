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
