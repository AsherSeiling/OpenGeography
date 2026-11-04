// Maps of a project (docs/UI_REDESIGN_PLAN.md §2b). Pure functions only: the store and the project file call them, and they are tested without a UI.
//
// A map owns its coordinate system, basemap, saved view, world view, and which project layers it shows with their order and visibility.
// Layer data, sources, node trees, styles and layouts stay project-wide.
import type { Camera } from "../map/mapController";
import { readBasemap } from "../map/basemaps";
import type { Basemap, MapCrs } from "./store";

export interface MapDoc {
  id: string;
  name: string;
  /** Explicit coordinate system. Null until a real source sets it (or the user picks one): it is never guessed. */
  crs: MapCrs | null;
  basemap: Basemap;
  /** The whole Earth in the map CRS (R8). Session-only: not saved. */
  worldView: boolean;
  /** Layers this map shows, top layer first. */
  order: string[];
  visible: Record<string, boolean>;
  /** Saved view (centre lon/lat and scale). Null = not framed yet. */
  camera: Camera | null;
}

/** Id of the map a version 1 project (and a new project) starts with. It is also its dock panel's id. */
export const DEFAULT_MAP_ID = "map";
/** Project file format. 1 = one map in `order` / `visible` / `mapCrs` / `basemap` / `camera`; 2 = `maps[]`. */
export const PROJECT_VERSION = 2;

export const isMapPanelId = (id: string) => id === DEFAULT_MAP_ID || id.startsWith("map-");

export function emptyMap(over: Partial<MapDoc> = {}): MapDoc {
  return { id: DEFAULT_MAP_ID, name: "Map", crs: null, basemap: "usgsTopo", worldView: false, order: [], visible: {}, camera: null, ...over };
}

export const mapOf = (s: { maps: MapDoc[] }, id: string): MapDoc | undefined => s.maps.find((m) => m.id === id);
/** The active map; the first one if the id is stale. A project always has at least one map. */
export const activeMapOf = (s: { maps: MapDoc[]; activeMapId: string }): MapDoc => mapOf(s, s.activeMapId) ?? s.maps[0];

export function newMapId(maps: MapDoc[]): string {
  const taken = new Set(maps.map((m) => m.id));
  for (;;) {
    const id = `map-${Math.random().toString(36).slice(2, 7)}`;
    if (!taken.has(id)) return id;
  }
}

/** "Map 2", "Map 3", …: the first name not used yet; `base` + " copy" style names go through `uniqueName`. */
export function uniqueName(maps: MapDoc[], wanted: string): string {
  const names = new Set(maps.map((m) => m.name.toLowerCase()));
  if (!names.has(wanted.toLowerCase())) return wanted;
  for (let n = 2; ; n++) if (!names.has(`${wanted} ${n}`.toLowerCase())) return `${wanted} ${n}`;
}

/** A new empty map. It takes the last map's coordinate system and basemap (null stays null) and says which map they came from. */
export function newMapFrom(maps: MapDoc[]): { map: MapDoc; inheritedFrom: MapDoc | null } {
  const last = maps[maps.length - 1] ?? null;
  const map = emptyMap({ id: newMapId(maps), name: uniqueName(maps, `Map ${maps.length + 1}`), crs: last?.crs ? { ...last.crs } : null, basemap: last?.basemap ?? "usgsTopo" });
  return { map, inheritedFrom: last?.crs ? last : null };
}

export function duplicateOf(maps: MapDoc[], src: MapDoc): MapDoc {
  return { ...src, id: newMapId(maps), name: uniqueName(maps, `${src.name} copy`), crs: src.crs ? { ...src.crs } : null, order: [...src.order], visible: { ...src.visible }, camera: src.camera ? { ...src.camera } : null };
}

/** Distinct basemaps in use by any map, for the citation record (each is recorded, not only the active map's). */
export function basemapsInUse(maps: MapDoc[]): Basemap[] {
  return [...new Set(maps.map((m) => m.basemap).filter((b): b is Basemap => !!b))];
}

// ---------- project file ----------

/** What a map looks like in the project file. `worldView` is not saved. */
export interface SavedMap {
  id: string;
  name: string;
  crs: MapCrs | null;
  basemap: string;
  order: string[];
  visible: Record<string, boolean>;
  camera: Camera | null;
}

export function serializeMaps(maps: MapDoc[]): SavedMap[] {
  return maps.map((m) => ({ id: m.id, name: m.name, crs: m.crs, basemap: m.basemap, order: m.order, visible: m.visible, camera: m.camera }));
}

