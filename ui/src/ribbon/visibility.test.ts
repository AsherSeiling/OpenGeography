// RB3: groups that work only in some situations are left out otherwise.
import { describe, expect, it } from "vitest";
import { visibleGroups } from "./tabs";
import { TABS } from "./tabsDef";

const names = (inLayout: boolean, developer: boolean) => visibleGroups(TABS.View, { inLayout, developer }).map((g) => g.label);

describe("conditional ribbon groups", () => {
  it("shows View > Zoom only while a layout is open", () => {
    expect(names(false, false)).not.toContain("Zoom");
    expect(names(true, false)).toContain("Zoom");
  });

  it("hides the Developer group unless the setting is on", () => {
    expect(names(false, false)).not.toContain("Developer");
    expect(names(true, false)).not.toContain("Developer");
    expect(names(false, true)).toContain("Developer");
  });

  it("keeps Windows, Appearance and Program always, in the planned order", () => {
    expect(names(false, false)).toEqual(["Windows", "Appearance", "Program"]);
    expect(names(true, true)).toEqual(["Zoom", "Windows", "Appearance", "Program", "Developer"]);
  });

  it("leaves tabs without conditional groups alone", () => {
    for (const t of ["Map", "Data", "Coordinates", "Analysis", "Insert"]) expect(visibleGroups(TABS[t], { inLayout: false, developer: false })).toEqual(TABS[t]);
  });
});
