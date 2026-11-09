// Shape editing state (SHAPE_EDITING_PLAN E2): which layer is being edited, on which map, with which tool. The engine owns the edits
// (the pending log) and every coordinate; this store only remembers what the user is doing and what the engine last said.
import { create } from "zustand";
import type { EditStatus } from "../ipc/api";

export type EditTool = "modify" | "select" | "point" | "line" | "polygon" | "rect" | "circle" | "split" | "hole" | "delete";

interface EditState {
  /** The layer in an edit session, and the map the tools work on (other maps showing it draw the pending shapes read-only). */
  layerId: string | null;
  mapId: string | null;
  tool: EditTool;
  snapping: boolean;
  /** Features picked with the Select tool (for the reshape commands). */
  selected: number[];
  status: EditStatus | null;
  /** The Z for new vertices on a layer with Z (null: none, stored as NaN). */
  zValue: number | null;
  /** How many shapes the edit overlay of the editing map has loaded from the engine (-1 until the first load finishes). */
  loaded: number;
  /** Bumped after every change so map overlays and tiles reload. */
  epoch: number;
  /** A sentence for the status line / notice (refusals, antimeridian). */
  notice: string | null;
  set: (p: Partial<EditState>) => void;
  bump: (status?: EditStatus | null) => void;
  clear: () => void;
}

export const useEdit = create<EditState>((set) => ({
  layerId: null,
  mapId: null,
  tool: "modify",
  snapping: true,
  selected: [],
  status: null,
  zValue: null,
  epoch: 0,
  loaded: -1,
  notice: null,
  set: (p) => set(p),
  bump: (status) => set((s) => ({ epoch: s.epoch + 1, status: status === undefined ? s.status : status })),
  clear: () => set((s) => ({ layerId: null, mapId: null, status: null, notice: null, selected: [], loaded: -1, tool: "modify", epoch: s.epoch + 1 })),
}));

/** Features copied for Paste: the layer they came from and their ids. */
export const useClip = create<{ clip: { layer: string; fids: number[] } | null; set: (c: { layer: string; fids: number[] } | null) => void }>((set) => ({ clip: null, set: (clip) => set({ clip }) }));
