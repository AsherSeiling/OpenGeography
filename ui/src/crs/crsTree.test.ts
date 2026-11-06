import { describe, expect, it } from "vitest";
import { loadCatalog, type CrsEntry } from "./crs";
import { buildCrsTree, datumOf, OTHER, regionOf, stateOf, treeEntries, type CrsNode } from "./crsTree";

const find = (nodes: CrsNode[], ...path: string[]): CrsNode | undefined => path.reduce<CrsNode | undefined>((n, l) => (n ? (n.children ?? []).find((c) => c.label === l) : undefined), { key: "", label: "", count: 0, children: nodes });

describe("UR-11 the CRS tree", () => {
  it("puts every catalog entry in exactly one leaf, and changes none of them", async () => {
    const list = await loadCatalog();
    const before = JSON.stringify(list);
    const tree = buildCrsTree(list);
    const placed = treeEntries(tree);
    expect(placed.length).toBe(list.length);
    expect(new Set(placed.map((e) => e.id)).size).toBe(list.length);
    expect(JSON.stringify(list)).toBe(before);
    // Counts add up at every level, and the kinds split the way the catalog does.
    const sum = (n: CrsNode): number => (n.entries ? n.entries.length : (n.children ?? []).reduce((a, c) => a + sum(c), 0));
    const check = (n: CrsNode) => { expect(sum(n)).toBe(n.count); (n.children ?? []).forEach(check); };
    tree.forEach(check);
    expect(tree.map((t) => [t.label, t.count])).toEqual([["Projected", list.filter((e) => e.projected).length], ["Geographic", list.filter((e) => !e.projected).length]]);
    // Every kind and region keeps its own "Other" last.
    for (const k of tree) { const labels = k.children!.map((c) => c.label); if (labels.includes(OTHER)) expect(labels[labels.length - 1]).toBe(OTHER); }
  });

  it("reads region, state and datum from the EPSG text", () => {
    expect(regionOf("USA - Texas - SPCS - N")).toBe("USA");
    expect(stateOf("USA - Texas - SPCS - N")).toBe("Texas");
    expect(stateOf("USA - Alaska - north of 54.5°N; west of 168°W")).toBe("Alaska");
    expect(stateOf("Canada - Alberta")).toBe("Alberta");
    expect(stateOf("USA - Montana, North Dakota, and South Dakota")).toBe("Several states or provinces");
    expect(stateOf("World - N hemisphere - 96°W to 90°W - by country")).toBeNull();
    expect(stateOf("Ireland - onshore")).toBeNull();
    expect(stateOf("Russia - 169.5°E to 172.5°E onshore")).toBeNull();
    expect(stateOf("Greece")).toBeNull();
    expect(regionOf("")).toBe(OTHER);
    const e = (name: string, projected = true): CrsEntry => ({ id: "X:1", name, projected, area: "", bounds: null });
    expect(datumOf(e("NAD83 / Texas North Central"))).toBe("NAD83");
    expect(datumOf(e("NAD83(2011) / Kansas LCC (ftUS)"))).toBe("NAD83(2011)");
    expect(datumOf(e("NAD_1983_CORS96_StatePlane_California_II_FIPS_0402"))).toBe("NAD83(CORS96)");
    expect(datumOf(e("NAD_1983_HARN_Adj_MN_Itasca_South_Feet"))).toBe("NAD83(HARN)");
    expect(datumOf(e("WGS_1984_ARC_System_Zone_08"))).toBe("WGS 84");
    expect(datumOf(e("NAD83", false))).toBe("NAD83");
    expect(datumOf(e("Hong Kong 1980 Grid System"))).toBe(OTHER);
  });

  it("State Plane and UTM entries land where a person would look", async () => {
    const tree = buildCrsTree(await loadCatalog());
    const ids = (n: CrsNode | undefined) => (n ? treeEntries([n]).map((e) => e.id) : []);
    // NAD83 / Texas North Central: Projected → USA → Texas → NAD83
    expect(ids(find(tree, "Projected", "USA", "Texas", "NAD83"))).toContain("EPSG:32138");
    expect(ids(find(tree, "Projected", "USA", "Texas", "NAD83"))).toContain("EPSG:2276");
    // WGS 84 / UTM zone 15N: Projected → World → WGS 84
    expect(ids(find(tree, "Projected", "World", "WGS 84"))).toContain("EPSG:32615");
    // NAD83 / UTM zone 14N covers North America; NAD83 / Conus Albers is a US-wide system
    expect(ids(find(tree, "Projected", "North America", "NAD83"))).toContain("EPSG:26914");
    expect(ids(find(tree, "Projected", "USA", "All of USA", "NAD83"))).toContain("EPSG:5070");
    // Geographic: WGS 84 under World, NAD83 under North America
    expect(ids(find(tree, "Geographic", "World", "WGS 84"))).toContain("EPSG:4326");
    expect(ids(find(tree, "Geographic", "North America", "NAD83"))).toContain("EPSG:4269");
  });
});
