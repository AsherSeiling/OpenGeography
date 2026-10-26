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
});
