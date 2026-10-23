// Per-user bibliography preferences (citation style shown in the Sources
// pane; whether saving a project also writes its bibliography files).
import { create } from "zustand";
import type { Style } from "./cite";

interface BibPrefs {
  style: Style;
  autoExport: boolean;
  set: (p: Partial<Pick<BibPrefs, "style" | "autoExport">>) => void;
}

const KEY = "gis-studio.bibliography.v1";

function load(): Pick<BibPrefs, "style" | "autoExport"> {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { style: ["apa", "mla", "chicago"].includes(v.style) ? v.style : "apa", autoExport: v.autoExport !== false };
  } catch {
    return { style: "apa", autoExport: true };
  }
}

export const useBibPrefs = create<BibPrefs>((set, get) => ({
  ...load(),
  set: (p) => {
    set(p);
    try {
      localStorage.setItem(KEY, JSON.stringify({ style: get().style, autoExport: get().autoExport }));
    } catch {
      // Not saved; applies for this session.
    }
  },
}));
