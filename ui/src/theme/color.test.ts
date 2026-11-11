import { describe, expect, it } from "vitest";
import { alphaOf, alphaToPct, cssColor, formatColor, hsvToRgb, opaqueHex, parseColor, pctToAlpha, rgbToHsv, scaleAlpha, withAlpha } from "./color";

describe("colour parsing", () => {
  it("reads 3, 4, 6 and 8 digit hex, with or without #", () => {
    expect(parseColor("#f00")).toEqual({ r: 255, g: 0, b: 0, a: 1 });
    expect(parseColor("0f08")).toEqual({ r: 0, g: 255, b: 0, a: 0x88 / 255 });
    expect(parseColor("#336699")).toEqual({ r: 0x33, g: 0x66, b: 0x99, a: 1 });
    expect(parseColor("#33669980")?.a).toBeCloseTo(128 / 255);
  });
  it("rejects everything else", () => {
    for (const bad of ["", "#12", "#12345", "#1234567", "red", "rgb(1,2,3)", "#gg0000", null, undefined, 5]) expect(parseColor(bad)).toBeNull();
  });
});

describe("formatting", () => {
  it("old #rrggbb values stay exactly as they were when opaque", () => {
    for (const c of ["#1f78b4", "#000000", "#ffffff", "#e31a1c"]) expect(formatColor(parseColor(c)!)).toBe(c);
  });
  it("writes #rrggbbaa only when not opaque", () => {
    expect(formatColor({ r: 255, g: 0, b: 0, a: 0.5 })).toBe("#ff000080");
    expect(formatColor({ r: 255, g: 0, b: 0, a: 1 })).toBe("#ff0000");
    expect(formatColor({ r: 255, g: 0, b: 0, a: 0 })).toBe("#ff000000");
  });
  it("normalises case", () => expect(withAlpha("#FF0000", 0.5)).toBe("#ff000080"));
});

describe("alpha", () => {
  it("every whole percent survives the byte round trip", () => {
    for (let p = 0; p <= 100; p++) expect(alphaToPct(parseColor(withAlpha("#123456", pctToAlpha(p)))!.a)).toBe(p);
  });
  it("is 1 for plain colours and non-colours", () => {
    expect(alphaOf("#abcdef")).toBe(1);
    expect(alphaOf(undefined)).toBe(1);
    expect(alphaOf("red")).toBe(1);
  });
  it("opaqueHex drops alpha and falls back", () => {
    expect(opaqueHex("#11223344")).toBe("#112233");
    expect(opaqueHex("none", "#abcdef")).toBe("#abcdef");
  });
  it("scaleAlpha multiplies with a slider", () => {
    expect(scaleAlpha("#ff0000", 0.5)).toBe("#ff000080");
    expect(alphaOf(scaleAlpha("#ff000080", 0.5))).toBeCloseTo(0.25, 1);
    expect(scaleAlpha("#ff0000", 1)).toBe("#ff0000");
  });
});

describe("css output for the map", () => {
  it("leaves opaque colours alone and turns alpha into rgba()", () => {
    expect(cssColor("#336699")).toBe("#336699");
    expect(cssColor("#ff000080")).toBe("rgba(255,0,0,0.502)");
    expect(cssColor("red")).toBe("red");
  });
});

describe("hsv", () => {
  it("round-trips primary and mixed colours", () => {
    for (const c of ["#ff0000", "#00ff00", "#0000ff", "#336699", "#808080", "#000000", "#ffffff", "#e69f00"]) {
      expect(formatColor(hsvToRgb(rgbToHsv(parseColor(c)!)))).toBe(c);
    }
  });
});
