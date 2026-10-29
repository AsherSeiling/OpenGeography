import { describe, expect, it } from "vitest";
import { buildProjString, builderName, DEFAULTS, FAMILIES, validate } from "./projectionBuilder";

describe("projection builder (R-W9)", () => {
  it("builds a conic with parallels 20°/60° and central meridian 10°", () => {
    const s = buildProjString({ ...DEFAULTS, family: "lcc", lat0: 40, lat1: 20, lat2: 60, lon0: 10 });
    expect(s).toBe("+proj=lcc +lat_1=20 +lat_2=60 +lat_0=40 +lon_0=10 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs");
  });
  it("is stable: the same parameters give the same string, and a moved meridian changes only lon_0", () => {
    const a = buildProjString({ ...DEFAULTS, lon0: 10 })!;
    expect(buildProjString({ ...DEFAULTS, lon0: 10 })).toBe(a);
    expect(buildProjString({ ...DEFAULTS, lon0: 11 })!.replace("+lon_0=11", "+lon_0=10")).toBe(a);
  });
  it("rejects parallels that are equal and opposite, outside ±90°, or poles, with a sentence", () => {
    expect(validate({ ...DEFAULTS, lat1: 30, lat2: -30 }).join(" ")).toMatch(/equal and opposite/);
    expect(validate({ ...DEFAULTS, lat1: 95 }).join(" ")).toMatch(/between −90° and 90°/);
    expect(validate({ ...DEFAULTS, lat2: 90 }).join(" ")).toMatch(/cannot be a pole/);
    expect(validate({ ...DEFAULTS, lon0: 200 }).join(" ")).toMatch(/central meridian/);
    expect(buildProjString({ ...DEFAULTS, lat1: 30, lat2: -30 })).toBeNull();
    expect(validate({ ...DEFAULTS, family: "lcc", lat1: 0, lat2: 0 }).join(" ")).toMatch(/away from the equator/);
  });
  it("only writes the parameters a family uses", () => {
    expect(buildProjString({ ...DEFAULTS, family: "robin", lon0: 150 })).toBe("+proj=robin +lon_0=150 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs");
    expect(buildProjString({ ...DEFAULTS, family: "laea", lat0: 90, lon0: 0 })).toContain("+lat_0=90");
    expect(buildProjString({ ...DEFAULTS, family: "eqc", latTs: 30 })).toContain("+lat_ts=30");
  });
  it("every family builds with defaults and has a note", () => {
    for (const f of FAMILIES) {
      const p = { ...DEFAULTS, family: f.id, lat1: 20, lat2: 50 };
      expect(buildProjString(p), f.id).toMatch(new RegExp(`^\\+proj=${f.id} `));
      expect(f.note.length).toBeGreaterThan(10);
      expect(builderName(p)).toContain(f.label);
    }
  });
});
