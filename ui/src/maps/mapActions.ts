// Things the user does to maps: new, duplicate, rename, delete, open. Used by the ribbon, the Catalog and the map tabs' "+".
import { dropMap, snapshotCamera } from "../map/mapController";
import { showMap, syncMapPanels } from "../panes/panes";
import { mapOf } from "../store/maps";
import { useApp } from "../store/store";

const status = (s: string) => useApp.getState().setStatus(s);

/** A new empty map, opened and active. It starts with the last map's coordinate system, and says so. */
export function newMap(opts: NonNullable<Parameters<typeof showMap>[1]> = {}) {
  const { map, inheritedFrom } = useApp.getState().addMap();
  showMap(map.id, opts);
  status(
    inheritedFrom
      ? `New map "${map.name}" uses the coordinate system of "${inheritedFrom.name}" (${map.crs?.name}). Change it under Coordinates › Coordinate System. Add layers with + in Map Layers.`
      : map.crs
        ? `New map "${map.name}" starts in your default coordinate system, ${map.crs.name} (Settings). Change it under Coordinates › Coordinate System.`
        : `New map "${map.name}" has no coordinate system yet; it takes one from the first layer you add.`,
  );
  return map;
}

export function duplicateMap(id = useApp.getState().activeMapId) {
  const src = mapOf(useApp.getState(), id);
  // The copy starts where the original is looking now.
  if (src) useApp.getState().setMapCamera(id, snapshotCamera(id) ?? src.camera);
  const copy = useApp.getState().duplicateMap(id);
  if (!copy) return null;
  showMap(copy.id, { side: true });
  status(`Duplicated "${src?.name}" as "${copy.name}". The layers are shared with the project; each map keeps its own order, visibility, coordinate system and basemap.`);
  return copy;
}

export function renameMap(id: string, name?: string) {
  const cur = mapOf(useApp.getState(), id);
  if (!cur) return;
  const next = name ?? window.prompt("Map name", cur.name);
  if (!next?.trim()) return;
  useApp.getState().renameMap(id, next);
  syncMapPanels();
}

/** Delete a map after asking. The project keeps every layer. */
export function deleteMap(id: string, ask = true) {
  const s = useApp.getState();
  const m = mapOf(s, id);
  if (!m) return false;
  if (s.maps.length < 2) {
    status("A project needs at least one map, so this one can't be deleted.");
    return false;
  }
  if (ask && !window.confirm(`Delete the map "${m.name}"? Its layers stay in the project and on the other maps.`)) return false;
  if (!s.deleteMap(id)) return false;
  dropMap(id);
  syncMapPanels();
  status(`Deleted map "${m.name}". Its layers are still in the project.`);
  return true;
}

export function openMap(id: string, side = false) {
  showMap(id, { side });
}

/** A map tab the user closed keeps the view it had, for when it is opened again. */
export function rememberView(mapId: string) {
  if (!mapOf(useApp.getState(), mapId)) return;
  const cam = snapshotCamera(mapId);
  if (cam) useApp.getState().setMapCamera(mapId, cam);
}
