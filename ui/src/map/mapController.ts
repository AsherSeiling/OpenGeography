// Imperative access to the OpenLayers maps, one per map of the project: the map CRS each is drawn in, the engine-derived view cache
// (centre, bounds and ground scale as lon/lat numbers) and the view operations other code needs.
// proj4 (inside OL) only maps pixels to the map CRS; every number comes from the engine (M-D5).
//
// Every function takes an optional map id and acts on the ACTIVE map when it is left out, so the status bar, the scale box, the
// download dialogs ("current view") and the layout "frame from map view" follow whichever map the user is working in.
import type OlMap from "ol/Map";
import View from "ol/View";
import type { Projection } from "ol/proj";
import { api, type CrsDisplayInfo, type Extent } from "../ipc/api";
import { useApp } from "../store/store";
import { mapOf } from "../store/maps";
import { displayCrsId, geographicK, groundPerPixel, PX_PER_M, resolutionForScale, WEB_MERCATOR } from "./crs";

export interface ActiveCrs {
  id: string;
  info: CrsDisplayInfo;
  projection: Projection;
}

interface Ctl {
  map: OlMap | null;
  active: ActiveCrs | null;
  pending: (() => void) | null;
  cache: ViewCache | null;
  cacheSeq: number;
}

const ctls = new Map<string, Ctl>();
function ctl(mapId?: string): Ctl {
  const id = mapId ?? useApp.getState().activeMapId;
  let c = ctls.get(id);
  if (!c) {
    c = { map: null, active: null, pending: null, cache: null, cacheSeq: 0 };
    ctls.set(id, c);
  }
  return c;
}

/** A map's panel mounted (or unmounted, with null). A view operation queued before the panel existed stays queued. */
export function setMap(m: OlMap | null, mapId?: string) {
  const c = ctl(mapId);
  c.map = m;
  if (!m) {
    c.active = null;
    c.cache = null;
  }
}

/** Forget a deleted map's controller. */
export function dropMap(mapId: string) {
  ctls.delete(mapId);
}

export function getMap(mapId?: string): OlMap | null {
  return ctl(mapId).map;
}

export function getActiveCrs(mapId?: string): ActiveCrs | null {
  return ctl(mapId).active;
}

/** Called by the map view once a CRS switch has finished; runs a view operation queued for it. */
export function setActiveCrs(c: ActiveCrs, mapId?: string) {
  const k = ctl(mapId);
  k.active = c;
  const p = k.pending;
  k.pending = null;
  p?.();
}

/** Run `fn` now if the map is drawn in its CRS, else when the switch to it finishes. */
function whenCrsReady(fn: () => void, mapId?: string) {
  const k = ctl(mapId);
  const doc = mapOf(useApp.getState(), mapId ?? useApp.getState().activeMapId);
  const want = displayCrsId(doc?.crs?.id ?? WEB_MERCATOR, doc?.worldView ?? false);
  if (k.map && k.active && k.active.id === want) fn();
  else k.pending = fn;
}

/** Scale factor k of the active CRS at a view position (engine; geographic CRSs use cos φ). */
export async function scaleFactorAt(crs: ActiveCrs, x: number, y: number): Promise<number> {
  return crs.info.geographic ? geographicK(y) : api.mapScaleFactor(crs.id, x, y);
}

// ---------- engine-derived view cache (lon/lat) ----------

export interface ViewCache {
  /** The CRS the map-CRS fields below are in. */
  crsId: string;
  /** View centre in the map CRS (what the user sees; no detour through lon/lat). */
  centerMap: [number, number];
  /** View centre as engine lon/lat. */
  center: [number, number];
  /** [west, south, east, north] degrees spanned by the visible map (edges sampled through the engine). */
  bounds: Extent;
  /** Ground metres per screen pixel at the centre. */
  groundPerPx: number;
  /** Engine scale factor at the centre. */
  k: number;
}

export function getCenterLonLat(mapId?: string): [number, number] | null {
  return ctl(mapId).cache?.center ?? null;
}
export function getViewBounds(mapId?: string): Extent | null {
  return ctl(mapId).cache?.bounds ?? null;
}
/** The view centre in the map CRS together with that CRS's id (null until the view is ready). */
export function getCenterInMapCrs(mapId?: string): { crs: string; center: [number, number] } | null {
  const cache = ctl(mapId).cache;
  return cache ? { crs: cache.crsId, center: cache.centerMap } : null;
}
/** Ground metres a screen pixel spans at the view centre. */
export function getGroundPerPixel(mapId?: string): number | null {
  return ctl(mapId).cache?.groundPerPx ?? null;
}

