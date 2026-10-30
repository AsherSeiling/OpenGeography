// The Items catalog's logic (LW2), pure so it is unit-tested: names, search, the order shown, moving an element in the stacking order,
// and range selection.
import type { LayoutElement } from "./types";

export const KIND_NAME: Record<string, string> = {
  map_frame: "Map frame",
  text: "Text",
  legend: "Legend",
  scale_bar: "Scale bar",
  north_arrow: "North arrow",
  sources: "Sources",
  shape: "Shape",
  picture: "Picture",
  table: "Table",
  chart: "Chart",
};

/** The name shown for an element: the user's name, else the text of a Text element, else "<Kind> <id>". */
export function itemName(el: LayoutElement): string {
  if (el.name && el.name.trim()) return el.name.trim();
  if (el.kind === "text") {
    const t = el.text.replace(/\s+/g, " ").trim();
    if (t) return t.length > 28 ? `${t.slice(0, 27)}…` : t;
  }
  return `${KIND_NAME[el.kind] ?? el.kind} ${el.id}`;
}

/** The items top of the stacking order first (what the list shows). */
export function displayOrder(elements: LayoutElement[]): LayoutElement[] {
  return [...elements].reverse();
}

/** Elements whose name or kind contains the text; an empty search keeps all. */
export function matchItems(elements: LayoutElement[], q: string): LayoutElement[] {
  const t = q.trim().toLowerCase();
  if (!t) return elements;
  return elements.filter((e) => `${itemName(e)} ${KIND_NAME[e.kind] ?? e.kind} ${e.id}`.toLowerCase().includes(t));
}

/**
 * Move `id` so it is drawn directly above (`after = false` in the list, meaning nearer the top) or below the element `targetId` in the
 * list as shown (top first). Returns the new array (stacking order, first = bottom), or the same array when nothing changes.
 */
export function moveInList(elements: LayoutElement[], id: string, targetId: string, below: boolean): LayoutElement[] {
  if (id === targetId) return elements;
  const from = elements.findIndex((e) => e.id === id);
  const to = elements.findIndex((e) => e.id === targetId);
  if (from < 0 || to < 0) return elements;
  const moving = elements[from];
  const rest = elements.filter((e) => e.id !== id);
  const t = rest.findIndex((e) => e.id === targetId);
  // Stacking order is bottom first: "drawn above the target" = after it in the array; "below the target" = before it.
  const at = below ? t : t + 1;
  const out = [...rest.slice(0, at), moving, ...rest.slice(at)];
  return out.every((e, i) => e === elements[i]) ? elements : out;
}

/** The ids from `anchor` to `id` in the list order shown (Shift-click), both included. */
export function rangeIds(elements: LayoutElement[], anchor: string, id: string): string[] {
  const list = displayOrder(elements);
  const a = list.findIndex((e) => e.id === anchor);
  const b = list.findIndex((e) => e.id === id);
  if (a < 0 || b < 0) return [id];
  return list.slice(Math.min(a, b), Math.max(a, b) + 1).map((e) => e.id);
}

/** Selection after a click: plain = just this one; Cmd/Ctrl = toggle it; Shift = the range from the anchor. */
export function clickSelection(elements: LayoutElement[], selected: string[], anchor: string | null, id: string, mod: { toggle: boolean; shift: boolean }): string[] {
  if (mod.shift && anchor) return rangeIds(elements, anchor, id);
  if (mod.toggle) return selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id];
  return [id];
}
