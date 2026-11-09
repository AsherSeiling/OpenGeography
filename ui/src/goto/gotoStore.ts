// Go To Coordinates (RB4): the panel's open state and the temporary marker. Neither is saved with the project (no layer, no data, and
// the project is not marked changed). The chosen format is remembered per computer in the webview's own storage.
import { create } from "zustand";
import type { CoordFormat } from "../statusbar/coordFormat";

export type GoToKind = "latlon" | "xy" | "utm" | "usng" | "crs";

export const GOTO_KINDS: { id: GoToKind; label: string }[] = [
  { id: "latlon", label: "Latitude and longitude (decimal or degrees minutes seconds)" },
  { id: "xy", label: "Map coordinates (x and y in the map's coordinate system)" },
  { id: "utm", label: "UTM (zone, hemisphere, easting, northing)" },
  { id: "usng", label: "USNG / MGRS grid reference" },
  { id: "crs", label: "Another coordinate system (x and y)" },
];

const KEY = "gis-studio.goto.format";

/** The panel's format that matches the status bar's (UX14): the list starts there until the user picks another. */
export function kindForStatusFormat(f: CoordFormat): GoToKind {
  return f === "projected" ? "xy" : f === "mgrs" ? "usng" : "latlon";
}

export function readSavedKind(): GoToKind | null {
  try {
    const v = localStorage.getItem(KEY);
    return GOTO_KINDS.some((k) => k.id === v) ? (v as GoToKind) : null;
  } catch {
    return null;
  }
}

export function saveKind(k: GoToKind) {
  try {
    localStorage.setItem(KEY, k);
  } catch {
    /* storage unavailable: the choice just isn't remembered */
  }
}

export interface Marker {
  mapId: string;
  lon: number;
  lat: number;
}

interface GoToState {
  open: boolean;
  marker: Marker | null;
  setOpen: (o: boolean) => void;
  setMarker: (m: Marker | null) => void;
}

export const useGoTo = create<GoToState>((set) => ({
  open: false,
  marker: null,
  setOpen: (open) => set({ open }),
  setMarker: (marker) => set({ marker }),
}));
