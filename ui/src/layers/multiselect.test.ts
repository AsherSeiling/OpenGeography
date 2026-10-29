import { describe, expect, it } from "vitest";
import { applyStyleParts, moveSelected, nextSelection, safeFileName, unionExtent } from "./multiselect";

const order = ["a", "b", "c", "d", "e"];

describe("nextSelection", () => {
  it("plain click selects one", () => expect(nextSelection(order, { selected: ["a", "b"], anchor: "a" }, "c", {})).toEqual({ selected: ["c"], anchor: "c" }));
  it("ctrl toggles and keeps drawing order", () => {
    let p = nextSelection(order, { selected: ["d"], anchor: "d" }, "b", { toggle: true });
    expect(p.selected).toEqual(["b", "d"]);
    p = nextSelection(order, p, "d", { toggle: true });
    expect(p.selected).toEqual(["b"]);
  });
  it("shift selects the range from the anchor, either direction", () => {
    expect(nextSelection(order, { selected: ["b"], anchor: "b" }, "d", { shift: true }).selected).toEqual(["b", "c", "d"]);
    expect(nextSelection(order, { selected: ["d"], anchor: "d" }, "b", { shift: true }).selected).toEqual(["b", "c", "d"]);
  });
  it("shift+ctrl adds the range", () => expect(nextSelection(order, { selected: ["a", "d"], anchor: "d" }, "e", { shift: true, toggle: true }).selected).toEqual(["a", "d", "e"]));
  it("shift without an anchor acts as a plain click", () => expect(nextSelection(order, { selected: [], anchor: null }, "c", { shift: true }).selected).toEqual(["c"]));
});

describe("moveSelected", () => {
  it("top and bottom keep relative order", () => {
    expect(moveSelected(order, ["c", "e"], "top")).toEqual(["c", "e", "a", "b", "d"]);
    expect(moveSelected(order, ["a", "c"], "bottom")).toEqual(["b", "d", "e", "a", "c"]);
  });
  it("up and down move one step, stopping at the edge", () => {
    expect(moveSelected(order, ["b", "d"], "up")).toEqual(["b", "a", "d", "c", "e"]);
    expect(moveSelected(order, ["a", "c"], "up")).toEqual(["a", "c", "b", "d", "e"]);
    expect(moveSelected(order, ["b", "e"], "down")).toEqual(["a", "c", "b", "d", "e"]);
  });
  it("gather makes them contiguous at the highest one", () => expect(moveSelected(order, ["b", "d"], "gather")).toEqual(["a", "b", "d", "c", "e"]));
});

describe("unionExtent", () => {
  it("covers all and skips layers without one", () => expect(unionExtent([{ extent: [0, 0, 1, 1] }, { extent: null }, { extent: [-2, 0.5, 0.5, 3] }])).toEqual([-2, 0, 1, 3]));
  it("is null with nothing", () => expect(unionExtent([{ extent: null }])).toBeNull());
});

describe("applyStyleParts", () => {
  const base = { kind: "single" as const, fill: "#111", stroke: "#222", strokeWidth: 2, opacity: 0.5, radius: 5 };
  const info = (names: string[], geomType = "Polygon") => ({ geomType: geomType as never, fields: names.map((name) => ({ name, type: "VARCHAR" })) as never });
  const target = { ...base, fill: "#999" };
  it("copies a single symbol", () => expect(applyStyleParts(base, target, info([]), "symbology")?.style.fill).toBe("#111"));
  it("class symbology needs the field", () => {
    const cat = { ...base, kind: "categorized" as const, field: "kind", classes: [{ value: "x", color: "#fff", label: "x" }] };
    const bad = applyStyleParts(cat, target, info(["name"]), "symbology")!;
    expect(bad.style.kind).toBe("single");
    expect(bad.note).toContain('"kind"');
    const ok = applyStyleParts(cat, target, info(["kind"]), "symbology")!;
    expect(ok.style.kind).toBe("categorized");
    expect(ok.style.classes).toEqual(cat.classes);
    expect(ok.style.classes).not.toBe(cat.classes);
  });
  it("labels copy when the field exists, else say so", () => {
    const lab = { ...base, label: { enabled: true, field: "name", size: 9, color: "#000", haloColor: "#fff", haloWidth: 1 } };
    expect(applyStyleParts(lab, target, info(["name"]), "labels")?.style.label?.field).toBe("name");
    expect(applyStyleParts(lab, target, info(["x"]), "labels")?.note).toContain("name");
  });
  it("rasters and tables take nothing", () => expect(applyStyleParts(base, target, info([], "Raster"), "symbology")).toBeNull());
});

describe("safeFileName", () => {
  it("cleans and de-duplicates ignoring case", () => {
    const t = new Set<string>();
    expect(safeFileName("a/b:c", "gpkg", t)).toBe("a_b_c.gpkg");
    expect(safeFileName("A_B_C", "gpkg", t)).toBe("A_B_C_2.gpkg");
    expect(safeFileName("", "gpkg", t)).toBe("layer.gpkg");
  });
});
