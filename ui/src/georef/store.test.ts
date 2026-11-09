import { beforeEach, describe, expect, it } from "vitest";
import { addPoint, errorOf, fitKey, fmtErr, parseCell, pointsShort, removePoint, summaryLine, updatePoint, useGeoref } from "./store";
import { METHODS, newControlSet, type ErrorReport } from "./types";
import { groundMarkers, imageMarkers } from "./maps";

const base = () => ({ ...newControlSet("A", null), targetCrs: "EPSG:26914" });

describe("control point edits", () => {
  it("gives each new point the next free id and starts it enabled", () => {
    let s = base();
    s = addPoint(s, { pixel: 1, line: 2, x: 3, y: 4 });
    s = addPoint(s, { pixel: 5, line: 6, x: 7, y: 8 });
    s = removePoint(s, 1);
    s = addPoint(s, { pixel: 9, line: 9, x: 9, y: 9 });
    expect(s.points.map((p) => p.id)).toEqual([2, 3]);
    expect(s.points.every((p) => p.enabled)).toBe(true);
  });

  it("turns a point off without losing it and edits only that point", () => {
    let s = addPoint(addPoint(base(), { pixel: 0, line: 0, x: 0, y: 0 }), { pixel: 1, line: 1, x: 1, y: 1 });
    s = updatePoint(s, 2, { enabled: false, x: 10 });
    expect(s.points[0]).toMatchObject({ id: 1, x: 0, enabled: true });
    expect(s.points[1]).toMatchObject({ id: 2, x: 10, enabled: false });
  });

  it("counts how many more enabled points a method needs", () => {
    let s = base();
    for (let i = 0; i < 4; i++) s = addPoint(s, { pixel: i, line: i * i, x: i, y: i });
    expect(pointsShort(s, "p1")).toBe(0);
    expect(pointsShort(s, "p2")).toBe(2);
    expect(pointsShort(s, "p3")).toBe(6);
    s = updatePoint(s, 1, { enabled: false });
    expect(pointsShort(s, "p1")).toBe(0);
    expect(pointsShort(s, "tps")).toBe(0);
    s = updatePoint(s, 2, { enabled: false });
    expect(pointsShort(s, "tps")).toBe(1);
    expect(METHODS.map((m) => m.min)).toEqual([3, 6, 10, 3]);
  });

  it("changes the fit key when what the fit uses changes, and not otherwise", () => {
    const s = addPoint(base(), { pixel: 1, line: 2, x: 3, y: 4 });
    const k = fitKey(s, "p1", "nearest", "/a.png");
    expect(fitKey({ ...s, name: "other name" }, "p1", "nearest", "/a.png")).toBe(k);
    expect(fitKey(updatePoint(s, 1, { enabled: false }), "p1", "nearest", "/a.png")).not.toBe(k);
    expect(fitKey(s, "p2", "nearest", "/a.png")).not.toBe(k);
    expect(fitKey(s, "p1", "bilinear", "/a.png")).not.toBe(k);
    expect(fitKey({ ...s, targetCrs: "EPSG:4326" }, "p1", "nearest", "/a.png")).not.toBe(k);
  });
});

describe("typed cells and figures", () => {
  it("accepts numbers with thousands separators and rejects the rest", () => {
    expect(parseCell(" 600,000.5 ")).toBe(600000.5);
    expect(parseCell("-1e3")).toBe(-1000);
    expect(parseCell("")).toBeNull();
    expect(parseCell("abc")).toBeNull();
    expect(parseCell("Infinity")).toBeNull();
  });

  it("shows zero as 0 (not a dash) and a missing figure as a dash", () => {
    expect(fmtErr(0, "m")).toBe("0");
    expect(fmtErr(null, "m")).toBe("–");
    expect(fmtErr(0.12345, "m")).toBe("0.123");
    expect(fmtErr(12.345, "m")).toBe("12.35");
    expect(fmtErr(1234.56, "m")).toBe("1234.6");
  });

  it("writes the summary with the check-point figure only when there are check points", () => {
    const r = { rmsM: 1.2, rmsPx: 0.5, enabledCount: 8, checkRmsM: null, checkRmsPx: null, points: [{ id: 3, check: true }] } as unknown as ErrorReport;
    expect(summaryLine(r)).toBe("RMS 1.20 m · 0.500 px (8 points in the fit)");
    expect(summaryLine({ ...r, checkRmsM: 4.5, checkRmsPx: 2 })).toContain("check points 4.50 m · 2.00 px");
    expect(errorOf(r, 3)?.id).toBe(3);
    expect(errorOf(r, 9)).toBeUndefined();
  });
});

describe("the window's state", () => {
  beforeEach(() => useGeoref.setState({ set: base(), pending: {}, modified: false, savedId: null, selected: null }));

  it("makes a point only when both halves are picked, in either order", () => {
    const st = useGeoref.getState();
    st.setPending({ pixel: 10, line: 20 });
    expect(useGeoref.getState().commitPending()).toBe(false);
    useGeoref.getState().setPending({ ...useGeoref.getState().pending, x: 600000, y: 3900000 });
    expect(useGeoref.getState().commitPending()).toBe(true);
    const s = useGeoref.getState();
    expect(s.set.points).toEqual([{ id: 1, enabled: true, pixel: 10, line: 20, x: 600000, y: 3900000 }]);
    expect(s.pending).toEqual({});
    expect(s.modified).toBe(true);
    s.setPending({ x: 1, y: 2 });
    expect(useGeoref.getState().commitPending()).toBe(false);
  });

  it("a loaded saved set is not marked as modified, a new one is", () => {
    const set = { ...base(), id: "x" };
    useGeoref.getState().setSet(set, true);
    expect(useGeoref.getState()).toMatchObject({ savedId: "x", modified: false });
    useGeoref.getState().edit((s) => ({ ...s, name: "B" }));
    expect(useGeoref.getState().modified).toBe(true);
    useGeoref.getState().setSet(set, false);
    expect(useGeoref.getState()).toMatchObject({ savedId: null, modified: true });
  });
});

describe("markers", () => {
  const pts = [
    { id: 1, pixel: 10, line: 20, x: 600000, y: 3900000, enabled: true },
    { id: 2, pixel: 30, line: 40, x: 600300, y: 3899800, enabled: false },
  ];
  it("puts image points at (pixel, -line) so the image is upright, and ground points at (x, y)", () => {
    expect(imageMarkers(pts, null, {}).map((m) => m.at)).toEqual([[10, -20], [30, -40]]);
    expect(groundMarkers(pts, null, {}).map((m) => m.at)).toEqual([[600000, 3900000], [600300, 3899800]]);
  });
  it("shows a half-made point on the map where it was clicked", () => {
    expect(imageMarkers([], null, { pixel: 5, line: 6 })).toEqual([{ id: "pending", enabled: true, at: [5, -6], selected: false }]);
    expect(groundMarkers([], 2, { x: 1, y: 2 })[0]).toMatchObject({ id: "pending", at: [1, 2] });
    expect(groundMarkers(pts, 2, {}).map((m) => m.selected)).toEqual([false, true]);
  });
});
