import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  // A fake projection: x doubles, y is kept, so the transformed box is easy to predict.
  api: { transformPoint: vi.fn(async (x: number, y: number) => [x * 2, y] as [number, number]) },
}));

import { api } from "../ipc/api";
import { extentIn, readBookmarks, samplePoints, scaleToFit, uniqueBookmarkName, useBookmarks, type Bookmark } from "./bookmarks";

const bm = (o: Partial<Bookmark> = {}): Bookmark => ({ id: "a", name: "Home", crs: "EPSG:4326", extent: [-1, 40, 1, 42], ...o });

beforeEach(() => useBookmarks.getState().set([]));

describe("readBookmarks", () => {
  it("an older project with no bookmarks field reads as none", () => {
    expect(readBookmarks(undefined)).toEqual([]);
    expect(readBookmarks(null)).toEqual([]);
    expect(readBookmarks("x")).toEqual([]);
  });
  it("keeps good entries and drops damaged ones without failing", () => {
    const out = readBookmarks([bm(), { name: "", crs: "EPSG:4326", extent: [0, 0, 1, 1] }, { name: "x", crs: "EPSG:4326", extent: [1, 1, 0, 0] }, { name: "y", crs: "", extent: [0, 0, 1, 1] }, { name: "z", crs: "EPSG:3857", extent: [0, 0, NaN, 1] }, null]);
    expect(out.map((b) => b.name)).toEqual(["Home"]);
  });
  it("gives a missing or repeated id a fresh one", () => {
    const out = readBookmarks([bm(), bm({ name: "Again" }), { name: "No id", crs: "EPSG:4326", extent: [0, 0, 1, 1] }]);
    expect(new Set(out.map((b) => b.id)).size).toBe(3);
  });
});

describe("names", () => {
  it("makes names unique ignoring case", () => {
    expect(uniqueBookmarkName([bm()], "home")).toBe("home 2");
    expect(uniqueBookmarkName([bm()], "Home", "a")).toBe("Home");
    expect(uniqueBookmarkName([], "  ")).toBe("Bookmark");
  });
  it("the store adds, renames and removes", () => {
    const s = useBookmarks.getState();
    const a = s.add("Site", "EPSG:3857", [0, 0, 10, 10]);
    const b = useBookmarks.getState().add("Site", "EPSG:3857", [0, 0, 5, 5]);
    expect(b.name).toBe("Site 2");
    useBookmarks.getState().rename(a.id, "Camp");
    expect(useBookmarks.getState().items.map((x) => x.name)).toEqual(["Camp", "Site 2"]);
    useBookmarks.getState().remove(a.id);
    expect(useBookmarks.getState().items).toHaveLength(1);
  });
});

describe("extentIn", () => {
  it("does not touch the engine for the same CRS", async () => {
    vi.mocked(api.transformPoint).mockClear();
    expect(await extentIn(bm(), "EPSG:4326")).toEqual([-1, 40, 1, 42]);
    expect(api.transformPoint).not.toHaveBeenCalled();
  });
  it("transforms 9 sample points from the stored CRS into the wanted one", async () => {
    vi.mocked(api.transformPoint).mockClear();
    expect(await extentIn(bm(), "EPSG:3857")).toEqual([-2, 40, 2, 42]);
    expect(api.transformPoint).toHaveBeenCalledTimes(9);
    expect(vi.mocked(api.transformPoint).mock.calls[0].slice(2)).toEqual(["EPSG:4326", "EPSG:3857"]);
  });
  it("refuses a bookmark that has no place in the other CRS", async () => {
    vi.mocked(api.transformPoint).mockResolvedValueOnce([Infinity, 0]);
    await expect(extentIn(bm(), "EPSG:3857")).rejects.toThrow(/can't be shown/);
  });
});

describe("scaleToFit", () => {
  it("picks the larger of the two scales", () => {
    // 1 mm covers 10 m at 1:10000, so a 100 mm by 50 mm frame shows 1000 m by 500 m there.
    expect(scaleToFit([0, 0, 1000, 500], [100, 50], [10, 10], 10000)).toBeCloseTo(10000);
    expect(scaleToFit([0, 0, 2000, 500], [100, 50], [10, 10], 10000)).toBeCloseTo(20000);
    expect(scaleToFit([0, 0, 1000, 1000], [100, 50], [10, 10], 10000)).toBeCloseTo(20000);
  });
  it("samples corners, edge midpoints and the centre", () => {
    expect(samplePoints([0, 0, 2, 2])).toHaveLength(9);
  });
});
