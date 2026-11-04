// The one list of basemaps the map view offers (the engine's matching list is `layout/basemap.rs`: what a layout may print, and
// `sources.rs::for_basemap`: each one's citation). Ribbon menu, map drawing, credits and the CRS rule all read it.
//
// Terms were re-read from each provider's live pages on 2026-11-04 (see the commit message); the credit wording below is the
// provider's own.
import type { Basemap } from "../store/store";
import { MERCATOR_KEY } from "./crs";

/**
 * How a basemap is drawn, which decides where it can be shown:
 *  - `xyz`: Web Mercator raster tiles that OpenLayers reprojects to the map CRS in the browser (proj4, no datum grids).
 *  - `vector`: Mapbox/MapLibre-style vector tiles. OpenLayers cannot reproject vector tiles, so they draw in Web Mercator only.
 *  - `engine`: the bundled Natural Earth raster, warped by the engine to the map CRS's own tile grid (any CRS, offline).
 */
export type BasemapKind = "xyz" | "vector" | "engine";

export interface BasemapDef {
  id: Basemap;
  label: string;
  kind: BasemapKind;
  /** `xyz`: tile URL templates. `engine`: unused. */
  tiles?: string[];
  /** `vector`: the style document (a URL, or a path inside the app for a bundled copy). */
  style?: string;
  /** True when `style` is a copy bundled with the app (read relative to the app, not the network). */
  bundled?: boolean;
  maxzoom: number;
  /** Credit shown on the map, exactly as the provider asks (HTML: links open the provider's own pages). */
  credit: string;
  /** The same credit as plain text (tests, status messages). */
  creditText: string;
  /** A layout frame may print it (USGS; Natural Earth). The rest stay on screen: their terms forbid bulk tile downloads. */
  print: boolean;
}

const USGS = (svc: string) => `https://basemap.nationalmap.gov/arcgis/rest/services/${svc}/MapServer/tile/{z}/{y}/{x}`;
const OSM_LINK = `<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap contributors</a>`;
const OFM_CREDIT = `<a href="https://openfreemap.org" target="_blank" rel="noopener">OpenFreeMap</a> <a href="https://openmaptiles.org/" target="_blank" rel="noopener">© OpenMapTiles</a> Data from <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap</a>`;
const OFM_TEXT = "OpenFreeMap © OpenMapTiles Data from OpenStreetMap";
const OFM = (name: string) => `https://tiles.openfreemap.org/styles/${name}`;

