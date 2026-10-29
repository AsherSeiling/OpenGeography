// The Symbology and Labels node (`symbology` tool): its settings become the owner layer's style after a run.
import { defaultLabel, type LayerStyle, type StyleClass } from "../store/store";

const HEX = /^#[0-9a-fA-F]{6}$/;
const num = (v: unknown, d: number, lo = 0, hi = Infinity) => {
  const n = typeof v === "number" ? v : typeof v === "string" && v.trim() !== "" ? Number(v) : NaN;
  return Number.isFinite(n) ? Math.min(hi, Math.max(lo, n)) : d;
};
const color = (v: unknown, d: string) => (typeof v === "string" && HEX.test(v) ? v : d);

/** `base` restyled by the node's params. Index lines (INDEX = 1) get their own colour through a two-class style on INDEX. */
export function styleFromSymbologyNode(params: Record<string, unknown>, base: LayerStyle): LayerStyle {
  const fill = color(params.fill, base.fill);
  const next: LayerStyle = {
    ...base,
    kind: "single",
    fill,
    stroke: color(params.stroke, fill),
    strokeWidth: num(params.width, base.strokeWidth),
    opacity: num(params.opacity, base.opacity, 0, 1),
  };
  delete next.field; delete next.classes; delete next.breaks; delete next.colors;
  const index = typeof params.index_color === "string" && HEX.test(params.index_color) ? params.index_color : null;
  if (index) {
    const classes: StyleClass[] = [
      { value: "1", color: index, label: "Index" },
      { value: "0", color: fill, label: "Other" },
    ];
    Object.assign(next, { kind: "categorized", field: "INDEX", classes });
  }
  const field = typeof params.label_field === "string" ? params.label_field : "";
  if (params.label === true && field) {
    next.label = {
      ...defaultLabel(field),
      size: num(params.label_size, 9, 3, 72),
      color: color(params.label_color, "#222222"),
      haloColor: color(params.halo_color, "#ffffff"),
      haloWidth: num(params.halo_width, 1.5, 0, 8),
      indexOnly: params.label_index_only === true || undefined,
    };
  } else {
    delete next.label;
  }
  return next;
}
