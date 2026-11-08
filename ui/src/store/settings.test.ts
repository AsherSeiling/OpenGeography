import { beforeEach, describe, expect, it, vi } from "vitest";

const saved: unknown[] = [];
let failSave = false;
vi.mock("../ipc/api", () => ({
  api: {
    getSettings: vi.fn(async () => ({ lengthUnit: "km", areaUnit: "nonsense", theme: "dark", defaultCrs: { id: "EPSG:5070", name: "NAD83 / Conus Albers" } })),
    setSettings: vi.fn(async (s: unknown) => { if (failSave) throw new Error("disk full"); saved.push(s); return s; }),
  },
  errorText: (e: unknown) => String(e instanceof Error ? e.message : e),
  IN_TAURI: false,
}));

import { useNav } from "../nav/navStore";
import { getDefaultMapCrs } from "./defaults";
import { newMapFrom } from "./maps";
import { applySettings, useSettings } from "./settings";
import { useApp } from "./store";

beforeEach(() => {
  saved.length = 0;
  failSave = false;
  useApp.setState({ dark: false });
  useNav.getState().set({ lengthUnit: "mi", areaUnit: "acres" });
  applySettings({});
});

describe("UR-16 program settings", () => {
  it("loads, applies what is valid and ignores what is not", async () => {
    await useSettings.getState().load();
    expect(useApp.getState().dark).toBe(true);
    expect(useNav.getState().lengthUnit).toBe("km");
    expect(useNav.getState().areaUnit).toBe("acres"); // an unknown unit key is ignored
    expect(getDefaultMapCrs()).toEqual({ id: "EPSG:5070", name: "NAD83 / Conus Albers" });
  });

  it("a new map with nothing to copy starts in the default coordinate system; one that can copy still copies", () => {
    applySettings({ defaultCrs: { id: "EPSG:3857", name: "Web Mercator" } });
    expect(newMapFrom([]).map.crs).toEqual({ id: "EPSG:3857", name: "Web Mercator" });
    const first = newMapFrom([]).map;
    const withCrs = { ...first, crs: { id: "EPSG:26914", name: "UTM 14N" } };
    const next = newMapFrom([withCrs]);
    expect(next.map.crs?.id).toBe("EPSG:26914");
    expect(next.inheritedFrom?.id).toBe(withCrs.id);
    applySettings({});
    expect(newMapFrom([]).map.crs).toBeNull();
  });

  it("update applies at once and saves; a failed save says so and the choice still applies", async () => {
    expect(await useSettings.getState().update({ lengthUnit: "ft" })).toBeNull();
    expect(useNav.getState().lengthUnit).toBe("ft");
    expect(saved).toHaveLength(1);
    failSave = true;
    const err = await useSettings.getState().update({ areaUnit: "ha" });
    expect(err).toContain("could not be saved");
    expect(useNav.getState().areaUnit).toBe("ha");
  });
});
