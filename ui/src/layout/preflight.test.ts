import { describe, expect, it } from "vitest";
import { allClear, preflight } from "./preflight";
import { newLayoutDoc, type Layout, type LayoutElement } from "./types";

const ctx = { mapExists: (id: string) => id === "m1", crsName: (id: string) => (id === "EPSG:26911" ? "NAD83 / UTM 11N" : "") };
const base = { rotation: 0, locked: false, visible: true, x: 0, y: 0, w: 10, h: 10 };
const frame = (over: Record<string, unknown> = {}) => ({ ...base, kind: "map_frame", id: "f1", crs: "EPSG:26911", center: [0, 0], scale: 24000, layers: "map", basemap: "none", graticule: null, grid: null, border: null, ...over }) as unknown as LayoutElement;
const el = (kind: string, id: string, over: Record<string, unknown> = {}) => ({ ...base, kind, id, frame: "f1", ...over }) as unknown as LayoutElement;
const doc = (...elements: LayoutElement[]): Layout => ({ ...newLayoutDoc("L1", "T", "Letter", "landscape"), elements });

describe("preflight", () => {
  it("fails a page without a map frame", () => {
    const r = preflight(doc(), ctx);
    expect(r).toHaveLength(1);
    expect(r[0]).toMatchObject({ id: "frames", ok: false });
  });
  it("passes a complete page and names the CRS", () => {
    const r = preflight(doc(frame(), el("sources", "src1"), el("scale_bar", "sb1"), el("legend", "lg1")), ctx);
    expect(allClear(r)).toBe(true);
    expect(r.find((c) => c.id === "crs")!.text).toContain("NAD83 / UTM 11N");
  });
  it("flags a frame with no CRS, no sources, a deleted map, and orphaned scale bars and legends", () => {
    const r = preflight(doc(frame({ crs: "", layers: { map: "m9" } }), el("scale_bar", "sb1", { frame: "zz" }), el("legend", "lg1", { frame: "zz" })), ctx);
    const bad = r.filter((c) => !c.ok).map((c) => c.id).sort();
    expect(bad).toEqual(["crs", "legend", "maps", "scale", "sources"]);
  });
  it("accepts a frame that follows a map that exists", () => {
    const r = preflight(doc(frame({ layers: { map: "m1" } }), el("sources", "src1")), ctx);
    expect(r.some((c) => c.id === "maps")).toBe(false);
  });
});
