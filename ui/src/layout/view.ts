// View state of the layout editor that the ribbon, the rail and the canvas share: zoom, the start gallery and the pre-flight drawer.
// Kept here (not in the canvas) so View › Zoom on the ribbon and the zoom bar over the page drive the same thing.
import { create } from "zustand";

export const MIN_ZOOM = 0.3;
export const MAX_ZOOM = 12;
/** Screen pixels per millimetre at 100 % (96 dpi). */
export const ACTUAL_PX_PER_MM = 96 / 25.4;

export const clampZoom = (v: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, v));

interface LayoutViewState {
  /** Fit the page into the canvas (the default). */
  fit: boolean;
  /** Screen px per mm when `fit` is false. */
  zoom: number;
  /** The zoom the canvas is showing now (px per mm); the canvas reports it so In and Out step from what is on screen. */
  shown: number;
  /** Layouts whose start gallery was closed (an empty page then shows a short hint instead). */
  galleryClosed: Record<string, boolean>;
  preflightOpen: boolean;
  /** A layout tab is the active tab of the window (the layout workspace); false when the map or another pane has focus, even with a layout still open. */
  workspace: boolean;
  setWorkspace: (on: boolean) => void;
  setShown: (z: number) => void;
  fitPage: () => void;
  actualSize: () => void;
  zoomBy: (factor: number) => void;
  setZoom: (z: number) => void;
  closeGallery: (layoutId: string, closed?: boolean) => void;
  setPreflight: (open: boolean) => void;
}

export const useLayoutView = create<LayoutViewState>((set, get) => ({
  fit: true,
  zoom: 2,
  shown: 2,
  galleryClosed: {},
  preflightOpen: false,
  workspace: false,
  setWorkspace: (workspace) => set((s) => (s.workspace === workspace ? s : { workspace })),
  setShown: (shown) => set((s) => (s.shown === shown ? s : { shown })),
  fitPage: () => set({ fit: true }),
  actualSize: () => set({ fit: false, zoom: ACTUAL_PX_PER_MM }),
  zoomBy: (f) => set({ fit: false, zoom: clampZoom(get().shown * f) }),
  setZoom: (z) => set({ fit: false, zoom: clampZoom(z) }),
  closeGallery: (layoutId, closed = true) => set((s) => ({ galleryClosed: { ...s.galleryClosed, [layoutId]: closed } })),
  setPreflight: (preflightOpen) => set({ preflightOpen }),
}));

/** The zoom as the percentage shown to the user (100 % = 96 dpi). */
export const zoomPercent = (pxPerMm: number) => Math.round(((pxPerMm * 25.4) / 96) * 100);

/**
 * How much one Ctrl/⌘+wheel event, or one pinch event, changes the zoom. A mouse-wheel notch (about 100) is a step of 10 %;
 * the small deltas of a trackpad pinch give proportionally small steps, so a pinch is smooth.
 */
export function wheelZoomFactor(deltaY: number): number {
  const d = Math.max(-25, Math.min(25, deltaY));
  return Math.exp(-d * 0.004);
}

/** The page position (mm) under a screen point, given the page's on-screen box (left/top) and the zoom (px per mm). */
export function pageMmAt(rectLeft: number, rectTop: number, zoom: number, clientX: number, clientY: number): { x: number; y: number } {
  return { x: (clientX - rectLeft) / zoom, y: (clientY - rectTop) / zoom };
}

/**
 * How far to scroll so the page position `mm` is under `client` again after the zoom changed to `zoom`, with the page's box now at
 * `rectLeft`/`rectTop`: positive = scroll right/down. (The page cannot move further than its scroll area allows, so near the page's
 * edge at a low zoom the point can only get as close as the scroll range lets it.)
 */
export function scrollToKeep(mm: { x: number; y: number }, rectLeft: number, rectTop: number, zoom: number, clientX: number, clientY: number): { dx: number; dy: number } {
  return { dx: rectLeft + mm.x * zoom - clientX, dy: rectTop + mm.y * zoom - clientY };
}
