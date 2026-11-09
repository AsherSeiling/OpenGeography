// UR-22: the block editor. Blockly (Apache-2.0, google/blockly, loaded only when a block editor opens) edits one `return` program; the Python it
// makes is shown beneath and is what runs. The generator is `blocks.ts`, which has the tests; this file only draws the workspace.
import { useEffect, useRef, useState } from "react";
import type { Field } from "../ipc/api";
import { blocksToPython, starterBlocks, type BlockJson } from "./blocks";

let defined = false;
let fieldNames: () => string[] = () => [];

function define(Blockly: typeof import("blockly/core")) {
  if (defined) return;
  defined = true;
  const dd = (name: string, opts: [string, string][]) => ({ type: "field_dropdown", name, options: opts });
  const fieldDropdown = () => {
    const n = fieldNames();
    return (n.length ? n.map((x) => [x, x]) : [["(no fields)", ""]]) as [string, string][];
  };
  Blockly.common.defineBlocksWithJsonArray([
    { type: "gis_return", message0: "return %1", args0: [{ type: "input_value", name: "VALUE" }], colour: 20, tooltip: "The answer for each feature." },
    { type: "gis_field", message0: "field %1", args0: [{ type: "field_dropdown", name: "FIELD", options: fieldDropdown }], output: null, colour: 210, tooltip: "The value of a field for this feature (empty if it has none)." },
    { type: "gis_number", message0: "%1", args0: [{ type: "field_input", name: "NUM", text: "0" }], output: null, colour: 120, tooltip: "A number." },
    { type: "gis_text", message0: "“%1”", args0: [{ type: "field_input", name: "TEXT", text: "" }], output: null, colour: 30, tooltip: "Text." },
    { type: "gis_bool", message0: "%1", args0: [dd("BOOL", [["True", "True"], ["False", "False"]])], output: null, colour: 260 },
    { type: "gis_none", message0: "empty (None)", output: null, colour: 260 },
    { type: "gis_compare", message0: "%1 %2 %3", args0: [{ type: "input_value", name: "A" }, dd("OP", [["=", "=="], ["≠", "!="], ["<", "<"], ["≤", "<="], [">", ">"], ["≥", ">="]]), { type: "input_value", name: "B" }], inputsInline: true, output: null, colour: 260 },
    { type: "gis_logic", message0: "%1 %2 %3", args0: [{ type: "input_value", name: "A" }, dd("OP", [["and", "and"], ["or", "or"]]), { type: "input_value", name: "B" }], inputsInline: true, output: null, colour: 260 },
    { type: "gis_not", message0: "not %1", args0: [{ type: "input_value", name: "A" }], output: null, colour: 260 },
    { type: "gis_isnone", message0: "%1 %2 empty", args0: [{ type: "input_value", name: "A" }, dd("OP", [["is", "is"], ["is not", "is not"]])], inputsInline: true, output: null, colour: 260, tooltip: "True when the feature has no value for the field." },
    { type: "gis_arith", message0: "%1 %2 %3", args0: [{ type: "input_value", name: "A" }, dd("OP", [["+", "+"], ["−", "-"], ["×", "*"], ["÷", "/"], ["remainder", "%"]]), { type: "input_value", name: "B" }], inputsInline: true, output: null, colour: 120 },
    { type: "gis_if", message0: "if %1 then %2 else %3", args0: [{ type: "input_value", name: "IF" }, { type: "input_value", name: "THEN" }, { type: "input_value", name: "ELSE" }], inputsInline: true, output: null, colour: 260 },
    { type: "gis_in", message0: "%1 is %2 one of %3", args0: [{ type: "input_value", name: "A" }, { type: "field_checkbox", name: "NOT", checked: false }, { type: "field_input", name: "LIST", text: "a, b" }], inputsInline: true, output: null, colour: 260, tooltip: "Tick the box for 'is not one of'. Values are separated by commas." },
    { type: "gis_func", message0: "%1 of %2", args0: [dd("FUNC", [["UPPER CASE", "upper"], ["lower case", "lower"], ["trim spaces", "strip"], ["length", "length"], ["absolute value", "abs"], ["round", "round"], ["as text", "text"]]), { type: "input_value", name: "A" }], inputsInline: true, output: null, colour: 30 },
  ]);
}

