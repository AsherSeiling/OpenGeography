// Topo Map Builder: turning what the user typed or picked into the sheet's centre (longitude, latitude), and the choices the dialog offers.
import { toPoint } from "mgrs";

export const SCALES: [number, string][] = [[24000, "1:24,000 (USGS 7.5′ quad)"], [25000, "1:25,000 (military)"], [50000, "1:50,000"], [100000, "1:100,000"]];
export const PAPERS: [string, string][] = [["letter", "Letter"], ["tabloid", "Tabloid (11 × 17 in)"], ["a4", "A4"], ["a3", "A3"]];

export { TOPO_PRESETS, type TopoPreset } from "./topoPresets";

/**
 * A centre typed as "lat, lon" (decimal degrees, south and west negative) or as an MGRS/USNG reference ("15T VG 975 475", "15TVG9747").
 * An MGRS reference gives the middle of the square it names. Returns [lon, lat] or an explanation.
 */
export function parseCenter(text: string): { center: [number, number] } | { error: string } {
  const t = text.trim();
  if (!t) return { error: "Type a position: latitude, longitude (for example 42.0, -93.0) or an MGRS reference (15T VG 975 475)." };
  const nums = t.split(/[\s,;]+/).filter(Boolean).map(Number);
  if (nums.length === 2 && nums.every(Number.isFinite)) {
    const [lat, lon] = nums;
    if (Math.abs(lat) > 80) return { error: "The latitude must be between 80°S and 80°N: UTM does not cover the poles." };
    if (Math.abs(lon) > 180) return { error: "The longitude must be between −180° and 180°." };
    return { center: [lon, lat] };
  }
  const compact = t.replace(/\s+/g, "").toUpperCase();
  if (/^\d{1,2}[C-HJ-NP-X][A-HJ-NP-Z]{2}(\d{2}|\d{4}|\d{6}|\d{8}|\d{10})?$/.test(compact)) {
    try {
      const [lon, lat] = toPoint(compact);
      if (Number.isFinite(lon) && Number.isFinite(lat)) return { center: [lon, lat] };
    } catch {
      /* falls through */
    }
  }
  return { error: `"${t}" is not a position I can read. Write latitude, longitude (42.0, -93.0) or an MGRS reference (15T VG 975 475).` };
}

/** The 7.5′ quadrangle (a 7.5′ × 7.5′ cell on multiples of 7.5′) that contains the point, and its centre. */
export function quadCenter(lon: number, lat: number): { center: [number, number]; sw: [number, number]; name: string } {
  const cell = 7.5 / 60;
  const w = Math.floor(lon / cell) * cell;
  const s = Math.floor(lat / cell) * cell;
  const dm = (v: number, p: string, n: string) => {
    const secs = Math.round(Math.abs(v) * 3600);
    const [d, m, sec] = [Math.floor(secs / 3600), Math.floor((secs % 3600) / 60), secs % 60];
    return `${d}°${String(m).padStart(2, "0")}′${sec ? `${String(sec).padStart(2, "0")}″` : ""}${v < 0 ? n : p}`;
  };
  return { center: [w + cell / 2, s + cell / 2], sw: [w, s], name: `${dm(s, "N", "S")} ${dm(w, "E", "W")}` };
}

/** NAD 83 covers North America (UTM zones 1–23 north); anywhere else the sheet is on WGS 84. */
export function defaultDatum(lon: number, lat: number): "nad83" | "wgs84" {
  const zone = Math.min(59, Math.max(0, Math.floor((lon + 180) / 6))) + 1;
  return lat >= 0 && zone <= 23 ? "nad83" : "wgs84";
}

/** Today as YYYY-MM-DD in the user's own calendar. */
export function todayIso(d = new Date()): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
