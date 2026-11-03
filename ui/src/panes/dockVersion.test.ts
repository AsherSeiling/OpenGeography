// A saved dock layout is used only when it has this version, names only known panes and has a map; anything else falls back to the default.
import { describe, expect, it } from "vitest";
import { DOCK_VERSION, usableDock } from "./panes";

const layout = (ids: string[]) => ({ version: DOCK_VERSION, layout: { panels: Object.fromEntries(ids.map((i) => [i, {}])) } });

describe("usableDock", () => {
  it("accepts the current version with known panes, and layout tabs", () => {
    expect(usableDock(layout(["map", "contents", "catalog", "properties", "sources"]))).toBe(true);
    expect(usableDock(layout(["map", "model", "table", "items", "layout:abc"]))).toBe(true);
  });
  it("rejects a layout saved before versions existed (the raw dockview JSON)", () => {
    expect(usableDock({ grid: {}, panels: { map: {} } })).toBe(false);
  });
  it("rejects another version", () => {
    expect(usableDock({ ...layout(["map"]), version: DOCK_VERSION - 1 })).toBe(false);
    expect(usableDock({ ...layout(["map"]), version: DOCK_VERSION + 1 })).toBe(false);
  });
  it("rejects a layout that names the removed Geoprocessing pane or an unknown one", () => {
    expect(usableDock(layout(["map", "contents", "tools"]))).toBe(false);
    expect(usableDock(layout(["map", "mystery"]))).toBe(false);
  });
  it("rejects a layout without a map, and junk", () => {
    expect(usableDock(layout(["contents"]))).toBe(false);
    for (const junk of [null, undefined, 3, "x", [], { version: DOCK_VERSION }]) expect(usableDock(junk)).toBe(false);
  });
});
