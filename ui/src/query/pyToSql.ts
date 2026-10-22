// Translate a Python-style filter expression into SQL for the engine.
//
//   POP100 > 1000 and COUNTY == '109'          → POP100 > 1000 AND COUNTY = '109'
//   NAME.startswith('Okla')                     → NAME LIKE 'Okla%'
//   CLASS in ['State', 'County']                → CLASS IN ('State', 'County')
//   NOTE is None                                → NOTE IS NULL
//   'Lake' in NAME                              → NAME LIKE '%Lake%'
//   !POP100! >= 50                              (ArcGIS-style field names work too)

type Tok = { t: "str" | "num" | "id" | "op" | "kw"; v: string };

const KEYWORDS = new Set(["and", "or", "not", "in", "is", "None", "True", "False"]);
const OPS = ["==", "!=", "<=", ">=", "//", "**", "<", ">", "+", "-", "*", "/", "%", "(", ")", "[", "]", ",", "."];

export class QueryError extends Error {}

function tokenize(src: string): Tok[] {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) {
      i++;
    } else if (c === "'" || c === '"') {
      let j = i + 1;
      let s = "";
      while (j < src.length && src[j] !== c) {
        if (src[j] === "\\" && j + 1 < src.length) {
          s += src[j + 1];
          j += 2;
        } else s += src[j++];
      }
      if (j >= src.length) throw new QueryError("A text value is missing its closing quote");
      out.push({ t: "str", v: s });
      i = j + 1;
    } else if (c === "!") {
      // ArcGIS field delimiters: !FIELD!  (but not the != operator)
      if (src[i + 1] === "=") {
        out.push({ t: "op", v: "!=" });
        i += 2;
        continue;
      }
      const j = src.indexOf("!", i + 1);
      if (j < 0) throw new QueryError("A !field! name is missing its closing !");
      out.push({ t: "id", v: src.slice(i + 1, j) });
      i = j + 1;
    } else if (/[0-9]/.test(c) || (c === "." && /[0-9]/.test(src[i + 1] ?? ""))) {
      const m = src.slice(i).match(/^\d*\.?\d+(?:[eE][+-]?\d+)?/)!;
      out.push({ t: "num", v: m[0] });
      i += m[0].length;
    } else if (/[A-Za-z_]/.test(c)) {
      const m = src.slice(i).match(/^[A-Za-z_][A-Za-z0-9_]*/)!;
      out.push({ t: KEYWORDS.has(m[0]) ? "kw" : "id", v: m[0] });
      i += m[0].length;
    } else {
      const op = OPS.find((o) => src.startsWith(o, i));
      if (!op) throw new QueryError(`Unexpected character "${c}"`);
      out.push({ t: "op", v: op });
      i += op.length;
    }
  }
  return out;
}

const sqlStr = (s: string) => `'${s.replace(/'/g, "''")}'`;

function sqlIdent(name: string, fields?: string[]) {
  const actual = fields?.find((f) => f === name) ?? fields?.find((f) => f.toLowerCase() === name.toLowerCase()) ?? name;
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(actual) ? actual : `"${actual.replace(/"/g, '""')}"`;
}

/** SQL for one token (operators and keywords mapped, values quoted). */
function piece(tok: Tok, fields?: string[]): string {
  switch (tok.t) {
    case "str":
      return sqlStr(tok.v);
    case "num":
      return tok.v;
    case "id":
      return sqlIdent(tok.v, fields);
    case "kw":
      return { and: "AND", or: "OR", not: "NOT", in: "IN", is: "IS", None: "NULL", True: "TRUE", False: "FALSE" }[tok.v]!;
    case "op":
      return { "==": "=", "!=": "<>", "**": "^", "[": "(", "]": ")" }[tok.v] ?? tok.v;
  }
}

const METHODS: Record<string, (target: string, args: string[]) => string> = {
  startswith: (t, a) => `${t} LIKE ${likeArg(a[0], "", "%")}`,
  endswith: (t, a) => `${t} LIKE ${likeArg(a[0], "%", "")}`,
  upper: (t) => `upper(${t})`,
  lower: (t) => `lower(${t})`,
  strip: (t) => `trim(${t})`,
};

const FUNCS: Record<string, string> = { len: "length", abs: "abs", round: "round", int: "trunc", float: "CAST_DOUBLE", str: "CAST_VARCHAR" };

