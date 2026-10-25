import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { defaultStyle, displayInfo, drawableLayerIds, layerList, ownerOf, useApp } from "./store";

const info = (id: string, over: Partial<LayerInfo> = {}): LayerInfo =>
  ({ id, name: id, geomType: "Polygon", crs: "EPSG:26914", crsName: "NAD83 / UTM 14N", version: 1, fields: [], ...over }) as never;

beforeEach(() => {
  useApp.setState({ layers: {}, results: {}, showOriginal: {}, order: [], visible: {}, styles: {}, selection: {}, activeLayer: null, properties: null, mapCrs: null, dirty: false });
});

describe("layer list", () => {
  it("puts new layers on top, visible, styled, and makes the first the active layer", () => {
    useApp.getState().addLayers([info("a")]);
    useApp.getState().addLayers([info("b")]);
    const s = useApp.getState();
    expect(s.order).toEqual(["b", "a"]);
    expect(s.visible).toEqual({ a: true, b: true });
    expect(s.styles.a.kind).toBe("single");
    expect(s.activeLayer).toBe("a");
    expect(s.dirty).toBe(true);
  });

  it("sets the map CRS from the first layer that has one, never from a table or a CRS-less layer", () => {
    useApp.getState().addLayers([info("t", { geomType: "Table", crs: null }), info("n", { crs: null }), info("p")]);
    expect(useApp.getState().mapCrs).toEqual({ id: "EPSG:26914", name: "NAD83 / UTM 14N" });
    useApp.getState().addLayers([info("q", { crs: "EPSG:4326", crsName: "WGS 84" })]);
    expect(useApp.getState().mapCrs?.id).toBe("EPSG:26914"); // later layers don't change it
  });

  it("leaves the map CRS unset when nothing has one", () => {
    useApp.getState().addLayers([info("n", { crs: null })]);
    expect(useApp.getState().mapCrs).toBeNull();
  });

  it("re-adding a layer updates its info but keeps its place, style and visibility", () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().setVisible("a", false);
    const style = useApp.getState().styles.a;
    useApp.getState().addLayers([info("a", { name: "renamed", version: 2 })]);
    const s = useApp.getState();
    expect(s.layers.a.name).toBe("renamed");
    expect(s.order).toEqual(["b", "a"]);
    expect(s.visible.a).toBe(false);
    expect(s.styles.a).toBe(style);
  });

  it("an empty batch changes nothing but the dirty flag", () => {
    useApp.getState().addLayers([]);
    expect(useApp.getState().order).toEqual([]);
    expect(useApp.getState().activeLayer).toBeNull();
  });

  it("moveLayer clamps to both ends", () => {
    useApp.getState().addLayers([info("a"), info("b"), info("c")]); // order c b a
    useApp.getState().moveLayer("c", 99);
    expect(useApp.getState().order).toEqual(["b", "a", "c"]);
    useApp.getState().moveLayer("c", -5);
    expect(useApp.getState().order).toEqual(["c", "b", "a"]);
    useApp.getState().moveLayer("ghost", 1); // unknown id is inserted, not lost or thrown
    expect(useApp.getState().order).toHaveLength(4);
  });

  it("removing the active layer activates another; removing the last leaves none", () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().setSelection("a", [1, 2]);
    useApp.getState().setProperties({ kind: "layer", id: "a" });
    useApp.getState().removeLayer("a");
    const s = useApp.getState();
    expect(s.activeLayer).toBe("b");
    expect(s.selection.a).toBeUndefined();
    expect(s.properties).toBeNull();
    useApp.getState().removeLayer("b");
    expect(useApp.getState().activeLayer).toBeNull();
    expect(layerList()).toEqual([]);
  });

  it("removing a layer keeps the Properties window of another layer or node", () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().setProperties({ kind: "layer", id: "b" });
    useApp.getState().removeLayer("a");
    expect(useApp.getState().properties).toEqual({ kind: "layer", id: "b" });
  });
});

