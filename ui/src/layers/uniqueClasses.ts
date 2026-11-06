// Editing the classes of a unique-values style by hand (UR-12): add one, group several values in one class, split, remove.
// Pure functions over `StyleClass[]`; the Properties pane calls them and stores the result.
import { CATEGORY_COLORS } from "../map/style";
import type { StyleClass } from "../store/store";

export const classValues = (c: StyleClass): string[] => [c.value, ...(c.values ?? [])];
const all = (classes: StyleClass[]) => new Set(classes.flatMap(classValues));
const show = (v: string) => v || "<empty>";

/** A colour not yet used by the classes, cycling through the palette. */
export function nextColor(classes: StyleClass[]): string {
  const used = new Set(classes.map((c) => c.color.toLowerCase()));
  return CATEGORY_COLORS.find((c) => !used.has(c)) ?? CATEGORY_COLORS[classes.length % CATEGORY_COLORS.length];
}

/** Add a class for `value` (label defaults to the value, with its count when known). A value already in a class is refused with the reason. */
export function addValueClass(classes: StyleClass[], value: string, count?: number): { classes: StyleClass[] } | { error: string } {
  if (all(classes).has(value)) return { error: `"${show(value)}" already has a class.` };
  return { classes: [...classes, { value, color: nextColor(classes), label: `${show(value)}${count !== undefined ? ` (${count})` : ""}` }] };
}

/** Classes for every value in `values` that has none yet, after the existing ones. */
export function addAllValues(classes: StyleClass[], values: [string, number][]): StyleClass[] {
  const have = all(classes);
  let out = classes;
  for (const [v, n] of values) {
    if (have.has(v)) continue;
    out = [...out, { value: v, color: nextColor(out), label: `${show(v)} (${n})` }];
    have.add(v);
  }
  return out;
}

/** Put the classes at `idxs` into one: the first of them keeps its colour and takes the others' values; the label is the joined values unless `label` is given. */
export function groupClasses(classes: StyleClass[], idxs: number[], label?: string): StyleClass[] {
  const picked = [...new Set(idxs)].filter((i) => i >= 0 && i < classes.length).sort((a, b) => a - b);
  if (picked.length < 2) return classes;
  const vals = picked.flatMap((i) => classValues(classes[i]));
  const head = classes[picked[0]];
  const merged: StyleClass = { ...head, value: vals[0], values: vals.slice(1), label: label ?? vals.map(show).join(", ") };
  return classes.flatMap((c, i) => (i === picked[0] ? [merged] : picked.includes(i) ? [] : [c]));
}

/** Split a grouped class back into one class per value (each keeps the group's colour except the later ones, which get fresh colours). */
export function ungroupClass(classes: StyleClass[], i: number): StyleClass[] {
  const c = classes[i];
  if (!c || !(c.values?.length)) return classes;
  const out = classes.slice();
  const singles: StyleClass[] = [];
  for (const [k, v] of classValues(c).entries()) singles.push({ value: v, color: k === 0 ? c.color : nextColor([...out.slice(0, i), ...singles, ...out.slice(i + 1)]), label: show(v), ...(k === 0 && c.strokes ? { strokes: c.strokes } : {}) });
  out.splice(i, 1, ...singles);
  return out;
}

export function removeClass(classes: StyleClass[], i: number): StyleClass[] {
  return classes.filter((_, j) => j !== i);
}

/** Put `value` into the class at `i` (group by hand); refused if another class already has it. */
export function addValueToClass(classes: StyleClass[], i: number, value: string): { classes: StyleClass[] } | { error: string } {
  if (all(classes).has(value)) return { error: `"${show(value)}" already has a class.` };
  const c = classes[i];
  if (!c) return { classes };
  return { classes: classes.map((x, j) => (j === i ? { ...x, values: [...(x.values ?? []), value] } : x)) };
}
