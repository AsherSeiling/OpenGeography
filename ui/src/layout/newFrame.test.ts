import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { addMapFrame, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { newLayoutDoc, normalizeLayout, type MapFrame } from "./types";

const L = useLayouts;
const frames = () => L.getState().doc!.elements.filter((e) => e.kind === "map_frame") as MapFrame[];

beforeEach(() => {
  L.getState().loadAll([newLayoutDoc("a", "A")]);
  L.getState().open("a");
  useApp.setState({ mapCrs: { id: "EPSG:26915", name: "NAD83 / UTM zone 15N" }, dirty: false });
});

describe("UR-4 a new map frame has no outline", () => {
  it("starts with no border and no decor frame; an explicit border (the overview inset) is kept", () => {
    addMapFrame();
    expect(frames()[0].border).toBeNull();
    expect(frames()[0].decor?.frameWidth ?? 0).toBe(0);
    addMapFrame({ border: { width: 0.35, color: "#000000" } });
    expect(frames()[1].border).toEqual({ width: 0.35, color: "#000000" });
  });

  it("a saved layout keeps the border it has", () => {
    const doc = newLayoutDoc("b", "B");
    doc.elements.push({ ...({ kind: "map_frame", id: "f1", x: 10, y: 10, w: 100, h: 80, rotation: 0, locked: false, visible: true, crs: "EPSG:26915", center: [0, 0], scale: 1e6, layers: "map", basemap: "none", graticule: null, grid: null, border: { width: 0.5, color: "#112233" } } as MapFrame) });
    const back = normalizeLayout(JSON.parse(JSON.stringify(doc)));
    expect((back.elements[0] as MapFrame).border).toEqual({ width: 0.5, color: "#112233" });
  });
});