const TOOLBOX = {
  kind: "categoryToolbox",
  contents: [
    { kind: "category", name: "Values", colour: "210", contents: ["gis_field", "gis_number", "gis_text", "gis_bool", "gis_none"].map((type) => ({ kind: "block", type })) },
    { kind: "category", name: "Decide", colour: "260", contents: ["gis_compare", "gis_logic", "gis_not", "gis_isnone", "gis_in", "gis_if"].map((type) => ({ kind: "block", type })) },
    { kind: "category", name: "Calculate", colour: "120", contents: ["gis_arith", "gis_func"].map((type) => ({ kind: "block", type })) },
  ],
};

export function BlocksEditor({
  fields,
  kind,
  initial,
  onChange,
}: {
  fields: Field[];
  kind: "filter" | "value";
  /** The blocks to open (from a script that carries them); a starter program when absent. */
  initial?: BlockJson | null;
  /** The program's Python, or why there is none yet. `root` is the block program (to keep with the script). */
  onChange: (root: BlockJson | null, error: string | null) => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [python, setPython] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const cb = useRef(onChange);
  cb.current = onChange;
  fieldNames = () => fields.map((f) => f.name);

  useEffect(() => {
    let dispose = () => {};
    let gone = false;
    (async () => {
      const Blockly = await import("blockly/core");
      const En = await import("blockly/msg/en");
      if (gone || !host.current) return;
      Blockly.setLocale(En as unknown as Record<string, string>);
      define(Blockly);
      const ws = Blockly.inject(host.current, { toolbox: TOOLBOX, trashcan: true, scrollbars: true, sounds: false, zoom: { controls: true, wheel: true, startScale: 0.9 }, move: { scrollbars: true, drag: true, wheel: false } });
      const first = fields.find((x) => !/^(OGC_FID|OBJECTID|OID|FID|fid|id)$/.test(x.name)) ?? fields[0];
      const numeric = /INT|DOUBLE|FLOAT|DECIMAL|REAL/i.test(first?.type ?? "");
      const root = initial ?? starterBlocks(kind, first?.name, numeric);
      Blockly.serialization.blocks.append(root as never, ws);
      const publish = () => {
        const tops = ws.getTopBlocks(true).filter((b) => b.type === "gis_return");
        let r: BlockJson | null = null;
        let msg: string | null = null;
        try {
          if (tops.length !== 1) throw new Error(tops.length ? "Use only one return block." : "Start with a return block.");
          r = Blockly.serialization.blocks.save(tops[0]) as BlockJson;
          setPython(blocksToPython(r));
        } catch (e) {
          msg = (e as Error).message;
          setPython("");
          r = null;
        }
        setError(msg);
        cb.current(msg ? null : r, msg);
      };
      ws.addChangeListener((e: { isUiEvent?: boolean }) => {
        if (!e.isUiEvent) publish();
      });
      publish();
      setLoading(false);
      dispose = () => ws.dispose();
    })();
    return () => {
      gone = true;
      dispose();
    };
  }, []);

  return (
    <div className="stack" data-testid="blocks-editor">
      <div ref={host} className="blocks-host" style={{ height: 260, border: "1px solid var(--rule)", borderRadius: 4 }} />
      {loading && <div className="muted small">Loading the block editor…</div>}
      <div className={`qe-preview ${error ? "err" : ""}`} data-testid="blocks-python">
        {error ?? <pre className="mono script-view" style={{ margin: 0 }}>{python}</pre>}
      </div>
      <div className="muted small">
        {kind === "filter" ? "Return a true/false answer: true keeps a feature." : "Return the value to store for each feature."} The Python above is what runs; a block script opens as text elsewhere, and returns to blocks only while its text is unchanged.
      </div>
    </div>
  );
}