describe("selection edge cases", () => {
  it("toggle flips membership, add dedupes, remove of a missing id is a no-op", () => {
    const sel = () => [...(useApp.getState().selection.L ?? [])].sort((a, b) => a - b);
    useApp.getState().setSelection("L", [1, 2, 3]);
    useApp.getState().setSelection("L", [3, 4], "toggle");
    expect(sel()).toEqual([1, 2, 4]);
    useApp.getState().setSelection("L", [1, 1, 1], "add");
    expect(sel()).toEqual([1, 2, 4]);
    useApp.getState().setSelection("L", [99], "remove");
    expect(sel()).toEqual([1, 2, 4]);
  });

  it("modes on a layer with no selection yet, and an empty id list", () => {
    useApp.getState().setSelection("N", [5], "remove");
    expect(useApp.getState().selection.N).toEqual([]);
    useApp.getState().setSelection("N", [], "subset");
    expect(useApp.getState().selection.N).toEqual([]);
    useApp.getState().setSelection("N", [], "new");
    expect(useApp.getState().selection.N).toEqual([]);
  });

  it("holds a very large selection", () => {
    const ids = Array.from({ length: 200_000 }, (_, i) => i);
    useApp.getState().setSelection("L", ids);
    useApp.getState().setSelection("L", [0, 199_999], "toggle");
    expect(useApp.getState().selection.L).toHaveLength(199_998);
  });

  it("clearSelection empties every layer", () => {
    useApp.getState().setSelection("a", [1]);
    useApp.getState().setSelection("b", [2]);
    useApp.getState().clearSelection();
    expect(useApp.getState().selection).toEqual({});
  });
});

describe("results shown in place of layers", () => {
  const withResult = () => {
    useApp.getState().addLayers([info("a"), info("b")]);
    useApp.getState().addLayers([info("a-result", { resultOf: "a", name: "result" })]);
  };

  it("a result replaces its layer's display until 'show original' is set; selection resets", () => {
    withResult();
    const s = () => useApp.getState();
    expect(s().order).toEqual(["b", "a"]); // results don't join the layer list
    expect(displayInfo(s(), "a")?.id).toBe("a-result");
    expect(displayInfo(s(), "b")?.id).toBe("b");
    s().setSelection("a", [1]);
    s().setShowOriginal("a", true);
    expect(displayInfo(s(), "a")?.id).toBe("a");
    expect(s().selection.a).toEqual([]);
    expect(ownerOf("a-result")).toBe("a");
    expect(ownerOf("a")).toBe("a");
    expect(ownerOf("unknown")).toBe("unknown");
  });

  it("a new result for the same layer clears the old selection", () => {
    withResult();
    useApp.getState().setSelection("a", [3, 4]);
    useApp.getState().updateLayer(info("a-result2", { resultOf: "a" }));
    expect(useApp.getState().selection.a).toEqual([]);
    expect(useApp.getState().results.a.id).toBe("a-result2");
  });

  it("updateLayer for an unknown layer adds it", () => {
    useApp.getState().updateLayer(info("z"));
    expect(useApp.getState().order).toEqual(["z"]);
  });

  it("drawableLayerIds skips hidden layers, tables and layers without a CRS, and uses results", () => {
    useApp.getState().addLayers([info("a"), info("t", { geomType: "Table" }), info("n", { crs: null }), info("h")]);
    useApp.getState().addLayers([info("a-res", { resultOf: "a" })]);
    useApp.getState().setVisible("h", false);
    expect(drawableLayerIds().sort()).toEqual(["a-res"]);
  });
});

describe("default symbology", () => {
  it("cycles colours and differs by geometry", () => {
    const poly = defaultStyle(info("p"));
    const line = defaultStyle(info("l", { geomType: "Line" }));
    const pt = defaultStyle(info("x", { geomType: "Point" }));
    expect(poly.stroke).toBe("#4d4d4d");
    expect(poly.opacity).toBeLessThan(1);
    expect(line.strokeWidth).toBe(1.5);
    expect(line.stroke).toBe(line.fill);
    expect(pt.opacity).toBe(1);
    const fills = new Set(Array.from({ length: 4 }, () => defaultStyle(info("p")).fill));
    expect(fills.size).toBeGreaterThan(1);
  });
});
