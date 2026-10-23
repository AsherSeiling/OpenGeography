import { describe, expect, it } from "vitest";
import {
  convertAzimuth, declination, degToMils, direct, gmAngle, gridConvergence, intersection, inverse, milsToDeg,
  parsePosition, polygonArea, resection, toMgrs, toUtm, utmZone, type LonLat,
} from "./geodesy";

describe("geodesics", () => {
  it("matches GeographicLib's published JFK → LHR example", () => {
    // Karney's example: JFK (40.6, -73.8) to LHR (51.6, -0.5) is 5,551,759.400 m.
    const r = inverse([-73.8, 40.6], [-0.5, 51.6]);
    expect(r.distance).toBeCloseTo(5551759.4, 1);
    expect(r.azimuth).toBeCloseTo(51.198882845579, 6);
  });

  it("direct and inverse round-trip", () => {
    const a: LonLat = [-97.5, 35.2];
    const b = direct(a, 33.3, 12345.678);
    const r = inverse(a, b);
    expect(r.distance).toBeCloseTo(12345.678, 6);
    expect(r.azimuth).toBeCloseTo(33.3, 9);
  });

  it("measures area on the ellipsoid", () => {
    // A 1°×1° cell at the equator is about 12,308 km².
    const { area } = polygonArea([[0, 0], [1, 0], [1, 1], [0, 1]]);
    expect(area / 1e6).toBeCloseTo(12308.778, 0);
  });
});

describe("north references and units", () => {
  it("converts mils", () => {
    expect(degToMils(90)).toBe(1600);
    expect(milsToDeg(4800)).toBe(270);
  });

  it("computes UTM grid convergence with the right sign", () => {
    // East of a zone's central meridian (N hemisphere), true north is west of grid north.
    expect(gridConvergence([-97.5, 35.2])).toBeLessThan(0); // zone 14 CM is -99
    expect(gridConvergence([-100.5, 35.2])).toBeGreaterThan(0);
    expect(Math.abs(gridConvergence([-99, 35.2]))).toBeLessThan(1e-6);
  });

  it("uses WMM2025 declination", () => {
    // Oklahoma City: declination is a few degrees east in 2026.
    const d = declination([-97.5, 35.47], new Date("2026-06-01"));
    expect(d).toBeGreaterThan(1);
    expect(d).toBeLessThan(6);
  });

  it("round-trips azimuth conversions", () => {
    const p: LonLat = [-97.5, 35.2];
    const g = convertAzimuth(100, "magnetic", "grid", p);
    expect(g - 100).toBeCloseTo(gmAngle(p), 9);
    expect(convertAzimuth(g, "grid", "magnetic", p)).toBeCloseTo(100, 9);
    expect(convertAzimuth(convertAzimuth(42, "true", "grid", p), "grid", "true", p)).toBeCloseTo(42, 9);
  });

  it("parses MGRS and lat/lon", () => {
    const p = parsePosition("35.2, -97.5");
    expect(p).toEqual([-97.5, 35.2]);
    const back = parsePosition(toMgrs(p));
    expect(back[0]).toBeCloseTo(-97.5, 4);
    expect(back[1]).toBeCloseTo(35.2, 4);
    expect(() => parsePosition("not a place")).toThrow(/MGRS/);
  });
});

describe("intersection and resection", () => {
  const target: LonLat = [-97.45, 35.25];
  const a: LonLat = [-97.5, 35.2];
  const b: LonLat = [-97.38, 35.21];
  const c: LonLat = [-97.46, 35.31];
  const { proj } = utmZone(a);
  const gridAz = (from: LonLat, to: LonLat) => {
    const [x0, y0] = toUtm(from, proj);
    const [x1, y1] = toUtm(to, proj);
    return ((Math.atan2(x1 - x0, y1 - y0) * 180) / Math.PI + 360) % 360;
  };

  it("intersection finds the target from two known points (grid azimuths)", () => {
    const fix = intersection([
      { point: a, azimuth: gridAz(a, target), north: "grid" },
      { point: b, azimuth: gridAz(b, target), north: "grid" },
    ]);
    expect(inverse(fix.position, target).distance).toBeLessThan(0.01);
    expect(fix.mgrs).toBe(toMgrs(target));
  });

  it("intersection works with magnetic azimuths and the G-M angle", () => {
    const gm = gmAngle(a);
    const fix = intersection(
      [a, b].map((p) => ({ point: p, azimuth: gridAz(p, target) - gm, north: "magnetic" as const })),
      gm,
    );
    expect(inverse(fix.position, target).distance).toBeLessThan(0.01);
  });

  it("resection finds your position from three landmarks, with a small error triangle", () => {
    const me = target;
    // Azimuths measured from me to each landmark, with half a degree of compass error on one.
    const fix = resection([
      { point: a, azimuth: gridAz(me, a), north: "grid" },
      { point: b, azimuth: gridAz(me, b) + 0.5, north: "grid" },
      { point: c, azimuth: gridAz(me, c), north: "grid" },
    ]);
    expect(fix.intersections.length).toBe(3);
    expect(inverse(fix.position, me).distance).toBeLessThan(60);
    expect(fix.errorSpread).toBeGreaterThan(0);
  });

  it("explains rays that don't cross", () => {
    expect(() => intersection([
      { point: a, azimuth: 90, north: "grid" },
      { point: b, azimuth: 90, north: "grid" },
    ])).toThrow(/don't cross/);
  });
});
