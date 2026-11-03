import { beforeEach, describe, expect, it, vi } from "vitest";

function fakeStorage(init: Record<string, string> = {}, broken = false) {
  const m = new Map(Object.entries(init));
  return {
    getItem: (k: string) => {
      if (broken) throw new Error("denied");
      return m.get(k) ?? null;
    },
    setItem: (k: string, v: string) => {
      if (broken) throw new Error("denied");
      m.set(k, v);
    },
    m,
  };
}

async function load(storage: ReturnType<typeof fakeStorage>) {
  vi.resetModules();
  vi.stubGlobal("localStorage", storage);
  return (await import("./toolMenuStore")).useToolMenu;
}

beforeEach(() => vi.unstubAllGlobals());

describe("tool menu preferences", () => {
  it("keeps favourites and the last 8 recent tools, newest first, and saves them", async () => {
    const st = fakeStorage();
    const use = await load(st);
    use.getState().toggleFav("clip");
    for (const id of ["a", "b", "c", "d", "e", "f", "g", "h", "i", "a"]) use.getState().addRecent(id);
    expect(use.getState().recent).toEqual(["a", "i", "h", "g", "f", "e", "d", "c"]);
    expect(JSON.parse(st.m.get("gis.toolmenu")!)).toEqual({ fav: ["clip"], recent: use.getState().recent });
    use.getState().toggleFav("clip");
    expect(use.getState().fav).toEqual([]);
  });
  it("reads what was saved, ignoring junk", async () => {
    const use = await load(fakeStorage({ "gis.toolmenu": JSON.stringify({ fav: ["buffer", 3], recent: "x" }) }));
    expect(use.getState().fav).toEqual(["buffer"]);
    expect(use.getState().recent).toEqual([]);
  });
  it("works for the session when storage throws", async () => {
    const use = await load(fakeStorage({}, true));
    expect(use.getState().fav).toEqual([]);
    expect(() => use.getState().toggleFav("clip")).not.toThrow();
    expect(use.getState().fav).toEqual(["clip"]);
  });
});
