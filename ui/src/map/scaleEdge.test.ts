import { describe, expect, it } from "vitest";
import { carryResolution, geographicK, groundPerPixel, PX_PER_M, resolutionForScale, scaleOf } from "./crs";

describe("scale maths at the limits", () => {
  it("scaleOf and resolutionForScale are inverses for metre, foot and degree CRS units", () => {
    for (const [unit, k] of [[1, 1], [0.3048006096012192, 0.9996], [111_319.49, 1.4]] as const) {
      for (const scale of [500, 24_000, 1e6, 5e7]) expect(scaleOf(resolutionForScale(scale, unit, k), unit, k)).toBeCloseTo(scale, 6);
    }
  });

  it("a feet CRS has a smaller resolution number than a metre CRS at the same ground scale", () => {
    const m = resolutionForScale(24_000, 1, 1);
    const ft = resolutionForScale(24_000, 0.3048006096012192, 1);
    expect(ft / m).toBeCloseTo(1 / 0.3048006096012192, 9);
  });

  it("the standard 1:24,000 at 96 dpi is ~6.35 m per pixel", () => {
    expect(groundPerPixel(1, 1, 1) * 24_000 / 24_000).toBe(1);
    expect(24_000 / PX_PER_M).toBeCloseTo(6.35, 2);
  });

  it("zero and NaN inputs propagate instead of crashing", () => {
    expect(scaleOf(0, 1, 1)).toBe(0);
    expect(Number.isNaN(scaleOf(NaN, 1, 1))).toBe(true);
    expect(resolutionForScale(24_000, 1, 0)).toBe(0); // k=0 would be a broken CRS; no throw
    expect(groundPerPixel(1, 1, 0)).toBe(Infinity);
  });

  it("carryResolution to the same CRS is the identity", () => {
    expect(carryResolution(12.5, 1, 0.9996, 1, 0.9996)).toBeCloseTo(12.5, 9);
  });

  it("carrying a view from a CRS with k=1 to one with k=0.9996 changes resolution by k", () => {
    expect(carryResolution(10, 1, 1, 1, 0.9996)).toBeCloseTo(10 * 0.9996, 9);
  });

  it("geographicK grows toward the poles and is finite at ±90", () => {
    expect(geographicK(0)).toBe(1);
    expect(geographicK(60)).toBeCloseTo(2, 9);
    expect(Number.isFinite(geographicK(90))).toBe(true);
    expect(geographicK(-45)).toBeCloseTo(geographicK(45), 12);
  });
});
