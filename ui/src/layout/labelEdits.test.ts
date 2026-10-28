import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { addMapFrame, useLayouts } from "../store/layout";
import { defaultLabel, defaultStyle, useApp } from "../store/store";
import type { LayerInfo } from "../ipc/api";
import { dragged, pinned, withFeatureEdit, withoutDropped, withStyleOverride } from "./labelEdits";
import { newLayoutDoc, normalizeLayout, type MapFrame } from "./types";

const L = useLayouts;
const frame = () => L.getState().doc!.elements.find((e) => e.kind === "map_frame") as MapFrame;

beforeEach(() => {
  L.getState().loadAll([newLayoutDoc("a", "A")]);
  L.getState().open("a");
  useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" }, dirty: false });
});

describe("label edits in a layout (L3)", () => {
  it("builds and prunes per-label edits", () => {
    let o = withFeatureEdit(undefined, "L1", 7, { dxMm: 2, dyMm: -1 });
    expect(o).toEqual({ L1: { features: { "7": { dxMm: 2, dyMm: -1 } } } });
    o = withFeatureEdit(o, "L1", 7, { text: "Hi" });
    expect(o!.L1.features!["7"]).toEqual({ dxMm: 2, dyMm: -1, text: "Hi" });
    o = withFeatureEdit(o, "L1", 7, { text: "" }); // empty text = automatic again
    expect(o!.L1.features!["7"]).toEqual({ dxMm: 2, dyMm: -1 });
    expect(withFeatureEdit(o, "L1", 7, null)).toBeUndefined(); // "Reset to automatic" leaves nothing behind
    expect(withFeatureEdit(o, "L1", 8, { hidden: true })!.L1.features).toMatchObject({ "7": {}, "8": { hidden: true } });
  });

  it("pinned means moved, turned, resized or re-worded; hidden alone is not pinned", () => {
    expect(pinned({ dxMm: 1 })).toBe(true);
    expect(pinned({ rotation: 0 })).toBe(true);
    expect(pinned({ text: "x" })).toBe(true);
    expect(pinned({ hidden: true })).toBe(false);
    expect(pinned(undefined)).toBe(false);
  });

  it("a drag adds to the earlier move; a tiny drag is a click", () => {
    expect(dragged(undefined, 5, -3)).toEqual({ dxMm: 5, dyMm: -3 });
    expect(dragged({ dxMm: 1, dyMm: 1 }, 2.005, 0.5)).toEqual({ dxMm: 3.01, dyMm: 1.5 });
    expect(dragged({ dxMm: 1 }, 0.05, 0.05)).toBeNull();
  });

  it("L3-P4 a per-layout style override is kept on the frame and never touches the map's style", () => {
    const style = { ...defaultStyle({ geomType: "Polygon" } as LayerInfo), label: defaultLabel("NAME") };
    useApp.setState({ styles: { L1: style } });
    addMapFrame();
    const f = frame();
    L.getState().update(f.id, { labelOverrides: withStyleOverride(f.labelOverrides, "L1", { size: 14, color: "#ff0000" }) });
    expect(frame().labelOverrides).toEqual({ L1: { style: { size: 14, color: "#ff0000" } } });
    expect(useApp.getState().styles.L1.label).toEqual(defaultLabel("NAME"));
    L.getState().update(f.id, { labelOverrides: withStyleOverride(frame().labelOverrides, "L1", null) });
    expect(frame().labelOverrides).toBeUndefined();
  });

  it("L3-P5 edits for deleted features are removed, only in the named frame", () => {
    let o = withFeatureEdit(undefined, "L1", 1, { dxMm: 1 });
    o = withFeatureEdit(o, "L1", 2, { dxMm: 2 });
    o = withFeatureEdit(o, "L2", 9, { hidden: true });
    const out = withoutDropped(o, "f1", [{ frame: "f1", layer: "L1", fid: 2 }, { frame: "f2", layer: "L2", fid: 9 }]);
    expect(Object.keys(out!.L1.features!)).toEqual(["1"]);
    expect(out!.L2.features!["9"]).toEqual({ hidden: true }); // that drop was for another frame
  });

  it("L3-P6 a move undoes and redoes as one step, and the selection stays", () => {
    addMapFrame();
    const f = frame();
    L.getState().select([f.id]);
    L.getState().update(f.id, { labelOverrides: withFeatureEdit(f.labelOverrides, "L1", 3, dragged(undefined, 4, 2)) });
    expect(frame().labelOverrides!.L1.features!["3"]).toEqual({ dxMm: 4, dyMm: 2 });
    L.getState().undo();
    expect(frame().labelOverrides).toBeUndefined();
    L.getState().redo();
    expect(frame().labelOverrides!.L1.features!["3"]).toEqual({ dxMm: 4, dyMm: 2 });
    expect(L.getState().selected).toEqual([f.id]);
  });

  it("L3-P1 edits survive the layout's JSON round trip (save and reopen)", () => {
    addMapFrame();
    const f = frame();
    L.getState().update(f.id, { labelOverrides: withFeatureEdit(f.labelOverrides, "L1", 3, { dxMm: 4, hidden: true }) });
    const saved = JSON.parse(JSON.stringify(L.getState().doc));
    const reopened = normalizeLayout(saved).elements.find((e) => e.kind === "map_frame") as MapFrame;
    expect(reopened.labelOverrides).toEqual({ L1: { features: { "3": { dxMm: 4, hidden: true } } } });
  });
});
