// UX9: copy and paste a whole style, applying a library style.
import { beforeEach, describe, expect, it } from "vitest";
import type { LayerInfo } from "../ipc/api";
import { defaultStyle, useApp, type LayerStyle } from "../store/store";
import { applyStyleTo, applyWholeStyle, copyStyle, isVector, pasteStyle, useStyleClip } from "./styleClip";

const info = (id: string, name: string, geomType: string, fields: string[]): LayerInfo => ({ id, name, geomType, fields: fields.map((f) => ({ name: f, type: "text" })), featureCount: 1 }) as unknown as LayerInfo;
const base = (over: Partial<LayerStyle>): LayerStyle => ({ kind: "single", fill: "#111111", stroke: "#222222", strokeWidth: 2, opacity: 0.5, radius: 7, ...over });

beforeEach(() => {
  useStyleClip.getState().set(null);
  const a = info("A", "Roads", "Line", ["class", "name"]);
  const b = info("B", "Parcels", "Polygon", ["name"]);
  const r = info("R", "DEM", "Raster", []);
  useApp.setState({
    layers: { A: a, B: b, R: r },
    styles: {
      A: base({ kind: "categorized", field: "class", classes: [{ value: "x", color: "#ff0000", label: "x" }] as never, label: { field: "name", size: 10, color: "#000000" } as never }),
      B: defaultStyle(b),
      R: defaultStyle(r),
    },
    status: "",
  });
});

describe("copy and paste style", () => {
  it("copies symbology and labels, and pastes onto another layer that has the same fields", () => {
    useApp.setState({ layers: { ...useApp.getState().layers, B: info("B", "Parcels", "Polygon", ["name", "class"]) } });
    copyStyle("A");
    expect(useStyleClip.getState().clip?.name).toBe("Roads");
    pasteStyle(["B"]);
    const s = useApp.getState().styles.B;
    expect(s.kind).toBe("categorized");
    expect(s.field).toBe("class");
    expect(s.label?.field).toBe("name");
    expect(s.fill).toBe("#111111");
    expect(useApp.getState().status).toMatch(/Applied the style “Roads” to 1 layer/);
  });

  it("falls back to the single symbol and says what was left out when a field is missing", () => {
    copyStyle("A");
    pasteStyle(["B"]);
    const s = useApp.getState().styles.B;
    expect(s.kind).toBe("single");
    expect(s.label?.field).toBe("name");
    expect(useApp.getState().status).toMatch(/Parcels: classes not copied: no field "class"/);
  });

  it("a later edit to the copied layer does not change what was copied", () => {
    copyStyle("A");
    useApp.getState().setStyle("A", base({ fill: "#abcdef" }));
    expect(useStyleClip.getState().clip?.style.fill).toBe("#111111");
  });

  it("rasters and tables take and give no vector style, with a sentence", () => {
    copyStyle("R");
    expect(useStyleClip.getState().clip).toBeNull();
    expect(useApp.getState().status).toMatch(/Only point, line and polygon/);
    copyStyle("A");
    const before = useApp.getState().styles.R;
    pasteStyle(["R"]);
    expect(useApp.getState().styles.R).toBe(before);
    expect(useApp.getState().status).toMatch(/DEM: not a vector layer/);
    expect(isVector(useApp.getState().layers.R)).toBe(false);
  });

  it("paste with nothing copied says so and changes nothing", () => {
    const before = useApp.getState().styles.B;
    pasteStyle(["B"]);
    expect(useApp.getState().styles.B).toBe(before);
    expect(useApp.getState().status).toMatch(/Nothing copied yet/);
  });
});

describe("a library style", () => {
  it("applies to several layers and never touches a layer that is not in the list", () => {
    const lib = base({ fill: "#00ff00", label: undefined });
    expect(applyStyleTo("Green", lib, ["B"])).toBe(1);
    expect(useApp.getState().styles.B.fill).toBe("#00ff00");
    expect(useApp.getState().styles.A.fill).toBe("#111111");
    expect(applyWholeStyle(lib, useApp.getState().styles.R, useApp.getState().layers.R)).toBeNull();
  });
});
