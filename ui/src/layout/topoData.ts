// Topo Map Builder, data step: which layer is the elevation, and how the contour layer is drawn (index contours darker and labelled).
import type { LabelStyle, LayerStyle } from "../store/store";
import type { LayerInfo } from "../ipc/api";

/** The elevation raster to make contours from: the topmost elevation raster in Contents (new downloads go on top). */
export function pickDem(order: string[], layers: Record<string, LayerInfo>): LayerInfo | null {
  for (const id of order) {
    const l = layers[id];
    if (l?.raster?.kind === "elevation") return l;
  }
  return null;
}

/** Brown contours; the index lines (INDEX = 1) darker, and only they carry the elevation numbers. */
export function contourStyle(base: LayerStyle): LayerStyle {
  const label: LabelStyle = { ...(base.label ?? ({} as LabelStyle)), enabled: true, field: "ELEV_LABEL", size: 7, color: "#6b4423", haloColor: "#ffffff", haloWidth: 2, indexOnly: true, linePos: "on" };
  return {
    ...base,
    kind: "categorized",
    field: "INDEX",
    classes: [
      { value: "1", color: "#8b5a2b", label: "Index contour" },
      { value: "0", color: "#c9a26b", label: "Intermediate contour" },
    ],
    stroke: "#8b5a2b",
    strokeWidth: 0.8,
    label,
  };
}

/** Shaded relief is drawn in grays on the fixed 0–255 scale gdal writes it in (the elevation colour ramp would hide the shading; a min–max stretch turns flat ground black). */
export function hillshadeStyle(base: LayerStyle): LayerStyle {
  return { ...base, raster: { renderer: "gray", band: 1, stretch: { mode: "manual", min: 0, max: 255 } } };
}

/** "Sheet name" used in layer names: the title, else the quadrangle-style default. */
export function sheetLabel(title: string): string {
  return title.trim() || "Topographic map";
}
