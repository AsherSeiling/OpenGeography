import { describe, expect, it } from "vitest";
import { cleanPatch, cleanTextStyle, cleanTextStyles, describePage, frames, LAYOUT_VERSION, newLayoutDoc, nextId, normalizeLayout, PAGE_SIZES, pageFor, roundScaleUp, rounded, STANDARD_SCALES, type Layout } from "./types";

describe("page sizes", () => {
  it("landscape swaps width and height; portrait keeps them", () => {
    const l = pageFor("Letter", "landscape");
    const p = pageFor("Letter", "portrait");
    expect([l.width, l.height]).toEqual([279.4, 215.9]);
    expect([p.width, p.height]).toEqual([215.9, 279.4]);
    expect(l.units).toBe("mm");
  });

  it("an unknown size name falls back to Letter", () => {
    expect(pageFor("Foolscap", "portrait").width).toBe(215.9);
  });

  it("describePage names every standard size in both orientations and rejects custom ones", () => {
    for (const size of Object.keys(PAGE_SIZES)) {
      for (const o of ["portrait", "landscape"] as const) expect(describePage(pageFor(size, o))).toEqual({ size, orientation: o });
    }
    expect(describePage({ ...pageFor("A4", "portrait"), width: 250 })).toBeNull();
    expect(describePage({ ...pageFor("A4", "portrait"), width: 210.4 })).toEqual({ size: "A4", orientation: "portrait" }); // within rounding
  });

  it("a square page is custom", () => {
    expect(describePage({ ...pageFor("A4", "portrait"), width: 200, height: 200 })).toBeNull();
  });
});

describe("scales", () => {
  it("standard scales are strictly ascending", () => {
    for (let i = 1; i < STANDARD_SCALES.length; i++) expect(STANDARD_SCALES[i]).toBeGreaterThan(STANDARD_SCALES[i - 1]);
  });
  it("rounds up to the next standard scale, within 0.1 % tolerance", () => {
    expect(roundScaleUp(23_000)).toBe(24_000);
    expect(roundScaleUp(24_000)).toBe(24_000);
    expect(roundScaleUp(24_010)).toBe(24_000); // 0.999 slack so 24,010 doesn't jump to 25,000
    expect(roundScaleUp(24_100)).toBe(25_000);
    expect(roundScaleUp(1)).toBe(500);
  });
  it("beyond the table rounds to whole millions; nonsense input doesn't throw", () => {
    expect(roundScaleUp(250_000_000)).toBe(250_000_000);
    expect(roundScaleUp(120_000_001)).toBe(121_000_000);
    for (const bad of [NaN, -5, 0, Infinity]) expect(roundScaleUp(bad), String(bad)).toBe(1_000_000); // invalid input falls back to a sane scale
  });
});

describe("rounded", () => {
  it("rounds half up and handles zero decimals, negatives and huge numbers", () => {
    expect(rounded(1.005, 0)).toBe(1);
    expect(rounded(2.5, 0)).toBe(3);
    expect(rounded(-2.4, 0)).toBe(-2);
    expect(rounded(123.456789, 3)).toBe(123.457);
    expect(rounded(1e15 + 0.3, 2)).toBeCloseTo(1e15, -1);
    expect(Number.isNaN(rounded(NaN, 2))).toBe(true);
  });
});

describe("layout documents", () => {
  const doc = (): Layout => newLayoutDoc("L1", "Test");
  it("a new document is empty, current-version, Letter landscape", () => {
    const l = doc();
    expect(l).toMatchObject({ id: "L1", name: "Test", version: LAYOUT_VERSION, body: "earth", elements: [], series: null });
    expect(describePage(l.page)).toEqual({ size: "Letter", orientation: "landscape" });
  });
  it("nextId skips ids in use, including gaps", () => {
    const l = doc();
    expect(nextId(l, "f")).toBe("f1");
    l.elements = [{ kind: "text", id: "f1" }, { kind: "text", id: "f2" }, { kind: "text", id: "f4" }] as never;
    expect(nextId(l, "f")).toBe("f3");
    expect(nextId(l, "t")).toBe("t1");
  });
  it("frames() returns only map frames, in order", () => {
    const l = doc();
    l.elements = [{ kind: "text", id: "t1" }, { kind: "map_frame", id: "m1" }, { kind: "scale_bar", id: "s1" }, { kind: "map_frame", id: "m2" }] as never;
    expect(frames(l).map((f) => f.id)).toEqual(["m1", "m2"]);
  });
});


describe("text styles (LW3)", () => {
  it("keeps only sensible values", () => {
    expect(cleanTextStyle({ size: 12, bold: true, color: "#112233", letterSpacing: 0.3 })).toEqual({ size: 12, bold: true, color: "#112233", letterSpacing: 0.3 });
    expect(cleanTextStyle({ size: 0, color: "red; evil", family: "  ", letterSpacing: 99, haloWidth: -1, junk: 1 })).toEqual({});
    expect(cleanTextStyle({ size: 401 })).toEqual({});
    expect(cleanTextStyle(null)).toEqual({});
  });
  it("drops empty slots and bad containers", () => {
    expect(cleanTextStyles({ "legend.title": { size: 14 }, "legend.entries": { size: -3 } })).toEqual({ "legend.title": { size: 14 } });
    expect(cleanTextStyles(undefined)).toBeUndefined();
    expect(cleanTextStyles({})).toBeUndefined();
  });
  it("normalizeLayout and cleanPatch carry them and refuse a bad opacity", () => {
    const l = normalizeLayout({ id: "x", name: "x", version: 1, body: "earth", page: pageFor("Letter", "landscape"), series: null, elements: [{ kind: "legend", id: "a", x: 0, y: 0, w: 20, h: 20, rotation: 0, locked: false, visible: true, frame: "f", title: "t", columns: 1, textStyles: { "legend.title": { size: 14, junk: 1 } as never }, opacity: 7 } as never] } as never);
    expect((l.elements[0] as { textStyles?: unknown }).textStyles).toEqual({ "legend.title": { size: 14 } });
    expect((l.elements[0] as { opacity?: number }).opacity).toBeUndefined();
    expect(cleanPatch({ opacity: 2, x: 5 })).toEqual({ x: 5 });
    expect(cleanPatch({ opacity: 0.5, textStyles: { "legend.title": { size: 9 } } })).toEqual({ opacity: 0.5, textStyles: { "legend.title": { size: 9 } } });
  });
});
