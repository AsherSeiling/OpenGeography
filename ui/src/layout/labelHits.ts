// The labels the engine placed in the last preview of each layout (L3): the Layout view draws hit boxes from them and the
// Properties pane reads a label's automatic text and size.
import { create } from "zustand";
import type { PlacedLabel } from "./types";

export const useLabelHits = create<{ byLayout: Record<string, PlacedLabel[]>; set: (layout: string, labels: PlacedLabel[]) => void }>((set) => ({
  byLayout: {},
  set: (layout, labels) => set((s) => ({ byLayout: { ...s.byLayout, [layout]: labels } })),
}));

export const sameLabel = (a: { frame: string; layer: string; fid: number }, b: { frame: string; layer: string; fid: number }) => a.frame === b.frame && a.layer === b.layer && a.fid === b.fid;
