// Program settings (UR-16): loaded once at start from the engine's per-user file, applied to the app, and saved when the user changes them.
// Missing or invalid settings are the defaults; a failed save is reported once and the choice still applies for this session.
import { create } from "zustand";
import { api, errorText, type ProgramSettings } from "../ipc/api";
import { useNav } from "../nav/navStore";
import { LENGTH_UNITS, AREA_UNITS } from "../nav/geodesy";
import { defaultLayout, getDock, serializeDock, setStartupLayout } from "../panes/panes";
import { setDefaultMapCrs } from "./defaults";
import { useApp } from "./store";

interface SettingsState {
  settings: ProgramSettings;
  loaded: boolean;
  load: () => Promise<void>;
  /** Merge `patch`, apply it, and save. Returns an error sentence if the file could not be written. */
  update: (patch: ProgramSettings) => Promise<string | null>;
}

/** Put settings into the running app. Unknown unit keys and themes are ignored. */
export function applySettings(s: ProgramSettings) {
  if (s.theme === "light" || s.theme === "dark") {
    if (useApp.getState().dark !== (s.theme === "dark")) useApp.setState({ dark: s.theme === "dark" });
  }
  const nav: { lengthUnit?: string; areaUnit?: string } = {};
  if (s.lengthUnit && LENGTH_UNITS[s.lengthUnit]) nav.lengthUnit = s.lengthUnit;
  if (s.areaUnit && AREA_UNITS[s.areaUnit]) nav.areaUnit = s.areaUnit;
  if (Object.keys(nav).length) useNav.getState().set(nav);
  setDefaultMapCrs(s.defaultCrs ?? null);
  setStartupLayout(s.startupLayout ?? null);
  // An empty first map starts in the default system too.
  const a = useApp.getState();
  if (s.defaultCrs && !Object.keys(a.layers).length) {
    const m = a.maps.find((x) => x.id === a.activeMapId);
    if (m && !m.crs) a.setMapCrs(s.defaultCrs, m.id);
  }
}

/** Save the panes as they are now as the arrangement new projects and empty starts open with (UR-17). */
export async function saveStartupLayout(): Promise<string | null> {
  const dock = serializeDock();
  if (!dock) return "There is no pane arrangement to save yet.";
  const err = await useSettings.getState().update({ startupLayout: dock });
  useApp.getState().setStatus(err ?? "Saved the current panes as your startup layout. New projects and empty starts open with it; a project keeps the layout it was saved with.");
  return err;
}

/** Forget the startup layout: new projects open with the built-in panes again. */
export async function resetStartupLayout(): Promise<string | null> {
  const err = await useSettings.getState().update({ startupLayout: null });
  useApp.getState().setStatus(err ?? "Startup layout reset: new projects open with the built-in panes.");
  return err;
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: {},
  loaded: false,
  load: async () => {
    let s: ProgramSettings = {};
    try {
      s = (await api.getSettings()) ?? {};
    } catch {
      /* no settings file or no engine: the defaults */
    }
    set({ settings: s, loaded: true });
    applySettings(s);
    // An empty start (no project, no layers) that already drew the built-in panes takes the user's startup layout.
    const a = useApp.getState();
    if (s.startupLayout && getDock() && !a.projectDir && !Object.keys(a.layers).length) defaultLayout();
  },
  update: async (patch) => {
    const next = { ...get().settings, ...patch };
    set({ settings: next });
    applySettings(next);
    try {
      set({ settings: await api.setSettings(next) });
      return null;
    } catch (e) {
      return `The settings could not be saved (${errorText(e)}). They apply until you close the app.`;
    }
  },
}));
