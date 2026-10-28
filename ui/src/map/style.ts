// Converts layer symbology into Mapbox-style layers (drawn by OpenLayers through ol-mapbox-style). Pure functions (tested).
import type { GeomType } from "../ipc/api";
import type { Basemap, LayerStyle } from "../store/store";

/** ArcGIS-style cyan selection highlight. */
export const SELECTION_COLOR = "#00ffff";

/** A style expression (Mapbox style spec); kept loose because ol-mapbox-style evaluates it. */
export type ExpressionSpecification = unknown[];
/** One Mapbox-style layer as `buildLayers` emits it. */
export interface LayerSpecification {
  id: string;
  type: "raster" | "fill" | "line" | "circle";
  source?: string;
  "source-layer"?: string;
  filter?: ExpressionSpecification;
  layout?: Record<string, unknown>;
  paint?: Record<string, unknown>;
}

const selected: ExpressionSpecification = ["boolean", ["feature-state", "selected"], false];

/** Attribute fields a style needs inside the vector tiles. */
export function styleFields(style: LayerStyle): string[] {
  return style.kind !== "single" && style.field ? [style.field] : [];
}

/** The label field, only while labels are on (so tiles carry it only then). */
export function labelField(style: LayerStyle): string | null {
  return style.label?.enabled && style.label.field ? style.label.field : null;
}

function colorExpr(style: LayerStyle, base: string): ExpressionSpecification | string {
  if (style.kind === "categorized" && style.field && style.classes?.length) {
    const pairs = style.classes.flatMap((c) => [c.value, c.color]);
    return ["match", ["to-string", ["get", style.field]], ...pairs, base] ;
  }
  if (style.kind === "graduated" && style.field && style.breaks?.length && style.colors?.length) {
    const steps = style.breaks.flatMap((b, i) => [b, style.colors![i + 1]]);
    return ["step", ["to-number", ["get", style.field], 0], style.colors[0], ...steps] ;
  }
  return base;
}

function selectionExpr(expr: ExpressionSpecification | string): ExpressionSpecification {
  return ["case", selected, SELECTION_COLOR, expr];
}

export function layerIds(id: string): string[] {
  return [`${id}-raster`, `${id}-fill`, `${id}-line`, `${id}-circle`];
}

/**
 * `forOl`: OpenLayers draws the selection highlight itself (ol-mapbox-style has no
 * `feature-state`), so the layers carry no selection expressions.
 */
export function buildLayers(id: string, geom: GeomType, style: LayerStyle, forOl = false): LayerSpecification[] {
  const withSelection = (expr: ExpressionSpecification | string): ExpressionSpecification | string => (forOl ? expr : selectionExpr(expr));
  const lineWidth = forOl ? style.strokeWidth : ["case", selected, Math.max(2, style.strokeWidth + 1), style.strokeWidth];
  const circleStroke = forOl ? "#333333" : ["case", selected, "#0060a0", "#333333"];
  const source = `src-${id}`;
  const common = { source, "source-layer": "layer" } as const;
  const fill = colorExpr(style, style.fill);
  const out: LayerSpecification[] = [];
  if (geom === "Raster") {
    return [{ id: `${id}-raster`, type: "raster", source, paint: { "raster-opacity": style.opacity, "raster-resampling": "linear" } }];
  }
  if (geom === "Polygon" || geom === "Mixed") {
    out.push({
      id: `${id}-fill`,
      type: "fill",
      ...common,
      filter: ["==", ["geometry-type"], "Polygon"],
      paint: { "fill-color": withSelection(fill), "fill-opacity": style.opacity },
    });
  }
  if (geom === "Polygon" || geom === "Line" || geom === "Mixed") {
    const lineColor = geom === "Line" ? fill : style.stroke;
    out.push({
      id: `${id}-line`,
      type: "line",
      ...common,
      filter: geom === "Line" ? ["==", ["geometry-type"], "LineString"] : ["!=", ["geometry-type"], "Point"],
      layout: { "line-join": "round", "line-cap": "round" },
      paint: {
        "line-color": withSelection(lineColor),
        "line-width": lineWidth,
        "line-opacity": geom === "Line" ? style.opacity : 1,
      },
    });
  }
  if (geom === "Point" || geom === "Mixed") {
    out.push({
      id: `${id}-circle`,
      type: "circle",
      ...common,
      filter: ["==", ["geometry-type"], "Point"],
      paint: {
        "circle-color": withSelection(fill),
        "circle-radius": style.radius,
        "circle-opacity": style.opacity,
        "circle-stroke-color": circleStroke,
        "circle-stroke-width": style.radius > 2 ? 0.6 : 0,
      },
    });
  }
  return out;
}