/** Recompute the cache through the engine (latest call wins). */
export async function refreshViewCache(mapId?: string): Promise<ViewCache | null> {
  const k0 = ctl(mapId);
  const my = ++k0.cacheSeq;
  const { map, active } = k0;
  if (!map || !active) return null;
  const crs = active;
  const size = map.getSize();
  const view = map.getView();
  const res = view.getResolution();
  const c = view.getCenter();
  if (!size || !res || !c) return null;
  const ext = view.calculateExtent(size);
  const [x0, y0, x1, y1] = ext;
  const xm = (x0 + x1) / 2;
  const ym = (y0 + y1) / 2;
  const pts: [number, number][] = [[c[0], c[1]], [x0, y0], [x1, y0], [x0, y1], [x1, y1], [xm, y0], [xm, y1], [x0, ym], [x1, ym]];
  try {
    const out = await Promise.all(pts.map(([x, y]) => api.transformPoint(x, y, crs.id, "EPSG:4326")));
    const k = await scaleFactorAt(crs, c[0], c[1]);
    if (my !== k0.cacheSeq) return k0.cache;
    const lons = out.slice(1).map((p) => p[0]);
    const lats = out.slice(1).map((p) => p[1]);
    k0.cache = {
      crsId: crs.id,
      centerMap: [c[0], c[1]],
      center: out[0],
      bounds: [Math.min(...lons), Math.min(...lats), Math.max(...lons), Math.max(...lats)],
      groundPerPx: groundPerPixel(res, crs.info.unitM, k),
      k,
    };
  } catch {
    /* outside the CRS's valid area: keep the previous cache */
  }
  return k0.cache;
}

// ---------- view operations ----------

