import { describe, expect, it } from "vitest";
import { blocksFromScript, blocksScript, blocksToPython, starterBlocks, type BlockJson } from "./blocks";

const field = (n: string): BlockJson => ({ type: "gis_field", fields: { FIELD: n } });
const num = (n: number | string): BlockJson => ({ type: "gis_number", fields: { NUM: n } });
const ret = (b?: BlockJson): BlockJson => ({ type: "gis_return", inputs: { VALUE: b ? { block: b } : {} } });
const bin = (type: string, op: string, a?: BlockJson, b?: BlockJson): BlockJson => ({ type, fields: { OP: op }, inputs: { A: { block: a }, B: { block: b } } });

describe("blocks to Python", () => {
  it("makes the calculate function", () => {
    expect(blocksToPython(ret(bin("gis_compare", ">", field("POP"), num(100))))).toBe('def calculate(row):\n    return (row["POP"] > 100)\n');
  });
  it("brackets nested operators so precedence never matters", () => {
    const e = bin("gis_arith", "*", bin("gis_arith", "+", num(1), num(2)), num(3));
    expect(blocksToPython(ret(e))).toContain("((1 + 2) * 3)");
    const l = bin("gis_logic", "or", bin("gis_logic", "and", { type: "gis_bool", fields: { BOOL: "True" } }, { type: "gis_bool", fields: { BOOL: "False" } }), { type: "gis_none" });
    expect(blocksToPython(ret(l))).toContain("((True and False) or None)");
  });
  it("quotes field names and text safely", () => {
    const t = bin("gis_compare", "==", field('A "x"\\y'), { type: "gis_text", fields: { TEXT: 'it\'s "q"\n' } });
    const py = blocksToPython(ret(t));
    expect(py).toContain('row["A \\"x\\"\\\\y"]');
    expect(py).toContain('"it\'s \\"q\\"\\n"');
  });
  it("is empty / is not empty, one of, if, functions", () => {
    expect(blocksToPython(ret({ type: "gis_isnone", fields: { OP: "is not" }, inputs: { A: { block: field("N") } } }))).toContain('(row["N"] is not None)');
    expect(blocksToPython(ret({ type: "gis_in", fields: { LIST: "TX, 12, a b,", NOT: true }, inputs: { A: { block: field("S") } } }))).toContain('(row["S"] not in ["TX", 12, "a b"])');
    const iff: BlockJson = { type: "gis_if", inputs: { IF: { block: { type: "gis_bool", fields: { BOOL: "True" } } }, THEN: { block: num(1) }, ELSE: { block: num(2.5) } } };
    expect(blocksToPython(ret(iff))).toContain("(1 if True else 2.5)");
    expect(blocksToPython(ret({ type: "gis_func", fields: { FUNC: "upper" }, inputs: { A: { block: field("S") } } }))).toContain('row["S"].upper()');
  });
  it("explains every way a program can be unfinished", () => {
    expect(() => blocksToPython(null)).toThrow("return block");
    expect(() => blocksToPython(ret())).toThrow("empty socket");
    expect(() => blocksToPython(ret(bin("gis_compare", "==", field("A"))))).toThrow("empty socket");
    expect(() => blocksToPython(ret(field("")))).toThrow("needs a field");
    expect(() => blocksToPython(ret(num("abc")))).toThrow("needs a number");
    expect(() => blocksToPython(ret(num("")))).toThrow("needs a number");
    expect(() => blocksToPython(ret(bin("gis_compare", "<>", num(1), num(2))))).toThrow("unknown choice");
    expect(() => blocksToPython(ret({ type: "mystery" }))).toThrow("Unknown block");
    expect(() => blocksToPython(ret({ type: "gis_in", fields: { LIST: " , " }, inputs: { A: { block: num(1) } } }))).toThrow("at least one value");
  });
  it("never writes anything but a plain number for numbers", () => {
    expect(blocksToPython(ret(num("1e3")))).toContain("return 1000");
    expect(blocksToPython(ret(num("1e21")))).toContain("return 1e21");
    expect(blocksToPython(ret(num("-0")))).toContain("return 0\n");
    expect(() => blocksToPython(ret(num("Infinity")))).toThrow();
  });
});

describe("blocks inside a script", () => {
  const prog = ret(bin("gis_compare", "==", field("NAME"), { type: "gis_text", fields: { TEXT: "Zürich ✓" } }));
  it("round-trips, including non-ASCII", () => {
    const s = blocksScript(prog);
    expect(s.split("\n")[0]).toBe("def calculate(row):");
    expect(s).toContain("# gis-blocks: ");
    expect(blocksFromScript(s)).toEqual(prog);
    expect(blocksFromScript(s.replace(/\n/g, "\r\n"))).toEqual(prog);
  });
  it("opens as text once the text has been edited", () => {
    expect(blocksFromScript(blocksScript(prog).replace("==", "!="))).toBeNull();
    expect(blocksFromScript(blocksScript(prog).replace("def calculate(row):", "# hi\ndef calculate(row):"))).toBeNull();
  });
  it("ignores plain scripts and damaged markers", () => {
    expect(blocksFromScript('def calculate(row):\n    return True\n')).toBeNull();
    expect(blocksFromScript("def calculate(row):\n    return True\n# gis-blocks: !!!notbase64")).toBeNull();
    expect(blocksFromScript("def calculate(row):\n    return True\n# gis-blocks: e30=")).toBeNull();
  });
  it("starter programs generate", () => {
    expect(blocksToPython(starterBlocks("filter", "POP", true))).toContain('row["POP"] > 0');
    expect(blocksToPython(starterBlocks("filter", "NAME", false))).toContain('row["NAME"] == "value"');
    expect(blocksToPython(starterBlocks("value", "NAME", false))).toBe('def calculate(row):\n    return row["NAME"]\n');
  });
});
