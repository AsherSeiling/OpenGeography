import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []), layoutSeriesPages: vi.fn() },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { api } from "../ipc/api";
import { useLayouts } from "../store/layout";
import { pageCaption, parseMargin, parseSeriesScale, useSeriesView, withSeries } from "./series";
import { newLayoutDoc, type SeriesSpec } from "./types";

const pages = (...names: string[]) => names.map((name, index) => ({ index, fid: index + 1, name }));
const spec: SeriesSpec = { indexLayer: "L1", nameField: "NAME", extent: "fit", marginPct: 10 };

beforeEach(() => {
  useSeriesView.setState({ pages: [], error: null, page: null });
  useLayouts.getState().loadAll([newLayoutDoc("a", "A")]);
  useLayouts.getState().open("a");
  vi.clearAllMocks();
});

describe("map series in the UI (C3.2)", () => {
  it("parses typed margins and scales", () => {
    expect(parseMargin("10")).toBe(10);
    expect(parseMargin(" 7.5 % ")).toBe(7.5);
    expect(parseMargin("0")).toBe(0);
    expect(parseMargin("50")).toBe(50);
    expect(parseMargin("51")).toBeNull();
    expect(parseMargin("-1")).toBeNull();
    expect(parseMargin("")).toBeNull();
    expect(parseMargin("x")).toBeNull();
    expect(parseSeriesScale("25000")).toBe(25000);
    expect(parseSeriesScale("1:25,000")).toBe(25000);
    expect(parseSeriesScale("0")).toBeNull();
    expect(parseSeriesScale("")).toBeNull();
    expect(parseSeriesScale("abc")).toBeNull();
  });

  it("captions the page being shown", () => {
    expect(pageCaption(null, [])).toBe("No pages");
    expect(pageCaption(null, pages("a", "b"))).toBe("2 pages");
    expect(pageCaption(null, pages("a"))).toBe("1 page");
    expect(pageCaption(1, pages("Dane", "Lane"))).toBe("Page 2 of 2 — Lane");
    expect(pageCaption(0, pages(""))).toBe("Page 1 of 1 — (no name)");
  });

  it("builds a series from defaults and patches", () => {
    expect(withSeries(null, { indexLayer: "L2" }, "NAME")).toEqual({ indexLayer: "L2", nameField: "NAME", extent: "fit", marginPct: 10 });
    expect(withSeries(spec, { extent: "fixed_scale", scale: 5000 }, "x")).toEqual({ ...spec, extent: "fixed_scale", scale: 5000 });
  });

  it("steps through the pages and stays inside them", () => {
    useSeriesView.setState({ pages: pages("a", "b", "c") });
    const v = useSeriesView.getState;
    v().show(1);
    expect(v().page).toBe(1);
    v().show(99);
    expect(v().page).toBe(2);
    v().show(-5);
    expect(v().page).toBe(0);
    v().show(null);
    expect(v().page).toBeNull();
  });

  it("refresh asks the engine for the pages, keeps the shown page in range, and clears when there is no series", async () => {
    const layout = { ...newLayoutDoc("a", "A"), series: spec };
    vi.mocked(api.layoutSeriesPages).mockResolvedValueOnce(pages("a", "b", "c"));
    useSeriesView.setState({ page: 2 });
    await useSeriesView.getState().refresh(layout);
    expect(useSeriesView.getState()).toMatchObject({ error: null, page: 2 });
    expect(useSeriesView.getState().pages).toHaveLength(3);
    vi.mocked(api.layoutSeriesPages).mockResolvedValueOnce(pages("a"));
    await useSeriesView.getState().refresh(layout);
    expect(useSeriesView.getState().page).toBe(0); // one page left
    vi.mocked(api.layoutSeriesPages).mockRejectedValueOnce("'Counties' has no features to make pages from");
    await useSeriesView.getState().refresh(layout);
    expect(useSeriesView.getState()).toMatchObject({ pages: [], page: null, error: "'Counties' has no features to make pages from" });
    await useSeriesView.getState().refresh({ ...layout, series: null });
    expect(useSeriesView.getState()).toMatchObject({ pages: [], page: null, error: null });
  });

  it("a stale answer never overwrites a newer one", async () => {
    const layout = { ...newLayoutDoc("a", "A"), series: spec };
    let release: (p: ReturnType<typeof pages>) => void = () => {};
    vi.mocked(api.layoutSeriesPages).mockImplementationOnce(() => new Promise((r) => (release = r)));
    const slow = useSeriesView.getState().refresh(layout);
    vi.mocked(api.layoutSeriesPages).mockResolvedValueOnce(pages("new"));
    await useSeriesView.getState().refresh(layout);
    release(pages("old1", "old2"));
    await slow;
    expect(useSeriesView.getState().pages.map((p) => p.name)).toEqual(["new"]);
  });

  it("setSeries is one undo step", () => {
    const s = useLayouts.getState;
    s().setSeries(spec);
    expect(s().doc!.series).toEqual(spec);
    s().undo();
    expect(s().doc!.series).toBeNull();
    s().redo();
    expect(s().doc!.series).toEqual(spec);
    s().setSeries(null);
    expect(s().doc!.series).toBeNull();
  });
});