/** The smallest map-CRS box holding a lon/lat extent (corners and edge midpoints, through the engine). */
async function extentInMapCrs(crs: ActiveCrs, ext: Extent): Promise<[number, number, number, number]> {
  const [w, s, e, n] = ext;
  const cx = (w + e) / 2;
  const cy = (s + n) / 2;
  const pts = [[w, s], [e, s], [w, n], [e, n], [cx, s], [cx, n], [w, cy], [e, cy], [cx, cy]];
  const out = await Promise.all(pts.map(([x, y]) => api.transformPoint(x, y, "EPSG:4326", crs.id)));
  const xs = out.map((p) => p[0]);
  const ys = out.map((p) => p[1]);
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/** Fit a lon/lat extent. `maxZoom` is a Web-Mercator-style level (z 16 ≈ 2.4 m per pixel at the equator). */
export function zoomToExtent(ext: Extent | null, maxZoom = 16, mapId?: string) {
  if (!ext) return;
  whenCrsReady(() => {
    const { map, active } = ctl(mapId);
    const crs = active!;
    void extentInMapCrs(crs, ext)
      .then((box) => {
        if (!box.every(Number.isFinite)) return;
        const minResolution = crs.info.grid.resolutions[Math.min(maxZoom, crs.info.grid.resolutions.length - 1)];
        map?.getView().fit(box, { padding: [40, 40, 40, 40], minResolution, duration: 600, size: map.getSize() });
      })
      .catch(() => undefined);
  }, mapId);
}

/** Set the `1:N` scale about the view centre (engine k). */
export async function setScale(scale: number, mapId?: string) {
  const { map, active } = ctl(mapId);
  if (!map || !active) return;
  const c = map.getView().getCenter();
  if (!c) return;
  const k = await scaleFactorAt(active, c[0], c[1]);
  map.getView().animate({ resolution: resolutionForScale(scale, active.info.unitM, k), duration: 300 });
}

export interface Camera {
  /** Centre as lon/lat (CRS-independent, so a project reopens at the same place in any map CRS). */
  center: [number, number];
  /** `1:N` ground scale at the centre (96 dpi). Projects saved before M3 have `zoom` instead. */
  scale?: number;
  /** Pre-M3 projects: a 512 px Web-Mercator-style zoom level. Read only. */
  zoom?: number;
}

/** A saved-project camera: centre lon/lat and the `1:N` scale at the centre. */
export function snapshotCamera(mapId?: string): Camera | null {
  const cache = ctl(mapId).cache;
  return cache ? { center: cache.center, scale: cache.groundPerPx * PX_PER_M } : null;
}

/** Ground metres per pixel of a pre-M3 saved zoom (MapLibre 512 px tiles); only for opening old projects. */
function groundForSavedZoom(zoom: number, latDeg: number): number {
  return (40075016.686 * Math.cos((latDeg * Math.PI) / 180)) / (512 * 2 ** zoom);
}

export function restoreCamera(cam: Camera, mapId?: string) {
  whenCrsReady(() => {
    const { map, active } = ctl(mapId);
    const crs = active!;
    void (async () => {
      try {
        const [x, y] = await api.transformPoint(cam.center[0], cam.center[1], "EPSG:4326", crs.id);
        const k = await scaleFactorAt(crs, x, y);
        const scale = cam.scale ?? (cam.zoom !== undefined ? groundForSavedZoom(cam.zoom, cam.center[1]) * PX_PER_M : undefined);
        if (!scale || !Number.isFinite(scale)) return;
        const view = map?.getView();
        view?.setCenter([x, y]);
        view?.setResolution(resolutionForScale(scale, crs.info.unitM, k));
        void refreshViewCache(mapId);
      } catch {
        /* keep the current view */
      }
    })();
  }, mapId);
}

export function makeView(crs: ActiveCrs, center: [number, number], resolution: number): View {
  const r = crs.info.grid.resolutions;
  return new View({ projection: crs.projection, center, resolution, maxResolution: r[0], minResolution: r[r.length - 1] / 2, constrainResolution: false });
}

export interface FlightResult {
  frames: number;
  seconds: number;
  avgFps: number;
  p95FrameMs: number;
  worstFrameMs: number;
}

/**
 * Scripted pan/zoom flight over an extent, recording frame times. Used by
 * View → Benchmark to check the map stays smooth (target: 60 fps, p95 < 20 ms).
 */
export async function benchmarkFlight(ext: Extent, onStep?: (fraction: number) => void, mapId?: string): Promise<FlightResult> {
  const { map, active } = ctl(mapId);
  if (!map || !active) throw new Error("Map not ready");
  const m = map;
  const crs = active;
  const view = m.getView();
  const [w, s, e, n] = ext;
  const at = (fx: number, fy: number): [number, number] => [w + (e - w) * fx, s + (n - s) * fy];
  const plan: { at: [number, number]; zoom: number }[] = [
    { at: at(0.5, 0.5), zoom: 5 },
    { at: at(0.5, 0.5), zoom: 9 },
    { at: at(0.25, 0.3), zoom: 11 },
    { at: at(0.7, 0.6), zoom: 10 },
    { at: at(0.5, 0.5), zoom: 13 },
    { at: at(0.5, 0.5), zoom: 6 },
  ];
  const r = crs.info.grid.resolutions;
  const steps = await Promise.all(
    plan.map(async (p) => ({
      center: (await api.transformPoint(p.at[0], p.at[1], "EPSG:4326", crs.id)) as [number, number],
      resolution: r[Math.min(p.zoom, r.length - 1)],
    })),
  );
  const frameTimes: number[] = [];
  let last = performance.now();
  let running = true;
  const tick = () => {
    const now = performance.now();
    frameTimes.push(now - last);
    last = now;
    if (running) requestAnimationFrame(tick);
  };
  view.setCenter(steps[0].center);
  view.setResolution(steps[0].resolution);
  await new Promise((res) => setTimeout(res, 500));
  const t0 = performance.now();
  last = t0;
  requestAnimationFrame(tick);
  for (const [i, step] of steps.slice(1).entries()) {
    onStep?.(i / (steps.length - 1));
    await new Promise<void>((resolve) => view.animate({ center: step.center, resolution: step.resolution, duration: 2000 }, () => resolve()));
  }
  running = false;
  const seconds = (performance.now() - t0) / 1000;
  const sorted = [...frameTimes].sort((a, b) => a - b);
  return {
    frames: frameTimes.length,
    seconds,
    avgFps: frameTimes.length / seconds,
    p95FrameMs: sorted[Math.floor(sorted.length * 0.95)] ?? 0,
    worstFrameMs: sorted[sorted.length - 1] ?? 0,
  };
}
