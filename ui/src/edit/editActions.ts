// Shape editing actions (SHAPE_EDITING_PLAN E2). The engine holds the edit session; these start, change and finish it. Every call that
// carries shapes names the CRS of the map being edited (`getActiveCrs(mapId).id`); no coordinate is computed here.
import { create } from "zustand";
import { api, errorText, type EditOp, type LayerInfo } from "../ipc/api";
import { getActiveCrs } from "../map/mapController";
import { useApp } from "../store/store";
import { useClip, useEdit, type EditTool } from "./editStore";

/** Dialogs of the edit mode (kept here so the global dialog list stays as it was). */
interface EditDialogs {
  finish: boolean;
  newLayer: boolean;
  coords: boolean;
  resume: string | null;
  setFinish: (v: boolean) => void;
  setNewLayer: (v: boolean) => void;
  setCoords: (v: boolean) => void;
  setResume: (v: string | null) => void;
}
export const useEditDialogs = create<EditDialogs>((set) => ({
  finish: false,
  newLayer: false,
  coords: false,
  resume: null,
  setFinish: (finish) => set({ finish }),
  setNewLayer: (newLayer) => set({ newLayer }),
  setCoords: (coords) => set({ coords }),
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

// ---- reshape, copy and paste, typed coordinates (E3) ----

const mapCrsId = () => getActiveCrs(useEdit.getState().mapId ?? useApp.getState().activeMapId)?.id;

/** Run a reshape of the selected features (merge, explode, make multipart, fill holes); the engine does it as one undo step. */
export async function reshapeSelected(kind: "merge" | "explode" | "make_multipart" | "fill_holes"): Promise<void> {
  const e = useEdit.getState();
  const id = e.layerId;
  const crs = mapCrsId();
  if (!id || !crs) return;
  const sel = e.selected;
  const need = kind === "merge" || kind === "make_multipart" ? 2 : 1;
  if (sel.length < need) {
    const msg = kind === "merge" ? "Select two or more features first (Select tool; Shift-click adds)." : kind === "make_multipart" ? "Select two or more features of the same shape type first (Select tool; Shift-click adds)." : "Select a feature first (Select tool).";
    useEdit.getState().set({ notice: msg });
    return;
  }
  const op = kind === "explode" ? { kind, fid: sel[0] } : { kind, fids: sel };
  const r = await run(() => api.editReshape(id, crs, op));
  if (r) {
    useEdit.getState().set({ selected: [], notice: null });
    useEdit.getState().bump(r.status);
  }
}

/** Copy the active layer's selected features (the map selection) for Paste into the layer being edited. */
export function copySelected(): void {
  const s = useApp.getState();
  const id = s.activeLayer;
  const sel = id ? s.selection[id] ?? [] : [];
  const picked = useEdit.getState().selected;
  const from = useEdit.getState().layerId && picked.length ? useEdit.getState().layerId! : id;
  const fids = useEdit.getState().layerId && picked.length ? picked : sel;
  if (!from || !fids.length) return void status("Select features to copy first (on the map or in the Attribute Table).");
  useClip.getState().set({ layer: from, fids });
  status(`Copied ${fids.length} feature(s). Paste puts them into the layer being edited.`);
}

export async function pasteCopied(): Promise<void> {
  const clip = useClip.getState().clip;
  const to = useEdit.getState().layerId;
  if (!to) return status("Start editing a layer first; Paste puts the copied features into it.");
  if (!clip) return status("Copy features first.");
  const r = await run(() => api.editPaste(clip.layer, clip.fids, to));
  if (!r) return;
  useEdit.getState().bump(r.apply.status);
  const bits = [`Pasted ${r.pasted} feature(s), moved into this layer's coordinate system.`];
  if (r.unmatchedFields.length) bits.push(`Fields with no match by name were left behind: ${r.unmatchedFields.join(", ")}.`);
  if (r.failedValues) bits.push(`${r.failedValues} value(s) did not fit their field and were pasted empty.`);
  useEdit.getState().set({ notice: bits.join(" ") });
}

/** Add one feature from typed coordinates. `kind`: x/y in the layer's CRS, lon/lat, USNG/MGRS, or bearing and distance (geodesic) from a first position. */
export async function addByCoordinates(kind: "xy" | "lonlat" | "usng" | "bearing", text: string): Promise<void> {
  const e = useEdit.getState();
  const id = e.layerId;
  const crs = mapCrsId();
  const layer = id ? useApp.getState().layers[id] : undefined;
  if (!id || !crs || !layer) return;
  let pts: [number, number][];
  if (kind === "bearing") {
    // First line: a position as x, y in the layer's CRS. Each next line: "bearing, distance" (degrees clockwise from north, metres).
    const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
    if (lines.length < 1) throw new Error("Type a start position, then one bearing and distance per line.");
    pts = [(await api.editParseCoords(id, crs, "xy", lines[0]))[0]];
    for (const [i, l] of lines.slice(1).entries()) {
      const [b, d] = l.split(/[,;\s]+/).map(Number);
      if (!Number.isFinite(b) || !Number.isFinite(d)) throw new Error(`Line ${i + 2}: expected a bearing and a distance, for example 45, 120.5`);
      pts.push(await api.editDestination(crs, pts[pts.length - 1], d, b));
    }
  } else {
    pts = await api.editParseCoords(id, crs, kind, text);
  }
  const { Point, LineString, Polygon } = await import("ol/geom");
  const WKB = (await import("ol/format/WKB")).default;
  const geom = layer.geomType === "Point" ? (pts.length === 1 ? new Point(pts[0]) : null) : layer.geomType === "Line" ? new LineString(pts) : new Polygon([[...pts, pts[0]]]);
  if (!geom) throw new Error("A point layer takes one position at a time.");
  if (layer.geomType === "Line" && pts.length < 2) throw new Error("A line needs at least two positions.");
  if (layer.geomType === "Polygon" && pts.length < 3) throw new Error("A polygon needs at least three positions.");
  const r = await api.editApply(id, crs, [{ op: "insert", geometry: new WKB({ hex: true }).writeGeometry(geom) as string }]);
  useEdit.getState().bump(r.status);
}
