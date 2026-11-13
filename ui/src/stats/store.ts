// Statistics sheets of the project: the open sheet, the selected card, and saving. A sheet is a document in the project database like a layout;
// edits save to the engine (debounced 500 ms) and mark the project dirty. Saves run one at a time per sheet, each sending the newest version
// (the same serialised pattern as the layouts' LAY-SAVE-1 fix), so two cannot collide in the database or land out of order.
import { create } from "zustand";
import { api, errorText } from "../ipc/api";
import { reportError } from "../errors/store";
import { useApp } from "../store/store";
import { SHEET_VERSION, type StatCard, type StatResult, type StatSheet } from "./types";

interface StatsState {
  /** Every sheet of the project. */
  docs: Record<string, StatSheet>;
  /** The open (active) sheet. */
  id: string | null;
  /** A stats tab is the active tab of the window (the statistics workspace); false when the map or another pane has focus. */
  workspace: boolean;
  /** Selected card of the open sheet (its settings show in Properties). */
  selected: string | null;
  /** Cards running now, as `sheet/card`. */
  running: Record<string, boolean>;
  /** Why a card's last run failed, as `sheet/card`; kept in the session only. */
  errors: Record<string, string>;
  /** Data pane and new cards: use the map's selected features only. */
  selectedOnly: boolean;
  loadAll: (sheets: StatSheet[]) => void;
  open: (id: string | null) => void;
  setWorkspace: (on: boolean) => void;
  list: () => StatSheet[];
  add: (sheet: StatSheet) => void;
  remove: (id: string) => void;
  rename: (id: string, name: string) => void;
  select: (cardId: string | null) => void;
  setSelectedOnly: (on: boolean) => void;
  addCard: (sheetId: string, card: StatCard) => void;
  removeCard: (sheetId: string, cardId: string) => void;
  moveCard: (sheetId: string, cardId: string, delta: -1 | 1) => void;
  /** Change a card's title, inputs or options. Its result is cleared: numbers from other settings must not look like these settings' numbers. */
  editCard: (sheetId: string, cardId: string, patch: Partial<Pick<StatCard, "title" | "inputs" | "options">>) => void;
  setResult: (sheetId: string, cardId: string, result: StatResult) => void;
  setRunning: (key: string, on: boolean) => void;
  setError: (key: string, message: string | null) => void;
}

const SAVE_DELAY = 500;
const timers: Record<string, ReturnType<typeof setTimeout>> = {};
/** The save in flight for each sheet. */
const saving: Record<string, Promise<void>> = {};
/** Sheets deleted in this session: a save still queued for one must not bring it back. */
const deleted = new Set<string>();

export const cardKey = (sheetId: string, cardId: string) => `${sheetId}/${cardId}`;

