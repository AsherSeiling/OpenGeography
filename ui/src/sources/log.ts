// Which layers have no citation (UR-13). The engine logs a source for every import and every layer made from other layers; a layer
// made some other way (created features, test data, a file added before sources were logged) has none until the user adds one.
import { create } from "zustand";
import { api, errorText, type LayerInfo, type SourceEntry } from "../ipc/api";

interface SourceLog {
  entries: SourceEntry[];
  /** False until the log has been read once, so nothing is flagged while it loads. */
  loaded: boolean;
  /** Layers waiting for the Add citation dialog (set by the Map Layers marker, read by the Sources pane). */
  request: string | null;
  error: string | null;
  refresh: () => Promise<void>;
  ask: (layerId: string | null) => void;
}

export const useSourceLog = create<SourceLog>((set) => ({
  entries: [],
  loaded: false,
  request: null,
  error: null,
  refresh: async () => {
    try {
      set({ entries: await api.sources(), loaded: true, error: null });
    } catch (e) {
      set({ error: errorText(e) });
    }
  },
  ask: (request) => set({ request }),
}));

/** The layers (in the given order) that no source entry lists. */
export function uncitedLayers(layers: Record<string, LayerInfo>, ids: string[], entries: SourceEntry[]): LayerInfo[] {
  const cited = new Set(entries.flatMap((e) => e.uses.map((u) => u.layerId)));
  return ids.map((id) => layers[id]).filter((l): l is LayerInfo => !!l && !cited.has(l.id));
}
