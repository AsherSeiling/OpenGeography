// Point-and-click query builder rows → SQL (pure, unit-tested).
import type { Field } from "../ipc/api";

export type Op =
  | "eq" | "ne" | "gt" | "ge" | "lt" | "le" | "between"
  | "contains" | "not_contains" | "starts" | "ends" | "in" | "null" | "not_null";

export interface Clause {
  join: "AND" | "OR";
  field: string;
  op: Op;
  value: string;
  value2: string;
}

export const isNumericType = (t: string) => /INT|DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC|HUGEINT/i.test(t);

export function opsFor(f: Field | undefined): [Op, string][] {
  const common: [Op, string][] = [["null", "is empty (null)"], ["not_null", "is not empty"]];
  if (f && isNumericType(f.type)) {
    return [["eq", "="], ["ne", "≠"], ["gt", ">"], ["ge", "≥"], ["lt", "<"], ["le", "≤"], ["between", "is between"], ["in", "is one of (comma list)"], ...common];
  }
  return [
    ["eq", "is equal to"], ["ne", "is not equal to"], ["contains", "contains"], ["not_contains", "does not contain"],
    ["starts", "begins with"], ["ends", "ends with"], ["in", "is one of (comma list)"], ...common,
  ];
}

export const needsValue = (op: Op) => op !== "null" && op !== "not_null";

const q = (s: string) => `'${s.replace(/'/g, "''")}'`;
const ident = (f: string) => (/^[A-Za-z_][A-Za-z0-9_]*$/.test(f) ? f : `"${f.replace(/"/g, '""')}"`);

/** Rows that are filled in enough to use; half-finished rows are skipped. */
export function isComplete(c: Clause): boolean {
  if (!c.field) return false;
  if (!needsValue(c.op)) return true;
  return c.value.trim() !== "" && (c.op !== "between" || c.value2.trim() !== "");
}

/** SQL for the completed rows ("" if none), or throws on an invalid value. */
export function builderToSql(clauses: Clause[], fields: Field[]): string {
  const parts: string[] = [];
  clauses.filter(isComplete).forEach((c, i) => {
    const f = fields.find((x) => x.name === c.field);
    if (!f) throw new Error(`Row ${i + 1}: choose a field`);
    const num = isNumericType(f.type);
    const val = (v: string) => {
      const t = v.trim();
      if (num) {
        if (t === "" || isNaN(Number(t))) throw new Error(`Row ${i + 1}: "${v}" is not a number`);
        return t;
      }
      return q(v);
    };
    const F = ident(f.name);
    let s: string;
    switch (c.op) {
      case "eq": s = `${F} = ${val(c.value)}`; break;
      case "ne": s = `${F} <> ${val(c.value)}`; break;
      case "gt": s = `${F} > ${val(c.value)}`; break;
      case "ge": s = `${F} >= ${val(c.value)}`; break;
      case "lt": s = `${F} < ${val(c.value)}`; break;
      case "le": s = `${F} <= ${val(c.value)}`; break;
      case "between": s = `${F} BETWEEN ${val(c.value)} AND ${val(c.value2)}`; break;
      case "contains": s = `${F} ILIKE ${q(`%${c.value}%`)}`; break;
      case "not_contains": s = `NOT (${F} ILIKE ${q(`%${c.value}%`)})`; break;
      case "starts": s = `${F} ILIKE ${q(`${c.value}%`)}`; break;
      case "ends": s = `${F} ILIKE ${q(`%${c.value}`)}`; break;
      case "in": s = `${F} IN (${c.value.split(",").map((v) => v.trim()).filter(Boolean).map(val).join(", ")})`; break;
      case "null": s = `${F} IS NULL`; break;
      case "not_null": s = `${F} IS NOT NULL`; break;
    }
    parts.push(i === 0 ? s : `${c.join} ${s}`);
  });
  return parts.join(" ");
}
