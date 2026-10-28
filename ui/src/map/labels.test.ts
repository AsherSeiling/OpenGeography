import Feature from "ol/Feature";
import type Style from "ol/style/Style";
import { describe, expect, it } from "vitest";
import { parseScale, withLabel } from "../panes/LabelSettings";
import { defaultLabel, defaultStyle } from "../store/store";
import type { LayerInfo } from "../ipc/api";
import { ANCHORS_FIELD, labelStyleFunction, labelText, labelTileFields, labelVisibleAt } from "./labels";
import { labelField } from "./style";

const poly = { geomType: "Polygon" } as LayerInfo;
const draw = (fn: ReturnType<typeof labelStyleFunction>, props: Record<string, unknown>) => fn(new Feature(props), 10) as Style | undefined;

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
