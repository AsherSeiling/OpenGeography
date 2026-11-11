// Build a filter four ways: a point-and-click Builder (the default, like ArcGIS Pro's clause builder: rows of field, operator and value
// joined by And/Or, values picked from the field's unique values), a Python function (run by CPython), Blocks, or SQL. Reports the text to
// run (SQL, or a Python script) with its language, or an error, via onChange. A saved Python query opens in Python and a saved SQL query
// opens in the Builder only when the rows rebuild exactly the same SQL; otherwise it opens in SQL, unchanged.
import { BlocksEditor } from "../scripts/BlocksEditor";
import { blocksFromScript, blocksScript, type BlockJson } from "../scripts/blocks";
import { Button, Tab, TabList, Textarea } from "@fluentui/react-components";
import { AddRegular, DeleteRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, type Field } from "../ipc/api";
import { builderToSql, needsValue, opsFor, type Clause } from "./builder";
import { sqlToClauses } from "./builderParse";
import { fieldRef, filterTemplate } from "../scripts/script";
import { pySubsetSql } from "./pySubset";

export type QueryMode = "builder" | "python" | "blocks" | "sql";
export type QueryLanguage = "sql" | "python";

/** Fields with more distinct values than this get a type-ahead box instead of a full list. */
const MAX_VALUES = 1000;

const newClause = (field = ""): Clause => ({ join: "AND", field, op: "eq", value: "", value2: "" });

/** Value picker: every value of the field as a dropdown (=, ≠), a checklist
 * ("is one of"), or a type-ahead box for fields with very many values. */
function ValueInput({ clause: c, values, listId, onChange }: { clause: Clause; values?: string[]; listId: string; onChange: (p: Partial<Clause>) => void }) {
  const [typed, setTyped] = useState(false);
  const complete = values && values.length < MAX_VALUES;
  if (complete && !typed && (c.op === "eq" || c.op === "ne")) {
    return (
      <select className="qe-ctl qe-val" value={values!.includes(c.value) ? c.value : ""} onChange={(e) => (e.target.value === "\u0000" ? setTyped(true) : onChange({ value: e.target.value }))}>
        <option value="" disabled>Choose a value… ({values!.length})</option>
        {values!.map((v) => (
          <option key={v} value={v}>{v === "" ? "(empty text)" : v}</option>
        ))}
        <option value={"\u0000"}>Type another value…</option>
      </select>
    );
  }
  if (complete && !typed && c.op === "in") {
    const chosen = new Set(c.value.split(",").map((v) => v.trim()).filter(Boolean));
    const toggle = (v: string) => {
      chosen.has(v) ? chosen.delete(v) : chosen.add(v);
      onChange({ value: [...chosen].join(", ") });
    };
    return (
      <div className="qe-checklist">
        {values!.map((v) => (
          <label key={v}>
            <input type="checkbox" checked={chosen.has(v)} onChange={() => toggle(v)} /> {v === "" ? "(empty)" : v}
          </label>
        ))}
      </div>
    );
  }
  return (
    <>
      <input className="qe-ctl qe-val" list={listId} value={c.value} placeholder={c.op === "in" ? "a, b, c" : "Value"} onChange={(e) => onChange({ value: e.target.value })} />
      <datalist id={listId}>
        {(values ?? []).slice(0, 500).map((v) => (
          <option key={v} value={v} />
        ))}
      </datalist>
      {c.op === "between" && (
        <>
          <span className="muted">and</span>
          <input className="qe-ctl qe-val" list={listId} value={c.value2} onChange={(e) => onChange({ value2: e.target.value })} />
        </>
      )}
    </>
  );
}

