import { describe, expect, it } from "vitest";
import { addAllValues, addValueClass, addValueToClass, classValues, groupClasses, removeClass, ungroupClass } from "./uniqueClasses";
import { buildLayers } from "../map/style";
import { classRanks } from "../map/categoryOrder";
import type { LayerStyle } from "../store/store";

const style = (over: Partial<LayerStyle>): LayerStyle => ({ kind: "categorized", fill: "#999999", stroke: "#4d4d4d", strokeWidth: 1, opacity: 1, radius: 3, field: "t", ...over });

describe("UR-12 hand-made unique-value classes", () => {
  it("adds a class by value, refuses a value that has one, and adds all values only where none exists", () => {
    let r = addValueClass([], "oak", 12);
    expect("classes" in r && r.classes).toEqual([{ value: "oak", color: expect.any(String), label: "oak (12)" }]);
    const cls = (r as { classes: never[] }).classes;
    expect(addValueClass(cls, "oak")).toEqual({ error: '"oak" already has a class.' });
    const all = addAllValues(cls, [["oak", 12], ["pine", 4], ["", 1]]);
    expect(all.map((c) => c.value)).toEqual(["oak", "pine", ""]);
    expect(all[2].label).toBe("<empty> (1)");
    expect(new Set(all.map((c) => c.color)).size).toBe(3); // distinct colours
  });

  it("groups several classes into one that keeps the first colour and lists every value; ungroup and remove undo it", () => {
    const base = addAllValues([], [["a", 1], ["b", 1], ["c", 1], ["d", 1]]);
    const g = groupClasses(base, [1, 3]);
    expect(g.map((c) => classValues(c))).toEqual([["a"], ["b", "d"], ["c"]]);
    expect(g[1].color).toBe(base[1].color);
    expect(g[1].label).toBe("b, d");
    expect(groupClasses(base, [2], "x")).toBe(base); // one class is not a group
    expect(groupClasses(base, [0, 1], "Few")[0].label).toBe("Few");
    const back = ungroupClass(g, 1);
    expect(back.map((c) => c.value)).toEqual(["a", "b", "d", "c"]);
    expect(back[1].color).toBe(base[1].color);
    expect(removeClass(back, 0).map((c) => c.value)).toEqual(["b", "d", "c"]);
    const added = addValueToClass(base, 0, "z");
    expect("classes" in added && classValues(added.classes[0])).toEqual(["a", "z"]);
    expect(addValueToClass(base, 0, "b")).toHaveProperty("error");
  });

  it("the map draws every value of a group in the group's colour, the rest in the layer fill, and can leave the rest undrawn", () => {
    const classes = [{ value: "a", values: ["b"], color: "#112233", label: "A+B" }, { value: "c", color: "#445566", label: "C" }];
    const fill = buildLayers("L", "Polygon", style({ classes })).find((l) => l.id === "L-fill")!;
    const expr = (fill.paint as Record<string, unknown>)["fill-color"] as unknown[];
    const match = (expr as unknown[]).flat(2);
    expect(JSON.stringify(match)).toContain('"match"');
    const m = JSON.stringify(expr);
    expect(m).toContain('"a","#112233","b","#112233","c","#445566"');
    expect(m.endsWith('"#999999"]]') || m.includes('"#999999"')).toBe(true);
    const hidden = buildLayers("L", "Polygon", style({ classes, otherHidden: true })).find((l) => l.id === "L-fill")!;
    expect(JSON.stringify((hidden as { filter?: unknown }).filter)).toContain('"literal",["a","b","c"]');
    // Without the switch the fill layer is filtered by geometry only.
    expect(JSON.stringify((fill as { filter?: unknown }).filter)).not.toContain("literal");
  });

  it("draw order covers every value of a group", () => {
    const r = classRanks([{ value: "a", values: ["b"], color: "#000", label: "" }, { value: "c", color: "#000", label: "" }]);
    expect(r.get("a")).toBe(2);
    expect(r.get("b")).toBe(2);
    expect(r.get("c")).toBe(1);
  });
});
