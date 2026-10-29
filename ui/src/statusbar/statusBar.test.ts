import { describe, expect, it } from "vitest";
import { describeCrs } from "../dialogs/Dialogs";
import { outsideArea } from "./StatusBar";

describe("outsideArea", () => {
  const utm14: [number, number, number, number] = [-102, 0, -96, 84];
  it("is false inside and on the edge, true outside", () => {
    expect(outsideArea(-99, 35, utm14)).toBe(false);
    expect(outsideArea(-96, 84, utm14)).toBe(false);
    expect(outsideArea(-90, 35, utm14)).toBe(true);
    expect(outsideArea(-99, -10, utm14)).toBe(true);
  });
  it("handles an area that crosses the antimeridian", () => {
    const fiji: [number, number, number, number] = [172, -22, -178, -12];
    expect(outsideArea(179, -17, fiji)).toBe(false);
    expect(outsideArea(-179, -17, fiji)).toBe(false);
    expect(outsideArea(0, -17, fiji)).toBe(true);
  });
});

describe("describeCrs", () => {
  it("lists name, code, kind, unit, datum and area", () => {
    const t = describeCrs({ name: "NAD83 / UTM zone 14N", code: "EPSG:26914", geographic: false, unit: "metre", datum: "North American Datum 1983", areaOfUse: [-102, 0, -96, 84], unitNote: null });
    expect(t).toContain("EPSG:26914");
    expect(t).toContain("projected");
    expect(t).toContain("North American Datum 1983");
    expect(t).toContain("valid 0.0°–84.0° N");
  });
  it("says so when the area is unknown", () => {
    expect(describeCrs({ name: "Custom", code: null, geographic: true, unit: "degree", datum: null, areaOfUse: null, unitNote: null })).toContain("area of use unknown");
  });
});
