// Build a filter three ways: point-and-click Builder, Python-style
// expression, or SQL. Reports the SQL to run (or an error) via onChange.
import { Button, Tab, TabList, Textarea } from "@fluentui/react-components";
import { AddRegular, DeleteRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { api, type Field } from "../ipc/api";
import { builderToSql, needsValue, opsFor, type Clause } from "./builder";
import { pyToSql } from "./pyToSql";

export type QueryMode = "builder" | "python" | "sql";

const newClause = (field = ""): Clause => ({ join: "AND", field, op: "eq", value: "", value2: "" });

export function QueryEditor({
  layerId,
  fields,
  initialSql,
  onChange,
}: {
  layerId: string;
  fields: Field[];
  initialSql?: string | null;
  onChange: (sql: string, error: string | null) => void;
}) {
  const [mode, setMode] = useState<QueryMode>(initialSql ? "sql" : "builder");
  const firstField = (fields.find((f) => !/^(OGC_FID|OBJECTID|OID|FID|fid|id)$/.test(f.name)) ?? fields[0])?.name;
  const [clauses, setClauses] = useState<Clause[]>([newClause(firstField)]);
  const [python, setPython] = useState("");
  const [sql, setSql] = useState(initialSql ?? "");
  const [values, setValues] = useState<Record<string, string[]>>({});
  const names = useMemo(() => fields.map((f) => f.name), [fields]);

  // Current SQL (or error) for the active mode.
  const result = useMemo((): [string, string | null] => {
    try {
      if (mode === "builder") return [builderToSql(clauses, fields), null];
      if (mode === "python") return [python.trim() ? pyToSql(python, names) : "", null];
      return [sql.trim(), null];
    } catch (e) {
      return ["", (e as Error).message];
    }
  }, [mode, clauses, python, sql, fields, names]);
  useEffect(() => onChange(result[0], result[1]), [result]);

  // Values for the builder's value pickers, fetched per field on demand.
  const loadValues = async (field: string) => {
    if (!field || values[field]) return;
    try {
      const v = await api.uniqueValues(layerId, field, 200);
      setValues((m) => ({ ...m, [field]: v.map(([x]) => x) }));
    } catch {
      /* value suggestions are optional */
    }
  };
  useEffect(() => {
    clauses.forEach((c) => loadValues(c.field));
  }, [clauses.map((c) => c.field).join("|")]);

  const update = (i: number, p: Partial<Clause>) => setClauses((cs) => cs.map((c, j) => (j === i ? { ...c, ...p } : c)));

  const switchTo = (m: QueryMode) => {
    // Carry the expression forward where it can be represented.
    if (m === "sql" && mode !== "sql" && result[0]) setSql(result[0]);
    setMode(m);
  };

  return (
    <div className="qe">
      <TabList size="small" selectedValue={mode} onTabSelect={(_, d) => switchTo(d.value as QueryMode)}>
        <Tab value="builder">Builder</Tab>
        <Tab value="python">Python</Tab>
        <Tab value="sql">SQL</Tab>
      </TabList>

      {mode === "builder" && (
        <div className="qe-rows">
          {clauses.map((c, i) => {
            const f = fields.find((x) => x.name === c.field);
            const ops = opsFor(f);
            const listId = `qe-vals-${layerId}-${i}`;
            return (
              <div key={i} className="qe-row">
                {i === 0 ? (
                  <span className="qe-where">Where</span>
                ) : (
                  <select className="qe-ctl qe-join" value={c.join} onChange={(e) => update(i, { join: e.target.value as Clause["join"] })}>
                    <option value="AND">And</option>
                    <option value="OR">Or</option>
                  </select>
                )}
                <select className="qe-ctl" value={c.field} onChange={(e) => update(i, { field: e.target.value, op: opsFor(fields.find((x) => x.name === e.target.value))[0][0], value: "", value2: "" })}>
                  <option value="">Field…</option>
                  {fields.map((x) => (
                    <option key={x.name} value={x.name}>{x.name}</option>
                  ))}
                </select>
                <select className="qe-ctl" value={c.op} onChange={(e) => update(i, { op: e.target.value as Clause["op"] })}>
                  {ops.map(([v, l]) => (
                    <option key={v} value={v}>{l}</option>
                  ))}
                </select>
                {needsValue(c.op) && (
                  <>
                    <input className="qe-ctl qe-val" list={listId} value={c.value} placeholder={c.op === "in" ? "a, b, c" : "Value"} onChange={(e) => update(i, { value: e.target.value })} />
                    <datalist id={listId}>
                      {(values[c.field] ?? []).map((v) => (
                        <option key={v} value={v} />
                      ))}
                    </datalist>
                    {c.op === "between" && (
                      <>
                        <span className="muted">and</span>
                        <input className="qe-ctl qe-val" list={listId} value={c.value2} onChange={(e) => update(i, { value2: e.target.value })} />
                      </>
                    )}
                  </>
                )}
                <button className="qe-del" title="Remove this row" disabled={clauses.length === 1} onClick={() => setClauses((cs) => cs.filter((_, j) => j !== i))}>
                  <DeleteRegular />
                </button>
              </div>
            );
          })}
          <Button size="small" icon={<AddRegular />} onClick={() => setClauses((cs) => [...cs, newClause(firstField)])}>
            Add clause
          </Button>
        </div>
      )}

      {mode === "python" && (
        <div className="stack">
          <Textarea className="mono" rows={4} value={python} onChange={(_, d) => setPython(d.value)} placeholder={"POP100 > 1000 and NAME.startswith('Okla')\nCLASS in ['State', 'County']\n!Area sq mi! >= 2 and NOTE is not None"} />
          <div className="qe-fields">
            {fields.map((x) => (
              <button key={x.name} className="chip" onClick={() => setPython((p) => `${p}${p && !p.endsWith(" ") ? " " : ""}${/^[A-Za-z_]\w*$/.test(x.name) ? x.name : `!${x.name}!`}`)}>
                {x.name}
              </button>
            ))}
          </div>
          <div className="muted small">Python operators: == != &lt; &gt; and or not in · is None · .startswith() .endswith() .upper() .lower() · len() · 'text' in FIELD</div>
        </div>
      )}

      {mode === "sql" && (
        <div className="stack">
          <Textarea className="mono" rows={4} value={sql} onChange={(_, d) => setSql(d.value)} placeholder="POP100 > 1000 AND COUNTY = '109'" />
          <div className="qe-fields">
            {fields.map((x) => (
              <button key={x.name} className="chip" onClick={() => setSql((p) => `${p}${p && !p.endsWith(" ") ? " " : ""}${/^[A-Za-z_]\w*$/.test(x.name) ? x.name : `"${x.name}"`}`)}>
                {x.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {mode !== "sql" && (
        <div className={`qe-preview ${result[1] ? "err" : ""}`}>
          {result[1] ? result[1] : result[0] ? <>SQL: <code>{result[0]}</code></> : <span className="muted">Build an expression above.</span>}
        </div>
      )}
    </div>
  );
}

