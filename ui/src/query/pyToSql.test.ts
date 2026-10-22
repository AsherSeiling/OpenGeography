import { describe, expect, it } from "vitest";
import { pyToSql, QueryError } from "./pyToSql";

const F = ["POP100", "COUNTY", "NAME", "CLASS", "NOTE", "Area sq mi"];

describe("Python-style definition queries", () => {
  it("translates comparisons and boolean logic", () => {
    expect(pyToSql("POP100 > 1000 and COUNTY == '109'", F)).toBe("POP100 > 1000 AND COUNTY = '109'");
    expect(pyToSql("not (CLASS != 'State' or POP100 <= 5)", F)).toBe("NOT (CLASS <> 'State' OR POP100 <= 5)");
  });

  it("handles in-lists, None and booleans", () => {
    expect(pyToSql("CLASS in ['State', 'County']", F)).toBe("CLASS IN ('State', 'County')");
    expect(pyToSql("CLASS not in ('A',)", F)).toBe("CLASS NOT IN ('A')");
    expect(pyToSql("NOTE is None", F)).toBe("NOTE IS NULL");
    expect(pyToSql("NOTE is not None", F)).toBe("NOTE IS NOT NULL");
    expect(pyToSql("NOTE == None", F)).toBe("NOTE IS NULL");
  });

  it("supports string methods and contains", () => {
    expect(pyToSql("NAME.startswith('Okla')", F)).toBe("NAME LIKE 'Okla%'");
    expect(pyToSql("NAME.upper().endswith('CITY')", F)).toBe("upper(NAME) LIKE '%CITY'");
    expect(pyToSql("NAME.lower() == 'norman'", F)).toBe("lower(NAME) = 'norman'");
    expect(pyToSql("'Lake' in NAME", F)).toBe("NAME LIKE '%Lake%'");
    expect(pyToSql("len(NAME) > 5", F)).toBe("length(NAME) > 5");
  });

  it("quotes field names and text safely", () => {
    expect(pyToSql("!Area sq mi! > 2", F)).toBe('"Area sq mi" > 2');
    expect(pyToSql("NAME == \"O'Brien\"", F)).toBe("NAME = 'O''Brien'");
    expect(pyToSql("pop100 > 1", F)).toBe("POP100 > 1");
  });

  it("explains mistakes", () => {
    expect(() => pyToSql("NAME == 'open", F)).toThrow(QueryError);
    expect(() => pyToSql("(POP100 > 1", F)).toThrow(/not closed/);
    expect(() => pyToSql("NAME.title() == 'X'", F)).toThrow(/isn't supported/);
  });
});
