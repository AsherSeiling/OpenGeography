# Server connections plan: REST and WMS

_Written 2026-11-04 at the user's request. Plan only; nothing here is built. This replaces the earlier decision not to build WMS/WMTS display (`docs/DECISIONS.md`, Phase D6)._

**Goal:** paste a server address, browse what it offers, and add its layers to a map. They stay live on the server, work on Earth and planetary maps, and can be printed or saved locally where that's allowed.

**Where it fits:** after the bug batch and the Mac app test, and **before planetary**. Planetary's zoomed-in basemaps use the USGS WMS, so they can build on this instead of adding a second WMS path.

---

## 1. What exists today

| Capability | State |
|---|---|
| ArcGIS REST: download vector features from a MapServer or FeatureServer, by bundled catalogue or pasted address (`arcgis.rs`) | Built |
| ArcGIS REST: draw a service live on the map | Not built |
| ArcGIS REST: browse a whole server's folders and services | Not built; only one service at a time |
| ArcGIS ImageServer: download a clip | Only for 3DEP (`tnm.rs`), not general |
| OGC API – Features download (`ogcapi.rs`) | Built |
| WMS and WMTS on the map | Not built; the map only draws XYZ tiles, vector styles and the bundled raster |
| WMS, WMTS and WFS in GDAL | The local build has the drivers. Windows and Linux builds need checking in `--self-test`. |
| Printing from a tile service | Built for USGS through GDAL's WMS driver (`layout/basemap.rs`) |

Checked 2026-11-04: the USGS planetary WMS and National Map REST server both answer and send `Access-Control-Allow-Origin: *`. Many other servers won't, which is why §3 fetches through the engine.

---

## 2. What the user sees

**Add Server.** A new item in Catalog › Public Data and on the Data tab:
1. Paste an address. The app works out the type: WMS, WMTS, ArcGIS REST, and WFS if included (§5). If it can't tell, it says so in a sentence.
2. Browse the server's folders and layers, with name, description, scale range and coordinate systems.
3. Add to Map, or for data services, Download.

**Saved servers** appear under **Servers** in the Catalog, for reuse across projects. Only public servers for now; see §5.

**Server layers in Map Layers** look like other layers, with an icon showing they're live:
- on/off, order, opacity, and Zoom To Layer;
- Properties shows the address, server layer, CRS, credit and legend;
- identify, through the server's own query (`GetFeatureInfo` or `identify`);
- no node tree or geoprocessing input, since there's no local data. A tool asking for one explains how to download a copy first.

---

## 3. How it works

| Topic | Rule |
|---|---|
| **Layer type** | One new `server` layer type in the project, holding the address, protocol, server layer, chosen style or format, and credit. Layers stay project-wide and each map chooses them, as today. |
| **Coordinate systems** | Request the map's own CRS if the server lists it. Otherwise OpenLayers reprojects from a CRS the server does offer, and the layer says so. **If there's no usable CRS or the body differs, it's refused in a sentence. Nothing is guessed.** |
| **Fetching** | Through the engine's `gis://` protocol and the devserver, which solves servers without CORS headers and gives one disk cache with a size limit |
| **Politeness** | A clear User-Agent, limited parallel requests, retries with backoff, and Cancel |
| **Failure** | A server that's down or changed doesn't stop a project opening. The layer shows as unavailable with the reason. |
| **Untrusted content** | Server text is shown as plain text, never as HTML. Only http and https addresses are accepted, and http gets a visible note. |
| **Citations** | `sources.rs` records the server, layer, publisher and access date, and derived output keeps them |
| **Project file** | Saved in the project, with a format version bump. If planetary hasn't shipped by then, the two changes can share one version. |

**Printing.** Layouts render through GDAL's WMS and WMTS drivers, and ArcGIS through its `export` call, warped to the frame's CRS with the disk cache. Servers can have their own terms, which the app can't read for an address someone pasted. So a user-added server prints with its credit, and the export says it's your responsibility to check the provider's terms. Built-in basemaps with known restrictions stay refused.

**Saving a copy.** "Save view as raster" clips the current area into a local raster for analysis, showing the size first and using the same limits and terms note.

---

## 4. Work packages

| WP | Work | Done when | Sessions |
|---|---|---|---|
| **S1** | Server layer type, project format, `gis://` fetching and cache, WMS and WMTS on the map, Add Server, saved servers, legend, identify, credit | A WMS and a WMTS layer draw in Web Mercator, a UTM CRS and a polar CRS, then survive save and reopen. A server that's offline fails in words. | 1–1.5 |
| **S2** | ArcGIS REST: browse a server, draw MapServer and ImageServer, identify, general ImageServer clip. Existing vector download moves into the same browser. | National Map drawn live, and a public ImageServer clip with the right CRS and citation | 1 |
| **S3** | Printing and Save view as raster, plus a driver check in `--self-test` | A pixel test on a WMS frame; a saved raster matches the map's area | 0.5 |

**Total: about 2.5–3 sessions.** Run in one Sonnet session, S1 then S2 then S3, per `docs/prompts/bugs-and-planetary-lead.md`.

**Tests:** a small local fake server for WMS, WMTS and REST so tests don't depend on the network, including slow, offline and malformed responses; ignored network tests against USGS; CRS cases in Earth and, once planetary lands, on Mars; devserver parity for each new command.

---

## 5. Decisions

**Defaults, unless the user changes them:**
| Topic | Default | Why |
|---|---|---|
| Sign-in | Public servers only at first. If a server asks for a sign-in, it says so in words. | Tokens need the OS keychain and a separate flow; `CLAUDE.md` says never to store them elsewhere |
| WFS | Include if it's cheap once the dialog exists, since GDAL's WFS driver reads it | Same dialog, but downloads rather than draws |
| WCS | Leave out | Rarely needed; ImageServer and STAC cover most raster downloads |
| OGC API – Maps and Tiles | Leave out for now | Not widely deployed yet |

**Question for the user:** should people be able to sign in to private or token-protected servers in the first release? That adds about one more session. *Recommended: public only, and add sign-in after release.*
