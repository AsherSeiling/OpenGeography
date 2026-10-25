import { describe, expect, it } from "vitest";
import type { Field } from "../ipc/api";
import { builderToSql, isComplete, isNumericType, needsValue, opsFor, type Clause } from "./builder";
import { pyToSql, QueryError } from "./pyToSql";

const F = (name: string, type: string): Field => ({ name, type }) as Field;
const fields = [F("POP", "BIGINT"), F("NAME", "VARCHAR"), F("weird name", "VARCHAR"), F('q"uote', "DOUBLE")];
const row = (over: Partial<Clause>): Clause => ({ join: "AND", field: "NAME", op: "eq", value: "x", value2: "", ...over });

describe("builder edge cases", () => {
  it("an empty or all-incomplete list gives an empty filter", () => {
    expect(builderToSql([], fields)).toBe("");
    expect(builderToSql([row({ value: "  " }), row({ field: "" })], fields)).toBe("");
  });

  it("the first complete row's join word is dropped even when earlier rows were skipped", () => {
    expect(builderToSql([row({ value: "" , join: "OR"}), row({ join: "OR", value: "a" }), row({ join: "AND", value: "b" })], fields)).toBe("NAME = 'a' AND NAME = 'b'");
  });

  it("quotes values and identifiers so they can't break out", () => {
    expect(builderToSql([row({ value: "O'Brien; DROP TABLE x" })], fields)).toBe("NAME = 'O''Brien; DROP TABLE x'");
    expect(builderToSql([row({ field: "weird name" })], fields)).toBe('"weird name" = \'x\'');
    expect(builderToSql([row({ field: 'q"uote', value: "1" })], fields)).toBe('"q""uote" = 1');
    expect(builderToSql([row({ op: "contains", value: "50%_" })], fields)).toBe("NAME ILIKE '%50%_%'");
  });

  it("numeric fields refuse non-numbers, NaN and empty list items; accept signs and exponents", () => {
    for (const v of ["abc", "NaN", "1,5", "--1"]) expect(() => builderToSql([row({ field: "POP", value: v })], fields), v).toThrow(/not a number|Row 1/);
    expect(builderToSql([row({ field: "POP", value: "-1.5e3" })], fields)).toBe("POP = -1.5e3");
    expect(builderToSql([row({ field: "POP", op: "in", value: "1, 2,,3," })], fields)).toBe("POP IN (1, 2, 3)");
    expect(builderToSql([row({ field: "POP", op: "between", value: "1", value2: "9" })], fields)).toBe("POP BETWEEN 1 AND 9");
  });

  it("a row naming a field that no longer exists is reported by row number", () => {
    expect(() => builderToSql([row({}), row({ field: "GONE" })], fields)).toThrow("Row 2: choose a field");
  });

  it("null checks need no value; between needs both", () => {
    expect(isComplete(row({ op: "null", value: "" }))).toBe(true);
    expect(isComplete(row({ op: "between", value: "1", value2: "" }))).toBe(false);
    expect(needsValue("not_null")).toBe(false);
    expect(builderToSql([row({ op: "not_null", value: "" })], fields)).toBe("NAME IS NOT NULL");
  });

  it("operators offered depend on the field type", () => {
    expect(opsFor(undefined).map(([o]) => o)).not.toContain("gt");
    expect(opsFor(F("a", "DOUBLE")).map(([o]) => o)).toContain("between");
    expect(opsFor(F("a", "VARCHAR")).map(([o]) => o)).toContain("starts");
    expect(["INTEGER", "HUGEINT", "decimal(10,2)", "FLOAT"].every(isNumericType)).toBe(true);
    expect(isNumericType("VARCHAR")).toBe(false);
  });
});

describe("Python filter translation: edge inputs", () => {
  it("empty and whitespace-only input means no filter", () => {
    expect(pyToSql("")).toBe("");
    expect(pyToSql("   \n ")).toBe("");
  });

  it.each(["(POP > 5", "POP > 5)", "NAME.startswith(", "[1, 2", "x ~ y", "POP @ 5", "a $ b"])("rejects unbalanced brackets and unknown characters in %j with a QueryError", (src) => {
    expect(() => pyToSql(src)).toThrow(QueryError);
  });

  it.each(["POP >", "and POP", "POP > 5 and", "POP == ", "POP in", "POP > 5 6"])("grammar errors in %j are left for the engine's SQL parser to report (no crash here)", (src) => {
    expect(() => pyToSql(src)).not.toThrow();
  });

  it("unterminated strings and !fields! are reported", () => {
    expect(() => pyToSql("NAME == 'abc")).toThrow(/closing quote/);
    expect(() => pyToSql("!NAME == 1")).toThrow(/closing !/);
  });

  it("quotes inside strings are escaped for SQL", () => {
    expect(pyToSql(`NAME == "it's"`)).toBe("NAME = 'it''s'");
    expect(pyToSql("NAME == 'a\\'b'")).toBe("NAME = 'a''b'");
  });

  it("a semicolon or comment marker inside a string stays data", () => {
    expect(pyToSql("NAME == 'a; DROP TABLE t; --'")).toBe("NAME = 'a; DROP TABLE t; --'");
  });

  it("a very long expression translates in reasonable time", () => {
    const src = Array.from({ length: 2000 }, (_, i) => `POP > ${i}`).join(" or ");
    const t = Date.now();
    const sql = pyToSql(src);
    expect(sql.split(" OR ")).toHaveLength(2000);
    expect(Date.now() - t).toBeLessThan(1000);
  });

  it("numbers: floats, exponents and a leading dot", () => {
    expect(pyToSql("A > .5")).toBe("A > .5");
    expect(pyToSql("A < 1e3")).toBe("A < 1e3");
  });

  it("field names are matched to the table's spelling case-insensitively; unknown ones pass through for the engine to reject", () => {
    expect(pyToSql("pop > 1", ["POP"])).toBe("POP > 1");
    expect(pyToSql("NOPE > 1", ["POP"])).toBe("NOPE > 1");
    expect(pyToSql("!my field! > 1", ["my field"])).toBe('"my field" > 1');
  });
});
