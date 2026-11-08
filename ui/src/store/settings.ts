// Program settings (UR-16): loaded once at start from the engine's per-user file, applied to the app, and saved when the user changes them.
// Missing or invalid settings are the defaults; a failed save is reported once and the choice still applies for this session.
import { create } from "zustand";
import { api, errorText, type ProgramSettings } from "../ipc/api";
import { useNav } from "../nav/navStore";
import { LENGTH_UNITS, AREA_UNITS } from "../nav/geodesy";
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
  // An empty first map starts in the default system too.
  const a = useApp.getState();
  if (s.defaultCrs && !Object.keys(a.layers).length) {
    const m = a.maps.find((x) => x.id === a.activeMapId);
    if (m && !m.crs) a.setMapCrs(s.defaultCrs, m.id);
  }
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
