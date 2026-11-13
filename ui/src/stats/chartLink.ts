// Linked selection between a chart and the map (docs/STATISTICS_WORKSPACE_PLAN.md, "ST0 result"). The engine draws marks as `data-mark="<index>"`;
// the index counts every selectable mark of every series in order (series drawn as lines have none). Here: that index back to the mark, the SVG made
// safe to insert, which marks a map selection covers, and the select mode a click means. Pure functions, no DOM state of their own.
import type { ChartSpec, Mark } from "./types";

/** The marks `data-mark` indices point into, in the engine's order (`chart::selectable_marks`). */
export function selectableMarks(spec: ChartSpec): Mark[] {
  return spec.series.filter((s) => s.style !== "line").flatMap((s) => s.marks);
}

/** Parse engine SVG and drop everything that could run: scripts, foreign objects, event handlers, javascript: links. Null when it is not SVG. */
export function sanitizeSvg(text: string): SVGSVGElement | null {
  const doc = new DOMParser().parseFromString(text, "image/svg+xml");
  const root = doc.documentElement;
  if (!root || root.nodeName.toLowerCase() !== "svg" || doc.getElementsByTagName("parsererror").length) return null;
  for (const bad of Array.from(root.querySelectorAll("script, foreignObject, iframe, object, embed"))) bad.remove();
  for (const el of [root, ...Array.from(root.querySelectorAll("*"))]) {
    for (const a of Array.from(el.attributes)) {
      if (/^on/i.test(a.name) || /^\s*javascript:/i.test(a.value)) el.removeAttribute(a.name);
    }
  }
  return root as unknown as SVGSVGElement;
}

export type Cover = "all" | "some" | "none";

/** How much of a mark's features a map selection covers. A mark with no listed features (a density cell, a heat map cell) is "none". */
export function cover(mark: Mark | undefined, selected: ReadonlySet<number>): Cover {
  const f = mark?.fids;
  if (!f || f.length === 0 || selected.size === 0) return "none";
  let hit = 0;
  for (const id of f) if (selected.has(id)) hit++;
  return hit === 0 ? "none" : hit === f.length ? "all" : "some";
}

/** Shift adds to the selection, Ctrl or Cmd toggles, a plain click replaces it: the same as clicking features on the map. */
export function modeFor(e: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }): "new" | "add" | "toggle" {
  return e.shiftKey ? "add" : e.ctrlKey || e.metaKey ? "toggle" : "new";
}

/** Put the `sel` and `part` classes on the marks a selection covers. Touches classes only: the chart is never drawn again. Returns how many are covered. */
export function applySelection(root: Element, spec: ChartSpec, selected: ReadonlySet<number>): number {
  const marks = selectableMarks(spec);
  let n = 0;
  root.querySelectorAll("[data-mark]").forEach((el) => {
    const c = cover(marks[Number(el.getAttribute("data-mark"))], selected);
    el.classList.toggle("sel", c === "all");
    el.classList.toggle("part", c === "some");
    if (c !== "none") n++;
  });
  root.classList.toggle("has-sel", n > 0);
  return n;
}
