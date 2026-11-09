# Shape editing: plan

Status: planned 2026-11-06; E1 (engine) built 2026-11-09, UI not built. Requested by the user on 2026-11-06. Order (user, 2026-11-06): after the user-requests list, before the Census importer.

## What the user asked for

A separate **shape editing mode**, where you can either:
- edit the shapes of an existing layer, or
- make a whole new layer and add points, lines or polygons to it, then edit them.

## What exists today

- **No shape drawing or editing.** The map's sketch layer (`MapView.tsx`) is only for measuring and land navigation.
- **Attribute editing exists:** the Attribute Table edits cells, adds and deletes fields, calculates fields and geometry, with undo.
- **New Table (UR-14)** makes an empty table with typed fields; it has no geometry.
- **The model cache** keys Layer Input by the layer's `id:version:crs` (`graph.rs`; `docs/ATTRIBUTE_HISTORY_PLAN.md` §1). Any change to a layer's data must bump its version, so node trees that use it re-run.
- **OpenLayers**, already used for the map, has Draw, Modify, Snap and Translate interactions (BSD-2-Clause, already in the shipped set).

## Design

### The mode

- **Start Editing** on the map ribbon or a layer's right-click menu turns on edit mode for **one layer**.
  - An **Edit** ribbon tab appears with the tools.
  - The layer is marked in Map Layers, the status bar says "Editing <layer>", and the map gets a thin coloured border.
- **Finish Editing** asks **Save edits** / **Discard edits** / **Cancel**. Switching to another layer, closing the project or quitting asks the same.
- **Edits are an edit session.** Changes are kept in a pending-edits table in the project, so a crash doesn't lose them; the next open offers to resume or discard.
  - **Save** applies them to the layer in one transaction, bumps the layer's version and re-runs node trees that depend on it.
  - **Discard** drops them.
- **Undo and redo** (Mod+Z, Mod+Shift+Z) work step by step inside the session, then the session itself is one undo step after Save.
- **While editing:**
  - Tools that would write to the same layer are blocked with a sentence.
  - Other layers, maps and the attribute table stay usable.
  - The attribute table shows pending edits marked.

### What can be edited

- **A layer's own data:** an imported layer is the project's copy, so the original file on disk is never touched (project rule).
- **Not editable:** results of a node tree, rasters, and live server layers. Each offers **Make an editable copy**, which becomes a normal layer with its citations kept.
- **Big layers:** only features in and near the view are loaded for editing, and the rest stay in the engine. An edit never rewrites features it didn't touch.

### New layer

**New Layer** (Catalog, ribbon and the Edit tab) asks for:
- a name;
- the geometry type: point, multipoint, line or polygon;
- a coordinate system: the map's by default, with any CRS from the picker; never left blank;
- fields: same editor as New Table (UR-14). An id field is added by default.

It creates an empty layer and starts editing it.

### Tools (Edit tab)

| Group | Tools |
|---|---|
| **Create** | Point; line; polygon; freehand; rectangle and circle (circles drawn as true geodesic circles, a stated number of vertices). After each shape, the attribute form opens (skippable, with defaults). |
| **Modify** | Select; move vertices; add and delete vertices; move, rotate and scale features; delete features. |
| **Reshape** | Split by a line; merge features (attributes from the first, or chosen); reshape an edge; cut a hole; fill a hole; explode multipart; make multipart. |
| **Exact input** | Type coordinates for the next vertex: x/y in the layer's CRS, longitude/latitude, or USNG/MGRS (the parsers exist). Also a distance and bearing from the last vertex (geodesic). |
| **Snapping** | To vertices, edges, endpoints and intersections of chosen layers, with a tolerance in screen pixels. Snapping is toggled with a key while drawing. |
| **Clipboard** | Copy and paste features between layers. Shapes move into the target layer's CRS explicitly; fields match by name, and unmatched ones are reported. |

Tracing along another layer's edges is left for later.

### Correctness rules

- **Coordinate systems:** drawing happens in the map's CRS, but the layer stores its own. Every vertex is transformed explicitly from the map CRS to the layer CRS with the existing CRS code. A layer without a CRS can't be edited until it gets one through Define Projection.
- **Measurements while drawing** (segment length, area, bearing) are geodesic by default, as everywhere else.
- **Validity:**
  - A polygon that crosses itself, or a degenerate line, is flagged when finished, with "Fix" (`ST_MakeValid`) or "Keep as is" (counted).
  - Save reports any invalid shapes left.
- **Z and M values:** existing ones are kept. New vertices on a layer with Z get Z from a chosen DEM, a fixed value or none; the choice is per session and stated.
- **History:** Save records what changed (added, changed and deleted features) for the layer's history and diff (`docs/ATTRIBUTE_HISTORY_PLAN.md`).
- **Citations:** a new layer's citation names the user as author. An edited layer keeps its sources and gains a "modified by the user" note with the date.

### Engine

- **New commands**, also added to `crates/devserver`:
  - `edit_begin` / `edit_apply` (a batch of inserts, updates and deletes, with geometry as WKB or GeoJSON in the layer CRS) / `edit_save` / `edit_discard` / `edit_pending`;
  - `create_layer` (schema plus CRS).
- **Pending edits** go in a per-layer `__edits_<layer>` table. Save runs in one transaction, bumps `version` and invalidates dependent caches.
- **Feature ids** (`__fid`) stay stable for edited features, and new features get fresh ids.

## Work packages

| Id | Who | What | Done when |
|---|---|---|---|
| E0 | Opus | Short design check: OpenLayers Draw, Modify and Snap in non-Mercator map CRSs; pending-edits table and version bump against the cache; crash recovery; how the attribute table's existing undo fits | Decisions written into this file |
| E1 | Sonnet | Engine: edit session, pending table, save and discard, version bump and dependent re-run, create layer, validity check, history record, citations; project format addition with old projects opening | Engine tests: save and discard; crash recovery; node tree re-runs after save; a UTM layer edited on a Web Mercator map gets exact coordinates; old projects open |
| E2 | Sonnet | UI: edit mode, Edit tab, New Layer dialog, create point, line and polygon, vertex editing, move and delete, attribute form, snapping, undo and redo, finish prompts | e2e: new layer → add points → edit → save → reopen; edit an existing polygon; discard; snapping; a11y audit clean |
| E3 | Sonnet | Reshape tools, exact coordinate entry (x/y, lon/lat, USNG/MGRS, distance and bearing), copy and paste between layers, Z from DEM | Engine and e2e tests per tool, including CRS cases |

Close review by Opus: E1 (transactions, version bump, project format) and every CRS transform in E2 and E3.

## Decisions for the user

1. **Order: decided by the user, 2026-11-06:** after the request list, before the Census importer.
2. **Separate mode** (recommended, as asked) versus editing any time: the mode makes it clear what's being changed, and gives one Save or Discard point.
