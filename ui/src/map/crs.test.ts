import { describe, expect, it } from "vitest";
import { carryResolution, groundForLegacyZoom, legacyZoom, resolutionForScale, scaleOf } from "./crs";

describe("scale ↔ resolution", () => {
  it("round-trips", () => {
    const r = resolutionForScale(24000, 1, 0.9996);
    expect(scaleOf(r, 1, 0.9996)).toBeCloseTo(24000, 6);
  });

  it("a CRS switch keeps the ground scale within 0.5 % (P9)", () => {
    // Web Mercator at 40°N (k = 1/cos φ ≈ 1.305) to UTM (k ≈ 0.9996).
    const resMerc = 611.49;
    const kMerc = 1.3054;
    const resUtm = carryResolution(resMerc, 1, kMerc, 1, 0.9996);
    expect(Math.abs(scaleOf(resUtm, 1, 0.9996) / scaleOf(resMerc, 1, kMerc) - 1)).toBeLessThan(0.005);
  });

  it("US survey feet CRS: unit conversion is part of the scale", () => {
    const ft = 1200 / 3937;
    expect(scaleOf(10, ft, 1)).toBeCloseTo(scaleOf(10 * ft, 1, 1), 9);
  });

  it("legacy zoom round-trips", () => {
    expect(groundForLegacyZoom(legacyZoom(37.5, 40), 40)).toBeCloseTo(37.5, 9);
  });
});
