import { describe, expect, it } from "vitest";
import { TOPO_PRESETS, defaultDatum, parseCenter, quadCenter, todayIso } from "./topoArea";

describe("Topo Map Builder area", () => {
  it("reads latitude, longitude", () => {
    expect(parseCenter("42.0, -93.0")).toEqual({ center: [-93, 42] });
    expect(parseCenter("  45.5 -122.7 ")).toEqual({ center: [-122.7, 45.5] });
    expect(parseCenter("85, 10")).toHaveProperty("error");
    expect(parseCenter("40, 200")).toHaveProperty("error");
    expect(parseCenter("")).toHaveProperty("error");
  });
  it("reads MGRS and gives the middle of the named square", () => {
    const r = parseCenter("15T VG 9747") as { center: [number, number] };
    expect(r.center[0]).toBeCloseTo(-93.0302, 3);
    expect(r.center[1]).toBeCloseTo(41.9795, 3);
    expect(parseCenter("15TVG")).toHaveProperty("center");
    expect(parseCenter("hello there")).toHaveProperty("error");
    expect(parseCenter("99ZZZ123")).toHaveProperty("error");
  });
  it("finds the 7.5′ quad containing a point", () => {
    const q = quadCenter(-93.03, 41.98);
    expect(q.sw[0]).toBeCloseTo(-93.0 - 0.125, 6); // -93.125 is a multiple of 0.125
    expect(q.sw[1]).toBeCloseTo(41.875, 6);
    expect(q.center[0]).toBeCloseTo(-93.0625, 6);
    expect(q.center[1]).toBeCloseTo(41.9375, 6);
    expect(q.name).toBe("41°52′30″N 93°07′30″W");
  });
  it("picks the datum by place", () => {
    expect(defaultDatum(-93, 42)).toBe("nad83");
    expect(defaultDatum(10, 50)).toBe("wgs84");
    expect(defaultDatum(-70, -33)).toBe("wgs84");
  });
  it("offers four presets with distinct names and sensible sheets", () => {
    expect(TOPO_PRESETS.map((p) => p.id)).toEqual(["land24", "land25", "land50", "trail"]);
    expect(new Set(TOPO_PRESETS.map((p) => p.label)).size).toBe(4);
    for (const p of TOPO_PRESETS) {
      expect(p.scale).toBeLessThanOrEqual(50000);
      expect(p.data.length).toBeGreaterThan(3);
    }
    expect(TOPO_PRESETS.find((p) => p.id === "trail")!.landscape).toBe(false);
  });
  it("formats today", () => {
    expect(todayIso(new Date(2026, 0, 5))).toBe("2026-01-05");
  });
});
