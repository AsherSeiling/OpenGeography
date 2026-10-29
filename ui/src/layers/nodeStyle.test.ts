import { describe, expect, it } from "vitest";
import { styleFromSymbologyNode } from "./nodeStyle";

const base = { kind: "single" as const, fill: "#111111", stroke: "#222222", strokeWidth: 1, opacity: 1, radius: 3 };

describe("styleFromSymbologyNode", () => {
  it("sets colour, width and opacity; clamps and ignores bad values", () => {
    const s = styleFromSymbologyNode({ fill: "#a67c52", width: 2, opacity: 7, stroke: "red" }, base);
    expect(s).toMatchObject({ kind: "single", fill: "#a67c52", stroke: "#a67c52", strokeWidth: 2, opacity: 1 });
  });
  it("index colour makes a two-class style on INDEX", () => {
    const s = styleFromSymbologyNode({ fill: "#a67c52", index_color: "#5c3d1e" }, base);
    expect(s.kind).toBe("categorized");
    expect(s.field).toBe("INDEX");
    expect(s.classes?.map((c) => [c.value, c.color])).toEqual([["1", "#5c3d1e"], ["0", "#a67c52"]]);
  });
  it("labels need the switch and a field; index-only is carried", () => {
    expect(styleFromSymbologyNode({ label: true, label_field: "" }, { ...base, label: undefined }).label).toBeUndefined();
    const s = styleFromSymbologyNode({ label: true, label_field: "ELEV", label_index_only: true, label_size: 10 }, base);
    expect(s.label).toMatchObject({ enabled: true, field: "ELEV", size: 10, indexOnly: true });
    expect(styleFromSymbologyNode({ label: false, label_field: "ELEV" }, { ...base, label: s.label }).label).toBeUndefined();
  });
});
