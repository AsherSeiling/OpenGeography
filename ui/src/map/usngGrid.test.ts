import { describe, expect, it } from "vitest";
import { gridLabel, gridSizeFor, groundWidthM, usngGridStyle } from "./usngGrid";
import type { LayerStyle } from "../store/store";

describe("USNG grid on the map", () => {
  it("measures a view in metres", () => {
    expect(groundWidthM([-93.1, 44, -92.9, 46])).toBeCloseTo(0.2 * 111_320 * Math.cos((45 * Math.PI) / 180), 0);
  });
  it("picks squares so about 4–20 fit across, finer as you zoom in", () => {
    expect(gridSizeFor(2_000_000)).toBe(100_000);
    expect(gridSizeFor(150_000)).toBe(10_000);
    expect(gridSizeFor(15_000)).toBe(1_000);
    expect(gridSizeFor(1_500)).toBe(100);
    expect(gridSizeFor(300)).toBe(100); // never finer than 100 m
    expect(gridSizeFor(50_000_000)).toBe(100_000);
  });
  it("names sizes", () => {
    expect(gridLabel(1000)).toBe("1 km");
    expect(gridLabel(100_000)).toBe("100 km");
  });
  it("draws outlines and labels without a fill", () => {
    const s = usngGridStyle({ kind: "single", fill: "#123456", stroke: "#000", strokeWidth: 3, opacity: 1, radius: 4 } as LayerStyle);
    expect(s.opacity).toBe(0);
    expect(s.label).toMatchObject({ enabled: true, field: "MGRS" });
    expect(usngGridStyle({} as LayerStyle, "USNG").label?.field).toBe("USNG");
  });
});
