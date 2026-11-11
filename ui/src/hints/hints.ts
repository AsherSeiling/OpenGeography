// UX15: first-run hints. A short tip shows in an empty node editor and an empty layout until it is dismissed, and all of them can be turned
// off in Settings. Which tips were dismissed is a per-viewer convenience (localStorage, ignored when unavailable); the on/off choice is a
// program setting (`showHints`).
import { create } from "zustand";

export type HintId = "nodeEditor" | "layout";

export const HINTS: Record<HintId, { title: string; text: string }> = {
  nodeEditor: {
    title: "Node editor",
    text: "Every layer has its own node tree. Press Shift+A to add a tool, drag from a card's output dot to another card's input to connect them, then press Run. The Output card never changes your source data.",
  },
  layout: {
    title: "Layout",
    text: "A layout is a printed page. Use Insert › Map Frame to put a map on it, then add a legend, scale bar and north arrow. Everything on the page stays editable, and the map frame can be a different scale or coordinate system from your map.",
  },
};

const KEY = "gis.hints.dismissed";

function read(): HintId[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x): x is HintId => x in HINTS) : [];
  } catch {
    return [];
  }
}

function write(ids: HintId[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(ids));
  } catch {
    /* storage unavailable: dismissed for this session only */
  }
}

export const useHints = create<{ dismissed: HintId[]; dismiss: (id: HintId) => void; reset: () => void }>((set, get) => ({
  dismissed: read(),
  dismiss: (id) => {
    const dismissed = [...new Set([...get().dismissed, id])];
    set({ dismissed });
    write(dismissed);
  },
  reset: () => {
    set({ dismissed: [] });
    write([]);
  },
}));

/** Whether a tip shows: tips are on (unset = on), this one was not dismissed, the place is still empty, and no test driver is running the page. */
export function hintVisible(o: { showHints: boolean | null | undefined; dismissed: readonly HintId[]; id: HintId; empty: boolean; automated?: boolean }): boolean {
  return o.showHints !== false && !o.dismissed.includes(o.id) && o.empty && !o.automated;
}
