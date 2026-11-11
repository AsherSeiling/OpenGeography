// UX11: what the ribbon search can find beyond commands, tools and layers: fields of the project's layers and coordinate systems.
// Pure, so it is tested without a window. A field hit opens that layer's attribute table; a coordinate system hit sets the map's.
import { searchCatalog, type CrsEntry } from "../crs/crs";
import type { LayerInfo } from "../ipc/api";
import { searchTools, type Searchable } from "../nodes/toolSearch";

export interface FieldHit {
  layerId: string;
  layerName: string;
  field: string;
  type: string;
}

/** Fields (by name) of the given layers that match `q`, best first. Rasters and layers without fields give none. */
export function fieldHits(layers: readonly Pick<LayerInfo, "id" | "name" | "fields" | "temporary">[], q: string, limit = 3): FieldHit[] {
  const items: (Searchable & { hit: FieldHit })[] = [];
  for (const l of layers) {
    if (l.temporary) continue;
    for (const f of l.fields ?? []) {
      // The layer's name is searchable too ("parcels owner"), but only through the category, so a field name still ranks first.
      items.push({ id: `${l.id}\u0000${f.name}`, name: f.name, category: l.name, description: f.type ?? "", hit: { layerId: l.id, layerName: l.name, field: f.name, type: f.type ?? "" } });
    }
  }
  return searchTools(items, q).slice(0, limit).map((h) => h.item.hit);
}

/** Coordinate systems matching `q`. Needs at least 3 characters (2 for an EPSG number prefix would match too much) so ordinary command and tool searches stay clean. */
export function crsHits(catalog: readonly CrsEntry[], q: string, limit = 3): CrsEntry[] {
  const t = q.trim();
  if (t.length < 3 || !catalog.length) return [];
  return searchCatalog(catalog as CrsEntry[], t, { limit });
}
