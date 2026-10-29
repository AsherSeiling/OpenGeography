// Feature labels on the map (L1): an OpenLayers text style per layer, drawn on its own declutter layer so overlapping labels hide and symbols never do.
import type { FeatureLike } from "ol/Feature";
import Fill from "ol/style/Fill";
import Stroke from "ol/style/Stroke";
import Style, { type StyleFunction } from "ol/style/Style";
import Text from "ol/style/Text";
import type { GeomType } from "../ipc/api";
import type { LabelPosition, LabelStyle } from "../store/store";

/** Pseudo tile field: the engine returns one label anchor point per feature (`tiles::ANCHORS`). */
export const ANCHORS_FIELD = "__anchors";

/** Contour index flag written by the Contour node: 1 on every nth line. */
export const INDEX_FIELD = "INDEX";

const PT_TO_PX = 96 / 72;

/** Text of a feature's label, or null when the field is empty. Numbers drop float noise. */
export function labelText(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const t = typeof value === "number" ? String(Number(value.toPrecision(12))) : String(value).trim();
  return t === "" ? null : t;
}

/** CSS font shorthand of a label: style, weight, size, then the chosen installed family before the app's fallbacks. */
export function labelFont(label: Pick<LabelStyle, "size" | "fontFamily" | "bold" | "italic">): string {
  const family = label.fontFamily?.replace(/["\\]/g, "").trim();
  const stack = [family ? `"${family}"` : null, '"Noto Sans"', '"Segoe UI"', "sans-serif"].filter(Boolean).join(", ");
  return `${label.italic ? "italic " : ""}${label.bold ? "bold " : ""}${Math.round(label.size * PT_TO_PX * 10) / 10}px ${stack}`;
}

/** True when a label should draw at map scale `1:scale`; an unknown scale draws. */
export function labelVisibleAt(label: Pick<LabelStyle, "minScale" | "maxScale">, scale: number | null): boolean {
  if (scale === null || !Number.isFinite(scale)) return true;
  if (label.maxScale && scale > label.maxScale) return false;
  if (label.minScale && scale < label.minScale) return false;
  return true;
}

/** Fields (other than the label text) the label style reads per feature. */
export function labelExtraFields(label: Pick<LabelStyle, "rotationField" | "sizeField" | "colorField" | "indexOnly">): string[] {
  return [label.rotationField, label.sizeField, label.colorField, label.indexOnly ? INDEX_FIELD : undefined].filter((f): f is string => !!f);
}

/** Which tile source a layer's labels read: polygons use anchor points, points and lines their own geometry; `extra` are the data-defined fields. */
export function labelTileFields(geom: GeomType, field: string, indexOnly = false, extra: string[] = []): string[] {
  const f = geom === "Polygon" ? [field, ANCHORS_FIELD] : [field];
  return Array.from(new Set([...f, ...(indexOnly ? [INDEX_FIELD] : []), ...extra]));
}

/** `text` broken at spaces into lines of at most `width` characters (a longer word keeps its own line); newlines are kept. */
export function wrapText(text: string, width?: number): string {
  if (!width || width < 1) return text;
  return text
    .split("\n")
    .flatMap((para) => {
      const out: string[] = [];
      let line = "";
      for (const w of para.trim().split(/\s+/).filter(Boolean)) {
        if (line && line.length + 1 + w.length > width) {
          out.push(line);
          line = "";
        }
        line = line ? `${line} ${w}` : w;
      }
      out.push(line);
      return out;
    })
    .join("\n");
}

/** OpenLayers alignment of a point label for each QGIS-style position, and the offset directions (-1, 0, 1). */
export function positionAnchor(pos: LabelPosition | undefined): { align: "left" | "right" | "center"; baseline: "top" | "bottom" | "middle"; ox: number; oy: number } {
  switch (pos) {
    case "left": return { align: "right", baseline: "middle", ox: -1, oy: 0 };
    case "above": return { align: "center", baseline: "bottom", ox: 0, oy: -1 };
    case "below": return { align: "center", baseline: "top", ox: 0, oy: 1 };
    case "center": return { align: "center", baseline: "middle", ox: 0, oy: 0 };
    case "aboveRight": return { align: "left", baseline: "bottom", ox: 1, oy: -1 };
    case "aboveLeft": return { align: "right", baseline: "bottom", ox: -1, oy: -1 };
    case "belowRight": return { align: "left", baseline: "top", ox: 1, oy: 1 };
    case "belowLeft": return { align: "right", baseline: "top", ox: -1, oy: 1 };
    default: return { align: "left", baseline: "middle", ox: 1, oy: 0 };
  }
}

const HEX = /^#[0-9a-fA-F]{6}$/;
const number = (v: unknown): number | null => {
  const n = typeof v === "number" ? v : typeof v === "string" && v.trim() !== "" ? Number(v) : NaN;
  return Number.isFinite(n) ? n : null;
};

/**
 * Style function of a layer's label layer. `scale()` is read at draw time, so zooming needs no new tiles.
 * Points go where `position` says (default right of the symbol), lines run the label along the line (above, on or below it),
 * polygon anchors centre it. Per-feature rotation, size and colour come from the label's data-defined fields; `priority`
 * is the style's zIndex, which the declutter honours.
 */
export function labelStyleFunction(geom: GeomType, label: LabelStyle, scale: () => number | null): StyleFunction {
  const cache = new Map<string, Style[]>();
  const GAP = 8;
  const pos = positionAnchor(label.position);
  const gap = GAP + (label.distance ?? 0);
  const make = (text: string, sizePt: number, color: string, rotationDeg: number): Style[] => {
    const font = labelFont({ ...label, size: sizePt });
    const text1 = (fill: string, ox: number, oy: number, halo: boolean) =>
      new Text({
        text,
        font,
        fill: new Fill({ color: fill }),
        stroke: halo && label.haloWidth > 0 ? new Stroke({ color: label.haloColor, width: label.haloWidth * 2 }) : undefined,
        placement: geom === "Line" ? "line" : "point",
        textAlign: geom === "Point" ? pos.align : label.align ?? "center",
        textBaseline: geom === "Line" ? (label.linePos === "above" ? "bottom" : label.linePos === "below" ? "top" : "middle") : geom === "Point" ? pos.baseline : "middle",
        offsetX: (geom === "Point" ? pos.ox * gap : 0) + ox,
        offsetY: (geom === "Point" ? pos.oy * gap : geom === "Line" ? (label.linePos === "above" ? -1 : label.linePos === "below" ? 1 : 0) * (label.distance ?? 0) : 0) + oy,
        rotation: (rotationDeg * Math.PI) / 180,
        overflow: geom === "Line" ? false : true,
        padding: [2, 2, 2, 2],
        backgroundFill: label.background ? new Fill({ color: label.background.fill }) : undefined,
        backgroundStroke: label.background?.stroke ? new Stroke({ color: label.background.stroke, width: 1 }) : undefined,
      });
    const z = label.priority ?? 5;
    const t = text1(color, 0, 0, true);
    if (label.background) t.setPadding(Array(4).fill(label.background.padding) as number[]);
    const out = [new Style({ text: t, zIndex: z })];
    if (label.shadow) out.unshift(new Style({ text: text1(label.shadow.color, label.shadow.dx, label.shadow.dy, false), zIndex: z }));
    return out;
  };
  return (feature: FeatureLike) => {
    if (!labelVisibleAt(label, scale())) return undefined;
    if (label.indexOnly && Number(feature.get(INDEX_FIELD)) !== 1) return undefined;
    const raw = labelText(feature.get(label.field));
    if (raw === null) return undefined;
    const text = wrapText(raw, label.wrapAt);
    const size = (label.sizeField && number(feature.get(label.sizeField))) || label.size;
    const c = label.colorField ? feature.get(label.colorField) : null;
    const color = typeof c === "string" && HEX.test(c) ? c : label.color;
    const rot = (label.rotationField && number(feature.get(label.rotationField))) || 0;
    const key = `${text}|${size}|${color}|${rot}`;
    let st = cache.get(key);
    if (!st) {
      st = make(text, size, color, rot);
      if (cache.size > 5000) cache.clear();
      cache.set(key, st);
    }
    return st;
  };
}
