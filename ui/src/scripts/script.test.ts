import { describe, expect, it } from "vitest";
import { fieldRef, filterTemplate, isWaiting, valueTemplate } from "./script";

const F = [{ name: "OBJECTID", type: "BIGINT" }, { name: "POP 2020", type: "DOUBLE" }, { name: "NAME", type: "VARCHAR" }];

describe("script helpers", () => {
  it("quotes any field name as a Python row key", () => {
    expect(fieldRef("POP")).toBe('row["POP"]');
    expect(fieldRef('odd "name"')).toBe('row["odd \\"name\\""]');
  });
  it("makes a starter script from the first useful field", () => {
    expect(filterTemplate(F)).toBe('def calculate(row):\n    return row["POP 2020"] > 0\n');
    expect(filterTemplate([{ name: "NAME", type: "VARCHAR" }])).toBe('def calculate(row):\n    return row["NAME"] == "value"\n');
    expect(valueTemplate(F)).toBe('def calculate(row):\n    return row["OBJECTID"]\n');
    expect(filterTemplate([])).toContain("def calculate(row)");
  });
  it("recognises the engine's waiting-for-approval error", () => {
    expect(isWaiting("Script waiting for approval. Review the script…")).toBe(true);
    expect(isWaiting("3 of 10 rows failed")).toBe(false);
    expect(isWaiting(null)).toBe(false);
  });
});
