// The part of a Python filter that can be answered by SQL with exactly the same rows as CPython would give (UR-20).
//
// Python and SQL disagree about nulls and types: `None > 3` raises in Python and is NULL in SQL, `x != 1` is True for None in Python and NULL in
// SQL, `1 / 0` raises in Python, `'5' == 5` is False in Python and may be a cast in SQL. A script must not select different rows depending on
// which path ran it, so this translator accepts only what cannot differ, and everything else goes to CPython (which is the real meaning):
//
//   script := def calculate(row):  return expr           (exactly one return; anything else is Python's)
//   atom := row["FIELD"] == literal | row["FIELD"] in [literal, ...] | row["FIELD"] is None | row["FIELD"] is not None | ( expr )
//   expr := atom (and|or atom)*         (and/or only: no `not`, `!=`, `not in`, orderings, arithmetic, methods)
//
// Why this is safe: a WHERE keeps a row only when the condition is TRUE. Under and/or, SQL's NULL acts like False for that purpose as long as
// no NULL-producing atom sits under a negation, and the atoms here are the equality ones (NULL when the field is null; Python gives False)
// and the two `is` ones (never NULL). Literals must match the field's type (text with a text field, number with a number field) and field
// names must match exactly (Python row keys are case sensitive). `crates/engine/tests/pyconformance.rs` runs the cases in
// `conformance.json` through DuckDB and through CPython and compares the rows; `pySubset.test.ts` keeps the file current.

export interface SubsetField {
  name: string;
  /** The column's SQL type name as the engine reports it (INTEGER, BIGINT, DOUBLE, VARCHAR, …). */
  type: string;
}

type Tok = { t: "str" | "num" | "id" | "kw" | "op"; v: string };

const NUMERIC = /^(TINYINT|SMALLINT|INTEGER|BIGINT|UTINYINT|USMALLINT|UINTEGER|FLOAT|REAL|DOUBLE)$/i;
const KEYWORDS = new Set(["and", "or", "in", "is", "not", "None", "row"]);

function tokenize(src: string): Tok[] | null {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) i++;
    else if (c === "'" || c === '"') {
      const j = src.indexOf(c, i + 1);
      if (j < 0) return null;
      const s = src.slice(i + 1, j);
      if (s.includes("\\") || s.includes("\n")) return null; // escapes: leave them to Python
      out.push({ t: "str", v: s });
      i = j + 1;
    } else if (/[0-9]/.test(c) || (c === "-" && /[0-9]/.test(src[i + 1] ?? ""))) {
      const m = src.slice(i).match(/^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/);
      if (!m) return null;
      out.push({ t: "num", v: m[0] });
      i += m[0].length;
    } else if (/[A-Za-z_]/.test(c)) {
      const m = src.slice(i).match(/^[A-Za-z_][A-Za-z0-9_]*/)!;
      out.push({ t: KEYWORDS.has(m[0]) ? "kw" : "id", v: m[0] });
      i += m[0].length;
    } else if (src.startsWith("==", i)) {
      out.push({ t: "op", v: "==" });
      i += 2;
    } else if ("()[],".includes(c)) {
      out.push({ t: "op", v: c });
      i++;
    } else return null;
  }
  return out;
}

const sqlStr = (s: string) => `'${s.replace(/'/g, "''")}'`;
const sqlIdent = (n: string) => `"${n.replace(/"/g, '""')}"`;

const SCRIPT = /^\s*def\s+calculate\s*\(\s*row\s*\)\s*:\s*(?:\r?\n\s+)?return\s+([^\r\n]+?)\s*$/;

/** SQL with exactly CPython's result rows for the script `src`, or null when it is outside the safe subset (run it with Python instead). */
export function pySubsetSql(src: string, fields: SubsetField[]): string | null {
  const m = SCRIPT.exec(src);
  if (!m) return null;
  const toks = tokenize(m[1]);
  if (!toks || !toks.length) return null;
  const byName = new Map(fields.map((f) => [f.name, f.type]));
  let p = 0;
  const peek = () => toks[p];
  const isKw = (v: string) => peek()?.t === "kw" && peek().v === v;
  const isOp = (v: string) => peek()?.t === "op" && peek().v === v;

  const literal = (fieldType: string): string | null => {
    const t = toks[p];
    if (!t) return null;
    const numeric = NUMERIC.test(fieldType);
    const text = /^VARCHAR$/i.test(fieldType);
    if (t.t === "num" && numeric) {
      p++;
      return t.v;
    }
    if (t.t === "str" && text) {
      p++;
      return sqlStr(t.v);
    }
    return null; // wrong kind of literal for the field, or a field type this path does not handle
  };

  const atom = (): string | null => {
    if (isOp("(")) {
      p++;
      const e = orExpr();
      if (e === null || !isOp(")")) return null;
      p++;
      return `(${e})`;
    }
    // row["FIELD"]
    if (!isKw("row")) return null;
    p++;
    if (!isOp("[")) return null;
    p++;
    const f = peek();
    if (!f || f.t !== "str" || !byName.has(f.v)) return null;
    p++;
    if (!isOp("]")) return null;
    p++;
    const type = byName.get(f.v)!;
    const col = sqlIdent(f.v);
    if (isOp("==")) {
      p++;
      const lit = literal(type);
      return lit === null ? null : `${col} = ${lit}`;
    }
    if (isKw("in")) {
      p++;
      if (!isOp("[")) return null;
      p++;
      const items: string[] = [];
      while (true) {
        const lit = literal(type);
        if (lit === null) return null;
        items.push(lit);
        if (isOp(",")) {
          p++;
          if (isOp("]")) break; // trailing comma
          continue;
        }
        break;
      }
      if (!isOp("]")) return null;
      p++;
      return `${col} IN (${items.join(", ")})`;
    }
    if (isKw("is")) {
      p++;
      let neg = false;
      if (isKw("not")) {
        neg = true;
        p++;
      }
      if (!isKw("None")) return null;
      p++;
      return `${col} IS ${neg ? "NOT " : ""}NULL`;
    }
    return null;
  };
  const andExpr = (): string | null => {
    let l = atom();
    while (l !== null && isKw("and")) {
      p++;
      const r = atom();
      l = r === null ? null : `${l} AND ${r}`;
    }
    return l;
  };
  const orExpr = (): string | null => {
    let l = andExpr();
    while (l !== null && isKw("or")) {
      p++;
      const r = andExpr();
      l = r === null ? null : `${l} OR ${r}`;
    }
    return l;
  };
  const sql = orExpr();
  return sql !== null && p === toks.length ? sql : null;
}
