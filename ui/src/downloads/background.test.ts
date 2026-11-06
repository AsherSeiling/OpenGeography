import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({ api: { cancelTask: vi.fn(async () => {}) }, errorText: (e: unknown) => String(e instanceof Error ? e.message : e), IN_TAURI: false }));
const reported: unknown[] = [];
vi.mock("../errors/store", () => ({ reportError: (e: unknown) => reported.push(e) }));

import { api } from "../ipc/api";
import { useProgress } from "../progress/progress";
import { useApp } from "../store/store";
import { cancelDownloads, confirmDownloads, downloadsRunning, startDownload, useDownloads } from "./background";

beforeEach(() => {
  useDownloads.setState({ jobs: [] });
  useProgress.setState({ tasks: [], pending: 0 });
  reported.length = 0;
});
afterEach(() => vi.unstubAllGlobals());

const gate = () => {
  let open!: (v?: unknown) => void;
  const p = new Promise((r) => (open = r));
  return { p, open };
};

describe("UR-9 background downloads", () => {
  it("returns at once, counts as running until it ends, then hands the result over and reports it", async () => {
    const g = gate();
    const done = vi.fn(() => "Added 3 layers");
    startDownload({ label: "Test download", match: ["Downloading"], run: async () => (await g.p, 3), done });
    expect(downloadsRunning()).toBe(true);
    expect(useApp.getState().status).toContain("runs in the background");
    g.open();
    await vi.waitFor(() => expect(downloadsRunning()).toBe(false));
    expect(done).toHaveBeenCalledWith(3);
    expect(useApp.getState().status).toBe("Added 3 layers");
  });

  it("a failure is shown, not thrown, and a cancel is just a status", async () => {
    startDownload({ label: "Bad one", match: [], run: async () => { throw new Error("server said no"); }, done: () => {} });
    await vi.waitFor(() => expect(downloadsRunning()).toBe(false));
    expect(useApp.getState().status).toBe("Bad one failed: server said no");
    expect(reported).toHaveLength(1);
    startDownload({ label: "Stopped one", match: [], run: async () => { throw new Error("Cancelled"); }, done: () => {} });
    await vi.waitFor(() => expect(downloadsRunning()).toBe(false));
    expect(useApp.getState().status).toBe("Stopped one cancelled");
    expect(reported).toHaveLength(1);
  });

  it("with nothing running nothing is asked; with a download running Save and leaving ask, and No keeps everything", async () => {
    const ask = vi.fn(() => false);
    vi.stubGlobal("confirm", ask);
    expect(await confirmDownloads("save")).toBe(true);
    expect(ask).not.toHaveBeenCalled();
    const g = gate();
    startDownload({ label: "Slow", match: ["Downloading"], run: async () => g.p, done: () => {} });
    expect(await confirmDownloads("save")).toBe(false);
    expect(await confirmDownloads("leave")).toBe(false);
    expect(ask).toHaveBeenCalledTimes(2);
    expect(String(ask.mock.calls[0])).toContain("Slow");
    expect(downloadsRunning()).toBe(true);
    g.open();
    await vi.waitFor(() => expect(downloadsRunning()).toBe(false));
  });

  it("answering Yes to leaving cancels the engine task of the download and waits for it to end", async () => {
    vi.stubGlobal("confirm", () => true);
    const g = gate();
    startDownload({ label: "Slow", match: ["Downloading from X"], run: async () => g.p, done: () => {} });
    useProgress.setState({ tasks: [{ id: 7, label: "Downloading from X for map view", fraction: 0.2, detail: "", cancellable: true }, { id: 8, label: "Something else", fraction: 0, detail: "", cancellable: true }] as never });
    (api.cancelTask as ReturnType<typeof vi.fn>).mockImplementation(async () => g.open());
    expect(await confirmDownloads("leave")).toBe(true);
    expect(api.cancelTask).toHaveBeenCalledTimes(1);
    expect(api.cancelTask).toHaveBeenCalledWith(7);
    expect(downloadsRunning()).toBe(false);
    await cancelDownloads(); // nothing left: returns
  });
});