export function newStatId(prefix: string): string {
  return `${prefix}${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

export function newSheetDoc(name: string): StatSheet {
  return { id: newStatId("S"), name, version: SHEET_VERSION, cards: [] };
}

function latest(id: string): StatSheet | null {
  return useStats.getState().docs[id] ?? null;
}

function saveNow(id: string): Promise<void> {
  clearTimeout(timers[id]);
  delete timers[id];
  return (saving[id] = (saving[id] ?? Promise.resolve()).then(() => writeLatest(id)));
}

async function writeLatest(id: string) {
  const s = latest(id);
  if (!s || deleted.has(id)) return;
  try {
    await api.saveStatSheet(s);
  } catch (e) {
    useApp.getState().setStatus(`Statistics sheet '${s.name}' could not be saved: ${errorText(e)}`);
    reportError(e, `stat-sheet-save:${id}`, { title: `Statistics sheet '${s.name}' could not be saved`, context: "saving a statistics sheet" });
  }
}

function scheduleSave(id: string) {
  clearTimeout(timers[id]);
  timers[id] = setTimeout(() => void saveNow(id), SAVE_DELAY);
  useApp.getState().set({ dirty: true });
}

/** Write any sheet edits still waiting for their debounce (before saving or closing the project). */
export async function flushStatSaves() {
  await Promise.all([...Object.keys(timers).map(saveNow), ...Object.values(saving)]);
}

const mapCards = (s: StatSheet, f: (c: StatCard) => StatCard): StatSheet => ({ ...s, cards: s.cards.map(f) });

export const useStats = create<StatsState>()((set, get) => {
  const change = (id: string, f: (s: StatSheet) => StatSheet) => {
    const s = get().docs[id];
    if (!s) return;
    set({ docs: { ...get().docs, [id]: f(s) } });
    scheduleSave(id);
  };
  return {
    docs: {},
    id: null,
    workspace: false,
    selected: null,
    running: {},
    errors: {},
    selectedOnly: false,
    loadAll: (sheets) => {
      for (const k of Object.keys(timers)) clearTimeout(timers[k]), delete timers[k];
      deleted.clear();
      set({ docs: Object.fromEntries(sheets.map((s) => [s.id, s])), id: null, selected: null, running: {}, errors: {} });
    },
    open: (id) => {
      if (id === get().id) return;
      set({ id: id && get().docs[id] ? id : null, selected: null });
    },
    setWorkspace: (on) => set({ workspace: on }),
    list: () => Object.values(get().docs).sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id)),
    add: (sheet) => {
      set({ docs: { ...get().docs, [sheet.id]: sheet } });
      scheduleSave(sheet.id);
    },
    remove: (id) => {
      const { [id]: _gone, ...docs } = get().docs;
      clearTimeout(timers[id]);
      delete timers[id];
      deleted.add(id);
      set({ docs, id: get().id === id ? null : get().id, selected: get().id === id ? null : get().selected });
      useApp.getState().set({ dirty: true });
      // After any save of it still in flight, so the delete is the last word.
      saving[id] = (saving[id] ?? Promise.resolve()).then(async () => {
        try {
          await api.deleteStatSheet(id);
        } catch (e) {
          reportError(e, `stat-sheet-delete:${id}`, { title: "The statistics sheet could not be deleted", context: "deleting a statistics sheet" });
        }
      });
    },
    rename: (id, name) => change(id, (s) => ({ ...s, name })),
    select: (cardId) => set({ selected: cardId }),
    setSelectedOnly: (on) => set({ selectedOnly: on }),
    addCard: (sheetId, card) => {
      change(sheetId, (s) => ({ ...s, cards: [...s.cards, card] }));
      set({ selected: card.id });
    },
    removeCard: (sheetId, cardId) => {
      change(sheetId, (s) => ({ ...s, cards: s.cards.filter((c) => c.id !== cardId) }));
      if (get().selected === cardId) set({ selected: null });
    },
    moveCard: (sheetId, cardId, delta) =>
      change(sheetId, (s) => {
        const i = s.cards.findIndex((c) => c.id === cardId);
        const j = i + delta;
        if (i < 0 || j < 0 || j >= s.cards.length) return s;
        const cards = [...s.cards];
        [cards[i], cards[j]] = [cards[j], cards[i]];
        return { ...s, cards };
      }),
    editCard: (sheetId, cardId, patch) => {
      change(sheetId, (s) => mapCards(s, (c) => (c.id === cardId ? { ...c, ...patch, ...("inputs" in patch || "options" in patch ? { result: null } : {}) } : c)));
      set({ errors: { ...get().errors, [cardKey(sheetId, cardId)]: "" } });
    },
    setResult: (sheetId, cardId, result) => change(sheetId, (s) => mapCards(s, (c) => (c.id === cardId ? { ...c, result } : c))),
    setRunning: (key, on) => set({ running: { ...get().running, [key]: on } }),
    setError: (key, message) => set({ errors: { ...get().errors, [key]: message ?? "" } }),
  };
});
