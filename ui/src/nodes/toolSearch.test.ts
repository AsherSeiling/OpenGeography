import { describe, expect, it } from "vitest";
import { scoreWord, searchTools, words, type Searchable } from "./toolSearch";

const t = (id: string, name: string, category = "Vector", description = "", keywords: string[] = []): Searchable => ({ id, name, category, description, keywords });
const ITEMS = [
  t("clip", "Clip", "Vector › Overlay", "Cut features to a boundary.", ["cut", "trim", "mask"]),
  t("execute", "Execute Plan", "Analysis", "Runs it."),
  t("union", "Union", "Vector › Overlay", "Combine layers.", ["merge", "combine"]),
  t("merge", "Merge", "Vector", "Append layers.", ["append"]),
  t("mosaic", "Mosaic Rasters", "Raster", "Join rasters.", ["merge", "stitch"]),
  t("warp", "Warp (Reproject Raster)", "Raster", "", []),
  t("usng", "Add USNG / MGRS Field", "Tables", "", []),
  t("buffer", "Buffer", "Vector", "Grow features.", ["expand"]),
];

describe("words", () => {
  it("splits on anything that is not a letter or digit", () => {
    expect(words("Warp (Reproject Raster)")).toEqual(["warp", "reproject", "raster"]);
    expect(words(" Add USNG / MGRS Field ")).toEqual(["add", "usng", "mgrs", "field"]);
    expect(words("")).toEqual([]);
  });
});

describe("scoreWord", () => {
  it("ranks name start, name word, keyword, category, description", () => {
    expect(scoreWord(ITEMS[0], "cl")?.rank).toBe(0);
    expect(scoreWord(ITEMS[5], "raster")?.rank).toBe(1);
    expect(scoreWord(ITEMS[0], "trim")).toEqual({ rank: 2, hit: "trim" });
    expect(scoreWord(ITEMS[0], "overlay")?.rank).toBe(3);
    expect(scoreWord(ITEMS[0], "boundary")?.rank).toBe(4);
  });
  it("does not match inside a word", () => {
    expect(scoreWord(ITEMS[1], "cut")).toBeNull();
    expect(scoreWord(ITEMS[1], "ecute")).toBeNull();
  });
});

describe("searchTools", () => {
  const ids = (q: string, fav: string[] = []) => searchTools(ITEMS, q, fav).map((h) => h.item.id);
  it("finds Clip by the plain word cut and not Execute", () => {
    expect(ids("cut")).toEqual(["clip"]);
  });
  it("finds Union, Merge and Mosaic Rasters by merge, name first", () => {
    expect(ids("merge")).toEqual(["merge", "mosaic", "union"]);
    expect(searchTools(ITEMS, "merge").find((h) => h.item.id === "union")?.hit).toBe("merge");
  });
  it("finds tools by their full names", () => {
    expect(ids("Warp (Reproject Raster)")).toEqual(["warp"]);
    expect(ids("Add USNG / MGRS Field")).toEqual(["usng"]);
  });
  it("needs every word to match", () => {
    expect(ids("cut features")).toEqual(["clip"]);
    expect(ids("cut raster")).toEqual([]);
  });
  it("breaks ties with favourites, then name", () => {
    const a = [t("b", "Beta"), t("a", "Alpha")];
    expect(searchTools(a, "", []).length).toBe(0);
    const same = [t("x", "Zed tool"), t("y", "Yak tool")];
    expect(searchTools(same, "tool").map((h) => h.item.id)).toEqual(["y", "x"]);
    expect(searchTools(same, "tool", ["x"]).map((h) => h.item.id)).toEqual(["x", "y"]);
  });
});
