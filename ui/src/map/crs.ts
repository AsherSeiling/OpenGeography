// Map coordinate systems for the OpenLayers view. proj4 only maps screen pixels to the map CRS
// (M-D5); every number shown or stored goes through the engine.
import { get as getProjection, type Projection } from "ol/proj";
import { register } from "ol/proj/proj4";
import TileGrid from "ol/tilegrid/TileGrid";
import proj4 from "proj4";
import type { CrsDisplayInfo, CrsGrid } from "../ipc/api";

/** Web Mercator's engine key; its tiles and grid are the classic XYZ ones. */
export const MERCATOR_KEY = "3857";
export const WEB_MERCATOR = "EPSG:3857";

/** CSS pixels per metre at 96 dpi (the scale convention of the status bar and the layouts). */
export const PX_PER_M = 96 / 0.0254;

const registered = new Map<string, string>();

/** Register `info`'s proj4 definition with OpenLayers and return its projection. Web Mercator uses OL's own. */
export function registerCrs(info: CrsDisplayInfo): Projection {
  const name = info.crsKey === MERCATOR_KEY ? WEB_MERCATOR : `GIS:${info.crsKey}`;
  if (info.crsKey !== MERCATOR_KEY && registered.get(name) !== info.proj4) {
    proj4.defs(name, info.proj4);
    register(proj4);
    registered.set(name, info.proj4);
  }
  const proj = getProjection(name);
  if (!proj) throw new Error(`The map view can't draw this coordinate system (${name}).`);
  proj.setExtent(info.grid.extent);
  return proj;
}

/** The engine's tile grid, exactly (the UI never derives one). */
export function tileGridFor(g: CrsGrid, maxLevel = g.resolutions.length - 1): TileGrid {
  // Levels past `maxLevel` are drawn by over-zooming the last one, as before.
  return new TileGrid({ extent: g.extent, origin: g.origin, resolutions: g.resolutions.slice(0, maxLevel + 1), tileSize: g.tileSize });
}

/** Ground metres a screen pixel spans at map `resolution` (CRS units/px) where the CRS's scale factor is `k`. */
export function groundPerPixel(resolution: number, unitM: number, k: number): number {
  return (resolution * unitM) / k;
}

/** `1:N` scale of a view at `resolution` (96 dpi). */
export function scaleOf(resolution: number, unitM: number, k: number): number {
  return groundPerPixel(resolution, unitM, k) * PX_PER_M;
}

/** Inverse of `scaleOf`. */
export function resolutionForScale(scale: number, unitM: number, k: number): number {
  return (scale / PX_PER_M) * (k / unitM);
}

/**
 * Resolution in the new CRS that keeps the ground scale of the old view (M-D7); `kOld`/`kNew` are the
 * engine's scale factors at the (same) view centre.
 */
export function carryResolution(resOld: number, unitOld: number, kOld: number, unitNew: number, kNew: number): number {
  return resolutionForScale(scaleOf(resOld, unitOld, kOld), unitNew, kNew);
}

/** Scale factor to use for a geographic CRS, whose degree is shorter east–west away from the equator (the engine reports k = 1). */
export function geographicK(latDeg: number): number {
  return 1 / Math.max(1e-6, Math.cos((latDeg * Math.PI) / 180));
}

/** Prefix the engine reads as "show the whole Earth in this CRS" (display.rs WORLD_PREFIX). */
export const WORLD_PREFIX = "world:";
/** The id the map draws with: the CRS itself, or its world view. */
export const displayCrsId = (id: string, world: boolean) => (world && !id.startsWith(WORLD_PREFIX) ? WORLD_PREFIX + id : id);
/** The CRS behind a display id (what layouts and exports store). */
export const baseCrsId = (id: string) => (id.startsWith(WORLD_PREFIX) ? id.slice(WORLD_PREFIX.length) : id);
