// Topographic symbols (USGS sheet, military overlay): the engine owns the catalogue; these helpers wear a symbol on a layer's style.
import type { GeomType, TopoSymbol } from "../ipc/api";
import type { LayerStyle } from "../store/store";

/** The symbol kinds a layer of this geometry can wear. */
export function symbolKinds(geom: GeomType): TopoSymbol["geom"][] {
  return geom === "Line" ? ["line"] : geom === "Polygon" ? ["area"] : geom === "Point" ? ["point"] : geom === "Mixed" ? ["line", "area", "point"] : [];
}

/** Symbols for a layer's geometry, grouped by the sheet's headings in catalogue order. */
export function symbolGroups(symbols: TopoSymbol[], geom: GeomType): { category: string; items: TopoSymbol[] }[] {
  const kinds = symbolKinds(geom);
  const out: { category: string; items: TopoSymbol[] }[] = [];
  for (const s of symbols.filter((x) => kinds.includes(x.geom))) {
    let g = out.find((x) => x.category === s.category);
    if (!g) out.push((g = { category: s.category, items: [] }));
    g.items.push(s);
  }
  return out;
}

/** A layer's style wearing `symbol`: the symbol's fields replace the layer's, classes are cleared so the symbol shows. */
export function wearSymbol(style: LayerStyle, symbol: TopoSymbol): LayerStyle {
  const { classes: _c, breaks: _b, colors: _k, field: _f, ...rest } = style;
  void _c, _b, _k, _f;
  const sym = symbol.style;
  return {
    ...rest,
    kind: "single",
    fill: sym.fill ?? style.fill,
    stroke: sym.stroke ?? style.stroke,
    strokeWidth: sym.strokeWidth ?? style.strokeWidth,
    opacity: sym.opacity ?? style.opacity,
    radius: sym.radius ?? style.radius,
    strokes: sym.strokes,
    pattern: sym.pattern,
    patternSpacing: sym.patternSpacing,
    marker: sym.marker,
    symbol: symbol.id,
  };
}

/** The same layer with the symbol parts removed (plain stroke, no pattern or marker). */
export function clearSymbol(style: LayerStyle): LayerStyle {
  return { ...style, symbol: undefined, strokes: undefined, pattern: undefined, patternSpacing: undefined, marker: undefined };
}

/** An engine-drawn SVG as an <img> source. */
export const svgUri = (svg: string) => `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
