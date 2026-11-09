// Map bookmarks (UX3): a named place, saved in the project. Each stores its extent together with the explicit CRS the extent is
// in; it is moved into the map's (or a layout frame's) CRS through the engine when it is used, never assumed to match.
import { create } from "zustand";
import { api } from "../ipc/api";

export type BoxXY = [number, number, number, number];

export interface Bookmark {
  id: string;
  name: string;
  /** The coordinate system `extent` is in. */
  crs: string;
  /** [xmin, ymin, xmax, ymax] in `crs`. */
  extent: BoxXY;
}

const isBox = (e: unknown): e is BoxXY => Array.isArray(e) && e.length === 4 && e.every((v) => typeof v === "number" && Number.isFinite(v)) && (e as number[])[2] >= (e as number[])[0] && (e as number[])[3] >= (e as number[])[1];

/** Bookmarks as saved in a project file. Absent (an older project) or damaged entries are dropped, never an error. */
export function readBookmarks(raw: unknown): Bookmark[] {
  if (!Array.isArray(raw)) return [];
  const used = new Set<string>();
  const out: Bookmark[] = [];
  for (const r of raw as Partial<Bookmark>[]) {
    if (!r || typeof r !== "object" || typeof r.name !== "string" || !r.name.trim() || typeof r.crs !== "string" || !r.crs.trim() || !isBox(r.extent)) continue;
    let id = typeof r.id === "string" && r.id && !used.has(r.id) ? r.id : "";
    if (!id) id = newBookmarkId([...used].map((u) => ({ id: u }) as Bookmark));
    used.add(id);
    out.push({ id, name: r.name.trim(), crs: r.crs, extent: [...r.extent] as BoxXY });
  }
  return out;
}

export function newBookmarkId(items: Bookmark[]): string {
  const taken = new Set(items.map((b) => b.id));
  for (;;) {
    const id = `bm-${Math.random().toString(36).slice(2, 8)}`;
    if (!taken.has(id)) return id;
  }
}

/** The wanted name, or "name 2", "name 3" … when it is taken (ignoring case). */
export function uniqueBookmarkName(items: Bookmark[], wanted: string, except?: string): string {
  const names = new Set(items.filter((b) => b.id !== except).map((b) => b.name.toLowerCase()));
  const base = wanted.trim() || "Bookmark";
  if (!names.has(base.toLowerCase())) return base;
  for (let n = 2; ; n++) if (!names.has(`${base} ${n}`.toLowerCase())) return `${base} ${n}`;
}

export const useBookmarks = create<{
  items: Bookmark[];
  set: (items: Bookmark[]) => void;
  add: (name: string, crs: string, extent: BoxXY) => Bookmark;
  rename: (id: string, name: string) => void;
  remove: (id: string) => void;
}>((set, get) => ({
  items: [],
  set: (items) => set({ items }),
  add: (name, crs, extent) => {
    const items = get().items;
    const b: Bookmark = { id: newBookmarkId(items), name: uniqueBookmarkName(items, name), crs, extent };
    set({ items: [...items, b] });
    return b;
  },
  rename: (id, name) => set((s) => ({ items: s.items.map((b) => (b.id === id ? { ...b, name: uniqueBookmarkName(s.items, name, id) } : b)) })),
  remove: (id) => set((s) => ({ items: s.items.filter((b) => b.id !== id) })),
}));

/** Points along the extent's edges (corners, edge midpoints and centre), so a curved edge in another CRS is covered. */
export function samplePoints(e: BoxXY): [number, number][] {
  const [w, s, ea, n] = e;
  const xs = [w, (w + ea) / 2, ea];
  const ys = [s, (s + n) / 2, n];
  return ys.flatMap((y) => xs.map((x) => [x, y] as [number, number]));
}

export const boxOf = (pts: [number, number][]): BoxXY => [Math.min(...pts.map((p) => p[0])), Math.min(...pts.map((p) => p[1])), Math.max(...pts.map((p) => p[0])), Math.max(...pts.map((p) => p[1]))];

/** The smallest box in `toCrs` holding the bookmark: the stored extent is transformed point by point through the engine. */
export async function extentIn(b: Bookmark, toCrs: string): Promise<BoxXY> {
  if (b.crs === toCrs) return [...b.extent];
  const out = await Promise.all(samplePoints(b.extent).map(([x, y]) => api.transformPoint(x, y, b.crs, toCrs)));
  const box = boxOf(out);
  if (!box.every(Number.isFinite)) throw new Error(`This bookmark can't be shown in that coordinate system (${toCrs}); it lies outside what it can draw.`);
  return box;
}

/**
 * The scale (1:N) at which a frame of `frameMm` shows the box, from the ground units one millimetre covers at a reference scale
 * (`unitsPerMmAtRef`, [east, north]). Ground size scales linearly with the scale number.
 */
export function scaleToFit(box: BoxXY, frameMm: [number, number], unitsPerMmAtRef: [number, number], refScale: number): number {
  const w = box[2] - box[0];
  const h = box[3] - box[1];
  const sw = (w / (frameMm[0] * unitsPerMmAtRef[0])) * refScale;
  const sh = (h / (frameMm[1] * unitsPerMmAtRef[1])) * refScale;
  return Math.max(sw, sh);
}
