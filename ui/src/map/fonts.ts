// Fonts installed on this computer, listed once by the engine (Tauri shell or dev server). Falls back to a short list of
// generic families when the list can't be had, so the picker always works.
import { create } from "zustand";
import { api, type FontFamily } from "../ipc/api";

export const FALLBACK_FONTS: FontFamily[] = ["Arial", "Georgia", "Times New Roman", "Courier New", "Verdana"].map((name) => ({ name, bold: true, italic: true, monospace: name === "Courier New" }));

export const useFonts = create<{ fonts: FontFamily[] | null; failed: boolean; load: () => Promise<void> }>((set, get) => ({
  fonts: null,
  failed: false,
  load: async () => {
    if (get().fonts) return;
    try {
      const f = await api.listFonts();
      set({ fonts: f.length ? f : FALLBACK_FONTS, failed: !f.length });
    } catch {
      set({ fonts: FALLBACK_FONTS, failed: true });
    }
  },
}));

/** Families whose name contains `q` (case-insensitive), at most `limit`. */
export function matchFonts(fonts: FontFamily[], q: string, limit = 80): FontFamily[] {
  const w = q.trim().toLowerCase();
  return (w ? fonts.filter((f) => f.name.toLowerCase().includes(w)) : fonts).slice(0, limit);
}
