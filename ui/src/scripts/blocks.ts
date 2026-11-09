// UR-22: block programs. A block program is a tree of blocks (Blockly's own JSON shape) that makes the same `def calculate(row)` function
// the Python editors take. The generator here is pure (no Blockly, no DOM) so it is tested alone. A script made from blocks carries its
// blocks in a last comment line; text converts back to blocks only while the text still equals what those blocks generate.
import { fieldRef } from "./script";

export interface BlockJson {
  type: string;
  fields?: Record<string, string | number | boolean>;
  inputs?: Record<string, { block?: BlockJson }>;
}

export const MARKER = "# gis-blocks: ";

const COMPARE = ["==", "!=", "<", "<=", ">", ">="];
const ARITH = ["+", "-", "*", "/", "%"];
const LOGIC = ["and", "or"];
const FUNCS: Record<string, (a: string) => string> = {
  upper: (a) => `${a}.upper()`,
  lower: (a) => `${a}.lower()`,
  strip: (a) => `${a}.strip()`,
  length: (a) => `len(${a})`,
  abs: (a) => `abs(${a})`,
  round: (a) => `round(${a})`,
  text: (a) => `str(${a})`,
};

export const BLOCK_TYPES = ["gis_return", "gis_field", "gis_number", "gis_text", "gis_bool", "gis_none", "gis_compare", "gis_logic", "gis_not", "gis_isnone", "gis_arith", "gis_if", "gis_in", "gis_func"];

const pick = (v: unknown, allowed: string[], what: string): string => {
  if (typeof v === "string" && allowed.includes(v)) return v;
  throw new Error(`The ${what} block has an unknown choice.`);
};

/** A number literal, or an error for one Python would not read as a plain number. */
function numberLiteral(v: unknown): string {
  const n = typeof v === "number" ? v : Number(String(v ?? "").trim());
  if (!Number.isFinite(n) || String(v ?? "").trim() === "") throw new Error("A number block needs a number.");
  return Object.is(n, -0) ? "0" : String(n).includes("e") ? n.toExponential().replace("e+", "e") : String(n);
}

/** A Python expression for one block (nested operators always in brackets, so the meaning never depends on precedence). */
function expr(b: BlockJson | undefined, label: string): string {
  if (!b) throw new Error(`Fill the empty socket in the ${label} block.`);
  const input = (name: string, of: string) => expr(b.inputs?.[name]?.block, of);
  const f = b.fields ?? {};
  switch (b.type) {
    case "gis_field": {
      const name = String(f.FIELD ?? "");
      if (!name) throw new Error("A field block needs a field.");
      return fieldRef(name);
    }
    case "gis_number":
      return numberLiteral(f.NUM);
    case "gis_text":
      return JSON.stringify(String(f.TEXT ?? ""));
    case "gis_bool":
      return pick(f.BOOL, ["True", "False"], "True/False");
    case "gis_none":
      return "None";
    case "gis_compare":
      return `(${input("A", "compare")} ${pick(f.OP, COMPARE, "compare")} ${input("B", "compare")})`;
    case "gis_logic":
      return `(${input("A", "and/or")} ${pick(f.OP, LOGIC, "and/or")} ${input("B", "and/or")})`;
    case "gis_not":
      return `(not ${input("A", "not")})`;
    case "gis_isnone":
      return `(${input("A", "is empty")} ${pick(f.OP, ["is", "is not"], "is empty")} None)`;
    case "gis_arith":
      return `(${input("A", "arithmetic")} ${pick(f.OP, ARITH, "arithmetic")} ${input("B", "arithmetic")})`;
    case "gis_if":
      return `(${input("THEN", "if")} if ${input("IF", "if")} else ${input("ELSE", "if")})`;
    case "gis_in": {
      const items = String(f.LIST ?? "")
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s !== "")
        .map((s) => (/^-?\d+(\.\d+)?$/.test(s) ? s : JSON.stringify(s)));
      if (!items.length) throw new Error("The 'is one of' block needs at least one value, separated by commas.");
      return `(${input("A", "is one of")} ${f.NOT ? "not in" : "in"} [${items.join(", ")}])`;
    }
    case "gis_func": {
      const fn = FUNCS[String(f.FUNC)];
      if (!fn) throw new Error("The function block has an unknown choice.");
      return fn(input("A", "function"));
    }
    case "gis_return":
      throw new Error("A return block cannot go inside another block.");
    default:
      throw new Error(`Unknown block "${b.type}".`);
  }
}

/** The Python function for a block program (the `gis_return` block), without the blocks marker. */
export function blocksToPython(root: BlockJson | undefined | null): string {
  if (!root || root.type !== "gis_return") throw new Error("Start with a return block.");
  return `def calculate(row):\n    return ${expr(root.inputs?.VALUE?.block, "return")}\n`;
}

const toB64 = (s: string): string => {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  bytes.forEach((c) => (bin += String.fromCharCode(c)));
  return btoa(bin);
};
const fromB64 = (s: string): string => {
  const bin = atob(s);
  return new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
};

/** The script to save: the function plus the blocks it came from. */
export function blocksScript(root: BlockJson | undefined | null): string {
  return `${blocksToPython(root)}${MARKER}${toB64(JSON.stringify(root))}\n`;
}

const norm = (s: string) => s.replace(/\r\n/g, "\n").replace(/\s+$/, "");

/** The blocks behind a script, only when it carries them and the text has not been changed since (otherwise it opens as text). */
export function blocksFromScript(script: string): BlockJson | null {
  const lines = script.replace(/\r\n/g, "\n").replace(/\s+$/, "").split("\n");
  const last = lines[lines.length - 1] ?? "";
  if (!last.startsWith(MARKER)) return null;
  try {
    const root = JSON.parse(fromB64(last.slice(MARKER.length).trim())) as BlockJson;
    return norm(blocksToPython(root)) === norm(lines.slice(0, -1).join("\n")) ? root : null;
  } catch {
    return null;
  }
}

/** A starting program: filter (a comparison on the first field) or value (the first field). */
export function starterBlocks(kind: "filter" | "value", field: string | undefined, numeric: boolean): BlockJson {
  const fieldBlock: BlockJson = { type: "gis_field", fields: { FIELD: field ?? "" } };
  if (kind === "value") return { type: "gis_return", inputs: { VALUE: { block: fieldBlock } } };
  return {
    type: "gis_return",
    inputs: {
      VALUE: {
        block: {
          type: "gis_compare",
          fields: { OP: numeric ? ">" : "==" },
          inputs: { A: { block: fieldBlock }, B: { block: numeric ? { type: "gis_number", fields: { NUM: 0 } } : { type: "gis_text", fields: { TEXT: "value" } } } },
        },
      },
    },
  };
}
