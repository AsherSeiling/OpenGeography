// State of the Georeference window (GR1, GR2): the image, the working control point set, and the pure edits on it.
// Everything numeric that matters (errors, the warp) comes from the engine; this file only holds what the user typed or clicked.
import { create } from "zustand";
import type { ControlPoint, ControlSet, ErrorReport, ImageInfo, PointError, Resampling } from "./types";
import { METHODS, newControlSet, nextPointId } from "./types";

export interface GeorefImage {
  /** File the engine reads (read-only). For a project raster this is its file in the raster store. */
  path: string;
  info: ImageInfo;
  /** Project layer the image came from, so the output keeps that layer's citation. */
  sourceLayer: string | null;
  label: string;
}

// ---------- pure edits (tested) ----------

export function addPoint(set: ControlSet, p: { pixel: number; line: number; x: number; y: number }): ControlSet {
  return { ...set, points: [...set.points, { id: nextPointId(set.points), enabled: true, ...p }] };
}

export function updatePoint(set: ControlSet, id: number, patch: Partial<Omit<ControlPoint, "id">>): ControlSet {
  return { ...set, points: set.points.map((p) => (p.id === id ? { ...p, ...patch } : p)) };
}

export function removePoint(set: ControlSet, id: number): ControlSet {
  return { ...set, points: set.points.filter((p) => p.id !== id) };
}

/** How many more enabled points a method needs, or 0. */
export function pointsShort(set: ControlSet, methodId: string): number {
  const m = METHODS.find((x) => x.id === methodId);
  const have = set.points.filter((p) => p.enabled).length;
  return m ? Math.max(0, m.min - have) : 0;
}

/** What changes the fit: the enabled/disabled state and positions of points, the CRS and the method. Used to key debounced work. */
export function fitKey(set: ControlSet, methodId: string, resampling: Resampling, image: string | null): string {
  return JSON.stringify([set.targetCrs, methodId, resampling, image, set.points.map((p) => [p.id, p.pixel, p.line, p.x, p.y, p.enabled])]);
}

/** Parse a number typed into a cell; null for empty or not a number (the cell then keeps its old value). */
export function parseCell(text: string): number | null {
  const t = text.trim().replace(/,/g, "");
  if (!t) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

/** An error figure for display: fixed decimals suited to its unit. */
export function fmtErr(v: number | null | undefined, unit: "m" | "px" | "deg" | string): string {
  if (v == null || !Number.isFinite(v)) return "–";
  if (unit === "deg" || unit === "degree") return v === 0 ? "0" : v.toExponential(2).replace(/e\+?/, "e");
  const a = Math.abs(v);
  return a >= 100 ? v.toFixed(1) : a >= 1 ? v.toFixed(2) : a === 0 ? "0" : v.toFixed(3);
}

export function errorOf(report: ErrorReport | null, id: number): PointError | undefined {
  return report?.points.find((p) => p.id === id);
}

/** One line for the footer: the RMS figures and how many points are in the fit. */
export function summaryLine(r: ErrorReport): string {
  const parts = [`RMS ${fmtErr(r.rmsM, "m")} m · ${fmtErr(r.rmsPx, "px")} px (${r.enabledCount} point${r.enabledCount === 1 ? "" : "s"} in the fit)`];
  if (r.checkRmsM != null) parts.push(`check points ${fmtErr(r.checkRmsM, "m")} m · ${fmtErr(r.checkRmsPx, "px")} px`);
  return parts.join(" — ");
}

// ---------- the store ----------

export interface GeorefState {
  open: boolean;
  image: GeorefImage | null;
  set: ControlSet;
  /** The saved set this working copy came from (or was saved as). */
  savedId: string | null;
  /** The working copy differs from what is saved in the project. */
  modified: boolean;
  methodId: string;
  resampling: Resampling;
  /** A point being made: one half is clicked, the other is waited for. */
  pending: { pixel?: number; line?: number; x?: number; y?: number };
  selected: number | null;
  show: () => void;
  hide: () => void;
  setImage: (img: GeorefImage | null) => void;
  /** Replace the working set (new, loaded or imported). `saved` marks it as already in the project. */
  setSet: (set: ControlSet, saved: boolean) => void;
  edit: (fn: (s: ControlSet) => ControlSet) => void;
  setMethod: (id: string) => void;
  setResampling: (r: Resampling) => void;
  setPending: (p: GeorefState["pending"]) => void;
  /** Add the pending point once both halves are there. Returns true when it did. */
  commitPending: () => boolean;
  select: (id: number | null) => void;
  markSaved: (id: string) => void;
}

export const useGeoref = create<GeorefState>((set, get) => ({
  open: false,
  image: null,
  set: newControlSet("Control points 1", null),
  savedId: null,
  modified: false,
  methodId: "p1",
  resampling: "nearest",
  pending: {},
  selected: null,
  show: () => set({ open: true }),
  hide: () => set({ open: false }),
  setImage: (image) =>
    set((s) => ({ image, pending: {}, set: { ...s.set, image: image?.path ?? null }, modified: s.modified || (!!image && image.path !== s.set.image) })),
  setSet: (next, saved) => set({ set: next, savedId: saved ? next.id : null, modified: !saved, pending: {}, selected: null }),
  edit: (fn) => set((s) => ({ set: fn(s.set), modified: true })),
  setMethod: (methodId) => set({ methodId }),
  setResampling: (resampling) => set({ resampling }),
  setPending: (pending) => set({ pending }),
  commitPending: () => {
    const { pending, set: cs } = get();
    if (pending.pixel == null || pending.line == null || pending.x == null || pending.y == null) return false;
    set({ set: addPoint(cs, { pixel: pending.pixel, line: pending.line, x: pending.x, y: pending.y }), modified: true, pending: {} });
    return true;
  },
  select: (selected) => set({ selected }),
  markSaved: (id) => set({ savedId: id, modified: false }),
}));
