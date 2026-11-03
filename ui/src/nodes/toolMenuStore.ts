// State of the Shift+A tool menu: open or closed, where, and the user's favourites and recent tools (localStorage, ignored when the
// storage is unavailable). `openToolMenu` is the entry point for every opener: the shortcut, a right-click, the Node Editor's Add
// button and the ribbon's Add Tool.
import { create } from "zustand";

const KEY = "gis.toolmenu";
const RECENT_MAX = 8;

interface Prefs {
  fav: string[];
  recent: string[];
}

function readPrefs(): Prefs {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    const list = (x: unknown) => (Array.isArray(x) ? x.filter((s): s is string => typeof s === "string") : []);
    return { fav: list(v?.fav), recent: list(v?.recent).slice(0, RECENT_MAX) };
  } catch {
    return { fav: [], recent: [] };
  }
}

function writePrefs(p: Prefs) {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* storage unavailable: the choice lasts for this session only */
  }
}

export interface MenuPos {
  x: number;
  y: number;
}

interface ToolMenuState extends Prefs {
  open: boolean;
  /** Screen position the menu opened at (the pointer); none = centred on the window. */
  pos: MenuPos | null;
  /** Id of the connection under the pointer when the menu opened, so a pick splices into it. */
  wire: string | null;
  /** Last thing the shortcut or menu did, shown in the Node Editor toolbar for a few seconds. */
  note: { text: string; at: number } | null;
  close: () => void;
  toggleFav: (id: string) => void;
  addRecent: (id: string) => void;
  say: (text: string) => void;
}

export const useToolMenu = create<ToolMenuState>((set, get) => ({
  ...readPrefs(),
  open: false,
  pos: null,
  wire: null,
  note: null,
  close: () => set({ open: false }),
  toggleFav: (id) => {
    const fav = get().fav.includes(id) ? get().fav.filter((x) => x !== id) : [...get().fav, id];
    set({ fav });
    writePrefs({ fav, recent: get().recent });
  },
  addRecent: (id) => {
    const recent = [id, ...get().recent.filter((x) => x !== id)].slice(0, RECENT_MAX);
    set({ recent });
    writePrefs({ fav: get().fav, recent });
  },
  say: (text) => set({ note: { text, at: Date.now() } }),
}));

/** The connection drawn under a screen point, if any. */
export function wireAt(p: MenuPos): string | null {
  const el = document.elementFromPoint(p.x, p.y)?.closest?.(".react-flow__edge") as HTMLElement | null;
  return el?.dataset.id ?? null;
}

/** Open the tool menu at a screen position (the pointer), or centred on the window. */
export function openToolMenu(pos?: MenuPos | null) {
  const wire = pos ? wireAt(pos) : null;
  useToolMenu.setState({ open: true, pos: pos ?? null, wire, note: { text: "Add menu opened", at: Date.now() } });
}

export function closeToolMenu() {
  useToolMenu.getState().close();
}
