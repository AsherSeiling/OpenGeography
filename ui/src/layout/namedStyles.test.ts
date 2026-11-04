import { describe, expect, it } from "vitest";
import { DEFAULT_STYLES, restyle, slotPatch, textMatches, textPatch, usesOf, parseStyleSize } from "./namedStyles";
import { newLayoutDoc, type Layout, type LayoutElement } from "./types";

const [title, , , legendBody] = DEFAULT_STYLES;
const base = { rotation: 0, locked: false, visible: true, x: 0, y: 0, w: 10, h: 10 };
const text = (id: string, over: Record<string, unknown> = {}) => ({ ...base, kind: "text", id, text: "x", font: "sans-bold", size: 21, align: "left", color: "#000000", ...over }) as unknown as LayoutElement;
const legend = (id: string, textStyles: Record<string, unknown>) => ({ ...base, kind: "legend", id, frame: "f1", title: "L", columns: 1, textStyles }) as unknown as LayoutElement;
const doc = (...elements: LayoutElement[]): Layout => ({ ...newLayoutDoc("L1", "T", "Letter", "landscape"), elements });

describe("named text styles", () => {
  it("recognises a Text element and a legend slot by their look", () => {
    expect(textMatches(text("t1") as never, title)).toBe(true);
    expect(textMatches(text("t2", { size: 18 }) as never, title)).toBe(false);
    const d = doc(text("t1"), text("t2", { size: 18 }), legend("lg1", { "legend.entry": { size: 8, bold: false, italic: false, color: "#000000" } }));
    expect(usesOf(d, title)).toEqual([{ id: "t1" }]);
    expect(usesOf(d, legendBody)).toEqual([{ id: "lg1", slot: "legend.entry" }]);
  });
  it("changes every use at once, text elements and slots, leaving the others alone", () => {
    const d = doc(text("t1"), text("t2"), text("t3", { size: 18 }));
    const next = { ...title, size: 24, color: "#112233" };
    const edits = restyle(d, title, next);
    expect(Object.keys(edits).sort()).toEqual(["t1", "t2"]);
    expect(edits.t1).toMatchObject({ size: 24, font: "sans-bold", color: "#112233" });
  });
  it("keeps halo and underline when a slot takes a style", () => {
    const e = legend("lg1", { "legend.entry": { size: 12, haloWidth: 0.5, haloColor: "#ffffff", underline: true } });
    const p = slotPatch(e, "legend.entry", legendBody) as { textStyles: Record<string, Record<string, unknown>> };
    expect(p.textStyles["legend.entry"]).toMatchObject({ size: 8, haloWidth: 0.5, underline: true });
  });
  it("builds Text element fields from a style", () => {
    expect(textPatch({ name: "Italic", size: 10, italic: true })).toMatchObject({ font: "sans-italic", size: 10 });
  });
  it("parses sizes", () => {
    expect(parseStyleSize("12.5")).toBe(12.5);
    expect(parseStyleSize("0")).toBeNull();
    expect(parseStyleSize("")).toBeNull();
  });
});
