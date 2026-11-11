/**
 * The one parser and formatter for stored colours (docs/COLOUR_AND_OUTLINE_PLAN.md, CP1).
 * A stored colour is `#rrggbb` (opaque) or `#rrggbbaa`. Old projects hold only the first form and mean the same as before.
 * A colour that is fully opaque is written back as `#rrggbb`, so editing a colour never turns old data into the longer form
 * unless the user actually made it transparent.
 */

export interface Rgba {
  r: number;
  g: number;
  b: number;
  /** 0 (clear) to 1 (opaque). */
  a: number;
}

const HEX = /^#?([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

/** Parses `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` (the `#` is optional). Anything else is null. */
export function parseColor(s: unknown): Rgba | null {
  if (typeof s !== "string") return null;
  const m = HEX.exec(s.trim());
  if (!m) return null;
  let h = m[1];
  if (h.length <= 4) h = [...h].map((c) => c + c).join("");
  const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
  return { r: n(0), g: n(2), b: n(4), a: h.length === 8 ? n(6) / 255 : 1 };
}

const byte = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
const h2 = (v: number) => byte(v).toString(16).padStart(2, "0");

/** `#rrggbb` when opaque, `#rrggbbaa` otherwise (lower case). */
export function formatColor(c: Rgba): string {
  const a = byte(c.a * 255);
  return `#${h2(c.r)}${h2(c.g)}${h2(c.b)}${a === 255 ? "" : h2(a)}`;
}

/** Alpha 0..1 of a stored colour; 1 for a plain `#rrggbb` or anything that isn't a hex colour. */
export function alphaOf(s: unknown): number {
  return parseColor(s)?.a ?? 1;
}

/** The colour as `#rrggbb`, alpha dropped. Falls back to `fallback` when `s` isn't a hex colour. */
export function opaqueHex(s: unknown, fallback = "#000000"): string {
  const c = parseColor(s);
  return c ? formatColor({ ...c, a: 1 }) : fallback;
}

/** The same colour with alpha `a` (0..1). A non-hex input is returned unchanged. */
export function withAlpha(s: string, a: number): string {
  const c = parseColor(s);
  return c ? formatColor({ ...c, a: Math.max(0, Math.min(1, a)) }) : s;
}

/** Alpha as a whole percent, and back. Every whole percent survives the round trip through the stored 0-255 byte. */
export const alphaToPct = (a: number): number => Math.round(Math.max(0, Math.min(1, a)) * 100);
export const pctToAlpha = (p: number): number => byte((Math.max(0, Math.min(100, p)) / 100) * 255) / 255;

/** A CSS colour the map engine accepts: `#rrggbb` when opaque, `rgba(r,g,b,a)` otherwise (the style parser behind OpenLayers has no 8-digit hex). */
export function cssColor(s: string): string {
  const c = parseColor(s);
  if (!c || c.a >= 1) return s;
  return `rgba(${c.r},${c.g},${c.b},${Math.round(c.a * 1000) / 1000})`;
}

/** Colour with alpha multiplied by `k` (0..1): the final opacity is the colour's own alpha times a layer slider. */
export function scaleAlpha(s: string, k: number): string {
  const c = parseColor(s);
  return c ? formatColor({ ...c, a: c.a * k }) : s;
}

export interface Hsv {
  h: number;
  s: number;
  v: number;
  a: number;
}

export function rgbToHsv({ r, g, b, a }: Rgba): Hsv {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const d = max - Math.min(R, G, B);
  let h = 0;
  if (d) h = max === R ? ((G - B) / d + 6) % 6 : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return { h: h * 60, s: max ? d / max : 0, v: max, a };
}

export function hsvToRgb({ h, s, v, a }: Hsv): Rgba {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  return { r: byte(f(5) * 255), g: byte(f(3) * 255), b: byte(f(1) * 255), a };
}

/** Okabe-Ito colour-blind-safe set (the charts use it too). */
export const DEFAULT_COLOUR = "#000000";

export const OKABE_ITO = ["#000000", "#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00", "#cc79a7"];

const RECENT_KEY = "gis-studio.recent-colours";

/** Recent colours, newest first (per computer). Storage may be unavailable; then the list is empty. */
export function recentColours(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => parseColor(x)).slice(0, 12) : [];
  } catch {
    return [];
  }
}

export function rememberColour(c: string): void {
  if (!parseColor(c)) return;
  try {
    const next = [c.toLowerCase(), ...recentColours().filter((x) => x.toLowerCase() !== c.toLowerCase())].slice(0, 12);
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    /* storage unavailable: nothing is remembered */
  }
}
