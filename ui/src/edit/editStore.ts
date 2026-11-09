// Shape editing state (SHAPE_EDITING_PLAN E2): which layer is being edited, on which map, with which tool. The engine owns the edits
// (the pending log) and every coordinate; this store only remembers what the user is doing and what the engine last said.
import { create } from "zustand";
import type { EditStatus } from "../ipc/api";

export type EditTool = "modify" | "point" | "line" | "polygon" | "delete";

interface EditState {
  /** The layer in an edit session, and the map the tools work on (other maps showing it draw the pending shapes read-only). */
  layerId: string | null;
  mapId: string | null;
  tool: EditTool;
  snapping: boolean;
  status: EditStatus | null;
  /** The Z for new vertices on a layer with Z (null: none, stored as NaN). */
  zValue: number | null;
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
  status: null,
  zValue: null,
  epoch: 0,
  notice: null,
  set: (p) => set(p),
  bump: (status) => set((s) => ({ epoch: s.epoch + 1, status: status === undefined ? s.status : status })),
  clear: () => set((s) => ({ layerId: null, mapId: null, status: null, notice: null, tool: "modify", epoch: s.epoch + 1 })),
}));
