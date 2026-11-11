// UX2: the Start page's lists. Pure, so they are tested without a window.
import type { MapCrs } from "../store/store";

export const MAX_RECENT = 10;

/** `dir` first, any earlier entry for the same folder removed, at most MAX_RECENT kept. */
export function pushRecent(list: readonly string[], dir: string): string[] {
  const d = dir.trim();
  if (!d) return [...list];
  return [d, ...list.filter((p) => p !== d)].slice(0, MAX_RECENT);
}

/** The last folder name without ".gproj", the way a person would call the project. */
export function projectLabel(dir: string): string {
  const parts = dir.split(/[\\/]/).filter(Boolean);
  return (parts.pop() ?? dir).replace(/\.gproj$/i, "");
}

/** Where the project lives, shortened: the parent folder's last two names. */
export function projectWhere(dir: string): string {
  const parts = dir.split(/[\\/]/).filter(Boolean);
  parts.pop();
  return parts.slice(-2).join("/") || dir;
}

export interface StartTemplate {
  id: string;
  name: string;
  what: string;
  /** The coordinate system of the first map; null = chosen from the first layer added. */
  crs: MapCrs | null;
}

/** New-project starting points. They set only the first map's coordinate system; layers, styles and models are the person's. */
export const START_TEMPLATES: StartTemplate[] = [
  { id: "blank", name: "Blank project", what: "The map takes its coordinate system from the first layer you add (or your default in Settings).", crs: null },
  { id: "wgs84", name: "World, longitude and latitude", what: "WGS 84 (EPSG:4326): global data, GPS coordinates.", crs: { id: "EPSG:4326", name: "WGS 84" } },
  { id: "webmercator", name: "Web map", what: "Web Mercator (EPSG:3857): matches online basemaps. Areas and distances are not true on it.", crs: { id: "EPSG:3857", name: "WGS 84 / Pseudo-Mercator" } },
  { id: "conus", name: "United States, equal area", what: "NAD83 / Conus Albers (EPSG:5070): area-true maps of the lower 48 states.", crs: { id: "EPSG:5070", name: "NAD83 / Conus Albers" } },
];

export interface SampleData {
  id: "world" | "points" | "polygons";
  name: string;
  what: string;
}

/** Data that works with no download and no file of your own. */
export const SAMPLE_DATA: SampleData[] = [
  { id: "world", name: "World land outline", what: "Natural Earth 1:110m land, public domain, bundled with the app." },
  { id: "polygons", name: "Sample polygons", what: "1,000 random polygons with a class and a value field, to try tools on." },
  { id: "points", name: "Sample points", what: "1,000 random points with a class and a value field, to try tools on." },
];
