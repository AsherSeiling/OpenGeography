// Layout themes (CG9): the engine owns the definitions (`layout_themes`); this applies a theme's palette to the layers.
import type { LayoutTheme } from "../ipc/api";
import type { LayerInfo } from "../ipc/api";
import type { LayerStyle } from "../store/store";

/** New styles for the single-symbol vector layers, cycling the palette in draw order (bottom layer first so a layer keeps its colour when others are added on top). Categorized, graduated and raster layers are left alone. */
export function recolour(order: string[], layers: Record<string, LayerInfo>, styles: Record<string, LayerStyle>, palette: LayoutTheme["palette"]): Record<string, LayerStyle> {
  const out: Record<string, LayerStyle> = {};
  if (!palette.length) return out;
  let i = 0;
  for (const id of [...order].reverse()) {
    const info = layers[id];
    const st = styles[id];
    if (!info || !st || st.kind !== "single" || !(info.geomType === "Point" || info.geomType === "Line" || info.geomType === "Polygon" || info.geomType === "Mixed")) continue;
    const p = palette[i++ % palette.length];
    out[id] = {
      ...st,
      fill: p.fill,
      // Lines and points are drawn in their fill colour; polygons use the palette's outline.
      stroke: info.geomType === "Polygon" ? p.stroke : p.fill,
      strokeWidth: info.geomType === "Line" ? Math.max(st.strokeWidth, 1) : p.strokeWidth,
      opacity: p.opacity,
    };
  }
  return out;
}
