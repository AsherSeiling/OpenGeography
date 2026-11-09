// The status bar shows the pointer in ONE chosen format (UX14), remembered in the program settings.
import { forward as toMgrs } from "mgrs";

export type CoordFormat = "dd" | "dms" | "projected" | "mgrs";

export const COORD_FORMATS: { id: CoordFormat; label: string; title: string }[] = [
  { id: "dd", label: "Decimal degrees", title: "Latitude, longitude in decimal degrees (WGS 84)" },
  { id: "dms", label: "Degrees minutes seconds", title: "Latitude, longitude in degrees, minutes and seconds (WGS 84)" },
  { id: "projected", label: "Map coordinates (E, N)", title: "Easting and northing in the map's coordinate system" },
  { id: "mgrs", label: "MGRS / USNG", title: "MGRS / USNG grid reference (1 m)" },
];

export const DEFAULT_COORD_FORMAT: CoordFormat = "dd";

/** A saved value, or the default when it is missing or unknown. */
export function readCoordFormat(v: unknown): CoordFormat {
  return COORD_FORMATS.some((f) => f.id === v) ? (v as CoordFormat) : DEFAULT_COORD_FORMAT;
}

export function dms(v: number, pos: string, neg: string) {
  const a = Math.abs(v);
  const d = Math.floor(a);
  const m = Math.floor((a - d) * 60);
  const s = ((a - d) * 60 - m) * 60;
  return `${d}°${String(m).padStart(2, "0")}′${s.toFixed(1).padStart(4, "0")}″${v >= 0 ? pos : neg}`;
}

export function mgrsOf(lon: number, lat: number): string {
  if (lat < -80 || lat > 84) return "—";
  try {
    // "14SPF1234567890" → "14S PF 12345 67890" (readable grid reference)
    const m = toMgrs([lon, lat], 5);
    const r = m.match(/^(\d{1,2}[A-Z])([A-Z]{2})(\d{5})(\d{5})$/);
    return r ? `${r[1]} ${r[2]} ${r[3]} ${r[4]}` : m;
  } catch {
    return "—";
  }
}

export interface PointerPos {
  lon: number;
  lat: number;
  /** The pointer in the map's own coordinates. */
  x: number;
  y: number;
}

/**
 * The text for the chosen format. "Map coordinates" is for a projected map; when the map is drawn in degrees there are no
 * eastings and northings, so the reading falls back to decimal degrees and says so in `title`.
 */
export function formatPointer(fmt: CoordFormat, p: PointerPos, mapIsProjected: boolean): { text: string; title: string; format: CoordFormat } {
  const info = COORD_FORMATS.find((f) => f.id === fmt)!;
  switch (fmt) {
    case "dms":
      return { text: `${dms(p.lat, "N", "S")} ${dms(p.lon, "E", "W")}`, title: info.title, format: fmt };
    case "mgrs":
      return { text: mgrsOf(p.lon, p.lat), title: info.title, format: fmt };
    case "projected":
      if (mapIsProjected) return { text: `${p.x.toLocaleString("en-US", { maximumFractionDigits: 1 })} E ${p.y.toLocaleString("en-US", { maximumFractionDigits: 1 })} N`, title: info.title, format: fmt };
      return { text: `${p.lat.toFixed(5)}°, ${p.lon.toFixed(5)}°`, title: "This map is drawn in degrees, so there are no eastings and northings: showing decimal degrees (WGS 84)", format: "dd" };
    default:
      return { text: `${p.lat.toFixed(5)}°, ${p.lon.toFixed(5)}°`, title: info.title, format: "dd" };
  }
}
