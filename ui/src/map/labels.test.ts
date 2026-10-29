import Feature from "ol/Feature";
import type Style from "ol/style/Style";
import { describe, expect, it } from "vitest";
import { parseScale, withLabel } from "../panes/LabelSettings";
import { defaultLabel, defaultStyle } from "../store/store";
import type { LayerInfo } from "../ipc/api";
import { ANCHORS_FIELD, labelExtraFields, labelFont, positionAnchor, wrapText, labelStyleFunction, labelText, labelTileFields, labelVisibleAt } from "./labels";
import { labelField } from "./style";

const poly = { geomType: "Polygon" } as LayerInfo;
const drawAll = (fn: ReturnType<typeof labelStyleFunction>, props: Record<string, unknown>) => fn(new Feature(props), 10) as Style[] | undefined;
/** The main text style (a shadow, when there is one, is drawn first). */
const draw = (fn: ReturnType<typeof labelStyleFunction>, props: Record<string, unknown>) => drawAll(fn, props)?.at(-1);

describe("feature labels (L1)", () => {
  it("L1-P1 text: empty values draw nothing, numbers lose float noise", () => {
    expect(labelText("Madera")).toBe("Madera");
    expect(labelText("  ")).toBeNull();
    expect(labelText(null)).toBeNull();
    expect(labelText(undefined)).toBeNull();
    expect(labelText(0.1 + 0.2)).toBe("0.3");
    expect(labelText(42)).toBe("42");
  });

  it("L1-P1 polygons read anchor points from the engine, points and lines their own tiles", () => {
    expect(labelTileFields("Polygon", "NAME")).toEqual(["NAME", ANCHORS_FIELD]);
    expect(labelTileFields("Point", "NAME")).toEqual(["NAME"]);
    expect(labelTileFields("Line", "ELEV", true)).toEqual(["ELEV", "INDEX"]);
    expect(labelTileFields("Line", "NAME")).toEqual(["NAME"]);
  });

  it("L1-P1 the style puts the field's text on the feature, per geometry", () => {
    const label = defaultLabel("NAME");
    const pt = draw(labelStyleFunction("Point", label, () => null), { NAME: "A" })!.getText()!;
    expect(pt.getText()).toBe("A");
    expect(pt.getTextAlign()).toBe("left");
    expect(draw(labelStyleFunction("Line", label, () => null), { NAME: "A" })!.getText()!.getPlacement()).toBe("line");
    expect(draw(labelStyleFunction("Polygon", label, () => null), { NAME: "A" })!.getText()!.getTextAlign()).toBe("center");
    expect(draw(labelStyleFunction("Polygon", label, () => null), { NAME: null })).toBeUndefined();
    expect(draw(labelStyleFunction("Polygon", label, () => null), {})).toBeUndefined();
  });

  it("L1-P2 labels hide outside the scale range, and the style reads the scale at draw time", () => {
    const range = { minScale: 10_000, maxScale: 500_000 };
    expect(labelVisibleAt(range, 100_000)).toBe(true);
    expect(labelVisibleAt(range, 1_000_000)).toBe(false);
    expect(labelVisibleAt(range, 5_000)).toBe(false);
    expect(labelVisibleAt(range, 500_000)).toBe(true);
    expect(labelVisibleAt(range, null)).toBe(true);
    expect(labelVisibleAt({}, 1e9)).toBe(true);
    let scale = 100_000;
    const fn = labelStyleFunction("Polygon", { ...defaultLabel("NAME"), ...range }, () => scale);
    expect(draw(fn, { NAME: "A" })).toBeDefined();
    scale = 2_000_000;
    expect(draw(fn, { NAME: "A" })).toBeUndefined();
  });

  it("L1-P2 scale limits parse from what people type", () => {
    expect(parseScale("250000")).toBe(250_000);
    expect(parseScale("1:250,000")).toBe(250_000);
    expect(parseScale("")).toBeUndefined();
    expect(parseScale("0")).toBeUndefined();
    expect(parseScale("abc")).toBeUndefined();
  });

  it("L1-P3 settings survive save and reopen; old projects have labels off", () => {
    const style = withLabel(defaultStyle(poly), { enabled: true, minScale: 1000 }, "NAME");
    expect(style.label).toMatchObject({ enabled: true, field: "NAME", size: 9, minScale: 1000 });
    const reopened = JSON.parse(JSON.stringify({ a: style })).a;
    expect(reopened).toEqual(style);
    expect(labelField(reopened)).toBe("NAME");
    expect(labelField(defaultStyle(poly))).toBeNull(); // saved before L1: no label key
    expect(labelField(withLabel(style, { enabled: false }, "NAME"))).toBeNull(); // off: the tiles don't carry the field
  });

  it("turning labels on picks the first field, and a chosen field is kept", () => {
    const on = withLabel(defaultStyle(poly), { enabled: true }, "STATE");
    expect(on.label?.field).toBe("STATE");
    expect(withLabel(on, { field: "NAME" }, "STATE").label?.field).toBe("NAME");
    expect(withLabel(on, { size: 12 }, "STATE").label?.field).toBe("STATE");
  });
});

