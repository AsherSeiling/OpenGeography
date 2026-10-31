// Topo Map Builder: give each of the project's layers the topographic symbol its name and geometry suggest (National Map data, contours,
// and military overlay layers by keyword). Pure functions; the dialog applies the result and lists what it left alone.
import type { LayerInfo, TopoSymbol } from "../ipc/api";
import type { LayerStyle } from "../store/store";
import { symbolKinds, wearSymbol } from "./symbols";

/** Layer-name keywords → symbol id, first match wins (military overlay first: its words are specific). */
export const RULES: [RegExp, string][] = [
  [/impact|demolition/i, "mil_impact_area"],
  [/drop\s*zone|\bdz\b/i, "mil_drop_zone"],
  [/landing\s*zone|\blz\b/i, "mil_landing_zone"],
  [/training\s*area/i, "mil_training_area"],
  [/cantonment|garrison/i, "mil_cantonment"],
  [/restricted/i, "mil_restricted_boundary"],
  [/air\s*traffic|\batc\b/i, "mil_atc_boundary"],
  [/firing/i, "mil_firing_point"],
  [/observation/i, "mil_observation_point"],
  [/recreation|environment/i, "mil_recreation"],
  [/range\s*(boundary|limit)/i, "mil_range_boundary"],
  [/flowline|stream|river/i, "hyd_stream"],
  [/waterbod|lake|pond|reservoir/i, "hyd_lake"],
  [/water area|wide river|nhd areas/i, "hyd_river"],
  [/spring|water point/i, "hyd_spring"],
  [/highway|interstate/i, "road_primary"],
  [/local road|street/i, "road_light_paved"],
  [/4wd/i, "road_4wd"],
  [/trail(?!head)/i, "road_trail"],
  [/railroad|railway/i, "rail_standard"],
  [/school|college/i, "bld_school"],
  [/campground|trailhead|cabin|shelter/i, "bld_campground"],
  [/cemeter|government|landmark|hospital|police|fire/i, "bld_house"],
  [/built-?up|urban area/i, "bld_built_up"],
  [/woodland|forest cover|wooded/i, "veg_woodland"],
  [/marsh|swamp|wetland/i, "sub_marsh"],
  [/county/i, "bnd_county"],
  [/state/i, "bnd_state"],
  [/power|transmission/i, "tx_power"],
  [/pipeline/i, "tx_pipeline"],
];

export interface Assignment {
  layer: string;
  name: string;
  symbol: string;
}

/** Index and intermediate contours drawn as the USGS prints them (brown, the index line heavier), keeping the layer's labels. */
export function contourSymbolStyle(base: LayerStyle, symbols: TopoSymbol[]): LayerStyle | null {
  const idx = symbols.find((s) => s.id === "con_index");
  const mid = symbols.find((s) => s.id === "con_intermediate");
  if (!idx?.style.strokes || !mid?.style.strokes) return null;
  return {
    ...base,
    kind: "categorized",
    field: "INDEX",
    classes: [
      { value: "1", color: idx.style.fill ?? "#a8662f", label: "Index contour", strokes: idx.style.strokes },
      { value: "0", color: mid.style.fill ?? "#a8662f", label: "Intermediate contour", strokes: mid.style.strokes },
    ],
    stroke: idx.style.fill ?? "#a8662f",
    symbol: "con_intermediate",
  };
}

/** Which symbol each layer gets, the new styles, and the layers left alone (with the reason). Layers that already wear a symbol are kept. */
export function assignSymbols(order: string[], layers: Record<string, LayerInfo>, styles: Record<string, LayerStyle>, symbols: TopoSymbol[]) {
  const changes: Record<string, LayerStyle> = {};
  const assigned: Assignment[] = [];
  const skipped: string[] = [];
  for (const id of order) {
    const info = layers[id];
    const style = styles[id];
    if (!info || !style || info.geomType === "Raster" || info.geomType === "Table") continue;
    if (style.symbol) continue;
    if (info.geomType === "Line" && /contour/i.test(info.name)) {
      const s = contourSymbolStyle(style, symbols);
      if (s) {
        changes[id] = s;
        assigned.push({ layer: id, name: info.name, symbol: "Index and intermediate contours" });
      }
      continue;
    }
    const rule = RULES.find(([re]) => re.test(info.name));
    if (!rule) {
      skipped.push(`${info.name}: no symbol suggested by its name`);
      continue;
    }
    const sym = symbols.find((s) => s.id === rule[1]);
    if (!sym) continue;
    if (!symbolKinds(info.geomType).includes(sym.geom)) {
      skipped.push(`${info.name}: “${sym.name}” is a ${sym.geom} symbol and this layer holds ${info.geomType.toLowerCase()} features`);
      continue;
    }
    changes[id] = wearSymbol(style, sym);
    assigned.push({ layer: id, name: info.name, symbol: sym.name });
  }
  return { changes, assigned, skipped };
}
