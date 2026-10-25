import { describe, expect, it } from "vitest";
import { convertAzimuth, degToMils, direct, formatArea, formatLength, geodesicLine, inverse, milsToDeg, norm180, norm360, parsePosition, pathLength, polygonArea, toMgrs, utmZone } from "./geodesy";

describe("angle helpers", () => {
  it("norm360 and norm180 wrap negatives, multiples and large values", () => {
    expect(norm360(-1)).toBe(359);
    expect(norm360(360)).toBe(0);
    expect(norm360(720.5)).toBeCloseTo(0.5, 9);
    expect(norm360(-720 - 90)).toBe(270);
    expect(norm180(190)).toBe(-170);
    expect(norm180(-190)).toBe(170);
    expect(norm180(180)).toBe(-180);
    expect(Number.isNaN(norm360(NaN))).toBe(true);
  });
  it("mils: a circle is 6400, and conversion round-trips", () => {
    expect(degToMils(360)).toBe(0);
    expect(degToMils(90)).toBe(1600);
    expect(milsToDeg(3200)).toBe(180);
    for (const d of [0, 1, 45.5, 359.9]) expect(milsToDeg(degToMils(d))).toBeCloseTo(d, 9);
  });
});

describe("geodesic edge cases", () => {
  it("same point: zero distance", () => {
    expect(inverse([-97, 35], [-97, 35]).distance).toBe(0);
  });
  it("near-antipodal points converge to a finite answer", () => {
    const r = inverse([0, 0], [179.7, 0.1]);
    expect(Number.isFinite(r.distance) && Number.isFinite(r.azimuth)).toBe(true);
    expect(r.distance).toBeGreaterThan(19_900_000);
  });
  it("across the antimeridian takes the short way", () => {
    expect(inverse([179.9, 0], [-179.9, 0]).distance).toBeCloseTo(22_263.9, 0);
  });
  it("pole to pole is half a meridian (~20,003.9 km)", () => {
    expect(inverse([0, 90], [0, -90]).distance).toBeCloseTo(20_003_931.5, -1);
  });
  it("direct then inverse recovers the distance and azimuth from the poles' neighbourhood", () => {
    const p = direct([10, 89.9], 45, 5_000);
    const r = inverse([10, 89.9], p);
    expect(r.distance).toBeCloseTo(5_000, 3);
    expect(r.azimuth).toBeCloseTo(45, 4);
  });
  it("zero-length and one-point paths and areas", () => {
    expect(pathLength([])).toBe(0);
    expect(pathLength([[1, 1]])).toBe(0);
    expect(polygonArea([]).area).toBe(0);
    expect(polygonArea([[0, 0], [1, 1]]).area).toBe(0);
  });
  it("polygon area doesn't depend on winding direction", () => {
    const sq: [number, number][] = [[0, 0], [0.1, 0], [0.1, 0.1], [0, 0.1]];
    const a = polygonArea(sq);
    const b = polygonArea([...sq].reverse());
    expect(a.area).toBeCloseTo(b.area, 3);
    expect(a.area / 1e6).toBeCloseTo(123.09, 0); // 0.1° × 0.1° at the equator ≈ 123.1 km² on the ellipsoid
    expect(a.perimeter).toBeCloseTo(b.perimeter, 3);
  });
  it("geodesicLine has steps+1 points, hits both ends and stays on the geodesic", () => {
    const line = geodesicLine([-97, 35], [-0.1, 51.5], 16);
    expect(line).toHaveLength(17);
    expect(line[0][0]).toBeCloseTo(-97, 6);
    expect(line[16][1]).toBeCloseTo(51.5, 6);
    expect(pathLength(line)).toBeCloseTo(inverse([-97, 35], [-0.1, 51.5]).distance, 0);
  });
});

describe("position parsing", () => {
  it.each([
    ["35.5, -97.25", [-97.25, 35.5]],
    ["35.5 -97.25", [-97.25, 35.5]],
    ["  -10,20  ", [20, -10]],
    ["0,0", [0, 0]],
    ["90, 180", [180, 90]],
  ])("reads %j as lat, lon", (text, lonlat) => expect(parsePosition(text)).toEqual(lonlat));

  it.each(["91, 0", "0, 181", "-91, 0", "", "abc", "35.5", "35.5,", "35,-97,12", "NaN, NaN", "1e2, 1"])("rejects %j with a readable message", (text) => {
    expect(() => parsePosition(text)).toThrow(/Latitude|MGRS/);
  });

  it("MGRS round-trips through toMgrs at every precision", () => {
    const p: [number, number] = [-97.5164, 35.4676];
    for (const d of [1, 2, 3, 4, 5] as const) {
      const back = parsePosition(toMgrs(p, d));
      expect(Math.abs(back[0] - p[0])).toBeLessThan(10 ** (5 - d) / 111_000 * 1.5);
    }
  });

  it("MGRS parse tolerates spaces and lower case, and rejects odd digit counts", () => {
    const a = parsePosition("14S PF 12345 67890");
    expect(parsePosition("14spf1234567890")).toEqual(a);
    expect(() => parsePosition("14S PF 123 456 7")).toThrow();
    expect(() => parsePosition("14I PF 12345 67890")).toThrow(); // latitude band I is not used
  });
});

describe("UTM zone choice", () => {
  it("covers the edges of the longitude range and both hemispheres", () => {
    expect(utmZone([-180, 10]).zone).toBe(1);
    expect(utmZone([180, 10]).zone).toBe(60);
    expect(utmZone([-97.5, 35]).zone).toBe(14);
    expect(utmZone([-97.5, -35]).proj).toContain("+south");
    expect(utmZone([-97.5, 0]).proj).not.toContain("+south"); // the equator belongs to the north zone
  });
});

describe("azimuth conversion", () => {
  it("true→true is the identity and conversions are inverse", () => {
    const p: [number, number] = [-97.5, 35.4];
    expect(convertAzimuth(123, "true", "true", p)).toBeCloseTo(123, 9);
    for (const from of ["grid", "magnetic", "true"] as const) for (const to of ["grid", "magnetic", "true"] as const) {
      const there = convertAzimuth(77, from, to, p, 1.2);
      expect(convertAzimuth(there, to, from, p, 1.2)).toBeCloseTo(77, 6);
    }
  });
  it("results are always within 0–360", () => {
    expect(convertAzimuth(359, "true", "magnetic", [-97.5, 35.4], 5)).toBeLessThan(360);
    expect(convertAzimuth(0, "true", "grid", [-97.5, 35.4])).toBeGreaterThanOrEqual(0);
  });
});

describe("formatting", () => {
  it("lengths use the unit's divisor and precision by magnitude", () => {
    expect(formatLength(1609.344, "mi")).toBe("1 mi");
    expect(formatLength(5, "m")).toBe("5 m");
    expect(formatLength(1_234_567, "km")).toBe("1,234.6 km");
    expect(formatLength(0, "m")).toBe("0 m");
    expect(formatLength(1200, "usft")).toContain("US ft");
  });
  it("an unknown unit falls back to metres / square metres", () => {
    expect(formatLength(12, "furlong")).toContain("12");
    expect(formatArea(12, "hide")).toContain("12");
  });
  it("NaN and Infinity don't throw", () => {
    expect(() => formatLength(NaN, "m")).not.toThrow();
    expect(() => formatArea(Infinity, "ha")).not.toThrow();
  });
});
