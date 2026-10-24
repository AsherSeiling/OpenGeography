// OpenLayers styles: layer symbology through ol-mapbox-style, the cyan selection highlight and the measure/land-nav sketch.
import type { FeatureLike } from "ol/Feature";
import type VectorTileLayer from "ol/layer/VectorTile";
import CircleStyle from "ol/style/Circle";
import Fill from "ol/style/Fill";
import Stroke from "ol/style/Stroke";
import Style, { type StyleFunction } from "ol/style/Style";
import { stylefunction } from "ol-mapbox-style";
import type { GeomType } from "../ipc/api";
import type { LayerStyle } from "../store/store";
import { buildLayers, SELECTION_COLOR } from "./style";

function rgba(hex: string, a: number): string {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return `rgba(${r},${g},${b},${a})`;
}

function highlight(style: LayerStyle): Record<string, Style> {
  const width = Math.max(2, style.strokeWidth + 1);
  const stroke = new Stroke({ color: SELECTION_COLOR, width });
  const line = new Style({ stroke });
  return {
    Polygon: new Style({ fill: new Fill({ color: rgba(SELECTION_COLOR, style.opacity) }), stroke }),
    LineString: line,
    Point: new Style({ image: new CircleStyle({ radius: style.radius, fill: new Fill({ color: SELECTION_COLOR }), stroke: new Stroke({ color: "#0060a0", width: 0.6 }) }) }),
  };
}

const BASE: Record<string, string> = { MultiPolygon: "Polygon", MultiLineString: "LineString", MultiPoint: "Point" };

/**
 * Style function of one data layer: the Mapbox-style layers `buildLayers` emits (through
 * ol-mapbox-style), with selected features drawn in cyan instead. `selected()` is read at draw time, so
 * a selection change only needs `layer.changed()`, not new tiles.
 */
export function layerStyleFunction(layer: VectorTileLayer, id: string, geom: GeomType, style: LayerStyle, resolutions: number[], selected: () => ReadonlySet<number> | undefined): StyleFunction {
  const source = `src-${id}`;
  const glStyle = { version: 8, sources: { [source]: { type: "vector" } }, layers: buildLayers(id, geom, style, true) };
  const base = stylefunction(layer, glStyle, source, resolutions);
  const hl = highlight(style);
  return (feature: FeatureLike, resolution: number) => {
    const fid = feature.getId();
    const sel = selected();
    if (sel && fid !== undefined && sel.has(Number(fid))) {
      const t = feature.getGeometry()?.getType?.() ?? (feature as unknown as { getType(): string }).getType();
      return hl[BASE[t] ?? t];
    }
    return base(feature, resolution);
  };
}

/** Style of the measure / land-navigation sketch features (`kind` property). */
export const sketchStyle: StyleFunction = (feature) => {
  const kind = feature.get("kind") as string;
  switch (kind) {
    case "area":
      return new Style({ fill: new Fill({ color: "rgba(255,185,0,0.18)" }) });
    case "triangle":
      return new Style({ fill: new Fill({ color: "rgba(232,17,35,0.18)" }) });
    case "measure":
      return new Style({ stroke: new Stroke({ color: "#d83b01", width: 2.5, lineCap: "round" }) });
    case "ray":
      return new Style({ stroke: new Stroke({ color: "#c50f1f", width: 2.5, lineDash: [9, 6] }) });
    default: {
      const color = kind === "fix" ? "#107c10" : kind === "known" ? "#0078d4" : "#ffffff";
      const radius = kind === "fix" ? 7 : kind === "known" ? 5 : 4;
      return new Style({ image: new CircleStyle({ radius, fill: new Fill({ color }), stroke: new Stroke({ color: "#333", width: 1.5 }) }), zIndex: 10 });
    }
  }
};
