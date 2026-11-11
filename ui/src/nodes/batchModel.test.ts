import { describe, expect, it } from "vitest";
import type { BatchItem } from "../ipc/api";
import type { ModelNode } from "../store/model";
import { addFiles, addLayers, defaultResultNode, inputNodes, reportText, summarize } from "./batchModel";

const node = (id: string, tool: string, extra: Partial<ModelNode["data"]> = {}): ModelNode => ({ id, type: "tool", position: { x: 0, y: 0 }, data: { tool, params: {}, ...extra } });
const item = (name: string, status: BatchItem["status"], message: string | null = null): BatchItem => ({ name, source: name, status, message, rows: 3, output: null, layer: null, millis: 1 });

describe("batch model", () => {
  it("defaults to the step that feeds the Output, and lists the tree's own Layer Input first", () => {
    const nodes = [node("a", "input_layer"), node("in", "input_layer", { fixed: true }), node("b", "buffer"), node("o", "layer_output", { fixed: true })];
    const edges = [{ id: "1", source: "a", target: "b" }, { id: "2", source: "b", target: "o", targetHandle: "in" }];
    expect(defaultResultNode(nodes, edges)).toBe("b");
    expect(inputNodes(nodes).map((n) => n.id)).toEqual(["in", "a"]);
    expect(defaultResultNode([node("a", "input_layer")], [])).toBeNull();
  });
  it("never lists the same file or layer twice", () => {
    const l = addFiles(addFiles([], ["/d/a.gpkg", "/d/b.gpkg"]), ["/d/b.gpkg", "/d/c.gpkg"]);
    expect(l).toHaveLength(3);
    const m = addLayers(addLayers(l, ["L1"]), ["L1", "L2"]);
    expect(m).toHaveLength(5);
  });
  it("summarizes what happened to each input in words", () => {
    expect(summarize([item("a", "done"), item("b", "failed"), item("c", "skipped"), item("d", "cancelled")])).toBe("4 inputs: 1 succeeded, 1 failed, 1 left as they were (file already exists), 1 cancelled");
    expect(summarize([item("a", "done")])).toBe("1 input: 1 succeeded, 0 failed");
  });
  it("writes the report as tab separated text with one row per input", () => {
    const t = reportText([item("a", "done"), item("b", "failed", "Input has\nno CRS")]).split("\n");
    expect(t).toHaveLength(3);
    expect(t[2]).toBe("b\tfailed\t3\t\tInput has no CRS");
  });
});
