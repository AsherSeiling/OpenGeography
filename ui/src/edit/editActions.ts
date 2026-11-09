// Shape editing actions (SHAPE_EDITING_PLAN E2). The engine holds the edit session; these start, change and finish it. Every call that
// carries shapes names the CRS of the map being edited (`getActiveCrs(mapId).id`); no coordinate is computed here.
import { create } from "zustand";
import { api, errorText, type EditOp, type LayerInfo } from "../ipc/api";
import { getActiveCrs } from "../map/mapController";
import { useApp } from "../store/store";
import { useEdit, type EditTool } from "./editStore";

/** Dialogs of the edit mode (kept here so the global dialog list stays as it was). */
interface EditDialogs {
  finish: boolean;
  newLayer: boolean;
  resume: string | null;
  setFinish: (v: boolean) => void;
  setNewLayer: (v: boolean) => void;
  setResume: (v: string | null) => void;
}
export const useEditDialogs = create<EditDialogs>((set) => ({
  finish: false,
  newLayer: false,
  resume: null,
  setFinish: (finish) => set({ finish }),
  setNewLayer: (newLayer) => set({ newLayer }),
  setResume: (resume) => set({ resume }),
}));

/** Layers whose latest Save can still be undone: layer id → the version that Save produced. */
export const useSaved = create<{ saved: Record<string, number>; set: (id: string, v: number | null) => void }>((set) => ({
  saved: {},
  set: (id, v) =>
    set((s) => {
      const { [id]: _gone, ...rest } = s.saved;
      return { saved: v === null ? rest : { ...rest, [id]: v } };
    }),
}));

const status = (m: string) => useApp.getState().setStatus(m);

/** Why a layer can't be edited, as a sentence, or null. (The engine checks too; this says it before a round trip.) */
export function editProblem(l: LayerInfo | null | undefined, resultOf?: boolean): string | null {
  if (!l) return "Select a layer to edit first.";
  if (l.geomType === "Raster") return `"${l.name}" is a raster; only vector layers can be edited.`;
  if (l.geomType === "Table") return `"${l.name}" is a table with no shapes.`;
  if (resultOf || l.resultOf) return `"${l.name}" is a node-tree result, which is rebuilt on every run. Make an editable copy of it first.`;
  if (!l.crs) return `"${l.name}" has no coordinate system. Use Define Projection first.`;
  return null;
}

export async function startEditing(layerId?: string, zValue: number | null = null): Promise<void> {
  const s = useApp.getState();
  const e = useEdit.getState();
  const id = layerId ?? s.activeLayer;
  const layer = id ? s.layers[id] : null;
  if (e.layerId) return status(`Finish editing "${s.layers[e.layerId]?.name ?? e.layerId}" first (Save or Discard).`);
  const problem = editProblem(layer, !!(id && s.results[id] && !s.showOriginal[id]));
  if (problem || !layer) return status(problem ?? "Select a layer to edit first.");
  const mapId = s.activeMapId;
  const crs = getActiveCrs(mapId);
  if (!crs) return status("The map is not ready yet.");
  try {
    const st = await api.editBegin(layer.id, crs.id, zValue);
    useApp.getState().updateLayer({ ...layer, editing: true });
    useEdit.getState().set({ layerId: layer.id, mapId, tool: layer.geomType === "Point" ? "point" : layer.geomType === "Line" ? "line" : "polygon", status: st, zValue, notice: null });
    useEdit.getState().bump(st);
    useApp.getState().setTool("pan");
    // A project that has never been saved lives in memory: the pending edits can't survive a crash.
    status(useApp.getState().projectDir ? `Editing ${layer.name}` : `Editing ${layer.name}. Save the project first: this project has never been saved, so edits made now would be lost if the program stops.`);
    if (!useApp.getState().projectDir) useEdit.getState().set({ notice: "This project has never been saved. Save it first so edits survive a crash." });
  } catch (err) {
    status(errorText(err));
  }
}

