import Feature from "ol/Feature";
import Fill from "ol/style/Fill";
import Style, { type StyleFunction } from "ol/style/Style";

import { describe, expect, it } from "vitest";
import type { StyleClass } from "../store/store";
import { classRanks, moveClass, reverseClasses } from "./categoryOrder";
import { withCategoryOrder } from "./olStyle";

const cls = (...v: string[]): StyleClass[] => v.map((x) => ({ value: x, color: "#000000", label: x }));
const values = (c: StyleClass[]) => c.map((x) => x.value);

describe("category drawing order (B7)", () => {
  it("moves a class and clamps, without changing the input", () => {
    const c = cls("a", "b", "c", "d");
    expect(values(moveClass(c, 0, 2))).toEqual(["b", "c", "a", "d"]);
    expect(values(moveClass(c, 3, 0))).toEqual(["d", "a", "b", "c"]);
    expect(values(moveClass(c, 1, 99))).toEqual(["a", "c", "d", "b"]);
    expect(moveClass(c, 2, 2)).toBe(c);
    expect(moveClass(c, 9, 0)).toBe(c);
    expect(values(c)).toEqual(["a", "b", "c", "d"]);
  });

  it("reverses", () => {
    expect(values(reverseClasses(cls("a", "b", "c")))).toEqual(["c", "b", "a"]);
  });

  it("ranks: the top of the list gets the highest rank, other values rank below all", () => {
    const r = classRanks(cls("a", "b", "c"));
    expect([r.get("a"), r.get("b"), r.get("c")]).toEqual([3, 2, 1]);
    expect(r.get("zzz") ?? 0).toBe(0);
    expect(classRanks(cls("a", "b", "a")).get("a")).toBe(3); // a value listed twice keeps its highest place
    expect(classRanks(undefined).size).toBe(0);
  });

  it("the map style gives each category its rank as zIndex, follows a reorder, and leaves the shared base style alone", () => {
    const shared = new Style({ fill: new Fill({ color: "#123456" }) });
    const base: StyleFunction = () => shared; // one style object for every feature, like a data-driven layer
    const feature = (v: string) => new Feature({ cls: v });
    const z = (fn: StyleFunction, v: string) => (fn(feature(v), 1) as Style).getZIndex();
    const fn = withCategoryOrder(base, "cls", classRanks(cls("a", "b", "c")));
    expect([z(fn, "a"), z(fn, "b"), z(fn, "c"), z(fn, "other")]).toEqual([3, 2, 1, 0]);
    expect(shared.getZIndex()).toBeUndefined();
    expect(fn(feature("a"), 1)).toBe(fn(feature("a"), 1)); // cached, not rebuilt per feature
    const flipped = withCategoryOrder(base, "cls", classRanks(cls("c", "b", "a")));
    expect([z(flipped, "a"), z(flipped, "c")]).toEqual([1, 3]);
    expect(withCategoryOrder(() => undefined, "cls", new Map())(feature("a"), 1)).toBeUndefined();
  });
});
