// Regression: Project → Open passed the ribbon's click event as the folder path, and
// Tauri failed with "JSON.stringify cannot serialize cyclic structures" (docs/BUGS.md U-4).
import { beforeEach, describe, expect, it, vi } from "vitest";

const openProjectApi = vi.fn(async () => ({ uiState: "{}", layers: [] }));
vi.mock("./ipc/api", () => ({
  api: { openProject: (...a: unknown[]) => openProjectApi(...(a as [])), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));
const dialogOpen = vi.fn(async () => null as string | null);
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: () => dialogOpen() }));

import { openProject } from "./actions";
import { useApp } from "./store/store";
import { useBookmarks } from "./map/bookmarks";

beforeEach(() => {
  openProjectApi.mockClear();
  dialogOpen.mockReset();
});

describe("openProject", () => {
  it("treats a non-string argument (a click event) as no path: asks for a folder, never sends it to the engine", async () => {
    const clickEvent = { type: "click", target: {} } as unknown as string;
    (clickEvent as unknown as { self: unknown }).self = clickEvent; // cyclic, like a real DOM event
    dialogOpen.mockResolvedValue(null); // user cancels the picker
    await openProject(clickEvent);
    expect(dialogOpen).toHaveBeenCalledOnce();
    expect(openProjectApi).not.toHaveBeenCalled();
  });

  it("opens the folder picked in the dialog", async () => {
    dialogOpen.mockResolvedValue("/tmp/Madera County.gproj");
    await openProject();
    expect(openProjectApi).toHaveBeenCalledWith("/tmp/Madera County.gproj");
  });

  it("refuses a project saved by a newer format with a readable message and leaves the open project as it was", async () => {
    useApp.setState({ status: "before", layers: {}, dirty: false, projectDir: null });
    openProjectApi.mockResolvedValueOnce({ uiState: JSON.stringify({ version: 99, maps: [] }), layers: [] });
    await openProject("/tmp/future.gproj");
    expect(useApp.getState().status).toMatch(/newer version of GIS Studio/);
    expect(useApp.getState().projectDir).toBeNull();
    expect(useApp.getState().maps).toHaveLength(1);
  });

  it("opens a version 1 project as one map named Map", async () => {
    openProjectApi.mockResolvedValueOnce({ uiState: JSON.stringify({ version: 1, order: [], visible: {}, mapCrs: null, basemap: "osm" }), layers: [] });
    await openProject("/tmp/old.gproj");
    const s = useApp.getState();
    expect(s.maps.map((m) => m.name)).toEqual(["Map"]);
    expect(s.mapCrs).toBeNull();
    expect(s.basemap).toBe("osm");
  });

  it("bookmarks: an older project with none opens with an empty list, a project with some brings them back and drops damaged ones", async () => {
    useBookmarks.getState().set([{ id: "x", name: "left over", crs: "EPSG:4326", extent: [0, 0, 1, 1] }]);
    openProjectApi.mockResolvedValueOnce({ uiState: JSON.stringify({ version: 2, maps: [] }), layers: [] });
    await openProject("/tmp/nobm.gproj");
    expect(useBookmarks.getState().items).toEqual([]);
    const saved = [{ id: "b1", name: "Site", crs: "EPSG:26911", extent: [1, 2, 3, 4] }, { name: "", crs: "EPSG:4326", extent: [0, 0, 1, 1] }];
    openProjectApi.mockResolvedValueOnce({ uiState: JSON.stringify({ version: 2, maps: [], bookmarks: saved }), layers: [] });
    await openProject("/tmp/bm.gproj");
    expect(useBookmarks.getState().items).toEqual([saved[0]]);
  });
});
