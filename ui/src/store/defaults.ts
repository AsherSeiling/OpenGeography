// The coordinate system a new, empty map starts with when there is none to copy (Settings › New maps, UR-16). Kept apart from the settings
// store so the map code can read it without importing the dialog's store.
import type { MapCrs } from "./store";

let defaultMapCrs: MapCrs | null = null;
export const getDefaultMapCrs = (): MapCrs | null => (defaultMapCrs ? { ...defaultMapCrs } : null);
export const setDefaultMapCrs = (c: MapCrs | null) => {
  defaultMapCrs = c;
};
