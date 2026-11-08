import { describe, expect, it } from "vitest";
import { activationRefusal, clampScale, frameWheelFactor, intoFrame, panCenter, zoomCenter } from "./activate";

const upm: [number, number] = [24, 24]; // 1:24,000: 24 m of ground per mm on the page

describe("UR-18 panning and zooming an activated map frame", () => {
  it("dragging right and down moves the map with the pointer: the centre goes left and up", () => {
    // Drag 10 mm right and 5 mm down: the ground that was 10 mm left of the pointer is now under it.
    expect(panCenter([1000, 5000], 10, 5, upm)).toEqual([760, 5120]);
    expect(panCenter([1000, 5000], 0, 0, upm)).toEqual([1000, 5000]);
  });
  it("a rotated frame pans along its own axes", () => {
    const [x, y] = intoFrame(10, 0, 90); // a 90° clockwise frame: page-right is the frame's "up" (negative y)
    expect(x).toBeCloseTo(0, 9);
    expect(y).toBeCloseTo(-10, 9);
    const c = panCenter([0, 0], 10, 0, upm, 90);
    expect(c[0]).toBeCloseTo(0, 9);
    expect(c[1]).toBeCloseTo(-240, 9);
  });
  it("zooming keeps the ground under the cursor under the cursor", () => {
    const start: [number, number] = [1000, 5000];
    const off: [number, number] = [30, -20]; // cursor 30 mm right of and 20 mm above the frame centre
    const ground = (c: [number, number], u: [number, number]) => [c[0] + off[0] * u[0], c[1] - off[1] * u[1]];
    for (const factor of [2, 0.5, 1.25, 8]) {
      const z = zoomCenter(start, 24_000, factor, off[0], off[1], upm);
      expect(z.scale).toBeCloseTo(24_000 / factor, 6);
      const g0 = ground(start, upm);
      const g1 = ground(z.center, [upm[0] / factor, upm[1] / factor]); // after zooming one mm covers 1/factor of the ground
      expect(g1[0]).toBeCloseTo(g0[0], 6);
      expect(g1[1]).toBeCloseTo(g0[1], 6);
    }
    expect(zoomCenter(start, 24_000, 2, 0, 0, upm).center).toEqual(start); // at the centre the centre stays
  });
  it("a wheel notch is a modest step, either way, and scale stays in range", () => {
    expect(frameWheelFactor(-100)).toBeGreaterThan(1.1);
    expect(frameWheelFactor(-100)).toBeLessThan(1.3);
    expect(frameWheelFactor(100)).toBeCloseTo(1 / frameWheelFactor(-100), 9);
    expect(frameWheelFactor(-100000)).toBe(frameWheelFactor(-60));
    expect(clampScale(0.01)).toBe(1);
    expect(clampScale(1e12)).toBe(1e9);
  });
  it("says why a frame can't be activated", () => {
    expect(activationRefusal({ id: "f1", locked: false, crs: "EPSG:26915" })).toBeNull();
    expect(activationRefusal({ id: "f1", name: "Main", locked: true, crs: "EPSG:26915" })).toContain("locked");
    expect(activationRefusal({ id: "f1", locked: false, followsSeries: true, crs: "x" })).toContain("map series");
    expect(activationRefusal({ id: "f1", locked: false, crs: "" })).toContain("coordinate system");
  });
});
