import { beforeEach, describe, expect, it } from "vitest";
import { recentCrs, rememberCrs } from "./Dialogs";

const store = new Map<string, string>();
beforeEach(() => {
  store.clear();
  (globalThis as any).localStorage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) };
});

describe("recent coordinate systems (R-H9)", () => {
  it("keeps the newest first, without duplicates, at most six", () => {
    for (let i = 0; i < 8; i++) rememberCrs(`EPSG:${i}`, `CRS ${i}`);
    rememberCrs("EPSG:5", "CRS 5");
    const r = recentCrs();
    expect(r).toHaveLength(6);
    expect(r[0].id).toBe("EPSG:5");
    expect(new Set(r.map((e) => e.id)).size).toBe(6);
  });
  it("ignores damaged storage", () => {
    store.set("gis.crs.recent", "not json");
    expect(recentCrs()).toEqual([]);
    store.set("gis.crs.recent", JSON.stringify([{ id: 1 }, { id: "EPSG:4326", name: "WGS 84" }]));
    expect(recentCrs()).toEqual([{ id: "EPSG:4326", name: "WGS 84" }]);
  });
  it("works when storage throws", () => {
    (globalThis as any).localStorage = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } };
    expect(recentCrs()).toEqual([]);
    expect(rememberCrs("EPSG:4326", "WGS 84", [])).toEqual([{ id: "EPSG:4326", name: "WGS 84" }]);
  });
});
