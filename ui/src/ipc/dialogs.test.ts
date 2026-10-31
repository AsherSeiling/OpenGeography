import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(async () => {
    throw new TypeError("Cannot read properties of undefined (reading 'invoke')");
  }),
  open: vi.fn(async () => {
    throw new TypeError("Cannot read properties of undefined (reading 'invoke')");
  }),
}));

import { open, save } from "./dialogs";

afterEach(() => vi.unstubAllGlobals());

describe("file dialogs in a browser", () => {
  it("ask for a path when the native dialog is not there", async () => {
    vi.stubGlobal("window", { prompt: vi.fn(() => "/tmp/out.gpkg") });
    expect(await save({ title: "Export", defaultPath: "x.gpkg" })).toBe("/tmp/out.gpkg");
    expect(await open({ multiple: true })).toEqual(["/tmp/out.gpkg"]);
    expect(await open({})).toBe("/tmp/out.gpkg");
  });
  it("a cancelled prompt is a cancelled dialog", async () => {
    vi.stubGlobal("window", { prompt: vi.fn(() => null) });
    expect(await save()).toBeNull();
    expect(await open()).toBeNull();
  });
});
