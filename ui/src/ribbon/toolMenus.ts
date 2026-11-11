// Analysis › Tools: the dropdowns of the ribbon, built from the engine's tool list (tool_specs) so a new tool appears with no ribbon change.
// Tools are grouped by `ToolSpec.category` with the same splitCategory the Shift+A menu uses. Data (input) and output tools stay in
// Shift+A; so do the two fixed Layer Output / Add to Map steps.
import type { ToolSpec } from "../ipc/api";
import { buildTree, splitCategory } from "../nodes/categories";

export interface ToolMenu {
  /** Button label: a top-level category, or a sub-group of "Analysis" (Network, Spatial Statistics, Summaries) promoted to its own dropdown. */
  label: string;
  /** Tools directly under the label (no sub-group). */
  tools: ToolSpec[];
  /** Sub-groups of a long menu (Raster › Focal, Vector › Overlay …), shown as headed sections. */
  subs: { name: string; tools: ToolSpec[] }[];
}

/** Dropdown order; groups not listed follow, alphabetically. */
export const MENU_ORDER = ["Vector", "Raster", "Hydrology", "Spatial Statistics", "Network", "Summaries", "Tables", "Coordinate Systems", "Styling"];

/** Tools the ribbon lists: real tools only (the same filter as the Shift+A menu). */
export const ribbonTools = (specs: ToolSpec[]): ToolSpec[] => specs.filter((s) => s.role === "tool" && s.id !== "layer_output" && s.id !== "add_to_map");

export function toolMenus(specs: ToolSpec[]): ToolMenu[] {
  const out: ToolMenu[] = [];
  for (const node of buildTree(ribbonTools(specs))) {
    if (node.group === "Analysis") {
      // The Analysis group is a grab-bag of three families; each is a dropdown of its own, as in the plan (Spatial Statistics, Network).
      if (node.tools.length) out.push({ label: "Analysis", tools: node.tools, subs: [] });
      for (const s of node.subs) out.push({ label: s.name, tools: s.tools, subs: [] });
    } else out.push({ label: node.group, tools: node.tools, subs: node.subs });
  }
  const rank = (l: string) => (MENU_ORDER.indexOf(l) < 0 ? MENU_ORDER.length : MENU_ORDER.indexOf(l));
  return out.sort((a, b) => rank(a.label) - rank(b.label) || a.label.localeCompare(b.label));
}

/** Every tool id in a menu, sub-groups included. */
export const menuToolIds = (m: ToolMenu): string[] => [...m.tools, ...m.subs.flatMap((s) => s.tools)].map((t) => t.id);

export { splitCategory };