export const BASEMAP_DEFS: BasemapDef[] = [
  { id: "usgsTopo", label: "USGS Topo", kind: "xyz", tiles: [USGS("USGSTopo")], maxzoom: 16, credit: "USGS The National Map", creditText: "USGS The National Map", print: true },
  { id: "usgsImagery", label: "USGS Imagery", kind: "xyz", tiles: [USGS("USGSImageryOnly")], maxzoom: 16, credit: "USGS The National Map", creditText: "USGS The National Map", print: true },
  { id: "usgsRelief", label: "USGS Shaded Relief", kind: "xyz", tiles: [USGS("USGSShadedReliefOnly")], maxzoom: 16, credit: "USGS The National Map", creditText: "USGS The National Map", print: true },
  { id: "osm", label: "OpenStreetMap", kind: "xyz", tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"], maxzoom: 19, credit: `© ${OSM_LINK}`, creditText: "© OpenStreetMap contributors", print: false },
  {
    id: "openTopoMap",
    label: "OpenTopoMap",
    kind: "xyz",
    tiles: ["a", "b", "c"].map((s) => `https://${s}.tile.opentopomap.org/{z}/{x}/{y}.png`),
    maxzoom: 17,
    credit: `Kartendaten: © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap-Mitwirkende</a>, SRTM | Kartendarstellung: © <a href="https://opentopomap.org" target="_blank" rel="noopener">OpenTopoMap</a> (<a href="https://creativecommons.org/licenses/by-sa/3.0/" target="_blank" rel="noopener">CC-BY-SA</a>)`,
    creditText: "Kartendaten: © OpenStreetMap-Mitwirkende, SRTM | Kartendarstellung: © OpenTopoMap (CC-BY-SA)",
    print: false,
  },
  { id: "ofmLiberty", label: "OpenFreeMap Liberty", kind: "vector", style: OFM("liberty"), maxzoom: 20, credit: OFM_CREDIT, creditText: OFM_TEXT, print: false },
  { id: "ofmBright", label: "OpenFreeMap Bright", kind: "vector", style: OFM("bright"), maxzoom: 20, credit: OFM_CREDIT, creditText: OFM_TEXT, print: false },
  { id: "ofmPositron", label: "OpenFreeMap Positron", kind: "vector", style: OFM("positron"), maxzoom: 20, credit: OFM_CREDIT, creditText: OFM_TEXT, print: false },
  {
    id: "versatiles",
    label: "VersaTiles Colorful",
    kind: "vector",
    style: "basemaps/versatiles/style.json",
    bundled: true,
    maxzoom: 20,
    credit: `© ${OSM_LINK} · © <a href="https://esa-worldcover.org/en/data-access" target="_blank" rel="noopener">ESA WorldCover project 2021</a> / Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover consortium`,
    creditText: "© OpenStreetMap contributors · © ESA WorldCover project 2021 / Contains modified Copernicus Sentinel data (2021) processed by ESA WorldCover consortium",
    print: false,
  },
  { id: "naturalEarth", label: "Natural Earth", kind: "engine", maxzoom: 6, credit: `Made with <a href="https://www.naturalearthdata.com" target="_blank" rel="noopener">Natural Earth</a>`, creditText: "Made with Natural Earth", print: true },
];

export const BASEMAP_IDS: Basemap[] = [...BASEMAP_DEFS.map((d) => d.id), "none"];

/** The Basemap menu's entries, in order. */
export const BASEMAP_OPTIONS: [Basemap, string][] = [...BASEMAP_DEFS.map((d): [Basemap, string] => [d.id, d.label]), ["none", "None"]];

export function basemapDef(id: string): BasemapDef | null {
  return BASEMAP_DEFS.find((d) => d.id === id) ?? null;
}

/** What a project's saved basemap value becomes in this build. A name this build doesn't know gives `none` and a sentence, never an error. */
export function readBasemap(value: unknown, fallback: Basemap = "usgsTopo"): { id: Basemap; message: string | null } {
  if (value === undefined || value === null) return { id: fallback, message: null };
  if (typeof value === "string" && (BASEMAP_IDS as string[]).includes(value)) return { id: value as Basemap, message: null };
  return { id: "none", message: `This project used a basemap this version of GIS Studio doesn't have (${typeof value === "string" ? `"${value}"` : "unreadable"}), so that map has no basemap. Choose one from Map → Basemap.` };
}

/** Where the map's view is, as far as the basemap rule needs. */
export interface BasemapView {
  crsKey: string;
  crsName: string;
  /** The map is a world view (the whole Earth cut at a meridian). */
  world: boolean;
  /** View centre, lon/lat from the engine; null until the first view is ready. */
  center: [number, number] | null;
  /** [west, south, east, north] of the CRS's area of use; `known` is false when PROJ has none (the grid then came from the data). */
  areaOfUse: [number, number, number, number];
  areaKnown: boolean;
}

/**
 * How far past a coordinate system's area of use (degrees) raster tiles still draw. The area of use is where the CRS is meant to be
 * used, not where it stops working: a map straddling a UTM zone edge or a state line reprojects correctly, and hiding its basemap
 * there would be wrong. Far past it the reprojection is what goes blank (MM-3).
 */
export const AREA_OF_USE_MARGIN_DEG = 10;

/** Latitude where Web Mercator tiles end. */
export const MERCATOR_MAX_LAT = 85.0511;

/**
 * Why `def` can't be drawn for this view, in words (null = it can). A basemap never changes the map's coordinate system and never
 * draws misaligned or partly blank where it can't be placed right:
 *  - vector tiles exist only in Web Mercator and can't be reprojected, so any other CRS refuses them;
 *  - Web Mercator raster tiles are reprojected in the browser, which holds only near the coordinate system's area of use and
 *    within Web Mercator's 85° limit, so a view centred well outside either is refused (docs/BUGS.md MM-3: USGS Imagery left a
 *    UTM 11N map blank when centred far outside the zone);
 *  - the bundled Natural Earth raster is warped by the engine to the map's own grid, so it draws in every CRS.
 */
export function basemapRefusal(def: BasemapDef | null, v: BasemapView): string | null {
  if (!def || def.kind === "engine") return null;
  if (def.kind === "vector") {
    if (v.crsKey === MERCATOR_KEY) return null;
    return `${def.label} is drawn in Web Mercator only, so it can't be shown in ${v.crsName}. Switch the map to Web Mercator, or choose Natural Earth or a raster basemap.`;
  }
  if (v.crsKey === MERCATOR_KEY) return null;
  if (!v.center) return null;
  const [lon, lat] = v.center;
  if (Math.abs(lat) > MERCATOR_MAX_LAT) return `${def.label} doesn't cover the poles (its tiles stop at ${MERCATOR_MAX_LAT.toFixed(0)}° latitude), so it isn't drawn here. Choose Natural Earth.`;
  if (v.areaKnown) {
    const [w, s, e, n] = v.areaOfUse;
    const m = AREA_OF_USE_MARGIN_DEG;
    const [west, east] = [wrapLon(w - m), wrapLon(e + m)];
    // A margin that closes the gap around the globe means every longitude is in range.
    const whole = (w <= e ? e - w : e - w + 360) + 2 * m >= 360;
    const inLon = whole || (west <= east ? lon >= west && lon <= east : lon >= west || lon <= east);
    if (!inLon || lat < s - m || lat > n + m) return `${def.label} isn't drawn here: the view is outside the area where ${v.crsName} is valid, and a basemap there would be misplaced or blank. Pan back into the area, or choose Natural Earth.`;
  }
  return null;
}

const wrapLon = (lon: number) => ((((lon + 180) % 360) + 360) % 360) - 180;

/** The sentence for a map frame that gets no basemap because the map's own can't be printed (null when it can, or there is none). */
export function printRefusal(id: string): string | null {
  const d = basemapDef(id);
  if (!d || d.print) return null;
  return `${d.label} can't be used in print layouts (its terms don't allow bulk tile downloads), so the frame has no basemap. Choose a USGS basemap or Natural Earth.`;
}
