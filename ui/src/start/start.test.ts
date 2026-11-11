// UX2: the Start page's lists and the actions behind it.
import { beforeEach, describe, expect, it, vi } from "vitest";

const setSettings = vi.fn(async (s: unknown) => s);
vi.mock("../ipc/api", () => ({
  api: { setSettings: (s: unknown) => setSettings(s), newProject: vi.fn(async () => null), listLayouts: vi.fn(async () => []), getSettings: vi.fn(async () => ({})), discardOwnRecovery: vi.fn(async () => {}) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));

import { forgetProject, newProjectFromTemplate, rememberProject } from "../actions";
import { useSettings } from "../store/settings";
import { useApp } from "../store/store";
import { MAX_RECENT, projectLabel, projectWhere, pushRecent, SAMPLE_DATA, START_TEMPLATES } from "./recent";

beforeEach(() => {
  setSettings.mockClear();
  useSettings.setState({ settings: {} });
  useApp.setState({ dirty: false, projectDir: null });
});

describe("recent projects", () => {
  it("puts the newest first, moves a repeat to the top, keeps ten", () => {
    expect(pushRecent(["/a", "/b"], "/b")).toEqual(["/b", "/a"]);
    expect(pushRecent([], "  ")).toEqual([]);
    const many = Array.from({ length: 12 }, (_, i) => `/p${i}`);
    expect(pushRecent(many, "/new")).toHaveLength(MAX_RECENT);
    expect(pushRecent(many, "/new")[0]).toBe("/new");
  });

  it("names a project by its folder without .gproj, on either kind of path", () => {
    expect(projectLabel("/work/Madera County.gproj")).toBe("Madera County");
    expect(projectLabel("C:\\Users\\a\\Parcels.gproj")).toBe("Parcels");
    expect(projectWhere("/home/a/work/Parcels.gproj")).toBe("a/work");
  });

  it("remembers and forgets through the per-user settings, never the project", async () => {
    await rememberProject("/work/a.gproj");
    await rememberProject("/work/b.gproj");
    expect(useSettings.getState().settings.recentProjects).toEqual(["/work/b.gproj", "/work/a.gproj"]);
    expect(setSettings).toHaveBeenCalledTimes(2);
    await rememberProject("/work/b.gproj"); // already first: no write
    expect(setSettings).toHaveBeenCalledTimes(2);
    await forgetProject("/work/b.gproj");
    expect(useSettings.getState().settings.recentProjects).toEqual(["/work/a.gproj"]);
  });
});

describe("templates and sample data", () => {
  it("offers a blank start and named coordinate systems, and sample data that needs no download", () => {
    expect(START_TEMPLATES[0].crs).toBeNull();
    expect(START_TEMPLATES.map((t) => t.crs?.id).filter(Boolean)).toEqual(["EPSG:4326", "EPSG:3857", "EPSG:5070"]);
    expect(SAMPLE_DATA.map((d) => d.id)).toEqual(["world", "polygons", "points"]);
  });

  it("a template starts a new project whose first map has that coordinate system, and it is not unsaved work", async () => {
    await newProjectFromTemplate(START_TEMPLATES[3]);
    const s = useApp.getState();
    expect(s.maps.find((m) => m.id === s.activeMapId)?.crs?.id).toBe("EPSG:5070");
    expect(s.dirty).toBe(false);
    expect(s.projectDir).toBeNull();
  });
});
