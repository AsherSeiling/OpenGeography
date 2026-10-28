// Layout editing state. The open layout lives in `doc` and is undoable (zundo);
// history is kept per layout, so switching tabs does not mix undo steps.
// Edits save to the engine (debounced 500 ms) and mark the project dirty.
import { create } from "zustand";
import { temporal } from "zundo";
import { api, errorText } from "../ipc/api";
import { alignPatches, reorder, type AlignOp, type OrderOp } from "../layout/arrange";
import { cleanPatch, nextId, normalizeLayout, type Layout, type LayoutElement, type MapFrame, type Page, type SurroundElement, type TextElement } from "../layout/types";
import { reportError } from "../errors/store";
import { useApp, type Basemap } from "./store";

interface LayoutState {
  /** Every layout of the project except the open one's latest edits (those are in `doc`). */
  docs: Record<string, Layout>;
  /** Id of the open (active) layout. */
  id: string | null;
  doc: Layout | null;
  /** Selected element ids of the open layout (not undoable). */
  selected: string[];
  loadAll: (layouts: Layout[]) => void;
  /** Make a layout the open one; its undo history comes back with it. */
  open: (id: string | null) => void;
  /** Layouts of the project, by name. */
  list: () => Layout[];
  add: (layout: Layout) => void;
  remove: (id: string) => void;
  rename: (id: string, name: string) => void;
  duplicate: (id: string) => Layout | null;
  select: (ids: string[]) => void;
  setPage: (page: Page) => void;
  addElement: (e: LayoutElement) => void;
  /** Change an element. `burst` groups quick edits of one setting (typing) into one undo step. */
  update: (elementId: string, patch: Record<string, unknown>, burst?: string) => void;
  removeElements: (ids: string[]) => void;
  /** Change the stacking order of the selected elements (C3.0), one undo step. */
  arrange: (op: OrderOp) => void;
  /** Line up the selected elements (to each other, or to the page for one), one undo step. */
  align: (op: AlignOp) => void;
  undo: () => void;
  redo: () => void;
}

interface History {
  past: { doc: Layout | null }[];
  future: { doc: Layout | null }[];
}
const histories: Record<string, History> = {};

const SAVE_DELAY = 500;
const timers: Record<string, ReturnType<typeof setTimeout>> = {};

const markDirty = () => useApp.getState().set({ dirty: true });

function latest(id: string): Layout | null {
  const s = useLayouts.getState();
  return s.id === id ? s.doc : (s.docs[id] ?? null);
}

async function saveNow(id: string) {
  clearTimeout(timers[id]);
  delete timers[id];
  const l = latest(id);
  if (!l) return;
  try {
    await api.saveLayout(l);
  } catch (e) {
    useApp.getState().setStatus(`Layout '${l.name}' could not be saved: ${errorText(e)}`);
    reportError(e, `layout-save:${id}`, { title: `Layout '${l.name}' could not be saved`, context: "saving a layout" });
  }
}

function scheduleSave(id: string) {
  clearTimeout(timers[id]);
  timers[id] = setTimeout(() => void saveNow(id), SAVE_DELAY);
  markDirty();
}

/** Write any layout edits still waiting for their debounce (before saving or closing the project). */
export async function flushLayoutSaves() {
  await Promise.all(Object.keys(timers).map(saveNow));
}

