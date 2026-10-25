import { afterEach, describe, expect, it, vi } from "vitest";

// The keymap persists to localStorage; vitest runs in node, so give it a fake one per test.
function fakeStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => void (data[k] = v),
    removeItem: (k: string) => void delete data[k],
  };
}

async function fresh(storage: unknown) {
  vi.resetModules();
  vi.stubGlobal("localStorage", storage);
  return import("./keymap");
}

afterEach(() => vi.unstubAllGlobals());

describe("default bindings", () => {
  it("no two actions share a default combo, and every action has at least one", async () => {
    const { ACTIONS } = await fresh(fakeStorage());
    const seen = new Map<string, string>();
    for (const a of ACTIONS) {
      expect(a.defaults.length, a.id).toBeGreaterThan(0);
      for (const c of a.defaults) {
        expect(seen.get(c), `${c} bound to ${seen.get(c)} and ${a.id}`).toBeUndefined();
        seen.set(c, a.id);
      }
    }
  });

  it("action ids are unique and each has a label, group and description", async () => {
    const { ACTIONS } = await fresh(fakeStorage());
    expect(new Set(ACTIONS.map((a) => a.id)).size).toBe(ACTIONS.length);
    for (const a of ACTIONS) expect(a.label && a.group && a.description, a.id).toBeTruthy();
  });

  it("a default combo round-trips: building the event for it gives the combo back", async () => {
    const { ACTIONS, eventCombo } = await fresh(fakeStorage());
    const event = (combo: string) => {
      const parts = combo.split("+");
      const k = parts[parts.length - 1];
      const mods = parts.slice(0, -1);
      const code = /^[A-Z]$/.test(k) ? `Key${k}` : k === "." ? "Period" : k;
      return { key: k.length === 1 ? k.toLowerCase() : k, code, ctrlKey: mods.includes("Mod"), metaKey: false, altKey: mods.includes("Alt"), shiftKey: mods.includes("Shift") };
    };
    for (const a of ACTIONS) for (const c of a.defaults) expect(eventCombo(event(c), false), `${a.id} ${c}`).toBe(c);
  });
});

describe("combo handling", () => {
  it("lone modifiers and dead keys give no combo", async () => {
    const { eventCombo } = await fresh(fakeStorage());
    for (const key of ["Shift", "Control", "Alt", "Meta"]) expect(eventCombo({ key, code: `${key}Left`, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, false)).toBeNull();
  });

  it("digits, space and the numpad point are named", async () => {
    const { eventCombo } = await fresh(fakeStorage());
    const e = (key: string, code: string) => eventCombo({ key, code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false }, false);
    expect(e("1", "Digit1")).toBe("1");
    expect(e("!", "Digit1")).toBe("1");
    expect(e(" ", "Space")).toBe("Space");
    expect(e(".", "NumpadDecimal")).toBe(".");
    expect(e("Enter", "Enter")).toBe("Enter");
  });

  it("displayCombo spells modifiers per platform", async () => {
    const { displayCombo } = await fresh(fakeStorage());
    expect(displayCombo("Mod+Shift+Z", false)).toBe("Ctrl+Shift+Z");
    expect(displayCombo("Mod+Shift+Z", true)).toBe("⌘⇧Z");
    expect(displayCombo("Alt+A", true)).toBe("⌥A");
    expect(displayCombo("", false)).toBe("");
  });

  it("actionFor and conflicts find the owners of a combo", async () => {
    const { actionFor, conflicts, defaultBindings } = await fresh(fakeStorage());
    const b = defaultBindings();
    expect(actionFor(b, "Shift+A")).toBe("add");
    expect(actionFor(b, "Ctrl+Q")).toBeUndefined();
    expect(conflicts(b, "X")).toEqual(["delete"]);
    expect(conflicts(b, "X", "delete")).toEqual([]);
  });
});

describe("saved bindings", () => {
  it("rebinding is saved, steals the combo from its old owner and survives a reload", async () => {
    const store = fakeStorage();
    let km = await fresh(store);
    km.useKeymap.getState().setBinding("mute", ["X"]);
    const b = km.useKeymap.getState().bindings;
    expect(b.mute).toEqual(["X"]);
    expect(b.delete).toEqual(["Delete", "Backspace"]);
    km = await fresh(store);
    expect(km.useKeymap.getState().bindings.mute).toEqual(["X"]);
  });

  it("duplicate combos in one binding collapse; an empty binding unbinds", async () => {
    const km = await fresh(fakeStorage());
    km.useKeymap.getState().setBinding("run", ["F5", "F5"]);
    expect(km.useKeymap.getState().bindings.run).toEqual(["F5"]);
    km.useKeymap.getState().setBinding("run", []);
    expect(km.useKeymap.getState().bindings.run).toEqual([]);
  });

  it("reset of one action takes its defaults back from whoever bound them", async () => {
    const km = await fresh(fakeStorage());
    km.useKeymap.getState().setBinding("mute", ["X"]); // steals X from delete
    km.useKeymap.getState().reset("delete");
    const b = km.useKeymap.getState().bindings;
    expect(b.delete).toEqual(["X", "Delete", "Backspace"]);
    expect(b.mute).toEqual([]);
    km.useKeymap.getState().reset();
    expect(km.useKeymap.getState().bindings).toEqual(km.defaultBindings());
  });

  it.each([
    ["not json", "{{{"],
    ["wrong shape", '"just a string"'],
    ["null", "null"],
    ["non-string combos", '{"mute":[1,null,"Q"],"copy":"Mod+C","bogus":["Z"]}'],
  ])("corrupt stored bindings (%s) fall back to defaults per action", async (_n, raw) => {
    const km = await fresh(fakeStorage({ "gis-studio.nodeKeymap.v1": raw }));
    const b = km.useKeymap.getState().bindings;
    expect(b.add).toEqual(["Shift+A"]);
    expect(b.copy).toEqual(["Mod+C"]);
    expect(Object.keys(b).sort()).toEqual(Object.keys(km.defaultBindings()).sort());
    if (raw.includes("Q")) expect(b.mute).toEqual(["Q"]);
  });

  it("storage that throws (private window) still works for the session", async () => {
    const angry = { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); } };
    const km = await fresh(angry);
    expect(() => km.useKeymap.getState().setBinding("add", ["Q"])).not.toThrow();
    expect(km.useKeymap.getState().bindings.add).toEqual(["Q"]);
  });

  it("no storage at all (undefined) uses defaults", async () => {
    const km = await fresh(undefined);
    expect(km.useKeymap.getState().bindings.add).toEqual(["Shift+A"]);
  });
});
