# Known bugs

## For M3 (files owned by the map session)

Nothing found. While cycling the map CRS (EPSG:3857 → 32615 → 5070 → 4326 → 3395 → ortho/laea custom → 3031/3413 → invalid ids set straight into the store, with layers loaded and a layout tab open), `MapView.tsx` never threw: invalid ids show the status message "The map can't be drawn in …" and keep the last good CRS. Checked at commit time of this branch; re-run after M3 lands (`window.__gis.useApp.getState().setMapCrs({id, name})` in the browser build).

## Fixed on `claude/errors-crash-fixes-0cffd7`

| Crash | Cause | Fix |
|---|---|---|
| Layout tab → "Maximum update depth exceeded" after opening a project with 2+ layouts | `LayoutView` selected `s.id === layoutId ? s.selected : []`; a new `[]` per call makes zustand loop forever for every layout tab that isn't the open one | `selectionOf()` returns a shared empty array |
| Frame CRS / "Set from Map View" with no layout open threw `Cannot read … of null` | `useLayouts.getState().doc!` | `frameById()` returns null; the user gets a message |
| Scale 0/NaN, w/h ≤ 0, NaN centre written into a layout | store `update` took any patch | `cleanPatch()` drops invalid values; `setPage` refuses 0/NaN pages |
| Older or damaged layout (no `page`, no `center`, null elements) | editor read fields directly | `normalizeLayout()` on load; `FrameFields` tolerates a missing centre |
| Page size 0 → fit zoom `Infinity` | `fitZoom` unguarded | non-finite → default zoom |
| Invalid map CRS accepted silently | `pickMapCrs` set it without checking | `setMapCrsChecked()` validates with the engine first |
| Damaged saved camera / dock layout aborted project open | uncaught in `restoreState` | wrapped; status message |
| Export with no map frame | engine error only in the status bar | explained up front |
