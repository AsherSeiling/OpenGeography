import { describe, expect, it } from "vitest";
import { barHeights, dragHandle, fmtValue, rampCss, valueToX, xToValue } from "./histogram";

const h = { min: 0, max: 100, width: 200 };

describe("histogram helpers", () => {
  it("maps values and pixels both ways and clamps", () => {
    expect(valueToX(50, h)).toBe(100);
    expect(xToValue(100, h)).toBe(50);
    expect(valueToX(-5, h)).toBe(0);
    expect(xToValue(999, h)).toBe(100);
  });
  it("keeps min below max when dragging", () => {
    expect(dragHandle("min", 100, 10, 40, h)[0]).toBeLessThan(40);
    expect(dragHandle("max", 0, 10, 40, h)[1]).toBeGreaterThan(10);
    expect(dragHandle("min", 20, 10, 40, h)).toEqual([10, 40]);
  });
  it("scales bars to 0–1", () => {
    const b = barHeights([0, 10, 1000]);
    expect(b[0]).toBe(0);
    expect(b[2]).toBe(1);
    expect(b[1]).toBeGreaterThan(0);
  });
  it("builds ramp css and formats values", () => {
    expect(rampCss(["#000", "#fff"], true)).toBe("linear-gradient(to right, #fff, #000)");
    expect(fmtValue(1234.56789)).toBe("1234.568");
    expect(fmtValue(2.5e7)).toBe("2.50e+7");
  });
});
