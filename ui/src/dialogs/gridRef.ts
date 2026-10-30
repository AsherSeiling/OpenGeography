// A USNG / MGRS reference column in exports: the choices offered and the node tree an export runs.
import type { GraphPayload } from "../ipc/api";

export type GridStyle = "usng" | "mgrs";
export interface GridChoice {
  style: GridStyle;
  /** Digits per axis, 0 (100 km square) to 5 (1 m). */
  precision: number;
}

/** The defaults are MGRS, 1 km (2 digits) and WGS 84; the lists start with them. */
export const GRID_PRECISIONS: [number, string][] = [[2, "1 km (2 digits)"], [5, "1 m (5 digits)"], [4, "10 m (4 digits)"], [3, "100 m (3 digits)"], [1, "10 km (1 digit)"], [0, "100 km square only"]];
export const DEFAULT_GRID: GridChoice = { style: "mgrs", precision: 2 };

/** The field name a reference column gets. */
export const gridField = (style: GridStyle) => style.toUpperCase();

/**
 * What an Export Features run executes: the layer (its selected features only when `fids`), then, when a grid reference is asked for, the
 * Add USNG / MGRS Field tool, then the export. The datum is WGS 84.
 */
export function exportGraph(layer: string, fids: number[] | null, path: string, format: string, grid: GridChoice | null): GraphPayload {
  const nodes: GraphPayload["nodes"] = [{ id: "in", tool: "input_layer", params: fids ? { layer, fids } : { layer } }];
  const edges: GraphPayload["edges"] = [];
  let last = "in";
  if (grid) {
    nodes.push({ id: "grid", tool: "add_usng", params: { field: gridField(grid.style), style: grid.style, precision: String(grid.precision), datum: "wgs84" } });
    edges.push({ source: last, target: "grid", targetHandle: "in" });
    last = "grid";
  }
  nodes.push({ id: "out", tool: "export", params: { path, format } });
  edges.push({ source: last, target: "out", targetHandle: "in" });
  return { nodes, edges };
}
