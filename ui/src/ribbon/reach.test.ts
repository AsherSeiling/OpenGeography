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

  it("has the tabs the plan asks for, no Data tab, and a key tip for each main tab", () => {
    expect(MAIN_TABS).toEqual(["Map", "Insert", "View", "Analysis"]);
    expect(Object.keys(TABS)).not.toContain("Data");
    expect(Object.keys(KEY_TIPS).sort()).toEqual([...MAIN_TABS].sort());
    for (const t of [...MAIN_TABS, ...LAYOUT_TABS, "Feature Layer"]) expect(TABS[t]?.length, t).toBeGreaterThan(0);
  });

  it("keeps the project menu to New, Open and Save As", () => {
    expect(PROJECT_MENU).toEqual(["new", "open", "saveAs"]);
  });

  it("has no Geoprocessing pane command", () => {
    expect(Object.keys(commands).filter((id) => /tools$/i.test(id) || /modelBuilder/.test(id))).toEqual([]);
  });
});
