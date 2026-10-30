import { describe, expect, it } from "vitest";
import { parseSize, resetSlot, withSlotStyle } from "./textStyles";

describe("text slot editing", () => {
  it("sets, merges and removes fields of one slot", () => {
    let s = withSlotStyle(undefined, "legend.title", { size: 14 });
    expect(s).toEqual({ "legend.title": { size: 14 } });
    s = withSlotStyle(s, "legend.title", { bold: true, color: "#112233" });
    expect(s["legend.title"]).toEqual({ size: 14, bold: true, color: "#112233" });
    s = withSlotStyle(s, "legend.title", { size: undefined });
    expect(s["legend.title"]).toEqual({ bold: true, color: "#112233" });
    s = withSlotStyle(s, "legend.entries", { italic: true });
    expect(Object.keys(s)).toEqual(["legend.title", "legend.entries"]);
  });
  it("removes a slot left empty and ignores bad values", () => {
    const s = withSlotStyle({ "legend.title": { size: 14 } }, "legend.title", { size: undefined });
    expect(s).toEqual({});
    expect(withSlotStyle(undefined, "x.y", { size: 9999 })).toEqual({});
    expect(withSlotStyle(undefined, "x.y", { color: "javascript:1" })).toEqual({});
  });
  it("does not change the input and resets a slot", () => {
    const a = { "legend.title": { size: 14 }, "legend.entries": { size: 9 } };
    const b = withSlotStyle(a, "legend.title", { size: 20 });
    expect(a["legend.title"].size).toBe(14);
    expect(b["legend.title"].size).toBe(20);
    expect(resetSlot(a, "legend.title")).toEqual({ "legend.entries": { size: 9 } });
  });
  it("parses a size box", () => {
    expect(parseSize("")).toBeUndefined();
    expect(parseSize(" 12.5 ")).toBe(12.5);
    expect(parseSize("0")).toBeNull();
    expect(parseSize("500")).toBeNull();
    expect(parseSize("abc")).toBeNull();
  });
});
