# Public Data node: plan

Status: planned 2026-11-06, nothing built. Requested by the user on 2026-11-06. It goes with the Census tables importer (`docs/CENSUS_TABLES_PLAN.md`), after the user-requests list and before server connections and planetary. See "Order" below.

## What it is

A **Public Data** node in Processing nodes brings a public dataset into a model as an input, like Layer Input does for a project layer. A saved model can then run start to finish on a new area without the user downloading anything by hand. Examples:
- clip my parcels to the county from TIGER;
- slope from 3DEP elevation for my study area;
- earthquakes within 50 km of my sites.

## Why it doesn't break D2

DECISIONS D2 kept downloads out of model runs, because a network request inside every run makes models slow, unusable offline and not reproducible (the source can change between runs). This node keeps those properties:

- **Downloaded once, kept in the project.** The node's data is a *snapshot* stored in the project, like an imported layer. A run uses the snapshot and makes no request.
- **A snapshot is identified by what was asked for:** source, dataset, layer, version or year where the source has one (TIGER year, Overture release, STAC item ids), and the area. The same request always gives the same snapshot, so the executor's cache (`graph.rs`, hash of tool + params + inputs) stays valid.
- **New data only when the request changes or the user asks.**
  - Changing the area or settings makes a new request: the run downloads once, after a confirmation that shows the source and an estimated size.
  - **Refresh** downloads the same request again for sources that change (live services, earthquake catalogues), then reports what changed: features added, removed and changed.
- **Offline:** a run that needs a snapshot it doesn't have stops before any work, naming the node and what it needs. It never falls back to stale or partial data silently.
- **The record:** the snapshot records its download date and its citation (`sources.rs`), and results inherit it as for any input.

D2 gets a note saying this node is the one sanctioned way to bring downloads into models, and why it keeps D2's reasons.

## Design

- **ToolSpec:** one tool, `public_data`, with Role `Data` (blue oval), output a vector or raster port.
  - **Settings inside the card:** source, dataset, layer, year or version where the source has one, the area, and options per source.
  - **Status line:** "Saved copy from 2026-11-06, 3,214 features, 2.1 MB".
  - **Buttons:** Refresh, and Open source page.
  - The settings come from the existing connectors, not a second catalog. Each connector exposes what it offers through one shared description, so the node and the dialogs read the same thing.
- **Area:** where the data covers.
  - **Fixed area:** a drawn or typed extent, or chosen states and counties.
  - **From an input (optional port):** the area of the layer connected to the node, plus a buffer, usually the tree's own Layer Input. The snapshot request then follows the input's extent. The extent is rounded outward to a grid, so a small edit to the input doesn't force a new download.
- **Sources:** the eight existing connectors first:
  - TIGER/Line;
  - USGS National Map;
  - 3DEP elevation;
  - STAC imagery and DEM;
  - Overture;
  - Earthquakes and soils;
  - ArcGIS services;
  - OGC API.

  The Census tables importer and server connections reuse the node when they're built, each adding itself as a source.
- **Storage:** snapshots are project tables or project raster files, listed in the Catalog under a "Downloaded data" section with size and date. "Remove unused copies" frees space. A snapshot still used by a node can't be removed.
- **Running:** downloads go through `progress::Task`, can be cancelled, and run in the background (UR-9). A cancelled download leaves no snapshot.
  - If several nodes need downloads, the run asks once, listing them all with sizes.
- **From the dialogs:** each Public Data dialog gets "Add as a node", which places a node with the same settings in the active tree. Downloading as a layer stays.
- **Secrets:** sources that need a key or sign-in (Census, Earthdata, server connections) get it from the keychain at download time. It is never stored in the node, snapshot, project, log, error or citation.

## Work packages

| Id | Who | What | Done when |
|---|---|---|---|
| N1 | Sonnet | Shared connector description, so each connector says what it offers and how to fetch a request into a snapshot. Snapshot store in the project: table or raster, request hash, date, citation, size. Catalog "Downloaded data" section and removing unused copies. Project format addition with old projects opening unchanged. | Engine tests: same request, same snapshot; snapshot survives save, reopen and Save As; an old project opens |
| N2 | Sonnet | `public_data` ToolSpec and executor hook: run-time check for missing snapshots, one confirmation listing all downloads, offline refusal naming the node, area from an input with outward rounding, Refresh with a change report. Card UI. "Add as a node" in the eight dialogs. | Engine tests with a local fake server: a run with snapshots makes no request; offline run refused before any work; Refresh reports added, removed and changed; e2e: dialog → node → run → result |

Close review by Opus: the cache-key and snapshot identity (N1), and offline and refresh behaviour (N2). That's where a model could silently use the wrong or stale data.

## Order

N1 and N2 come after the Census importer's C3 (fetch engine), because both share the caching and background-download work, and before C5 (Census UI), so the Census importer ships with "Add as a node" from the start. That's the recommendation; the Census lead decides the exact slot.

## Not decided yet

- Whether an area from an input should follow every change of that input automatically. Recommended: yes, with outward rounding and the confirmation before any download.
- The rounding grid (for example 0.01° or 1 km), set in N1 by measuring typical edits.
