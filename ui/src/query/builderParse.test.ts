// UX16: the builder shows the SQL it can and says plainly when it can't. Round trips: SQL → rows → the same SQL.
import { describe, expect, it } from "vitest";
import { builderToSql, type Clause } from "./builder";
import { sqlToClauses } from "./builderParse";

const fields = [
  { name: "POP100", type: "DOUBLE" },
  { name: "COUNT", type: "BIGINT" },
  { name: "NAME", type: "VARCHAR" },
  { name: "Land Use", type: "VARCHAR" },
];
const rows = (sql: string) => {
  const p = sqlToClauses(sql, fields);
  if (!p.ok) throw new Error(p.reason);
  return p.clauses;
};
const why = (sql: string) => {
  const p = sqlToClauses(sql, fields);
  return p.ok ? null : p.reason;
};

describe("SQL the builder can show", () => {
  const SHOWN = [
    "POP100 > 1000",
    "POP100 > 1000 OR NAME ILIKE 'Okla%'",
    "POP100 BETWEEN 5 AND 10",
    "NAME IN ('A', 'B', 'C')",
    "COUNT IN (1, 2, 3)",
    "\"Land Use\" IS NULL",
    "NAME IS NOT NULL AND POP100 <= 2.5",
    "NAME = 'O''Neil'",
    "NAME <> 'x'",
    "NAME ILIKE '%ville%'",
    "NAME ILIKE '%son'",
    "NOT (NAME ILIKE '%x%')",
    "POP100 >= -3 AND POP100 < 1e5 AND NAME = 'a' OR NAME = 'b'",
  ];
  for (const sql of SHOWN) {
    it(`round trips: ${sql}`, () => {
      expect(builderToSql(rows(sql), fields)).toBe(sql);
    });
  }

  it("reads the rows with the right operator, field and value", () => {
    const c = rows("POP100 BETWEEN 5 AND 10 OR NAME ILIKE 'Okla%'");
    expect(c).toEqual<Clause[]>([
      { join: "AND", field: "POP100", op: "between", value: "5", value2: "10" },
      { join: "OR", field: "NAME", op: "starts", value: "Okla", value2: "" },
    ]);
  });

  it("accepts any letter case and spacing, and writes the builder's own form back", () => {
    expect(builderToSql(rows("pop100>5  and  name  ilike  '%a%'"), fields)).toBe("POP100 > 5 AND NAME ILIKE '%a%'");
  });
});

describe("SQL the builder can't show says so plainly", () => {
  it("a function, parentheses, a field-to-field comparison, a subquery", () => {
    expect(why("UPPER(NAME) = 'A'")).toMatch(/can't be shown as rows.*It stays as SQL/);
    expect(why("(POP100 > 5 OR POP100 < 2) AND NAME = 'a'")).toMatch(/can't be shown as rows/);
    expect(why("POP100 > COUNT")).toMatch(/can't be shown as rows/);
    expect(why("NAME IN (SELECT NAME FROM other)")).toMatch(/can't be shown as rows/);
    expect(why("POP100 + 1 > 5")).toMatch(/can't be shown as rows/);
  });
  it("an unknown field, or a value of the wrong type, names the field", () => {
    expect(why("NOPE = 1")).toMatch(/no field “NOPE”/);
    expect(why("POP100 = 'big'")).toMatch(/does not fit the type of POP100/);
    expect(why("NAME = 5")).toMatch(/does not fit the type of NAME/);
    expect(why("POP100 ILIKE '%1%'")).toMatch(/does not fit the type of POP100/);
  });
  it("a dangling join, a text match without %, empty SQL", () => {
    expect(why("NAME = 'a' AND")).toMatch(/ends with AND or OR/);
    expect(why("NAME ILIKE 'abc'")).toMatch(/without %/);
    expect(why("   ")).toMatch(/no SQL to show/);
    expect(why("NAME = 'a' XOR NAME = 'b'")).toMatch(/something other than AND or OR/);
  });
});
