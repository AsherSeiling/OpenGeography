import { describe, expect, it } from "vitest";
import { cloneElements, freshIds } from "./clipboard";
import { newLayoutDoc, type Layout, type LayoutElement } from "./types";

const base = { rotation: 0, locked: true, visible: true, w: 10, h: 10 };
const mk = (kind: string, id: string, x: number, over: Record<string, unknown> = {}) => ({ ...base, kind, id, x, y: 1, ...over }) as unknown as LayoutElement;
const doc = (...elements: LayoutElement[]): Layout => ({ ...newLayoutDoc("L1", "T", "Letter", "landscape"), elements });

describe("clipboard", () => {
  it("picks ids that are not used", () => {
    const d = doc(mk("map_frame", "f1", 0), mk("map_frame", "f2", 0));
    expect(freshIds(d, ["map_frame", "map_frame", "legend"])).toEqual(["f3", "f4", "lg1"]);
  });
  it("moves copies, unlocks them and points a copied legend at the copied frame", () => {
    const f = mk("map_frame", "f1", 5);
    const lg = mk("legend", "lg1", 20, { frame: "f1", group: "g1" });
    const sb = mk("scale_bar", "sb1", 30, { frame: "f1", group: "g1" });
    const d = doc(f, lg, sb);
    const [cf, cl, cs] = cloneElements(d, [f, lg, sb], 5) as unknown as { id: string; x: number; y: number; locked: boolean; frame?: string; group?: string }[];
    expect([cf.id, cl.id, cs.id]).toEqual(["f2", "lg2", "sb2"]);
    expect(cf.x).toBe(10);
    expect(cf.y).toBe(6);
    expect(cf.locked).toBe(false);
    expect(cl.frame).toBe("f2");
    expect(cl.group).toBe(cs.group);
    expect(cl.group).not.toBe("g1");
  });
  it("keeps a legend pointing at its frame when only the legend is copied", () => {
    const lg = mk("legend", "lg1", 20, { frame: "f1" });
    const [c] = cloneElements(doc(mk("map_frame", "f1", 0), lg), [lg], 5) as unknown as { frame: string }[];
    expect(c.frame).toBe("f1");
  });
});
