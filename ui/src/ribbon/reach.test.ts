// Every command in the registry is on a ribbon tab, in a custom control's menu, in the project menu or in the Catalog's Public Data tree.
import { describe, expect, it } from "vitest";
import { CATALOG_COMMANDS, commands } from "./commands";
import { PROJECT_MENU } from "./projectBox";
import { commandsOnTabs, KEY_TIPS, LAYOUT_TABS, MAIN_TABS, TABS } from "./tabsDef";

describe("ribbon reach", () => {
  it("offers every command somewhere", () => {
    const reach = new Set([...commandsOnTabs(), ...PROJECT_MENU, ...CATALOG_COMMANDS, "save"]); // Save is in the tab row
    const lost = Object.keys(commands).filter((id) => !reach.has(id));
    expect(lost).toEqual([]);
  });

  it("names no command that does not exist", () => {
    const unknown = [...commandsOnTabs(), ...PROJECT_MENU].filter((id) => !commands[id]);
    expect(unknown).toEqual([]);
  });

  it("has the six tabs of the ribbon plan in order, with a key tip each", () => {
    expect(MAIN_TABS).toEqual(["Map", "Data", "Coordinates", "Analysis", "Insert", "View"]);
    expect(KEY_TIPS).toEqual({ Map: "M", Data: "D", Coordinates: "C", Analysis: "A", Insert: "I", View: "V" });
    for (const t of [...MAIN_TABS, ...LAYOUT_TABS, "Feature Layer"]) expect(TABS[t]?.length, t).toBeGreaterThan(0);
  });

  it("offers each command once across the main tabs, except Add Data (Map and Data)", () => {
    const seen = new Map<string, string[]>();
    for (const t of [...MAIN_TABS, "Feature Layer"]) for (const g of TABS[t]) for (const it of g.items) if ("cmd" in it) seen.set(it.cmd, [...(seen.get(it.cmd) ?? []), t]);
    const dups = [...seen].filter(([, tabs]) => tabs.length > 1).map(([id]) => id);
    expect(dups).toEqual(["addData"]);
    expect(seen.get("addData")).toEqual(["Map", "Data"]);
  });

  it("keeps the commands the plan puts on each tab", () => {
    const on = (tab: string) => new Set(commandsOnTabs({ [tab]: TABS[tab] }));
    for (const id of ["explore", "fullExtent", "goToCoordinates", "addData", "select", "identify", "measure", "startEditing"]) expect(on("Map").has(id), id).toBe(true);
    for (const id of ["addData", "catalog", "newLayer", "newTable", "georeference", "adjustShapes", "paneSources"]) expect(on("Data").has(id), id).toBe(true);
    for (const id of ["mapCrs", "projectionBuilder", "datumGrids", "tissot", "responsibleUse", "worldView", "worldGraticule", "worldOutline", "resection", "intersection", "usngGrid"]) expect(on("Coordinates").has(id), id).toBe(true);
    expect([...on("Analysis")].sort()).toEqual(["addTool", "runModel"]);
    expect(on("Feature Layer").has("removeLayer")).toBe(true);
    expect(on("Feature Layer").has("census")).toBe(true);
    // The Map tab keeps five groups and the quick everyday commands only.
    expect(TABS.Map.map((g) => g.label)).toEqual(["Navigate", "Layer", "Selection", "Inquiry", "Edit"]);
  });

  it("keeps the project menu to New, Open and Save As", () => {
    expect(PROJECT_MENU).toEqual(["new", "open", "saveAs", "startPage", "settings"]);
  });

  it("has no Geoprocessing pane command", () => {
    expect(Object.keys(commands).filter((id) => /tools$/i.test(id) || /modelBuilder/.test(id))).toEqual([]);
  });
});

describe("statistics workspace tabs", () => {
  it("has the six tabs of the statistics plan, each with a group", async () => {
    const { STATS_LABELS, STATS_ORDER, STATS_TABS } = await import("./tabsDef");
    expect(STATS_ORDER.map((t) => STATS_LABELS[t])).toEqual(["Explore", "Charts", "Tests", "Regression", "Spatial", "Export"]);
    for (const t of STATS_ORDER) expect(STATS_TABS[t]?.length, t).toBeGreaterThan(0);
  });

  it("puts every statistics command on a tab, and New Statistics Sheet on Insert and Explore", async () => {
    const { STATS_TABS } = await import("./tabsDef");
    const onStats = new Set(commandsOnTabs(STATS_TABS));
    for (const id of ["statNewSheet", "statRenameSheet", "statDeleteSheet", "statRun", "statRunAll", "statRemoveCard", "statCardUp", "statCardDown"]) expect(onStats.has(id), id).toBe(true);
    expect(new Set(commandsOnTabs({ Insert: TABS.Insert })).has("statNewSheet")).toBe(true);
  });

  it("leaves the not-yet-built tabs with plain text only: no button that would do nothing", async () => {
    const { STATS_TABS } = await import("./tabsDef");
    for (const t of ["stats-tests", "stats-regression", "stats-spatial", "stats-export"]) {
      const cmds = STATS_TABS[t].flatMap((g) => g.items).filter((i) => "cmd" in i);
      expect(cmds, t).toEqual([]);
    }
  });
});
