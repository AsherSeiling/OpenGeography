import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({
  api: { saveLayout: vi.fn(async () => {}), deleteLayout: vi.fn(async () => {}), listLayouts: vi.fn(async () => []) },
  errorText: (e: unknown) => String(e),
  IN_TAURI: false,
}));

import { DEFAULT_STYLES, textMatches, textPatch, useNamedStyles, type NamedStyle } from "../layout/namedStyles";
import { newLayoutDoc, normalizeLayout, type Layout, type TextElement } from "../layout/types";
import { useLayouts } from "./layout";

const L = useLayouts;
const TITLE: NamedStyle = { name: "Title", size: 30, bold: true, color: "#112233" };

beforeEach(() => {
  const mem = new Map<string, string>();
  vi.stubGlobal("localStorage", { getItem: (k: string) => mem.get(k) ?? null, setItem: (k: string, v: string) => void mem.set(k, v), removeItem: (k: string) => void mem.delete(k), clear: () => mem.clear() });
  useNamedStyles.setState({ byLayout: {} });
});

describe("LE-1 named text styles are saved in the layout", () => {
  it("a layout's own list is used, and an old layout (no list) gets the defaults and is not given a list", () => {
    const withList = { ...newLayoutDoc("a", "A"), namedStyles: [TITLE] };
    L.getState().loadAll([withList, newLayoutDoc("old", "Old")]);
    expect(useNamedStyles.getState().get("a")).toEqual([TITLE]);
    expect(useNamedStyles.getState().get("old")).toEqual(DEFAULT_STYLES);
    expect(L.getState().docs.old.namedStyles).toBeUndefined();
    expect("namedStyles" in JSON.parse(JSON.stringify(L.getState().docs.old))).toBe(false); // an old layout saves with no new key
  });

  it("styles kept in this browser profile are adopted into the layout and the stored copy is left alone", () => {
    localStorage.setItem("gis.textStyles.old", JSON.stringify([TITLE]));
    L.getState().loadAll([newLayoutDoc("old", "Old")]);
    expect(L.getState().docs.old.namedStyles).toEqual([TITLE]);
    expect(useNamedStyles.getState().get("old")).toEqual([TITLE]);
    expect(JSON.parse(localStorage.getItem("gis.textStyles.old")!)).toEqual([TITLE]); // never deleted
  });

  it("the project's list wins over the browser's", () => {
    localStorage.setItem("gis.textStyles.a", JSON.stringify([{ name: "Other", size: 5 }]));
    L.getState().loadAll([{ ...newLayoutDoc("a", "A"), namedStyles: [TITLE] }]);
    expect(L.getState().docs.a.namedStyles).toEqual([TITLE]);
  });

  it("editing styles writes them into the open layout, as one undo step with the elements they restyle", () => {
    L.getState().loadAll([newLayoutDoc("a", "A")]);
    L.getState().open("a");
    useNamedStyles.getState().set("a", [TITLE]);
    expect(L.getState().doc!.namedStyles).toEqual([TITLE]);
    L.getState().undo();
    expect(L.getState().doc!.namedStyles).toBeUndefined();
  });

  it("a duplicate of a layout keeps the styles", () => {
    L.getState().loadAll([{ ...newLayoutDoc("a", "A"), namedStyles: [TITLE] }]);
    const copy = L.getState().duplicate("a")!;
    expect(useNamedStyles.getState().get(copy.id)).toEqual([TITLE]);
  });

  it("malformed entries are dropped; a reference to a missing style leaves the look alone", () => {
    const raw = { ...newLayoutDoc("a", "A"), namedStyles: [TITLE, { name: 3 }, { name: "Huge", size: 9000 }] } as unknown as Layout;
    expect(normalizeLayout(raw).namedStyles).toEqual([TITLE]);
    const el = { kind: "text", id: "t", x: 0, y: 0, w: 50, h: 10, text: "x", font: "sans-bold", size: 21, align: "left", color: "#000000", styleName: "Gone" } as TextElement;
    const norm = normalizeLayout({ ...newLayoutDoc("a", "A"), elements: [el] });
    const t = norm.elements[0] as TextElement;
    expect([t.font, t.size, t.color, t.styleName]).toEqual(["sans-bold", 21, "#000000", "Gone"]);
    expect(textMatches(t, DEFAULT_STYLES[0])).toBe(true); // it still reads as the matching style by look
  });

  it("applying a style links it", () => {
    expect(textPatch(TITLE).styleName).toBe("Title");
  });
});