const USGS = (svc: string) => `https://basemap.nationalmap.gov/arcgis/rest/services/${svc}/MapServer/tile/{z}/{y}/{x}`;

export const BASEMAPS: Record<Basemap, { label: string; tiles: string[]; attribution: string; maxzoom: number } | null> = {
  usgsTopo: { label: "USGS Topo", tiles: [USGS("USGSTopo")], attribution: "USGS The National Map", maxzoom: 16 },
  usgsImagery: { label: "USGS Imagery", tiles: [USGS("USGSImageryOnly")], attribution: "USGS The National Map", maxzoom: 16 },
  usgsRelief: { label: "USGS Shaded Relief", tiles: [USGS("USGSShadedReliefOnly")], attribution: "USGS The National Map", maxzoom: 16 },
  osm: { label: "OpenStreetMap", tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"], attribution: "© OpenStreetMap contributors", maxzoom: 19 },
  none: null,
};

export const SCALE_PRESETS = [1000, 5000, 10000, 24000, 25000, 50000, 100000, 250000, 500000, 1000000, 5000000];

export function formatScale(s: number): string {
  return `1:${Math.round(s).toLocaleString("en-US")}`;
}

export function parseScale(text: string): number | null {
  const m = text.replace(/[,\s]/g, "").match(/^(?:1:)?(\d+(?:\.\d+)?)(k|m)?$/i);
  if (!m) return null;
  const mult = m[2]?.toLowerCase() === "k" ? 1000 : m[2]?.toLowerCase() === "m" ? 1e6 : 1;
  const v = parseFloat(m[1]) * mult;
  return v >= 1 ? v : null;
}

// --- Color ramps for graduated symbology ---
export const RAMPS: Record<string, string[]> = {
  Blues: ["#eff3ff", "#bdd7e7", "#6baed6", "#3182bd", "#08519c"],
  Greens: ["#edf8e9", "#bae4b3", "#74c476", "#31a354", "#006d2c"],
  "Yellow-Red": ["#ffffb2", "#fecc5c", "#fd8d3c", "#f03b20", "#bd0026"],
  Purples: ["#f2f0f7", "#cbc9e2", "#9e9ac8", "#756bb1", "#54278f"],
  Elevation: ["#2b83ba", "#abdda4", "#ffffbf", "#fdae61", "#d7191c"],
};

export const CATEGORY_COLORS = ["#1f78b4", "#33a02c", "#e31a1c", "#ff7f00", "#6a3d9a", "#b15928", "#a6cee3", "#b2df8a", "#fb9a99", "#fdbf6f", "#cab2d6", "#ffff99"];

/** Interpolate a ramp to n colors. */
export function rampColors(ramp: string[], n: number): string[] {
  if (n <= 1) return [ramp[ramp.length - 1]];
  const hex = (c: string) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
  return Array.from({ length: n }, (_, i) => {
    const t = (i / (n - 1)) * (ramp.length - 1);
    const lo = Math.floor(t);
    const hi = Math.min(lo + 1, ramp.length - 1);
    const [a, b] = [hex(ramp[lo]), hex(ramp[hi])];
    return "#" + a.map((v, k) => Math.round(v + (b[k] - v) * (t - lo)).toString(16).padStart(2, "0")).join("");
  });
}
