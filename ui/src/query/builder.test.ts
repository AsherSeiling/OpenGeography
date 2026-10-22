import { describe, expect, it } from "vitest";
import { builderToSql, type Clause } from "./builder";

const fields = [
  { name: "POP100", type: "DOUBLE" },
  { name: "NAME", type: "VARCHAR" },
  { name: "Land Use", type: "VARCHAR" },
];
const c = (p: Partial<Clause>): Clause => ({ join: "AND", field: "NAME", op: "eq", value: "", value2: "", ...p });

describe("query builder", () => {
  it("builds clauses with AND/OR", () => {
    expect(builderToSql([c({ field: "POP100", op: "gt", value: "1000" }), c({ join: "OR", op: "starts", value: "Okla" })], fields))
      .toBe("POP100 > 1000 OR NAME ILIKE 'Okla%'");
  });

  it("handles lists, ranges, nulls and quoting", () => {
    expect(builderToSql([c({ field: "POP100", op: "between", value: "5", value2: "10" })], fields)).toBe("POP100 BETWEEN 5 AND 10");
    expect(builderToSql([c({ op: "in", value: "A, B ,C" })], fields)).toBe("NAME IN ('A', 'B', 'C')");
    expect(builderToSql([c({ field: "Land Use", op: "null" })], fields)).toBe('"Land Use" IS NULL');
    expect(builderToSql([c({ op: "eq", value: "O'Neil" })], fields)).toBe("NAME = 'O''Neil'");
  });

  it("skips half-finished rows and explains bad values", () => {
    expect(builderToSql([c({ field: "" })], fields)).toBe("");
    expect(builderToSql([c({ field: "POP100", op: "gt", value: "" }), c({ join: "OR", op: "eq", value: "Tulsa" })], fields)).toBe("NAME = 'Tulsa'");
    expect(() => builderToSql([c({ field: "POP100", op: "gt", value: "abc" })], fields)).toThrow(/not a number/);
  });
});
