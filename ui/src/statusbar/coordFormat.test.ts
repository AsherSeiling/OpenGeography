import { describe, expect, it } from "vitest";
import { COORD_FORMATS, dms, formatPointer, mgrsOf, readCoordFormat } from "./coordFormat";

const p = { lon: -97.5, lat: 35.25, x: 650123.46, y: 3901234.5 };

describe("coordinate format", () => {
  it("reads a saved value, and falls back to decimal degrees for a missing or unknown one", () => {
    expect(readCoordFormat("mgrs")).toBe("mgrs");
    expect(readCoordFormat(undefined)).toBe("dd");
    expect(readCoordFormat("nonsense")).toBe("dd");
    expect(COORD_FORMATS.map((f) => f.id)).toEqual(["dd", "dms", "projected", "mgrs"]);
  });
  it("shows exactly one format", () => {
    expect(formatPointer("dd", p, true).text).toBe("35.25000°, -97.50000°");
    expect(formatPointer("dms", p, true).text).toBe(`${dms(35.25, "N", "S")} ${dms(-97.5, "E", "W")}`);
    expect(formatPointer("dms", p, true).text).toBe("35°15′00.0″N 97°30′00.0″W");
    expect(formatPointer("mgrs", p, true).text).toBe(mgrsOf(-97.5, 35.25));
    expect(formatPointer("projected", p, true).text).toBe("650,123.5 E 3,901,234.5 N");
  });
  it("map coordinates fall back to degrees, and say so, when the map is drawn in degrees", () => {
    const r = formatPointer("projected", p, false);
    expect(r.format).toBe("dd");
    expect(r.text).toBe("35.25000°, -97.50000°");
    expect(r.title).toMatch(/no eastings and northings/);
  });
  it("MGRS has no reading near the poles", () => {
    expect(mgrsOf(0, 85)).toBe("—");
    expect(mgrsOf(0, -81)).toBe("—");
  });
});
