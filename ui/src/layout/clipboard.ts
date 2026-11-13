// Cut, copy, paste, duplicate and lock for layout elements (the right-click menu and the Arrange tab). Pure helpers plus thin store calls.
import { beginStep, useLayouts } from "../store/layout";
import type { Layout, LayoutElement } from "./types";

/** What Copy and Cut hold; pasted copies are taken from here so a later edit of the original does not change them. */
let held: LayoutElement[] = [];
export const clipboardCount = () => held.length;

const PREFIX: Record<string, string> = { map_frame: "f", text: "t", legend: "lg", scale_bar: "sb", north_arrow: "na", declination: "dc", sources: "src", shape: "s", picture: "p", table: "tb", chart: "ch", stat_card: "st" };

/** Ids not used in `layout` yet, one per element, in the form the toolbar's inserts use ("f2", "lg3"). */
export function freshIds(layout: Layout, kinds: string[]): string[] {
  const used = new Set(layout.elements.map((e) => e.id));
  return kinds.map((kind) => {
    const p = PREFIX[kind] ?? "e";
    let n = 1;
    while (used.has(`${p}${n}`)) n++;
    used.add(`${p}${n}`);
    return `${p}${n}`;
  });
}

/**
 * Copies of `src` ready to add to `layout`: new ids, moved by `offset` mm, unlocked. Elements that pointed at another copied element
 * (a legend at its frame) point at the copy; ones that pointed at an element that was not copied keep pointing at it. Groups get new ids.
 */
export function cloneElements(layout: Layout, src: LayoutElement[], offset: number): LayoutElement[] {
  const ids = freshIds(layout, src.map((e) => e.kind));
  const map = new Map(src.map((e, i) => [e.id, ids[i]]));
  const groups = new Map<string, string>();
  return src.map((e) => {
    const copy = JSON.parse(JSON.stringify(e)) as LayoutElement & { frame?: string; indicator?: { frame: string } | null };
    copy.id = map.get(e.id)!;
    copy.x = Math.round((e.x + offset) * 10) / 10;
    copy.y = Math.round((e.y + offset) * 10) / 10;
    copy.locked = false;
    if (e.group) {
      if (!groups.has(e.group)) groups.set(e.group, `g${Date.now().toString(36)}${groups.size}`);
      copy.group = groups.get(e.group);
    }
    if (typeof copy.frame === "string" && map.has(copy.frame)) copy.frame = map.get(copy.frame)!;
    if (copy.indicator && map.has(copy.indicator.frame)) copy.indicator = { frame: map.get(copy.indicator.frame)! };
    return copy as LayoutElement;
  });
}

const selectedElements = () => {
  const s = useLayouts.getState();
  return (s.doc?.elements ?? []).filter((e) => s.selected.includes(e.id));
};

export function copySelected(): number {
  const els = selectedElements();
  if (els.length) held = JSON.parse(JSON.stringify(els));
  return els.length;
}

export function cutSelected(): number {
  const n = copySelected();
  const s = useLayouts.getState();
  if (n) s.removeElements(selectedElements().filter((e) => !e.locked).map((e) => e.id));
  return n;
}

function addCopies(src: LayoutElement[], offset: number): number {
  const s = useLayouts.getState();
  if (!s.doc || !src.length) return 0;
  const copies = cloneElements(s.doc, src, offset);
  const end = beginStep();
  for (const c of copies) useLayouts.getState().addElement(c);
  end();
  useLayouts.getState().select(copies.map((c) => c.id));
  return copies.length;
}

/** Paste the held elements, 5 mm down and right of where they were (or with their top-left corner at `at`, mm). */
export function pasteHeld(at?: { x: number; y: number }): number {
  if (!held.length) return 0;
  if (!at) return addCopies(held, 5);
  const minX = Math.min(...held.map((e) => e.x));
  const minY = Math.min(...held.map((e) => e.y));
  const n = addCopies(held, 0);
  if (!n) return 0;
  const s = useLayouts.getState();
  const end = beginStep();
  for (const id of s.selected) {
    const e = s.doc?.elements.find((x) => x.id === id);
    if (e) s.update(id, { x: Math.round((e.x - minX + at.x) * 10) / 10, y: Math.round((e.y - minY + at.y) * 10) / 10 });
  }
  end();
  return n;
}

export const duplicateSelected = () => addCopies(JSON.parse(JSON.stringify(selectedElements())), 5);

/** Lock the selection, or unlock it when every selected element is already locked. */
export function toggleLockSelected(): boolean | null {
  const els = selectedElements();
  if (!els.length) return null;
  const lock = !els.every((e) => e.locked);
  const end = beginStep();
  for (const e of els) useLayouts.getState().update(e.id, { locked: lock });
  end();
  return lock;
}
