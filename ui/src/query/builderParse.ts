// SQL → builder rows (UX16). The inverse of `builderToSql` for the SQL the builder can show; anything else is refused with a plain reason and
// stays SQL. Pure, unit-tested.
import type { Field } from "../ipc/api";
import { isNumericType, type Clause } from "./builder";

export type ParsedRows = { ok: true; clauses: Clause[] } | { ok: false; reason: string };

const IDENT = String.raw`"(?:[^"]|"")+"|[A-Za-z_][A-Za-z0-9_]*`;
const STR = String.raw`'(?:[^']|'')*'`;
const NUM = String.raw`[-+]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?`;
const VAL = `${STR}|${NUM}`;
const re = (body: string) => new RegExp(String.raw`^\s*${body}\s*`, "i");
const R_CMP = re(`(${IDENT})\\s*(<>|>=|<=|=|>|<)\\s*(${VAL})`);
const R_BETWEEN = re(`(${IDENT})\\s+BETWEEN\\s+(${VAL})\\s+AND\\s+(${VAL})`);
const R_LIKE = re(`(${IDENT})\\s+ILIKE\\s+(${STR})`);
const R_NOT_LIKE = re(`NOT\\s*\\(\\s*(${IDENT})\\s+ILIKE\\s+(${STR})\\s*\\)`);
const R_IN = re(`(${IDENT})\\s+IN\\s*\\(\\s*((?:${VAL})(?:\\s*,\\s*(?:${VAL}))*)\\s*\\)`);
const R_NULL = re(`(${IDENT})\\s+IS\\s+(NOT\\s+)?NULL`);
const R_JOIN = /^(AND|OR)\b\s*/i;

const unIdent = (s: string) => (s.startsWith('"') ? s.slice(1, -1).replace(/""/g, '"') : s);
const unStr = (s: string) => s.slice(1, -1).replace(/''/g, "'");

/**
 * Show SQL as builder rows when every part is a comparison, BETWEEN, text match (ILIKE with % at the start, end or both), IN list or NULL
 * test on one field, joined by AND and OR (any letter case). Anything else (parentheses, functions, field-to-field comparisons, a text value
 * on a number field, an unknown field) is refused with a plain reason. Joins are kept per row exactly as written, so the rows rebuild the
 * same SQL and mean what the SQL means (AND still binds tighter than OR, as it does in SQL).
 */
export function sqlToClauses(sql: string, fields: Field[]): ParsedRows {
  let rest = sql.trim();
  if (!rest) return { ok: false, reason: "There is no SQL to show as rows." };
  const clauses: Clause[] = [];
  let join: "AND" | "OR" = "AND";
  const fail = (why: string): ParsedRows => ({ ok: false, reason: `This SQL can't be shown as rows (${why}). It stays as SQL.` });
  const near = () => `near “${rest.slice(0, 30)}${rest.length > 30 ? "…" : ""}”`;
  // SQL names are not case-sensitive: an exact match first, then a single match ignoring case.
  const field = (raw: string) => {
    const n = unIdent(raw);
    const same = fields.filter((f) => f.name.toLowerCase() === n.toLowerCase());
    return fields.find((f) => f.name === n) ?? (same.length === 1 ? same[0] : undefined);
  };
  const noField = (raw: string) => fail(`there is no field “${unIdent(raw)}”`);
  const typeBad = (name: string) => fail(`the value does not fit the type of ${name}`);
  const value = (f: Field, raw: string): string | null => {
    const quoted = raw.startsWith("'");
    if (isNumericType(f.type)) return quoted ? null : raw;
    return quoted ? unStr(raw) : null;
  };
  while (rest) {
    let m: RegExpMatchArray | null;
    let c: Omit<Clause, "join">;
    if ((m = rest.match(R_NOT_LIKE))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      if (isNumericType(f.type)) return typeBad(f.name);
      const p = unStr(m[2]);
      if (!(p.length >= 2 && p.startsWith("%") && p.endsWith("%"))) return fail(`a text match ${near()}`);
      c = { field: f.name, op: "not_contains", value: p.slice(1, -1), value2: "" };
    } else if ((m = rest.match(R_BETWEEN))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      const a = value(f, m[2]);
      const b = value(f, m[3]);
      if (a === null || b === null) return typeBad(f.name);
      c = { field: f.name, op: "between", value: a, value2: b };
    } else if ((m = rest.match(R_LIKE))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      if (isNumericType(f.type)) return typeBad(f.name);
      const p = unStr(m[2]);
      const lead = p.startsWith("%");
      const trail = p.endsWith("%") && p.length > (lead ? 1 : 0);
      if (lead && trail) c = { field: f.name, op: "contains", value: p.slice(1, -1), value2: "" };
      else if (trail) c = { field: f.name, op: "starts", value: p.slice(0, -1), value2: "" };
      else if (lead) c = { field: f.name, op: "ends", value: p.slice(1), value2: "" };
      else return fail(`a text match without % ${near()}`);
    } else if ((m = rest.match(R_IN))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      const vals = (m[2].match(new RegExp(VAL, "g")) ?? []).map((v) => value(f, v));
      if (vals.some((v) => v === null)) return typeBad(f.name);
      if ((vals as string[]).some((v) => v.includes(","))) return fail(`a list value that contains a comma, in ${f.name}`);
      c = { field: f.name, op: "in", value: (vals as string[]).join(", "), value2: "" };
    } else if ((m = rest.match(R_NULL))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      c = { field: f.name, op: m[2] ? "not_null" : "null", value: "", value2: "" };
    } else if ((m = rest.match(R_CMP))) {
      const f = field(m[1]);
      if (!f) return noField(m[1]);
      const v = value(f, m[3]);
      if (v === null) return typeBad(f.name);
      const op = ({ "=": "eq", "<>": "ne", ">": "gt", ">=": "ge", "<": "lt", "<=": "le" } as const)[m[2] as "="];
      c = { field: f.name, op, value: v, value2: "" };
    } else {
      return fail(`a part the builder does not have, ${near()}`);
    }
    clauses.push({ join, ...c });
    rest = rest.slice(m[0].length);
    if (!rest) break;
    const j = rest.match(R_JOIN);
    if (!j) return fail(`something other than AND or OR, ${near()}`);
    join = j[1].toUpperCase() as "AND" | "OR";
    rest = rest.slice(j[0].length);
    if (!rest) return fail("it ends with AND or OR");
  }
  return { ok: true, clauses };
}
