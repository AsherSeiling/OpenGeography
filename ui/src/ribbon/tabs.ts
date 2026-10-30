// Contextual ribbon tabs a view can register (the layout workspace uses this). A registered tab appears in the tab row, in a coloured
// group, while `visible` is true; its groups are laid out by the same adaptive fitting as the built-in tabs.
import type { ReactNode } from "react";
import { create } from "zustand";

export type Size = "large" | "small";
export type Item = { cmd: string; size?: Size } | { custom: ReactNode };
export interface Group {
  label: string;
  items: Item[];
}

export interface ContextualTab {
  /** Unique id, also the tab's key. */
  id: string;
  /** Label above the tab, in the coloured band ("Layout", "Layer", …). */
  band: string;
  /** The tab's own label. */
  label: string;
  groups: Group[];
  /** When false the tab is hidden (and, if it was open, the ribbon falls back to the Map tab). */
  visible: boolean;
}

interface RibbonTabs {
  tabs: Record<string, ContextualTab>;
  register: (t: ContextualTab) => () => void;
  setVisible: (id: string, visible: boolean) => void;
}

export const useRibbonTabs = create<RibbonTabs>((set) => ({
  tabs: {},
  register: (t) => {
    set((s) => ({ tabs: { ...s.tabs, [t.id]: t } }));
    return () => set((s) => { const { [t.id]: _gone, ...rest } = s.tabs; return { tabs: rest }; });
  },
  setVisible: (id, visible) => set((s) => (s.tabs[id] ? { tabs: { ...s.tabs, [id]: { ...s.tabs[id], visible } } } : s)),
}));

/** Register a contextual tab; returns the function that removes it. */
export const registerContextualTab = (t: ContextualTab) => useRibbonTabs.getState().register(t);