export const useLayouts = create<LayoutState>()(
  temporal(
    (set, get) => ({
      docs: {},
      id: null,
      doc: null,
      selected: [],
      loadAll: (layouts) => {
        for (const k of Object.keys(histories)) delete histories[k];
        for (const k of Object.keys(timers)) clearTimeout(timers[k]), delete timers[k];
        set({ docs: Object.fromEntries(layouts.map(normalizeLayout).map((l) => [l.id, l])), id: null, doc: null, selected: [] });
        useLayouts.temporal.getState().clear();
      },
      open: (id) => {
        const s = get();
        if (id === s.id) return;
        const t = useLayouts.temporal.getState();
        const docs = { ...s.docs };
        if (s.id && s.doc) {
          docs[s.id] = s.doc;
          histories[s.id] = { past: t.pastStates as never, future: t.futureStates as never };
        }
        const doc = id ? (docs[id] ?? null) : null;
        set({ docs, id: doc ? id : null, doc, selected: [] });
        const h = id ? histories[id] : undefined;
        useLayouts.temporal.setState({ pastStates: (h?.past ?? []) as never, futureStates: (h?.future ?? []) as never });
      },
      list: () => {
        const s = get();
        const all = { ...s.docs, ...(s.id && s.doc ? { [s.id]: s.doc } : {}) };
        return Object.values(all).sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
      },
      add: (layout) => {
        set({ docs: { ...get().docs, [layout.id]: layout } });
        scheduleSave(layout.id);
      },
      remove: (id) => {
        const s = get();
        const docs = { ...s.docs };
        delete docs[id];
        delete histories[id];
        clearTimeout(timers[id]);
        delete timers[id];
        if (s.id === id) {
          set({ docs, id: null, doc: null, selected: [] });
          useLayouts.temporal.getState().clear();
        } else set({ docs });
        markDirty();
        api.deleteLayout(id).catch((e) => useApp.getState().setStatus(`Layout could not be deleted: ${errorText(e)}`));
      },
      rename: (id, name) => {
        const s = get();
        if (s.id === id && s.doc) set({ doc: { ...s.doc, name } });
        else if (s.docs[id]) set({ docs: { ...s.docs, [id]: { ...s.docs[id], name } } });
        else return;
        scheduleSave(id);
      },
      duplicate: (id) => {
        const src = latest(id);
        if (!src) return null;
        let n = 2;
        const names = new Set(get().list().map((l) => l.name));
        while (names.has(`${src.name} ${n}`)) n++;
        const copy: Layout = { ...JSON.parse(JSON.stringify(src)), id: newLayoutId(), name: `${src.name} ${n}` };
        get().add(copy);
        return copy;
      },
      select: (selected) => set({ selected }),
      setPage: (page) => {
        if (Number.isFinite(page.width) && Number.isFinite(page.height) && page.width > 0 && page.height > 0) edit((d) => ({ ...d, page }));
      },
      addElement: (e) => {
        flushBurst();
        edit((d) => ({ ...d, elements: [...d.elements, e] }));
        set({ selected: [e.id] });
      },
      update: (elementId, patch, burst) => {
        patch = cleanPatch(patch);
        if (!Object.keys(patch).length) return;
        if (burst) groupBurst(`${elementId}:${burst}`);
        else flushBurst();
        edit((d) => ({ ...d, elements: d.elements.map((e) => (e.id === elementId ? ({ ...e, ...patch } as LayoutElement) : e)) }));
      },
      removeElements: (ids) => {
        flushBurst();
        edit((d) => ({ ...d, elements: d.elements.filter((e) => !ids.includes(e.id)) }));
        set({ selected: get().selected.filter((s) => !ids.includes(s)) });
      },
      arrange: (op) => {
        flushBurst();
        const ids = get().selected;
        if (ids.length) edit((d) => ({ ...d, elements: reorder(d.elements, ids, op) }));
      },
      align: (op) => {
        flushBurst();
        const s = get();
        if (!s.doc || !s.selected.length) return;
        const patches = alignPatches(s.doc.elements, s.selected, op, s.doc.page);
        if (!Object.keys(patches).length) return;
        const end = beginStep();
        for (const [id, p] of Object.entries(patches)) get().update(id, p);
        end();
      },
      undo: () => {
        flushBurst();
        useLayouts.temporal.getState().undo();
        afterHistory();
      },
      redo: () => {
        flushBurst();
        useLayouts.temporal.getState().redo();
        afterHistory();
      },
    }),
    {
      // Only the open layout is undoable, not selection.
      partialize: (s) => ({ doc: s.doc }),
      equality: (a, b) => JSON.stringify(a) === JSON.stringify(b),
      limit: 100,
    },
  ),
);

function afterHistory() {
  const s = useLayouts.getState();
  if (s.id) scheduleSave(s.id);
  // Drop selections of elements an undo removed.
  const ids = new Set(s.doc?.elements.map((e) => e.id));
  const keep = s.selected.filter((x) => ids.has(x));
  if (keep.length !== s.selected.length) useLayouts.setState({ selected: keep });
}

function edit(f: (d: Layout) => Layout) {
  const s = useLayouts.getState();
  if (!s.doc || !s.id) return;
  useLayouts.setState({ doc: f(s.doc) });
  scheduleSave(s.id);
}

/** Typing in a field edits per keystroke; a burst on one setting is one undo step. */
let burst: { key: string; end: () => void; timer: ReturnType<typeof setTimeout> } | null = null;

export function flushBurst() {
  const b = burst;
  if (!b) return;
  burst = null;
  clearTimeout(b.timer);
  b.end();
}

function groupBurst(key: string) {
  if (burst?.key !== key) {
    flushBurst();
    burst = { key, end: beginStep(), timer: setTimeout(flushBurst, 800) };
  } else {
    clearTimeout(burst.timer);
    burst.timer = setTimeout(flushBurst, 800);
  }
}

