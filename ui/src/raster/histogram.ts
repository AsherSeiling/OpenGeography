// Helpers for the raster histogram widget: value ↔ x mapping and handle dragging.

export interface HistogramView {
  min: number;
  max: number;
  width: number;
}

export function valueToX(v: number, h: HistogramView): number {
  const span = h.max - h.min || 1;
  return Math.min(h.width, Math.max(0, ((v - h.min) / span) * h.width));
}

export function xToValue(x: number, h: HistogramView): number {
  const span = h.max - h.min || 1;
  return h.min + (Math.min(h.width, Math.max(0, x)) / h.width) * span;
}

/** Move one handle to `x`; the low handle stays below the high one by at least 1 px of range. */
export function dragHandle(which: "min" | "max", x: number, lo: number, hi: number, h: HistogramView): [number, number] {
  const v = xToValue(x, h);
  const gap = (h.max - h.min) / (h.width || 1);
  return which === "min" ? [Math.min(v, hi - gap), hi] : [lo, Math.max(v, lo + gap)];
}

/** Bar heights 0–1 on a log scale (a few huge bins would flatten the rest). */
export function barHeights(counts: number[]): number[] {
  const m = Math.log1p(Math.max(0, ...counts)) || 1;
  return counts.map((c) => Math.log1p(c) / m);
}

/** CSS gradient for a ramp preview. */
export function rampCss(colors: string[], invert = false): string {
  const c = invert ? [...colors].reverse() : colors;
  return `linear-gradient(to right, ${c.join(", ")})`;
}

/** Format a raster value compactly for labels and inputs. */
export function fmtValue(v: number): string {
  if (!Number.isFinite(v)) return "";
  const a = Math.abs(v);
  return a !== 0 && (a >= 1e6 || a < 1e-3) ? v.toExponential(2) : String(Math.round(v * 1000) / 1000);
}
