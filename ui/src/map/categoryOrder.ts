// Drawing order of unique-value categories (B7): the list order is the order on the map, in layouts and in the legend, and a
// category higher in the list draws on top.
import type { StyleClass } from "../store/store";

/** The classes with the one at `from` moved to position `to` (both clamped); the input is not changed. */
export function moveClass(classes: StyleClass[], from: number, to: number): StyleClass[] {
  if (from < 0 || from >= classes.length) return classes;
  const t = Math.max(0, Math.min(classes.length - 1, to));
  if (t === from) return classes;
  const out = classes.slice();
  const [c] = out.splice(from, 1);
  out.splice(t, 0, c);
  return out;
}

export const reverseClasses = (classes: StyleClass[]): StyleClass[] => classes.slice().reverse();

/**
 * Draw rank of each value: the top of the list gets the highest number. Values not in the list ("everything else") rank 0,
 * under all of them. A value listed twice keeps its first (highest) place.
 */
export function classRanks(classes: StyleClass[] | undefined): Map<string, number> {
  const out = new Map<string, number>();
  classes?.forEach((c, i) => {
    if (!out.has(c.value)) out.set(c.value, classes.length - i);
  });
  return out;
}