/**
 * Group several edits (a drag, a typing burst) into ONE undo step: call the
 * returned function when done. Nothing is recorded if nothing changed.
 */
export function beginStep(): () => void {
  flushBurst();
  const before = useLayouts.getState().doc;
  const t = useLayouts.temporal.getState();
  t.pause();
  return () => {
    useLayouts.temporal.getState().resume();
    if (JSON.stringify(before) === JSON.stringify(useLayouts.getState().doc)) return;
    useLayouts.temporal.setState((s) => ({ pastStates: [...s.pastStates, { doc: before }].slice(-100) as never, futureStates: [] }));
  };
}

let counter = 1;
export const newLayoutId = () => `L${Date.now().toString(36)}${counter++}`;

// ---- element factories used by the ribbon

/** A map frame in the open layout; refuses unless the map has a coordinate system. */
/** Room under a new frame for its Sources line, mm. */
const SOURCES_H = 9.5;

/** The map view's basemap as a print basemap. OpenStreetMap can't be printed (its tile policy forbids bulk downloads), so it maps to none. */
export function printBasemap(b: Basemap): MapFrame["basemap"] {
  return b === "usgsTopo" || b === "usgsImagery" || b === "usgsRelief" ? b : "none";
}

export function addMapFrame(): string | null {
  const s = useLayouts.getState();
  const crs = useApp.getState().mapCrs;
  if (!s.doc) return null;
  if (!crs) {
    useApp.getState().setStatus("Set a coordinate system for the map frame (Map tab → Coordinate System), then insert it again.");
    return null;
  }
  const p = s.doc.page;
  const m = p.margin;
  const top = 30;
  const frame: MapFrame = {
    kind: "map_frame",
    id: nextId(s.doc, "f"),
    x: m,
    y: Math.min(top, p.height / 4),
    w: p.width - 2 * m,
    h: p.height - Math.min(top, p.height / 4) - m - SOURCES_H,
    rotation: 0,
    locked: false,
    visible: true,
    crs: crs.id,
    center: [0, 0],
    scale: 1_000_000,
    layers: "map",
    basemap: printBasemap(useApp.getState().basemap),
    graticule: null,
    grid: null,
    border: { width: 0.35, color: "#000000" },
  };
  s.addElement(frame);
  // L6: a new frame comes with its Sources line under it.
  addSurround("sources", frame.id);
  s.select([frame.id]);
  return frame.id;
}

/** Defaults from docs/LAYOUTS_PLAN.md L2, placed inside the frame's corners (legend top right, arrow top left, scale bar bottom left); Sources goes under it. */
export function addSurround(kind: SurroundElement["kind"], frameId: string): string | null {
  const s = useLayouts.getState();
  const f = s.doc?.elements.find((e) => e.id === frameId && e.kind === "map_frame");
  if (!s.doc || !f) return null;
  const base = { id: nextId(s.doc, { legend: "lg", scale_bar: "sb", north_arrow: "na", sources: "src" }[kind]), rotation: 0, locked: false, visible: true, frame: frameId };
  const inset = 3;
  let el: SurroundElement;
  switch (kind) {
    case "legend":
      el = { ...base, kind, x: f.x + f.w - 46 - inset, y: f.y + inset, w: 46, h: 80, title: "Legend", columns: 1 };
      break;
    case "scale_bar":
      el = { ...base, kind, x: f.x + inset, y: f.y + f.h - 10 - inset, w: 46, h: 10, units: "km", segments: 4 };
      break;
    case "north_arrow":
      el = { ...base, kind, x: f.x + inset, y: f.y + inset, w: 12, h: 18, style: "simple" };
      break;
    case "sources":
      el = { ...base, kind, x: f.x, y: f.y + f.h + 1.5, w: f.w, h: 8, size: 6 };
      break;
  }
  s.addElement(el);
  return el.id;
}

export function addText(text = "{layout_name}"): string | null {
  const s = useLayouts.getState();
  if (!s.doc) return null;
  const p = s.doc.page;
  const el: TextElement = {
    kind: "text",
    id: nextId(s.doc, "t"),
    x: p.margin,
    y: Math.min(10, p.margin),
    w: p.width - 2 * p.margin,
    h: 14,
    rotation: 0,
    locked: false,
    visible: true,
    text,
    font: "sans-bold",
    size: 18,
    align: "center",
    color: "#000000",
  };
  s.addElement(el);
  return el.id;
}
