// Editing the text-slot styles of a layout element (docs/LAYOUT_REWORK_PLAN.md §4.2). Pure, so it is unit-tested.
import { cleanTextStyle, type TextStyle } from "./types";

/**
 * The element's `textStyles` after changing one slot: `patch` fields with a value are set, fields set to `undefined` are removed, and a
 * slot left with no field is removed. Invalid values (a size outside 1 to 400 pt, a bad colour) are dropped by `cleanTextStyle`.
 */
export function withSlotStyle(styles: Record<string, TextStyle> | undefined, slot: string, patch: Partial<Record<keyof TextStyle, unknown>>): Record<string, TextStyle> {
  const cur: Record<string, unknown> = { ...(styles?.[slot] ?? {}) };
  for (const [k, v] of Object.entries(patch)) {
    if (v === undefined) delete cur[k];
    else cur[k] = v;
  }
  const next = { ...(styles ?? {}) };
  const clean = cleanTextStyle(cur);
  if (Object.keys(clean).length) next[slot] = clean;
  else delete next[slot];
  return next;
}

/** Remove the style of one slot (back to the default look). */
export function resetSlot(styles: Record<string, TextStyle> | undefined, slot: string): Record<string, TextStyle> {
  const next = { ...(styles ?? {}) };
  delete next[slot];
  return next;
}

/** Parse what a size box holds: "" = back to the default (undefined), a number 1 to 400 = that size, anything else = no change (null). */
export function parseSize(text: string): number | undefined | null {
  const t = text.trim();
  if (t === "") return undefined;
  const v = Number(t);
  return Number.isFinite(v) && v >= 1 && v <= 400 ? v : null;
}
