import { afterEach, describe, expect, it, vi } from "vitest";

async function fresh(storage: unknown) {
  vi.resetModules();
  vi.stubGlobal("localStorage", storage);
  return (await import("./prefs")).useBibPrefs;
}
const store = (raw?: string) => {
  const data: Record<string, string> = raw === undefined ? {} : { "gis-studio.bibliography.v1": raw };
  return { data, getItem: (k: string) => data[k] ?? null, setItem: (k: string, v: string) => void (data[k] = v) };
};

afterEach(() => vi.unstubAllGlobals());

describe("bibliography preferences", () => {
  it("defaults to APA with auto-export on", async () => {
    const p = await fresh(store());
    expect(p.getState()).toMatchObject({ style: "apa", autoExport: true });
  });
  it("saves and reloads", async () => {
    const s = store();
    let p = await fresh(s);
    p.getState().set({ style: "mla", autoExport: false });
    p = await fresh(s);
    expect(p.getState()).toMatchObject({ style: "mla", autoExport: false });
  });
  it.each([["garbage", "not json"], ["unknown style", '{"style":"harvard"}'], ["wrong types", '{"style":5,"autoExport":"no"}'], ["empty object", "{}"]])("%s falls back to defaults", async (_n, raw) => {
    const p = await fresh(store(raw));
    expect(p.getState().style).toBe("apa");
    expect(p.getState().autoExport).toBe(true);
  });
  it("storage that throws doesn't break setting", async () => {
    const p = await fresh({ getItem: () => { throw new Error("x"); }, setItem: () => { throw new Error("x"); } });
    expect(() => p.getState().set({ style: "chicago" })).not.toThrow();
    expect(p.getState().style).toBe("chicago");
  });
});
