import { describe, expect, it } from "vitest";
import { adaptStartup } from "./panes";

const saved = (mapId: string) => ({
  version: 3,
  layout: {
    grid: { root: { type: "branch", data: [{ type: "leaf", data: { views: ["contents"], activeView: "contents", id: "1" }, size: 200 }, { type: "leaf", data: { views: [mapId], activeView: mapId, id: "2" }, size: 800 }] } },
    panels: { contents: { id: "contents", contentComponent: "contents", title: "Map Layers" }, [mapId]: { id: mapId, contentComponent: "map", title: "Map 1" } },
  },
});

describe("UR-17 the startup layout is adapted to the project it opens in", () => {
  it("points the saved map panel at this project's active map everywhere its id appears, and keeps the rest", () => {
    const out = adaptStartup(saved("map-ab12"), "map-zz99") as ReturnType<typeof saved>;
    const text = JSON.stringify(out);
    expect(text).not.toContain("map-ab12");
    expect(Object.keys(out.layout.panels).sort()).toEqual(["contents", "map-zz99"]);
    expect(text.match(/map-zz99/g)!.length).toBe(4); // the panel key, its id, the view and the active view
    expect(text).toContain('"size":200');
  });
  it("leaves it alone when the ids already match, and refuses what is not a usable dock", () => {
    expect(adaptStartup(saved("map"), "map")).toEqual(saved("map"));
    expect(adaptStartup({ version: 99, layout: {} }, "map")).toBeNull();
    expect(adaptStartup(null, "map")).toBeNull();
    expect(adaptStartup("nope", "map")).toBeNull();
  });
});
