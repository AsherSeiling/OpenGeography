import { describe, expect, it } from "vitest";
import { guessGeoidField, parseVariables, PRESETS, vintages } from "./presets";

describe("Census presets and checks", () => {
  it("splits typed variables, upper-cases, de-duplicates and reports non-codes", () => {
    expect(parseVariables("b01003_001e, B19013_001E\nb01003_001e;P1_001N")).toEqual({ codes: ["B01003_001E", "B19013_001E", "P1_001N"], invalid: [] });
    expect(parseVariables("B01003_001E, drop table;")).toEqual({ codes: ["B01003_001E", "DROP", "TABLE"], invalid: [] });
    expect(parseVariables("B01003-001E $x").invalid).toEqual(["B01003-001E", "$x"]);
    expect(parseVariables("  ")).toEqual({ codes: [], invalid: [] });
  });
  it("offers the years that exist", () => {
    expect(vintages("acs5", 2026)[0]).toBe(2024);
    expect(vintages("acs5", 2026).at(-1)).toBe(2009);
    expect(vintages("acs1", 2026).at(-1)).toBe(2005);
    expect(vintages("dec_dhc")).toEqual([2020]);
  });
  it("finds the GEOID field", () => {
    expect(guessGeoidField([{ name: "NAME" }, { name: "geoid" }])).toBe("geoid");
    expect(guessGeoidField([{ name: "STATEFP" }, { name: "COUNTY_FIPS" }])).toBe("COUNTY_FIPS");
    expect(guessGeoidField([{ name: "NAME" }])).toBeNull();
  });
  it("every preset is a well-formed code", () => {
    for (const p of PRESETS) expect(p.code).toMatch(/^[A-Z][0-9A-Z_]+$/);
    expect(new Set(PRESETS.map((p) => p.code)).size).toBe(PRESETS.length);
  });
});