export function QueryEditor({
  layerId,
  fields,
  initialSql,
  initialPython,
  onChange,
}: {
  layerId: string;
  fields: Field[];
  /** An existing SQL query: opens in the SQL tab, unchanged. */
  initialSql?: string | null;
  /** An existing Python script: opens in the Python tab. */
  initialPython?: string | null;
  /** `text` is SQL or a Python script, by `language`. */
  onChange: (text: string, error: string | null, language: QueryLanguage) => void;
}) {
  const firstField = (fields.find((f) => !/^(OGC_FID|OBJECTID|OID|FID|fid|id)$/.test(f.name)) ?? fields[0])?.name;
  // A saved SQL query opens as rows only when they rebuild exactly the same text, so opening never changes it.
  const initialRows = useMemo(() => {
    if (initialPython || !initialSql?.trim()) return null;
    const p = sqlToClauses(initialSql, fields);
    return p.ok && builderToSql(p.clauses, fields) === initialSql.trim() ? p.clauses : null;
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  const [mode, setMode] = useState<QueryMode>(initialPython ? "python" : initialSql?.trim() ? (initialRows ? "builder" : "sql") : "builder");
  const [clauses, setClauses] = useState<Clause[]>(initialRows ?? [newClause(firstField)]);
  /** Why the last switch to the Builder did not happen (SQL or Python it can't show as rows). */
  const [note, setNote] = useState<string | null>(null);
  const [python, setPython] = useState(initialPython ?? "");
  // Hand-written Python that was open when the Blocks tab was chosen: kept unless the blocks are actually edited.
  const kept = useRef<{ python: string; first: string | null } | null>(null);
  const [blocks, setBlocks] = useState<{ root: BlockJson | null; error: string | null }>({ root: null, error: null });
  const [sql, setSql] = useState(initialSql ?? "");
  const [values, setValues] = useState<Record<string, string[]>>({});

  // Current SQL (or error) for the active mode.
  const result = useMemo((): [string, string | null] => {
    try {
      if (mode === "builder") return [builderToSql(clauses, fields), null];
      if (mode === "blocks") return blocks.error || !blocks.root ? ["", blocks.error ?? "Start with a return block."] : [blocksScript(blocks.root), null];
      if (mode === "python") {
        if (!python.trim()) return ["", null];
        return /def\s+calculate\s*\(\s*row\s*\)/.test(python) ? [python, null] : ["", "Write a function:  def calculate(row):  that returns True to keep a feature, False to drop it."];
      }
      return [sql.trim(), null];
    } catch (e) {
      return ["", (e as Error).message];
    }
  }, [mode, clauses, python, sql, fields, blocks]);
  useEffect(() => onChange(result[0], result[1], mode === "python" || mode === "blocks" ? "python" : "sql"), [result]);
  // A short filter can also be answered by SQL with exactly the same rows (nulls and all): say so. Everything else runs in Python.
  const simple = useMemo(() => (mode === "python" && python.trim() ? pySubsetSql(python, fields.map((f) => ({ name: f.name, type: f.type }))) : null), [mode, python, fields]);
  useEffect(() => {
    if (mode === "python" && !python) setPython(filterTemplate(fields));
  }, [mode]);

  // Values for the builder's value pickers, fetched per field on demand.
  const loadValues = async (field: string) => {
    if (!field || !layerId || values[field]) return;
    try {
      const v = await api.uniqueValues(layerId, field, MAX_VALUES);
      const sorted = v.map(([x]) => x).sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
      setValues((m) => ({ ...m, [field]: sorted }));
    } catch {
      /* value suggestions are optional */
    }
  };
  useEffect(() => {
    clauses.forEach((c) => loadValues(c.field));
  }, [clauses.map((c) => c.field).join("|")]);

  const update = (i: number, p: Partial<Clause>) => setClauses((cs) => cs.map((c, j) => (j === i ? { ...c, ...p } : c)));

  const switchTo = (m: QueryMode) => {
    setNote(null);
    if (m === "builder" && mode !== "builder") {
      // Carry the expression into rows where it can be shown; otherwise stay where it is and say why.
      const text = mode === "sql" ? sql : mode === "python" ? (simple ?? "") : mode === "blocks" ? "" : "";
      if (text.trim()) {
        const p = sqlToClauses(text, fields);
        if (!p.ok) {
          setNote(p.reason);
          return;
        }
        setClauses(p.clauses);
      } else if (mode === "python" && python.trim() && python.trim() !== filterTemplate(fields).trim()) {
        setNote("This Python filter can't be shown as rows: it is more than a simple comparison. It stays as Python.");
        return;
      } else if (mode === "blocks") {
        setNote("Blocks can't be shown as rows. Use the Python tab to see what they make.");
        return;
      }
    }
    // Carry the expression forward where it can be represented (a Python filter that is plain SQL carries over as that SQL).
    if (m === "sql" && mode === "builder" && result[0]) setSql(result[0]);
    if (m === "sql" && mode === "python" && simple) setSql(simple);
    // Blocks carry into the Python tab as the script they made; text opens as blocks only while it still equals what its blocks generate.
    if (m === "python" && mode === "blocks" && blocks.root && !blocks.error) {
      // What the Blocks tab shows is what runs, so the Python tab shows it too.
      setPython(blocksScript(blocks.root));
      kept.current = null;
    }
    if (m === "blocks") {
      const root = blocksFromScript(python);
      kept.current = root || !python.trim() || python === filterTemplate(fields) ? null : { python, first: null };
      setBlocks({ root, error: null });
    }
    setMode(m);
  };

  return (
    <div className="qe">
      <TabList size="small" selectedValue={mode} onTabSelect={(_, d) => switchTo(d.value as QueryMode)}>
        <Tab value="builder">Builder</Tab>
        <Tab value="sql">SQL</Tab>
        <Tab value="python">Python</Tab>
        <Tab value="blocks">Blocks</Tab>
      </TabList>
      {note && <div className="qe-preview err" role="status" data-testid="qe-note">{note}</div>}

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
                  <ValueInput
                    clause={c}
                    values={values[c.field]}
                    listId={listId}
                    onChange={(p) => update(i, p)}
                  />
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
          <Textarea className="mono" rows={6} value={python} onChange={(_, d) => setPython(d.value)} aria-label="Python function" data-testid="qe-python" />
          <div className="qe-fields">
            {fields.map((x) => (
              <button key={x.name} className="chip" title={`Insert ${fieldRef(x.name)}`} onClick={() => setPython((p) => `${p.replace(/\s+$/, "")} ${fieldRef(x.name)}`.replace(/\n    return \s+/, "\n    return "))}>
                {x.name}
              </button>
            ))}
          </div>
          <div className="muted small">
            Return <code>True</code> to keep a feature, <code>False</code> to drop it — nothing else (not 1, not None). A field with no value is <code>None</code>. Attributes only, no geometry. The script runs in Python on this computer, once, not every time the map draws.
          </div>
        </div>
      )}

      {mode === "blocks" && (
        <>
          {kept.current && <div className="muted small" data-testid="blocks-replaces">The Python you wrote is not made of blocks, so it is not shown here. Going back to the Python tab shows the blocks' script instead.</div>}
          <BlocksEditor fields={fields} kind="filter" initial={blocks.root} onChange={(root, error) => {
            if (kept.current && kept.current.first === null && root) kept.current.first = JSON.stringify(root);
            setBlocks({ root, error });
          }} />
        </>
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

      {mode === "python" && (
        <div className={`qe-preview ${result[1] ? "err" : ""}`} data-testid="qe-preview">
          {result[1] ? result[1] : simple ? <>A simple filter: also valid as SQL — <code>{simple}</code></> : <span className="muted">Runs with Python.</span>}
        </div>
      )}
      {mode === "builder" && (
        <div className={`qe-preview ${result[1] ? "err" : ""}`}>
          {result[1] ? result[1] : result[0] ? <>SQL: <code>{result[0]}</code></> : <span className="muted">Build an expression above.</span>}
        </div>
      )}
    </div>
  );
}

