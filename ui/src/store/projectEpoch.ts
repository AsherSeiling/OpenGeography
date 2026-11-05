import { create } from "zustand";

/** Counts project changes (new, open). Per-project caches keyed by layer id and version (such as the display tables the map has built) are not valid in the next project, which may reuse both (U-6). */
export const useProjectEpoch = create<{ epoch: number }>(() => ({ epoch: 0 }));
export const nextProjectEpoch = () => useProjectEpoch.setState((s) => ({ epoch: s.epoch + 1 }));
