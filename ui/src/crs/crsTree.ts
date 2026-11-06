// A browsable tree over the coordinate-system catalog (UR-11): Projected / Geographic → region (continent or country) → state or
// province, where the EPSG area name gives one → datum → the systems. Built on load from the catalog's own `area` and `name` text;
// no definition is touched. What the text doesn't say lands in "Other" at its level, so every entry sits in exactly one leaf.
import type { CrsEntry } from "./crs";

export interface CrsNode {
  /** Unique within the whole tree, for expand/collapse state. */
  key: string;
  label: string;
  /** How many systems are below this node. */
  count: number;
  children?: CrsNode[];
  /** Leaves only. */
  entries?: CrsEntry[];
}

export const OTHER = "Other";
/** Areas that are not one country, so their second segment is not a state ("World - N hemisphere"). */
const WIDE = new Set(["World", "Europe", "Asia", "Africa", "North America", "South America", "Latin America", "Arctic", "Antarctica", "Oceania", "Pacific", "Atlantic", "Caribbean"]);
/** A second segment that describes a position or coverage rather than naming a state or province. */
const NOT_A_PLACE = /\d|°|hemisphere|onshore|offshore|\bby country\b|\bCONUS\b|\b(north|south|east|west|onshore|offshore|mainland|zone|SPCS|grid)\b/i;

/** The country-or-continent part of an EPSG area name ("USA - Texas - SPCS - N" → "USA"), or Other. */
export function regionOf(area: string): string {
  const first = area.split(" - ")[0].replace(/\s*\([^)]*\)\s*$/, "").trim(); // "World (by country)" → "World"
  return first || OTHER;
}

/** The state or province in an area name, only where the second segment names one. */
export function stateOf(area: string): string | null {
  const parts = area.split(" - ").map((p) => p.trim());
  if (parts.length < 2 || !parts[0] || WIDE.has(parts[0])) return null;
  const s = parts[1];
  if (!s) return null;
  if (!/\d|°/.test(s) && /,| and /.test(s) && !/onshore|offshore|by country/i.test(s)) return "Several states or provinces";
  if (NOT_A_PLACE.test(s)) return null;
  return s;
}

const ESRI_DATUMS: [RegExp, string][] = [
  [/^NAD[_ ]1983[_ ](2011|HARN|CORS96|NSRS2007|PA11|MA11)/i, "NAD83($1)"],
  [/^NAD[_ ]1983/i, "NAD83"],
  [/^NAD[_ ]1927/i, "NAD27"],
  [/^WGS[_ ]1984/i, "WGS 84"],
  [/^ETRS[_ ]1989/i, "ETRS89"],
  [/^GCS[_ ](.+)/i, "$1"],
];

/** The datum a system is on: the part of a "Datum / Projection" name before the slash, a geographic system's own name, or the datum an ESRI-style name starts with. */
export function datumOf(e: CrsEntry): string {
  const slash = e.name.indexOf(" / ");
  if (slash > 0) return e.name.slice(0, slash).trim();
  for (const [re, out] of ESRI_DATUMS) {
    const m = e.name.match(re);
    if (m) return m[0].replace(re, out).replace(/_/g, " ").trim() || OTHER;
  }
  if (!e.projected) return e.name.replace(/_/g, " ").trim() || OTHER;
  return OTHER;
}

const byLabel = (a: CrsNode, b: CrsNode) => (a.label === OTHER ? 1 : b.label === OTHER ? -1 : a.label.localeCompare(b.label));

function group<T>(items: T[], keyOf: (t: T) => string): Map<string, T[]> {
  const m = new Map<string, T[]>();
  for (const t of items) {
    const k = keyOf(t);
    m.set(k, [...(m.get(k) ?? []), t]);
  }
  return m;
}

function datumNodes(prefix: string, list: CrsEntry[]): CrsNode[] {
  return [...group(list, datumOf)].map(([d, es]) => ({ key: `${prefix}/${d}`, label: d, count: es.length, entries: [...es].sort((a, b) => a.name.localeCompare(b.name)) })).sort(byLabel);
}

/** The whole tree: [Projected, Geographic]. */
export function buildCrsTree(list: CrsEntry[]): CrsNode[] {
  return [true, false].map((projected) => {
    const kind = projected ? "Projected" : "Geographic";
    const regions = [...group(list.filter((e) => e.projected === projected), (e) => regionOf(e.area))].map(([region, es]) => {
      const rk = `${kind}/${region}`;
      const withState = group(es, (e) => stateOf(e.area) ?? "");
      const hasStates = [...withState.keys()].some((k) => k !== "");
      const children: CrsNode[] = hasStates
        ? [...withState].map(([st, ses]) => {
            const sk = `${rk}/${st || "(whole area)"}`;
            return { key: sk, label: st || `All of ${region}`, count: ses.length, children: datumNodes(sk, ses) };
          }).sort((a, b) => (a.label.startsWith("All of") ? -1 : b.label.startsWith("All of") ? 1 : byLabel(a, b)))
        : datumNodes(rk, es);
      return { key: rk, label: region, count: es.length, children } as CrsNode;
    }).sort(byLabel);
    return { key: kind, label: kind, count: regions.reduce((n, r) => n + r.count, 0), children: regions };
  });
}

/** Every leaf's entries, for checking that each system appears exactly once. */
export function treeEntries(nodes: CrsNode[]): CrsEntry[] {
  return nodes.flatMap((n) => (n.entries ? n.entries : treeEntries(n.children ?? [])));
}
