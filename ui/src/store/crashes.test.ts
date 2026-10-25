// Regression tests for crashes found while exercising CRS changes and the Layout view.
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), validateCrs: vi.fn(async (c: string) => { if (c === "bad") throw "Unknown CRS"; }) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));
vi.mock("../map/mapController", () => ({ getMap: () => null, getCenterLonLat: () => null, getGroundPerPixel: () => null }));

import { selectionOf } from "../layout/LayoutView";
import { exportLayout, pickFrameCrs, setFrameFromMapView } from "../layout/actions";
import { cleanPatch, newLayoutDoc, normalizeLayout, roundScaleUp, type Layout, type MapFrame } from "../layout/types";
import { setMapCrsChecked } from "../actions";
import { useErrors } from "../errors/store";
import { addMapFrame, useLayouts } from "./layout";
import { useApp } from "./store";

beforeEach(() => {
  useLayouts.getState().loadAll([newLayoutDoc("a", "A")]);
  useLayouts.getState().open("a");
  useApp.setState({ mapCrs: { id: "EPSG:26915", name: "UTM 15N" }, dirty: false, status: "" });
  useErrors.getState().clear();
});

describe("crash regressions", () => {
  it("frame actions with no open layout say so instead of throwing", async () => {
    const id = addMapFrame()!;
    useLayouts.getState().open(null);
    await expect(setFrameFromMapView(id)).resolves.toBeUndefined();
    expect(() => pickFrameCrs(id)).not.toThrow();
    expect(useApp.getState().status).toMatch(/Insert a map frame/);
  });

  it("export with no map frame explains instead of failing", async () => {
    await exportLayout("pdf");
    expect(useApp.getState().status).toMatch(/no map frame/);
  });

  it("invalid numbers never reach the document", () => {
    const id = addMapFrame()!;
    const u = useLayouts.getState().update;
    u(id, { scale: 0 });
    u(id, { w: -5, h: NaN });
    u(id, { x: Infinity, center: [NaN, 1] });
    const f = useLayouts.getState().doc!.elements.find((e) => e.id === id) as MapFrame;
    expect(f.scale).toBe(1_000_000);
    expect(f.w).toBeGreaterThan(1);
    expect(Number.isFinite(f.h) && Number.isFinite(f.x)).toBe(true);
    expect(f.center).toEqual([0, 0]);
    expect(cleanPatch({ w: 20, scale: -1 })).toEqual({ w: 20 });
  });

  it("a page with zero or NaN size is refused", () => {
    const before = useLayouts.getState().doc!.page;
    useLayouts.getState().setPage({ ...before, width: 0 });
    useLayouts.getState().setPage({ ...before, height: NaN });
    expect(useLayouts.getState().doc!.page).toEqual(before);
  });

  it("layouts from older or damaged projects get safe defaults", () => {
    const old = { id: "o", name: "", version: 1, body: "earth", elements: [{ kind: "map_frame", id: "f1", x: 1, y: 1, w: 0, h: 5 }, null], series: null } as unknown as Layout;
    const n = normalizeLayout(old);
    expect(n.page.width).toBeGreaterThan(0);
    expect(n.name).toBe("o");
    const f = n.elements[0] as MapFrame;
    expect(f.center).toEqual([0, 0]);
    expect(f.w).toBeGreaterThanOrEqual(1);
    expect(f.scale).toBe(1_000_000);
    expect(n.elements).toHaveLength(1);
    useLayouts.getState().loadAll([old]);
    expect(useLayouts.getState().list()).toHaveLength(1);
  });

  it("a layout tab that isn't open gets the same empty selection every time (no render loop after reopening a project)", () => {
    useLayouts.getState().loadAll([newLayoutDoc("a", "A"), newLayoutDoc("b", "B")]);
    useLayouts.getState().open("a");
    const pick = selectionOf("b");
    expect(pick(useLayouts.getState())).toBe(pick(useLayouts.getState()));
    expect(selectionOf("zzz")(useLayouts.getState())).toBe(pick(useLayouts.getState()));
  });

  it("roundScaleUp tolerates NaN, 0 and Infinity", () => {
    for (const v of [NaN, 0, -3, Infinity]) expect(Number.isFinite(roundScaleUp(v))).toBe(true);
  });

  it("an invalid map CRS is refused and the old one stays", async () => {
    await setMapCrsChecked("bad", "Bad");
    expect(useApp.getState().mapCrs?.id).toBe("EPSG:26915");
    expect(useApp.getState().status).toMatch(/can't be used/);
    await setMapCrsChecked("EPSG:5070", "Albers");
    expect(useApp.getState().mapCrs?.id).toBe("EPSG:5070");
  });
});
