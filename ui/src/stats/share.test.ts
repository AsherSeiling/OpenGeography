// ST7 sharing helpers: file names, numbered files for a card with several tables or charts, and putting a card on a layout (open or saved) without
// touching the layout's other elements.
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/api", () => ({ api: { saveLayout: vi.fn(async () => {}), saveStatSheet: vi.fn(async () => {}) }, errorText: (e: unknown) => String(e), IN_TAURI: false }));
vi.mock("../errors/store", () => ({ reportError: vi.fn() }));
vi.mock("../layout/actions", () => ({ newLayout: vi.fn(), layoutRequest: vi.fn(), enterWorkspace: vi.fn(), leaveWorkspace: vi.fn() }));
vi.mock("../ipc/dialogs", () => ({ save: vi.fn(async () => null), open: vi.fn(async () => null) }));

import { addStatCardToLayout, useLayouts } from "../store/layout";
import { newLayoutDoc } from "../layout/types";
import { fileStem, numberedPath } from "./share";

describe("file names", () => {
  it("removes characters a file system refuses and never returns an empty name", () => {
    expect(fileStem('Parcels: "area" / count?')).toBe("Parcels area count");
    expect(fileStem("  ...  ")).toBe("statistics");
    expect(fileStem("a".repeat(200)).length).toBe(80);
  });
  it("numbers the second and later files of one card before the extension", () => {
    expect(numberedPath("/x/out.csv", 0)).toBe("/x/out.csv");
    expect(numberedPath("/x/out.csv", 1)).toBe("/x/out-2.csv");
    expect(numberedPath("C:\\x\\out.png", 2)).toBe("C:\\x\\out-3.png");
    expect(numberedPath("/x.y/out", 1)).toBe("/x.y/out-2");
  });
});

describe("sending a card to a layout", () => {
  beforeEach(() => {
    useLayouts.getState().loadAll([newLayoutDoc("L1", "Layout 1"), newLayoutDoc("L2", "Layout 2")]);
  });

  it("adds one element to a saved layout that is not open, and leaves the others and the elements already there alone", () => {
    const before = JSON.stringify(useLayouts.getState().docs.L2);
    const id = addStatCardToLayout("L2", { sheet: "S1", card: "c1", part: "chart" });
    expect(id).toBeTruthy();
    const after = useLayouts.getState().docs;
    expect(after.L2.elements.length).toBe(JSON.parse(before).elements.length + 1);
    expect(after.L2.elements.at(-1)).toMatchObject({ kind: "stat_card", sheet: "S1", card: "c1", visible: true, locked: false });
    expect(after.L2.elements.at(-1)).not.toHaveProperty("part"); // a chart is the default; old and new files stay small
    expect(after.L1.elements).toEqual([]);
  });

  it("adds to the open layout through the editor, so it can be undone, and selects the new element", () => {
    useLayouts.getState().open("L1");
    const id = addStatCardToLayout("L1", { sheet: "S1", card: "c1", part: "table" })!;
    const s = useLayouts.getState();
    expect(s.doc!.elements.at(-1)).toMatchObject({ id, kind: "stat_card", part: "table" });
    expect(s.selected).toEqual([id]);
    useLayouts.getState().undo();
    expect(useLayouts.getState().doc!.elements.length).toBe(0);
  });

  it("gives each element its own id and refuses a layout that is gone", () => {
    const a = addStatCardToLayout("L2", { sheet: "S1", card: "c1", part: "chart" });
    const b = addStatCardToLayout("L2", { sheet: "S1", card: "c1", part: "chart" });
    expect(a).not.toBe(b);
    expect(addStatCardToLayout("nope", { sheet: "S1", card: "c1", part: "chart" })).toBeNull();
  });
});
