import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const saves: { name: string; cards: number; at: number }[] = [];
let slow = 0;
let deleted: string[] = [];
vi.mock("../ipc/api", () => ({
  api: {
    saveStatSheet: vi.fn(async (s: { name: string; cards: unknown[] }) => {
      const at = saves.length;
      if (slow) await new Promise((r) => setTimeout(r, slow));
      saves.push({ name: s.name, cards: s.cards.length, at });
    }),
    deleteStatSheet: vi.fn(async (id: string) => {
      deleted.push(id);
    }),
  },
  errorText: (e: unknown) => String(e),
}));
vi.mock("../errors/store", () => ({ reportError: vi.fn() }));

import { cardState, staleSentence } from "./staleness";
import { flushStatSaves, newSheetDoc, useStats } from "./store";
import type { StatCard, StatResult } from "./types";

const result = (versions: Record<string, number>): StatResult => ({ method: "m", citations: [], n_used: 4, dropped: [], tables: [], charts: [], notes: [], seed: null, data_version: versions });
const card = (over: Partial<StatCard> = {}): StatCard => ({ id: "c1", tool: "summary_statistics", title: null, inputs: { in: { layer: "L1" } }, options: { field: "pop" }, result: null, ...over });

describe("out-of-date marks", () => {
  it("a card that never ran is not run, never current", () => {
    expect(cardState(card(), { L1: { version: 1 } }).state).toBe("not-run");
  });
  it("is current while every input layer is at the version it ran on", () => {
    expect(cardState(card({ result: result({ L1: 3 }) }), { L1: { version: 3 } }).state).toBe("current");
  });
  it("is out of date as soon as a layer's version differs, and says which", () => {
    const s = cardState(card({ result: result({ L1: 3 }) }), { L1: { version: 4 } });
    expect(s.state).toBe("out-of-date");
    expect(s.changed).toEqual(["L1"]);
    expect(staleSentence(s, () => "Parcels")).toContain("Parcels changed since this ran");
  });
  it("is out of date when the layer is gone", () => {
    const s = cardState(card({ result: result({ L1: 3 }) }), {});
    expect(s.state).toBe("out-of-date");
    expect(s.missing).toEqual(["L1"]);
  });
  it("is out of date when an input names a layer the result was not computed from", () => {
    const s = cardState(card({ inputs: { in: { layer: "L2" } }, result: result({ L1: 3 }) }), { L1: { version: 3 }, L2: { version: 1 } });
    expect(s.state).toBe("out-of-date");
  });
});

describe("sheet store", () => {
  beforeEach(() => {
    saves.length = 0;
    deleted = [];
    slow = 0;
    useStats.getState().loadAll([]);
  });
  afterEach(async () => {
    await flushStatSaves();
  });

  it("adds a sheet and a card, and saves them", async () => {
    const sh = newSheetDoc("Sheet A");
    useStats.getState().add(sh);
    useStats.getState().addCard(sh.id, card());
    expect(useStats.getState().selected).toBe("c1");
    await flushStatSaves();
    expect(saves.at(-1)).toMatchObject({ name: "Sheet A", cards: 1 });
  });

  it("editing a card's settings clears its result; editing only the title keeps it", () => {
    const sh = newSheetDoc("S");
    useStats.getState().add(sh);
    useStats.getState().addCard(sh.id, card({ result: result({ L1: 1 }) }));
    useStats.getState().editCard(sh.id, "c1", { title: "My card" });
    expect(useStats.getState().docs[sh.id].cards[0].result).not.toBeNull();
    useStats.getState().editCard(sh.id, "c1", { options: { field: "other" } });
    expect(useStats.getState().docs[sh.id].cards[0].result).toBeNull();
  });

  it("moves and removes cards", () => {
    const sh = newSheetDoc("S");
    useStats.getState().add(sh);
    for (const id of ["a", "b", "c"]) useStats.getState().addCard(sh.id, card({ id }));
    useStats.getState().moveCard(sh.id, "c", -1);
    expect(useStats.getState().docs[sh.id].cards.map((c) => c.id)).toEqual(["a", "c", "b"]);
    useStats.getState().moveCard(sh.id, "a", -1); // already first: nothing
    expect(useStats.getState().docs[sh.id].cards.map((c) => c.id)).toEqual(["a", "c", "b"]);
    useStats.getState().removeCard(sh.id, "c");
    expect(useStats.getState().docs[sh.id].cards.map((c) => c.id)).toEqual(["a", "b"]);
  });

  it("saves one at a time and last-writer-wins: a slow save is never overtaken by a later one", async () => {
    const sh = newSheetDoc("v0");
    slow = 20;
    useStats.getState().add(sh);
    const p1 = flushStatSaves();
    useStats.getState().rename(sh.id, "v1");
    const p2 = flushStatSaves();
    useStats.getState().rename(sh.id, "v2");
    await Promise.all([p2, flushStatSaves(), p1]);
    expect(saves.at(-1)?.name).toBe("v2");
    // each save finished before the next began (their start order is their finish order)
    expect(saves.map((s) => s.at)).toEqual(saves.map((_, i) => i));
  });

  it("a deleted sheet is not brought back by a save still waiting", async () => {
    const sh = newSheetDoc("gone");
    useStats.getState().add(sh);
    useStats.getState().remove(sh.id);
    await flushStatSaves();
    expect(saves.length).toBe(0);
    expect(deleted).toEqual([sh.id]);
    expect(useStats.getState().docs[sh.id]).toBeUndefined();
  });

  it("opening a sheet that does not exist opens nothing", () => {
    useStats.getState().open("nope");
    expect(useStats.getState().id).toBeNull();
  });
});
