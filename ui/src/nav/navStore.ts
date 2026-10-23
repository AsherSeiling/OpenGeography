// State for the Measure tool and the Land Nav (resection/intersection) panel,
// plus the temporary sketch drawn on the map for both.
import { create } from "zustand";
import type { LonLat, North } from "./geodesy";

export type MeasureMode = "distance" | "area";
export type AngleUnit = "deg" | "mils";

interface NavState {
  measureMode: MeasureMode;
  points: LonLat[];
  done: boolean;
  lengthUnit: string;
  areaUnit: string;
  north: North;
  angleUnit: AngleUnit;
  /** Which land-nav panel is open. */
  navPanel: null | "resection" | "intersection";
  /** Set while the user is picking a point on the map for the nav panel. */
  pick: null | ((p: LonLat) => void);
  /** Drawn by the nav panel: rays, the fix, and the error triangle. */
  rays: [LonLat, LonLat][];
  fix: LonLat | null;
  triangle: LonLat[];
  known: LonLat[];
  set: (p: Partial<NavState>) => void;
  addPoint: (p: LonLat) => void;
  clearMeasure: () => void;
  clearNav: () => void;
}

export const useNav = create<NavState>((set, get) => ({
  measureMode: "distance",
  points: [],
  done: false,
  lengthUnit: "mi",
  areaUnit: "acres",
  north: "true",
  angleUnit: "deg",
  navPanel: null,
  pick: null,
  rays: [],
  fix: null,
  triangle: [],
  known: [],
  set: (p) => set(p),
  addPoint: (p) => set({ points: get().done ? [p] : [...get().points, p], done: false }),
  clearMeasure: () => set({ points: [], done: false }),
  clearNav: () => set({ rays: [], fix: null, triangle: [], known: [] }),
}));
