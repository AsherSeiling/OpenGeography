// The resolution box in Layout → Export (C3.0): PNG pixels, and the images (basemap, rasters) inside PDFs.
import { create } from "zustand";

export const DPI_MIN = 72;
export const DPI_MAX = 600;
export const DPI_DEFAULT = 300;

/** A typed dpi as a whole number in range, or null while it isn't one yet. */
export function parseDpi(text: string): number | null {
  const v = Math.round(Number(text.trim()));
  return text.trim() !== "" && Number.isFinite(v) && v >= DPI_MIN && v <= DPI_MAX ? v : null;
}

const KEY = "gis-studio.exportDpi";
const saved = (() => {
  try {
    return parseDpi(localStorage.getItem(KEY) ?? "") ?? DPI_DEFAULT;
  } catch {
    return DPI_DEFAULT;
  }
})();

export const useExportDpi = create<{ dpi: number; set: (dpi: number) => void }>((set) => ({
  dpi: saved,
  set: (dpi) => {
    try {
      localStorage.setItem(KEY, String(dpi));
    } catch {
      /* a private window: the choice lasts for this session */
    }
    set({ dpi });
  },
}));
