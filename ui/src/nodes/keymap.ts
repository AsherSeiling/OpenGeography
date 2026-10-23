// Node-editor keyboard shortcuts (Blender's node editor defaults). They are
// active only while the pointer is over the Model canvas or focus is inside
// it. Users can rebind them in the Shortcuts dialog; custom bindings are kept
// per user in browser storage.
import { create } from "zustand";

export type NodeAction =
  | "add"
  | "delete"
  | "deleteReconnect"
  | "duplicate"
  | "copy"
  | "paste"
  | "grab"
  | "selectAll"
  | "deselectAll"
  | "invertSelection"
  | "mute"
  | "collapse"
  | "makeLinks"
  | "frameAll"
  | "frameSelected"
  | "undo"
  | "redo"
  | "run";

export interface ActionDef {
  id: NodeAction;
  label: string;
  group: string;
  description: string;
  defaults: string[];
}

/** "Mod" is Ctrl on Windows/Linux and ⌘ on macOS. */
export const ACTIONS: ActionDef[] = [
  { id: "add", label: "Add node", group: "Edit", description: "Open the Add menu at the pointer", defaults: ["Shift+A"] },
  { id: "delete", label: "Delete", group: "Edit", description: "Delete the selected nodes and connections", defaults: ["X", "Delete", "Backspace"] },
  { id: "deleteReconnect", label: "Delete with reconnect", group: "Edit", description: "Delete the selected nodes and connect what fed them to what they fed", defaults: ["Mod+X"] },
  { id: "duplicate", label: "Duplicate", group: "Edit", description: "Duplicate the selected nodes, then move them", defaults: ["Shift+D"] },
  { id: "copy", label: "Copy", group: "Edit", description: "Copy the selected nodes (works across layers' trees)", defaults: ["Mod+C"] },
  { id: "paste", label: "Paste", group: "Edit", description: "Paste copied nodes at the pointer", defaults: ["Mod+V"] },
  { id: "grab", label: "Grab / move", group: "Edit", description: "Move the selected nodes with the pointer; click or Enter to place, Esc to cancel. Dropping a free node on a connection inserts it.", defaults: ["G"] },
  { id: "mute", label: "Mute (bypass)", group: "Node", description: "Toggle bypass: the node's first input passes straight through", defaults: ["M"] },
  { id: "collapse", label: "Collapse", group: "Node", description: "Hide or show the node's settings", defaults: ["H"] },
  { id: "makeLinks", label: "Make links", group: "Node", description: "Connect the selected nodes left to right", defaults: ["F"] },
  { id: "selectAll", label: "Select all", group: "Select", description: "Select every node", defaults: ["A"] },
  { id: "deselectAll", label: "Deselect all", group: "Select", description: "Clear the selection", defaults: ["Alt+A"] },
  { id: "invertSelection", label: "Invert selection", group: "Select", description: "Select the unselected nodes", defaults: ["Mod+I"] },
  { id: "frameAll", label: "Frame all", group: "View", description: "Fit the whole tree in view", defaults: ["Home"] },
  { id: "frameSelected", label: "Frame selected", group: "View", description: "Fit the selected nodes in view", defaults: ["."] },
  { id: "undo", label: "Undo", group: "History", description: "Undo the last change to this tree", defaults: ["Mod+Z"] },
  { id: "redo", label: "Redo", group: "History", description: "Redo", defaults: ["Mod+Shift+Z", "Mod+Y"] },
  { id: "run", label: "Run tree", group: "Run", description: "Run this layer's node tree", defaults: ["Mod+Enter", "F12"] },
];

/** Mouse gestures (not rebindable). */
export const GESTURES: { label: string; keys: string }[] = [
  { label: "Cut links", keys: "Mod+right-drag across connections" },
  { label: "Insert a node into a connection", keys: "Drop or grab a free node onto it" },
  { label: "Pan", keys: "Drag the background or scroll" },
  { label: "Box select", keys: "Shift+drag" },
];

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** Key name for an event, independent of keyboard layout for letters and digits. */
function keyName(e: Pick<KeyboardEvent, "key" | "code">): string | null {
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3);
  if (/^Digit[0-9]$/.test(e.code)) return e.code.slice(5);
  if (e.code === "Period" || e.code === "NumpadDecimal") return ".";
  if (["Shift", "Control", "Alt", "Meta", "OS"].includes(e.key)) return null;
  if (e.key === " ") return "Space";
  return e.key.length === 1 ? e.key.toUpperCase() : e.key;
}

/** Normalized combo for a key event, e.g. "Mod+Shift+Z"; null for a lone modifier. */
export function eventCombo(e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">, mac = isMac): string | null {
  const k = keyName(e);
  if (!k) return null;
  const mod = mac ? e.metaKey : e.ctrlKey;
  return [mod && "Mod", e.altKey && "Alt", e.shiftKey && "Shift", k].filter(Boolean).join("+");
}

/** How a combo reads on this platform. */
export function displayCombo(combo: string, mac = isMac): string {
  return combo
    .split("+")
    .map((p) => (p === "Mod" ? (mac ? "⌘" : "Ctrl") : p === "Alt" && mac ? "⌥" : p === "Shift" && mac ? "⇧" : p))
    .join(mac ? "" : "+");
}

export type Bindings = Record<NodeAction, string[]>;

export const defaultBindings = (): Bindings => Object.fromEntries(ACTIONS.map((a) => [a.id, [...a.defaults]])) as Bindings;

/** The action bound to a combo, if any. */
export function actionFor(bindings: Bindings, combo: string): NodeAction | undefined {
  return (Object.keys(bindings) as NodeAction[]).find((a) => bindings[a].includes(combo));
}

/** Other actions already using `combo`. */
export function conflicts(bindings: Bindings, combo: string, except?: NodeAction): NodeAction[] {
  return (Object.keys(bindings) as NodeAction[]).filter((a) => a !== except && bindings[a].includes(combo));
}

const STORAGE = "gis-studio.nodeKeymap.v1";

function load(): Bindings {
  const b = defaultBindings();
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE) ?? "{}") as Partial<Bindings>;
    for (const a of ACTIONS) if (Array.isArray(saved[a.id])) b[a.id] = saved[a.id]!.filter((c) => typeof c === "string");
  } catch {
    // No storage (private window etc.): defaults.
  }
  return b;
}

function save(b: Bindings) {
  try {
    localStorage.setItem(STORAGE, JSON.stringify(b));
  } catch {
    // Unsaved; still applies this session.
  }
}

interface KeymapState {
  bindings: Bindings;
  /** Replace an action's combos (also removes those combos from other actions). */
  setBinding: (action: NodeAction, combos: string[]) => void;
  reset: (action?: NodeAction) => void;
}

export const useKeymap = create<KeymapState>((set, get) => ({
  bindings: load(),
  setBinding: (action, combos) => {
    const b = { ...get().bindings };
    for (const a of Object.keys(b) as NodeAction[]) if (a !== action) b[a] = b[a].filter((c) => !combos.includes(c));
    b[action] = [...new Set(combos)];
    save(b);
    set({ bindings: b });
  },
  reset: (action) => {
    const d = defaultBindings();
    const b = action ? { ...get().bindings, [action]: d[action] } : d;
    // A reset default steals its combo back from any action that took it.
    if (action) for (const a of Object.keys(b) as NodeAction[]) if (a !== action) b[a] = b[a].filter((c) => !d[action].includes(c));
    save(b);
    set({ bindings: b });
  },
}));