/** The map part of a saved project, either format. Unknown fields are ignored; layer ids not in `layerIds` are dropped. */
interface SavedUi {
  version?: unknown;
  maps?: unknown;
  activeMapId?: unknown;
  order?: string[];
  visible?: Record<string, boolean>;
  mapCrs?: MapCrs | null;
  basemap?: string;
  camera?: Camera | null;
}

export type LoadedMaps = { ok: true; maps: MapDoc[]; activeMapId: string; /** Sentences for the status bar (a basemap this version doesn't have became none). */ notes: string[] } | { ok: false; error: string };

/** A saved basemap value: kept as written when this build has it, else none plus a note (never an error). */
function readMapBasemap(value: unknown, notes: string[]): Basemap {
  const r = readBasemap(value);
  if (r.message && !notes.includes(r.message)) notes.push(r.message);
  return r.id;
}

const isCrs = (c: unknown): c is MapCrs => !!c && typeof (c as MapCrs).id === "string" && typeof (c as MapCrs).name === "string";

/**
 * Maps of a saved project. Version 1 (and files with no version) become one map named "Map" with its layer order, visibility, CRS,
 * basemap and view unchanged; layers the order forgot are shown on top, as before. Version 2 is read as written. A file from a newer
 * version is refused with a readable message instead of being half-read, so a save can never drop what a newer app wrote.
 */
export function loadMaps(ui: SavedUi, layerIds: string[]): LoadedMaps {
  const known = new Set(layerIds);
  const v = ui.version;
  if (typeof v === "number" && v > PROJECT_VERSION) {
    return { ok: false, error: `This project was saved by a newer version of GIS Studio (project format ${v}; this version reads up to ${PROJECT_VERSION}). Update GIS Studio to open it. Nothing was changed.` };
  }
  if (v !== undefined && (typeof v !== "number" || !Number.isInteger(v) || v < 1)) {
    return { ok: false, error: `This project file has an unreadable format version (${JSON.stringify(v)}). Nothing was changed.` };
  }
  if (v === 2) {
    const raw = Array.isArray(ui.maps) ? (ui.maps as Partial<SavedMap>[]) : [];
    const used = new Set<string>();
    const maps: MapDoc[] = [];
    const notes: string[] = [];
    for (const r of raw) {
      if (!r || typeof r !== "object") continue;
      let id = typeof r.id === "string" && r.id && !used.has(r.id) ? r.id : "";
      if (!id) id = newMapId([...used].map((u) => emptyMap({ id: u })));
      used.add(id);
      const order = (Array.isArray(r.order) ? r.order : []).filter((x) => known.has(x));
      maps.push(emptyMap({
        id,
        name: typeof r.name === "string" && r.name.trim() ? r.name : "Map",
        crs: isCrs(r.crs) ? r.crs : null,
        basemap: readMapBasemap(r.basemap, notes),
        order,
        visible: { ...Object.fromEntries(order.map((x) => [x, true])), ...pickKnown(r.visible, known) },
        camera: r.camera ?? null,
      }));
    }
    if (!maps.length) maps.push(emptyMap());
    const active = typeof ui.activeMapId === "string" && maps.some((m) => m.id === ui.activeMapId) ? (ui.activeMapId as string) : maps[0].id;
    return { ok: true, maps, activeMapId: active, notes };
  }
  // Version 1.
  const order = (ui.order ?? []).filter((id) => known.has(id));
  for (const id of layerIds) if (!order.includes(id)) order.unshift(id);
  const notes: string[] = [];
  const map = emptyMap({
    crs: isCrs(ui.mapCrs) ? ui.mapCrs : null,
    basemap: readMapBasemap(ui.basemap, notes),
    order,
    visible: { ...Object.fromEntries(order.map((id) => [id, true])), ...pickKnown(ui.visible, known) },
    camera: ui.camera ?? null,
  });
  return { ok: true, maps: [map], activeMapId: map.id, notes };
}

function pickKnown(v: Record<string, boolean> | undefined, known: Set<string>): Record<string, boolean> {
  return Object.fromEntries(Object.entries(v ?? {}).filter(([k]) => known.has(k)));
}

/** Every project layer id: the active map's order first (top layer first), then the layers that map does not show. Pickers read this, not `order`. */
export function projectLayerIds(order: string[], layers: Record<string, unknown>): string[] {
  const seen = new Set(order.filter((id) => id in layers));
  return [...seen, ...Object.keys(layers).filter((id) => !seen.has(id))];
}
