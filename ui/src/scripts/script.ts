// Python scripts (UR-20/21): the shape of a script, its templates, and the approval state shown in the interface. The engine owns the
// rules (strict return values, failed-row reports, the approval record); this file only builds text and remembers what the engine said.
import { create } from "zustand";
import { api, type Field } from "../ipc/api";

/** The start of the engine's error while a script waits for approval. */
export const WAITING = "Script waiting for approval";
export const isWaiting = (msg?: string | null) => !!msg && msg.startsWith(WAITING);

/** `row["POP"]` for a field name (JSON quoting is valid Python for any field name). */
export const fieldRef = (name: string) => `row[${JSON.stringify(name)}]`;

/** A first script for a Definition Query / Select by Attributes: it returns True or False for each feature. */
export function filterTemplate(fields: Field[]): string {
  const f = fields.find((x) => !/^(OGC_FID|OBJECTID|OID|FID|fid|id)$/.test(x.name)) ?? fields[0];
  const numeric = /INT|DOUBLE|FLOAT|DECIMAL|REAL/i.test(f?.type ?? "");
  return `def calculate(row):\n    return ${f ? fieldRef(f.name) : 'row["FIELD"]'} ${numeric ? "> 0" : '== "value"'}\n`;
}

/** A first script for the field calculator: it returns the value of the field for each feature. */
export function valueTemplate(fields: Field[]): string {
  const f = fields[0];
  return `def calculate(row):\n    return ${f ? fieldRef(f.name) : 'row["FIELD"]'}\n`;
}

/** What the user may know about a script: approved on this computer or not (null = not asked yet). */
interface TrustState {
  approved: Record<string, boolean>;
  warning: string;
  /** Ask the engine (cached per script text). */
  check: (script: string) => Promise<boolean>;
  forget: () => void;
  /** The approval dialog: resolves true when the user approved, false when they refused. */
  request: { script: string; resolve: (ok: boolean) => void } | null;
  ask: (script: string) => Promise<boolean>;
  close: (ok: boolean) => void;
}

export const useScripts = create<TrustState>((set, get) => ({
  approved: {},
  warning: "",
  request: null,
  check: async (script) => {
    if (script in get().approved) return get().approved[script];
    const t = await api.scriptTrustInfo(script);
    set((s) => ({ approved: { ...s.approved, [script]: t.approved }, warning: t.warning }));
    return t.approved;
  },
  forget: () => set({ approved: {} }),
  ask: (script) =>
    new Promise<boolean>((resolve) => {
      get().request?.resolve(false);
      set({ request: { script, resolve } });
    }),
  close: (ok) => {
    get().request?.resolve(ok);
    set({ request: null });
  },
}));

/** Approve a script the user just wrote in this program (they are the author: no question, but the record is kept on this computer). */
export async function approveAuthored(script: string): Promise<void> {
  await api.approveScript(script);
  useScripts.setState((s) => ({ approved: { ...s.approved, [script]: true } }));
}

/** Show the approval dialog for a script that came from elsewhere (a project, a model). True when approved. */
export async function reviewScript(script: string): Promise<boolean> {
  const ok = await useScripts.getState().ask(script);
  if (ok) useScripts.setState((s) => ({ approved: { ...s.approved, [script]: true } }));
  return ok;
}