describe("labelFont", () => {
  it("puts the chosen family first, keeps the fallbacks, and adds style and weight", () => {
    expect(labelFont({ size: 9 })).toBe('12px "Noto Sans", "Segoe UI", sans-serif');
    expect(labelFont({ size: 12, fontFamily: 'Times "New" Roman', bold: true, italic: true })).toBe('italic bold 16px "Times New Roman", "Noto Sans", "Segoe UI", sans-serif');
  });
});

describe("label placement and rendering options (LB2)", () => {
  const base = defaultLabel("NAME");
  const fn = (geom: "Point" | "Line" | "Polygon", over: object) => labelStyleFunction(geom, { ...base, ...over }, () => null);
  it("point positions give the QGIS-style quadrants", () => {
    expect(positionAnchor(undefined)).toMatchObject({ align: "left", baseline: "middle", ox: 1 });
    expect(positionAnchor("aboveLeft")).toEqual({ align: "right", baseline: "bottom", ox: -1, oy: -1 });
    const t = draw(fn("Point", { position: "below", distance: 4 }), { NAME: "A" })!.getText()!;
    expect(t.getTextAlign()).toBe("center");
    expect(t.getTextBaseline()).toBe("top");
    expect(t.getOffsetY()).toBe(12);
  });
  it("line labels sit above, on or below the line", () => {
    const b = (linePos: "above" | "on" | "below") => draw(fn("Line", { linePos }), { NAME: "A" })!.getText()!.getTextBaseline();
    expect([b("above"), b("on"), b("below")]).toEqual(["bottom", "middle", "top"]);
  });
  it("wrap breaks at spaces, keeps long words and newlines", () => {
    expect(wrapText("North Fork Smith River", 10)).toBe("North Fork\nSmith\nRiver");
    expect(wrapText("Supercalifragilistic is long", 8)).toBe("Supercalifragilistic\nis long");
    expect(wrapText("a b", undefined)).toBe("a b");
    expect(draw(fn("Point", { wrapAt: 5 }), { NAME: "ab cd ef" })!.getText()!.getText()).toBe("ab cd\nef");
  });
  it("data-defined rotation, size and colour; bad values fall back", () => {
    const f = fn("Point", { rotationField: "R", sizeField: "S", colorField: "C", size: 9, color: "#111111" });
    const t = draw(f, { NAME: "A", R: 90, S: 15, C: "#ff0000" })!.getText()!;
    expect(t.getRotation()).toBeCloseTo(Math.PI / 2);
    expect(t.getFont()).toContain("20px");
    expect(t.getFill()!.getColor()).toBe("#ff0000");
    const bad = draw(f, { NAME: "A", R: "x", S: null, C: "red" })!.getText()!;
    expect([bad.getRotation(), bad.getFill()!.getColor()]).toEqual([0, "#111111"]);
    expect(labelExtraFields({ rotationField: "R", colorField: "C" })).toEqual(["R", "C"]);
    expect(labelTileFields("Point", "NAME", false, ["R", "NAME"])).toEqual(["NAME", "R"]);
  });
  it("priority is the zIndex; background and shadow add to the style", () => {
    const all = drawAll(fn("Point", { priority: 9, shadow: { color: "#888888", dx: 1, dy: 1 }, background: { fill: "#ffffcc", padding: 3 } }), { NAME: "A" })!;
    expect(all).toHaveLength(2);
    expect(all.map((s) => s.getZIndex())).toEqual([9, 9]);
    const main = all[1].getText()!;
    expect(main.getBackgroundFill()!.getColor()).toBe("#ffffcc");
    expect(main.getPadding()).toEqual([3, 3, 3, 3]);
    expect(all[0].getText()!.getOffsetX()).toBe(8 + 1);
  });
});
