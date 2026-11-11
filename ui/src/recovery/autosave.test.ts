// UX1: autosave writes only a separate copy, only when there is unsaved work, never marks the project saved, and the copy goes away
// after a clean save. Restoring brings the work back as unsaved and keeps the old copy until the new one is written.
import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: string[] = [];
const writeRecovery = vi.fn(async (..._a: unknown[]) => {});
const discardOwn = vi.fn(async () => {});
const discardRecovery = vi.fn(async (..._a: unknown[]) => {});
const saveProjectApi = vi.fn(async (..._a: unknown[]) => null); // Tauri answers null for a command that returns nothing
const restoreApi = vi.fn(async (..._a: unknown[]) => ({ uiState: JSON.stringify({ version: 2, maps: [] }), layers: [], origin: "/work/Madera.gproj" as string | null }));
vi.mock("../ipc/api", () => ({
  api: {
    writeRecovery: (...a: unknown[]) => (calls.push("write"), writeRecovery(...a)),
    discardOwnRecovery: () => (calls.push("clear"), discardOwn()),
    discardRecovery: (...a: unknown[]) => discardRecovery(...a),
    restoreRecovery: (...a: unknown[]) => restoreApi(...a),
    saveProject: (...a: unknown[]) => saveProjectApi(...a),
    listLayouts: vi.fn(async () => []),
    sources: vi.fn(async () => []),
    getSettings: vi.fn(async () => ({})),
  },
  errorText: (e: unknown) => (e instanceof Error ? e.message : String(e)),
  IN_TAURI: false,
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(async () => null), open: vi.fn(async () => null) }));

import { autosaveProject, restoreRecovery, saveProject } from "../actions";
import { useApp } from "../store/store";
import { AUTOSAVE_CHOICES, autosaveEveryMinutes, describeEntry } from "./format";

beforeEach(() => {
  calls.length = 0;
  writeRecovery.mockClear();
  discardOwn.mockClear();
  discardRecovery.mockClear();
  saveProjectApi.mockClear();
  useApp.setState({ dirty: false, projectDir: null });
});

describe("autosaveProject", () => {
  it("does nothing when there is no unsaved work", async () => {
    expect(await autosaveProject()).toBe("skipped");
    expect(writeRecovery).not.toHaveBeenCalled();
  });

  it("writes the recovery copy, leaves the project marked unsaved, and never calls Save", async () => {
    useApp.setState({ dirty: true, projectDir: "/work/Madera.gproj" });
    expect(await autosaveProject()).toBe("saved");
    expect(writeRecovery).toHaveBeenCalledOnce();
    const [ui, origin, name] = writeRecovery.mock.calls[0] as [string, string | null, string];
    expect(JSON.parse(ui).version).toBe(2);
    expect(origin).toBe("/work/Madera.gproj");
    expect(name).toBe("Madera");
    expect(useApp.getState().dirty).toBe(true);
    expect(useApp.getState().projectDir).toBe("/work/Madera.gproj");
    expect(saveProjectApi).not.toHaveBeenCalled();
  });

  it("names a project that was never saved, and sends no folder", async () => {
    useApp.setState({ dirty: true, projectDir: null });
    await autosaveProject();
    const [, origin, name] = writeRecovery.mock.calls[0] as [string, string | null, string];
    expect(origin).toBeNull();
    expect(name).toBe("Untitled project");
  });

  it("removes the copy again when the user saved while it was being written", async () => {
    useApp.setState({ dirty: true });
    writeRecovery.mockImplementationOnce(async () => {
      useApp.setState({ dirty: false });
    });
    await autosaveProject();
    expect(calls).toEqual(["write", "clear"]);
  });

  it("reports a failure as the engine's sentence, once, and never throws", async () => {
    useApp.setState({ dirty: true, status: "" });
    writeRecovery.mockRejectedValue(new Error("The recovery copy could not be written: disk full. Your project is unchanged."));
    expect(await autosaveProject()).toMatch(/disk full/);
    expect(useApp.getState().status).toMatch(/Autosave: The recovery copy could not be written/);
    useApp.setState({ status: "later" });
    await autosaveProject();
    expect(useApp.getState().status).toBe("later"); // same message: not repeated every try
    writeRecovery.mockResolvedValue(undefined);
  });
});

describe("a clean save", () => {
  it("removes this run's recovery copy", async () => {
    useApp.setState({ dirty: true, projectDir: "/work/Madera.gproj" });
    await saveProject();
    expect(saveProjectApi).toHaveBeenCalled();
    expect(useApp.getState().dirty).toBe(false);
    expect(calls).toContain("clear");
  });
});

describe("restoreRecovery", () => {
  it("brings the work back unsaved and tied to no folder, then removes the old copy once the new one is written", async () => {
    useApp.setState({ dirty: false, projectDir: "/somewhere" });
    expect(await restoreRecovery("s1-abc")).toBe(true);
    expect(useApp.getState().dirty).toBe(true);
    expect(useApp.getState().projectDir).toBeNull();
    expect(useApp.getState().status).toMatch(/Madera\.gproj.*not changed/);
    expect(writeRecovery).toHaveBeenCalled();
    expect(discardRecovery).toHaveBeenCalledWith("s1-abc");
  });

  it("keeps the old copy when the new one can't be written", async () => {
    writeRecovery.mockRejectedValueOnce(new Error("no space"));
    expect(await restoreRecovery("s1-abc")).toBe(true);
    expect(discardRecovery).not.toHaveBeenCalled();
  });

  it("changes nothing when the engine refuses a damaged copy", async () => {
    useApp.setState({ dirty: false, projectDir: "/somewhere" });
    restoreApi.mockRejectedValueOnce(new Error("This recovery copy can't be used: its files are incomplete or damaged."));
    expect(await restoreRecovery("s1-bad")).toBe(false);
    expect(useApp.getState().projectDir).toBe("/somewhere");
    expect(discardRecovery).not.toHaveBeenCalled();
  });
});

describe("wording and interval", () => {
  it("defaults to every 2 minutes, 0 is off, odd values are kept sane", () => {
    expect(autosaveEveryMinutes(undefined)).toBe(2);
    expect(autosaveEveryMinutes(null)).toBe(2);
    expect(autosaveEveryMinutes(0)).toBe(0);
    expect(autosaveEveryMinutes(5)).toBe(5);
    expect(autosaveEveryMinutes(-3)).toBe(1);
    expect(autosaveEveryMinutes(9999)).toBe(120);
    expect(AUTOSAVE_CHOICES.map(([n]) => n)).toContain(autosaveEveryMinutes(undefined));
  });

  it("describes a copy in a sentence, and shows a damaged one's problem", () => {
    const now = 1_000_000_000_000;
    const base = { id: "s1", name: "Madera", origin: "/work/Madera.gproj", writtenAt: now / 1000 - 600, problem: null };
    expect(describeEntry(base, now)).toBe("Madera, last saved automatically 10 minutes ago, from /work/Madera.gproj");
    expect(describeEntry({ ...base, origin: null }, now)).toMatch(/never saved to a folder/);
    expect(describeEntry({ ...base, problem: "This recovery copy can't be used: x." }, now)).toBe("This recovery copy can't be used: x.");
  });
});
