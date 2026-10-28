// Feature labels on the map (L1): an OpenLayers text style per layer, drawn on its own declutter layer so overlapping labels hide and symbols never do.
import type { FeatureLike } from "ol/Feature";
import Fill from "ol/style/Fill";
import Stroke from "ol/style/Stroke";
import Style, { type StyleFunction } from "ol/style/Style";
import Text from "ol/style/Text";
import type { GeomType } from "../ipc/api";
import type { LabelStyle } from "../store/store";

/** Pseudo tile field: the engine returns one label anchor point per feature (`tiles::ANCHORS`). */
export const ANCHORS_FIELD = "__anchors";

const PT_TO_PX = 96 / 72;

/** Text of a feature's label, or null when the field is empty. Numbers drop float noise. */
export function labelText(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const t = typeof value === "number" ? String(Number(value.toPrecision(12))) : String(value).trim();
  return t === "" ? null : t;
}

/** True when a label should draw at map scale `1:scale`; an unknown scale draws. */
export function labelVisibleAt(label: Pick<LabelStyle, "minScale" | "maxScale">, scale: number | null): boolean {
  if (scale === null || !Number.isFinite(scale)) return true;
  if (label.maxScale && scale > label.maxScale) return false;
  if (label.minScale && scale < label.minScale) return false;
  return true;
}

/** Which tile source a layer's labels read: polygons use anchor points, points and lines their own geometry. */
export function labelTileFields(geom: GeomType, field: string): string[] {
  return geom === "Polygon" ? [field, ANCHORS_FIELD] : [field];
}

/**
 * Style function of a layer's label layer. `scale()` is read at draw time, so zooming needs no new tiles.
 * Points get the label to the right of the symbol, lines run it along the line, polygon anchors centre it.
 */
export function labelStyleFunction(geom: GeomType, label: LabelStyle, scale: () => number | null): StyleFunction {
  const cache = new Map<string, Style>();
  const font = `${Math.round(label.size * PT_TO_PX * 10) / 10}px "Noto Sans", "Segoe UI", sans-serif`;
  const make = (text: string) =>
    new Style({
      text: new Text({
        text,
        font,
        fill: new Fill({ color: label.color }),
        stroke: label.haloWidth > 0 ? new Stroke({ color: label.haloColor, width: label.haloWidth * 2 }) : undefined,
        placement: geom === "Line" ? "line" : "point",
        textAlign: geom === "Point" ? "left" : "center",
        offsetX: geom === "Point" ? 8 : 0,
        overflow: geom === "Line" ? false : true,
        padding: [2, 2, 2, 2],
      }),
    });
  return (feature: FeatureLike) => {
    if (!labelVisibleAt(label, scale())) return undefined;
    const text = labelText(feature.get(label.field));
    if (text === null) return undefined;
    let st = cache.get(text);
    if (!st) {
      st = make(text);
      if (cache.size > 5000) cache.clear();
      cache.set(text, st);
    }
    return st;
  };
}