function likeArg(sql: string, pre: string, post: string) {
  const m = sql.match(/^'(.*)'$/s);
  if (m) return sqlStr(pre + m[1].replace(/''/g, "'") + post);
  return `${pre ? "'%' || " : ""}${sql}${post ? " || '%'" : ""}`;
}

/** Convert a Python-style expression to SQL. Throws QueryError with a readable message. */
export function pyToSql(src: string, fields?: string[]): string {
  const toks = tokenize(src.trim());
  if (!toks.length) return "";
  let depth = 0;
  for (const t of toks) {
    if (t.v === "(" || t.v === "[") depth++;
    if (t.v === ")" || t.v === "]") depth--;
    if (depth < 0) throw new QueryError("There is a closing bracket without an opening one");
  }
  if (depth !== 0) throw new QueryError("A bracket is not closed");

  // Parse a call's arguments starting after "(" at index i; returns [args, index after ")"].
  const args = (i: number): [string[], number] => {
    const parts: string[] = [];
    let cur: Tok[] = [];
    let d = 0;
    for (; i < toks.length; i++) {
      const t = toks[i];
      if (t.t === "op" && (t.v === "(" || t.v === "[")) d++;
      if (t.t === "op" && (t.v === ")" || t.v === "]")) {
        if (d === 0) {
          if (cur.length) parts.push(convert(cur));
          return [parts, i + 1];
        }
        d--;
      }
      if (t.t === "op" && t.v === "," && d === 0) {
        parts.push(convert(cur));
        cur = [];
      } else cur.push(t);
    }
    throw new QueryError("A function call is not closed");
  };

  const convert = (ts: Tok[]): string => {
    const out: string[] = [];
    for (let i = 0; i < ts.length; i++) {
      const t = ts[i];
      const next = ts[i + 1];
      // <expr>.method(args) — applies to whatever came just before (so calls chain)
      if (t.t === "op" && t.v === "." && next?.t === "id" && ts[i + 2]?.v === "(" && out.length) {
        const fn = METHODS[next.v];
        if (!fn) throw new QueryError(`.${next.v}() isn't supported (try startswith, endswith, upper, lower, strip)`);
        const start = toks.indexOf(ts[i + 2]) + 1;
        const [a, end] = args(start);
        out.push(fn(out.pop()!, a));
        i += 2 + (end - start);
        continue;
      }
      // func(args)
      if (t.t === "id" && next?.v === "(" && FUNCS[t.v] !== undefined && !(fields ?? []).includes(t.v)) {
        const start = toks.indexOf(next) + 1;
        const [a, end] = args(start);
        const consumed = end - start;
        const f = FUNCS[t.v];
        out.push(f === "CAST_DOUBLE" ? `CAST(${a[0]} AS DOUBLE)` : f === "CAST_VARCHAR" ? `CAST(${a[0]} AS VARCHAR)` : `${f}(${a.join(", ")})`);
        i += 1 + consumed;
        continue;
      }
      // 'text' in FIELD  → FIELD LIKE '%text%'
      if (t.t === "str" && next?.t === "kw" && next.v === "in" && ts[i + 2]?.t === "id") {
        out.push(`${piece(ts[i + 2], fields)} LIKE ${sqlStr(`%${t.v}%`)}`);
        i += 2;
        continue;
      }
      // not in → NOT IN ; is not → IS NOT
      if (t.t === "kw" && t.v === "not" && next?.t === "kw" && next.v === "in") {
        out.push("NOT IN");
        i++;
        continue;
      }
      if (t.t === "kw" && t.v === "is" && next?.t === "kw" && next.v === "not") {
        out.push("IS NOT");
        i++;
        continue;
      }
      // x == None / x != None (common mistake) → IS NULL / IS NOT NULL
      if (t.t === "op" && (t.v === "==" || t.v === "!=") && next?.t === "kw" && next.v === "None") {
        out.push(t.v === "==" ? "IS NULL" : "IS NOT NULL");
        i++;
        continue;
      }
      out.push(piece(t, fields));
    }
    return out.join(" ").replace(/\( /g, "(").replace(/ \)/g, ")").replace(/ ,/g, ",").replace(/,\)/g, ")");
  };

  return convert(toks);
}
