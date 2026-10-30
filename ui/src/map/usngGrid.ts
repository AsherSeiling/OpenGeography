// The USNG / MGRS grid on the map: which square size suits the view, and how the grid layer is drawn (outlines and labels, no fill).
import type { LabelStyle, LayerStyle } from "../store/store";

export const GRID_SIZES: [number, string][] = [[100_000, "100 km"], [10_000, "10 km"], [1_000, "1 km"], [100, "100 m"]];

/** Ground width in metres of a longitude/latitude box at its middle latitude. */
export function groundWidthM(bbox: [number, number, number, number]): number {
  const lat = (bbox[1] + bbox[3]) / 2;
  return Math.abs(bbox[2] - bbox[0]) * 111_320 * Math.cos((lat * Math.PI) / 180);
}

/**
 * The square size that gives about 4 to 20 squares across the view: the smallest size that keeps it under 20, so zooming in shows finer
 * squares. Never finer than 100 m or coarser than 100 km.
 */
export function gridSizeFor(widthM: number): number {
  for (const [size] of [...GRID_SIZES].reverse()) {
    if (widthM / size <= 20) return size;
  }
  return 100_000;
}

export function gridLabel(size: number): string {
  return GRID_SIZES.find((g) => g[0] === size)?.[1] ?? `${size} m`;
}

/** Outlines in red with the reference written in each square; the fill is switched off (opacity 0 hides it, outlines stay). */
export function usngGridStyle(base: LayerStyle, field = "MGRS"): LayerStyle {
  const label: LabelStyle = { ...(base.label ?? ({} as LabelStyle)), enabled: true, field, size: 9, color: "#a61e1e", haloColor: "#ffffff", haloWidth: 2 };
  return { ...base, kind: "single", fill: "#ffffff", opacity: 0, stroke: "#c0392b", strokeWidth: 1, label };
}
