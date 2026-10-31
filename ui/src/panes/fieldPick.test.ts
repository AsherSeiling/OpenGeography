import { describe, expect, it } from "vitest";
import { defaultClassField } from "./fieldPick";

describe("defaultClassField", () => {
  it("skips identifier columns", () => {
    expect(defaultClassField([{ name: "OGC_FID" }, { name: "CELL_ID" }, { name: "COL" }, { name: "COUNT" }])).toBe("COUNT");
    expect(defaultClassField([{ name: "OGC_FID" }, { name: "AREALAND" }, { name: "POP100" }])).toBe("AREALAND");
    expect(defaultClassField([{ name: "objectid" }, { name: "gaz_id" }, { name: "mag" }])).toBe("mag");
  });
  it("falls back to the first field when everything looks like an id", () => {
    expect(defaultClassField([{ name: "fid" }, { name: "oid" }])).toBe("fid");
    expect(defaultClassField([])).toBeUndefined();
  });
});
