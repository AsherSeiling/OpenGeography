// Imperative access to the one map instance (zoom-to, benchmark flight).
import type { Map as MlMap } from "maplibre-gl";
import type { Extent } from "../ipc/api";

let map: MlMap | null = null;

export function setMap(m: MlMap | null) {
  map = m;
}

export function getMap(): MlMap | null {
  return map;
}

export function zoomToExtent(ext: Extent | null, maxZoom = 16) {
  if (!map || !ext) return;
  const [w, s, e, n] = ext;
  if (w === e && s === n) {
    map.flyTo({ center: [w, s], zoom: Math.min(maxZoom, 14) });
    return;
  }
  map.fitBounds([[w, s], [e, n]], { padding: 40, maxZoom, duration: 600 });
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
export async function benchmarkFlight(ext: Extent, onStep?: (fraction: number) => void): Promise<FlightResult> {
  if (!map) throw new Error("Map not ready");
  const m = map;
  const [w, s, e, n] = ext;
  const cx = (w + e) / 2;
  const cy = (s + n) / 2;
  const steps: { center: [number, number]; zoom: number }[] = [
    { center: [cx, cy], zoom: 5 },
    { center: [cx, cy], zoom: 9 },
    { center: [w + (e - w) * 0.25, s + (n - s) * 0.3], zoom: 11 },
    { center: [w + (e - w) * 0.7, s + (n - s) * 0.6], zoom: 10 },
    { center: [cx, cy], zoom: 13 },
    { center: [cx, cy], zoom: 6 },
  ];
  const frameTimes: number[] = [];
  let last = performance.now();
  let running = true;
  const tick = () => {
    const now = performance.now();
    frameTimes.push(now - last);
    last = now;
    if (running) requestAnimationFrame(tick);
  };
  m.jumpTo(steps[0]);
  await new Promise((r) => setTimeout(r, 500));
  const t0 = performance.now();
  last = t0;
  requestAnimationFrame(tick);
  for (const [i, step] of steps.slice(1).entries()) {
    onStep?.(i / (steps.length - 1));
    await new Promise<void>((resolve) => {
      m.once("moveend", () => resolve());
      m.flyTo({ ...step, duration: 2000, essential: true });
    });
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
