// Tool categories are one string in the engine's ToolSpec: "Group" or "Group › Sub-group". The Geoprocessing pane shows them as nested dropdowns and the Add menu keeps them in this order.
export const SEP = " › ";

/** Order of the top-level groups; unknown groups go after these, alphabetically. */
export const GROUP_ORDER = ["Input", "Vector", "Tables", "Coordinate Systems", "Raster", "Hydrology", "Analysis", "Save to File", "Output"];

export const splitCategory = (c: string): [string, string | null] => {
  const i = c.indexOf(SEP);
  return i < 0 ? [c, null] : [c.slice(0, i), c.slice(i + SEP.length)];
};

const rank = (g: string) => (GROUP_ORDER.indexOf(g) < 0 ? GROUP_ORDER.length - 2 + 0.5 : GROUP_ORDER.indexOf(g));

/** Sort comparator for category strings: by group order, then sub-group name. */
export function compareCategories(a: string, b: string): number {
  const [ga, sa] = splitCategory(a);
  const [gb, sb] = splitCategory(b);
  return rank(ga) - rank(gb) || ga.localeCompare(gb) || (sa ?? "").localeCompare(sb ?? "");
}

export interface CategoryNode<T> {
  group: string;
  /** Tools directly under the group (no sub-group). */
  tools: T[];
  subs: { name: string; tools: T[] }[];
  count: number;
}

/** Tools arranged as group → sub-group → tools, tools sorted by name. */
export function buildTree<T extends { category: string; name: string }>(tools: T[]): CategoryNode<T>[] {
  const groups = new Map<string, CategoryNode<T>>();
  for (const t of [...tools].sort((a, b) => a.name.localeCompare(b.name))) {
    const [g, s] = splitCategory(t.category);
    const node = groups.get(g) ?? { group: g, tools: [], subs: [], count: 0 };
    groups.set(g, node);
    node.count++;
    if (s === null) node.tools.push(t);
    else (node.subs.find((x) => x.name === s) ?? (node.subs.push({ name: s, tools: [] }), node.subs.at(-1)!)).tools.push(t);
  }
  const out = [...groups.values()].sort((a, b) => compareCategories(a.group, b.group));
  for (const n of out) n.subs.sort((a, b) => a.name.localeCompare(b.name));
  return out;
}
