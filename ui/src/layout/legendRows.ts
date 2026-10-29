// The legend entries the engine generated in the last preview of each layout; the Legend's Properties list edits them.
import { create } from "zustand";
import type { LegendRow } from "./types";

export const useLegendRows = create<{ byLayout: Record<string, LegendRow[]>; set: (layout: string, rows: LegendRow[]) => void }>((set) => ({
  byLayout: {},
  set: (layout, rows) => set((s) => ({ byLayout: { ...s.byLayout, [layout]: rows } })),
}));

type Overrides = NonNullable<import("./types").LegendElement["overrides"]>;

/** Overrides after setting `patch` for `key`; an override that says nothing is removed. */
export function withOverride(overrides: Overrides | undefined, key: string, patch: { label?: string; hidden?: boolean }): Overrides {
  const merged = { ...(overrides?.[key] ?? {}), ...patch };
  if (!merged.hidden) delete merged.hidden;
  if (merged.label !== undefined && merged.label.trim() === "") delete merged.label;
  const out = { ...(overrides ?? {}) };
  if (Object.keys(merged).length) out[key] = merged;
  else delete out[key];
  return out;
}