export const setEditTool = (tool: EditTool) => useEdit.getState().set({ tool });
export const toggleSnapping = () => useEdit.getState().set({ snapping: !useEdit.getState().snapping });

async function run<T>(f: () => Promise<T>): Promise<T | null> {
  try {
    return await f();
  } catch (err) {
    useEdit.getState().set({ notice: errorText(err) });
    status(errorText(err));
    return null;
  }
}

export async function editUndo(): Promise<void> {
  const id = useEdit.getState().layerId;
  if (!id) return;
  const st = await run(() => api.editUndo(id));
  if (st) useEdit.getState().bump(st);
}
export async function editRedo(): Promise<void> {
  const id = useEdit.getState().layerId;
  if (!id) return;
  const st = await run(() => api.editRedo(id));
  if (st) useEdit.getState().bump(st);
}

/** Ask Save / Discard / Cancel (the finish dialog). */
export function finishEditing(): void {
  if (useEdit.getState().layerId) useEditDialogs.getState().setFinish(true);
}

export async function saveEdits(): Promise<boolean> {
  const id = useEdit.getState().layerId;
  if (!id) return true;
  const r = await run(() => api.editSave(id));
  if (!r) return false;
  useApp.getState().updateLayer(r.layer);
  useSaved.getState().set(id, r.added + r.changed + r.deleted > 0 ? r.layer.version : null);
  status(r.added + r.changed + r.deleted ? `Saved: ${r.added} added, ${r.changed} changed, ${r.deleted} deleted.${r.note ? ` ${r.note}` : ""}` : "There were no changes to save.");
  useEdit.getState().clear();
  return true;
}

export async function discardEdits(): Promise<boolean> {
  const id = useEdit.getState().layerId ?? useEditDialogs.getState().resume;
  if (!id) return true;
  const l = await run(() => api.editDiscard(id));
  if (!l) return false;
  useApp.getState().updateLayer(l);
  useEdit.getState().clear();
  status("Edits discarded.");
  return true;
}

export async function undoLastSave(): Promise<void> {
  const s = useApp.getState();
  const id = s.activeLayer;
  if (!id) return;
  const l = await run(() => api.editUndoSave(id));
  if (l) {
    s.updateLayer(l);
    useSaved.getState().set(id, null);
    status("The last Save was undone.");
  }
}

/** Repair the pending shapes that are not valid (ST_MakeValid in the engine). */
export async function fixInvalid(mapId: string): Promise<void> {
  const e = useEdit.getState();
  const id = e.layerId;
  const crs = getActiveCrs(mapId);
  const bad = e.status?.invalid ?? [];
  if (!id || !crs || !bad.length) return;
  const r = await run(() => api.editFeaturesIn(id, crs.id, [-180, -85, 180, 85], 100000));
  if (!r) return;
  const ops: EditOp[] = r.features.filter((f) => bad.includes(f.fid) && f.geometry).map((f) => ({ op: "update", fid: f.fid, geometry: f.geometry!, fix: true }));
  if (!ops.length) return;
  const res = await run(() => api.editApply(id, crs.id, ops));
  if (res) useEdit.getState().bump(res.status);
}

/** A reopened project with edits waiting: offer to resume or discard them. */
export function offerResume(layers: Record<string, LayerInfo>): void {
  if (useEdit.getState().layerId || useEditDialogs.getState().resume) return;
  const pending = Object.values(layers).find((l) => l.editing);
  if (pending) useEditDialogs.getState().setResume(pending.id);
}

export async function resumeEditing(layerId: string): Promise<void> {
  useEditDialogs.getState().setResume(null);
  await startEditing(layerId, useEdit.getState().zValue);
}

export async function newLayer(name: string, geomType: string, crs: string, fields: [string, string][]): Promise<void> {
  const l = await api.createLayer(name, geomType, crs, fields);
  const s = useApp.getState();
  s.updateLayer(l);
  s.addToMap([l.id], s.activeMapId);
  s.setActiveLayer(l.id);
  await startEditing(l.id);
}
