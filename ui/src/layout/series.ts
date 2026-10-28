// Map series in the UI (C3.2): the page list the engine works out, and which page the Layout view is showing.
import { create } from "zustand";
import { api, errorText, type SeriesPageInfo } from "../ipc/api";
import type { Layout, SeriesSpec } from "./types";

interface SeriesView {
  /** Pages of the open layout's series (empty when it has none, or while it can't be worked out). */
  pages: SeriesPageInfo[];
  /** Why there are no pages, in words, or null. */
  error: string | null;
  /** The page shown in the Layout view (0-based), or null: the frames as saved. */
  page: number | null;
  /** Work out the pages of `layout`'s series again. */
  refresh: (layout: Layout | null) => Promise<void>;
  show: (page: number | null) => void;
}

let seq = 0;

export const useSeriesView = create<SeriesView>((set, get) => ({
  pages: [],
  error: null,
  page: null,
  refresh: async (layout) => {
    const mine = ++seq;
    if (!layout?.series) {
      set({ pages: [], error: null, page: null });
      return;
    }
    try {
      const pages = await api.layoutSeriesPages(layout);
      if (mine !== seq) return;
      const page = get().page;
      set({ pages, error: null, page: page === null ? null : Math.min(page, pages.length - 1) });
    } catch (e) {
      if (mine === seq) set({ pages: [], error: errorText(e), page: null });
    }
  },
  show: (page) => set((s) => ({ page: page === null ? null : Math.max(0, Math.min(s.pages.length - 1, page)) })),
}));

/** The series with `patch` applied, or a first series on `indexLayer`. */
export function withSeries(cur: SeriesSpec | null, patch: Partial<SeriesSpec>, firstField: string): SeriesSpec {
  return { indexLayer: "", nameField: firstField, extent: "fit", marginPct: 10, ...(cur ?? {}), ...patch };
}

/** A margin typed as text: 0–50 percent, or null. */
export function parseMargin(text: string): number | null {
  const v = Number(text.trim().replace("%", ""));
  return text.trim() !== "" && Number.isFinite(v) && v >= 0 && v <= 50 ? v : null;
}

/** A scale typed as "25000", "1:25,000": a whole number of at least 1, or null. */
export function parseSeriesScale(text: string): number | null {
  const v = Math.round(Number(text.replace(/[, ]/g, "").replace(/^1:/, "")));
  return Number.isFinite(v) && v >= 1 ? v : null;
}

/** The "Page 3 of 10 — Dane" caption. */
export function pageCaption(page: number | null, pages: SeriesPageInfo[]): string {
  if (!pages.length) return "No pages";
  if (page === null) return `${pages.length} page${pages.length === 1 ? "" : "s"}`;
  return `Page ${page + 1} of ${pages.length} — ${pages[page]?.name || "(no name)"}`;
}
