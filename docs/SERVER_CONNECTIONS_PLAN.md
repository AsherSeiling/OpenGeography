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

**Saved servers** appear under **Servers** in the Catalog, for reuse across projects. Private servers ask you to sign in (§3a).

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

## 3a. Signing in to private servers (user, 2026-11-04: in the first release)

**What's supported**
| Method | Used by |
|---|---|
| Username and password, exchanged for a short-lived token | ArcGIS Server and Enterprise (`generateToken`) |
| API key or token you paste | ArcGIS Online, hosted WMS and WMTS services |
| HTTP Basic or a bearer token | WMS, WMTS and WFS servers |

Browser sign-in with OAuth or company single sign-on is left for after release, because each provider needs its own registered app. A server that only offers those says so in a sentence.

**Rules for secrets** (`CLAUDE.md`: entered by the user, stored in the OS keychain, never in code, logs or projects)
- Secrets live only in the OS keychain, through the existing `src-tauri/src/secrets.rs`, under one entry per saved server. A generated token stays in memory and is renewed when it expires.
- **A project stores the server's address and a reference, never the secret.** Opening a project on another computer asks you to sign in there.
- **A pasted address containing `token=` or `key=` has it removed**, offered for the keychain, and never saved in the project, citation or layout.
- Secrets never reach the web page. The engine adds them when it fetches through `gis://`, so they can't appear in the UI's memory, layer URLs or error messages.
- Logs, errors and citations show the address with credentials removed. A test fails if a known test secret appears in any log, project file or error.
- Credentials go only to the host they were saved for, and redirects to another host drop them.
- **Credentials go only over https.** A plain http server with sign-in is refused in a sentence.
- Printing and Save view as raster go through the same engine path. If GDAL needs credentials, they're passed in memory through its HTTP options and **never written into a WMS definition file on disk**.
- The devserver keeps its existing test-only secret handling, so browser tests use fake credentials against the local fake server.

**What happens when access fails**
- An expired or wrong login shows a sign-in prompt once, not one per tile.
- A layer that can't sign in shows as unavailable and the project still opens.
- Remove Server deletes its keychain entry after asking.
- Layers from a private server say so in Properties, and an export reminds you that the output may contain restricted data.

---

## 4. Work packages

| WP | Work | Done when | Sessions |
|---|---|---|---|
| **S1** | Server layer type, project format, `gis://` fetching and cache, WMS and WMTS on the map, Add Server, saved servers, legend, identify, credit | A WMS and a WMTS layer draw in Web Mercator, a UTM CRS and a polar CRS, then survive save and reopen. A server that's offline fails in words. | 1–1.5 |
| **S2** | ArcGIS REST: browse a server, draw MapServer and ImageServer, identify, general ImageServer clip. Existing vector download moves into the same browser. | National Map drawn live, and a public ImageServer clip with the right CRS and citation | 1 |
| **S3** | Printing and Save view as raster, plus a driver check in `--self-test` | A pixel test on a WMS frame; a saved raster matches the map's area | 0.5 |
| **S4** | Sign-in (§3a): the three methods, keychain entries, token renewal, redaction, sign-in prompt | Each method works against the fake server; a project saved with a private layer contains no secret; the leak test passes. **Opus reads this diff closely.** | 1 |

**Total: about 3.5–4 sessions.** Run in one Sonnet session, S1 to S4 in order, per `docs/prompts/bugs-and-planetary-lead.md`.

**Tests:** a small local fake server for WMS, WMTS and REST so tests don't depend on the network, including slow, offline, malformed and sign-in responses; ignored network tests against USGS; CRS cases in Earth and, once planetary lands, on Mars; devserver parity for each new command.

---

## 4a. Every USGS service, with a way to download it (user, 2026-11-06)

**The user's rule:** if the USGS has a REST server, GIS Studio has it, and has a way to download data from it.

S1–S4 give the general machinery: browse any ArcGIS server, draw it, clip an ImageServer, save the view as a raster, and WFS (§5). This section makes it complete for the USGS: a directory of every USGS service, and a download path for every kind of service in it.

**The USGS directory.** A new Catalog › Public Data › **USGS** section lists every USGS REST service, grouped by theme:
- elevation, hydrography, boundaries, transportation and structures, names, land cover;
- geology and minerals, hazards, water data, imagery, biology and habitat;
- topographic maps, and planetary (the Astrogeology servers that Batch 3 needs).

It has the same kind of offline search as the Census importer (`docs/CENSUS_TABLES_PLAN.md` §2), sharing its search code, plus keywords. Each entry shows its publisher, a description, the link to the USGS page and what you can do with it: draw, download features, download a raster clip, or download files.

- **Built by a crawler script** (`scripts/usgs_directory.py`). It walks each USGS ArcGIS server's `?f=json` service directory (folders, services, layers) and the known non-ArcGIS APIs. It ships the result as a bundled resource with the crawl date, and the app can refresh it.
- **The list of USGS hosts to crawl is decided in U0.** It is checked against USGS's own listings, not guessed. It includes the National Map servers already used (`index.nationalmap.gov`, `basemap.nationalmap.gov`, 3DEP elevation) and every other USGS ArcGIS server U0 confirms.
- **Health check:** a script, also run in CI later, reports services that moved or stopped answering, so the directory doesn't silently rot. A dead service shows as unavailable with its reason, never as an empty download.

**A download path for every service type:**

| Service type | Download | Where built |
|---|---|---|
| FeatureServer / queryable MapServer layer | Features for an area, paged, with all fields | Built (`arcgis.rs`); moves into the S2 browser |
| ImageServer (elevation, imagery, land cover) | Raster clip at a chosen resolution, with size shown first | S2 (general clip; 3DEP-only today) |
| MapServer drawn as images only, or cached tiles | Save view as raster (a picture, not data; says so) | S3 |
| WMS / WFS (for example USGS mineral resources) | WMS draw and save view; WFS features | S1, and WFS per §5 (now included) |
| OGC API – Features (USGS Water Data) | Features, plus a USGS water preset: sites, then daily or continuous values for a date range as a table joined to sites | Built generically (`ogcapi.rs`); preset in U2 |
| Earthquakes (FDSN event API) | Built (`jsonapis.rs`) | Built |
| TNM Access products API | File products for an area: staged NHD, WBD and boundaries as GeoPackage, topo map GeoPDFs, 3DEP DEM tiles. Shows sizes first, downloads in the background, adds what it can open. | U2 |
| ScienceBase catalog | Search items, list attached files and services, download files, open what's readable (GeoTIFF, shapefile, GeoPackage, CSV) | U2 |
| Other USGS REST APIs that U0 finds | One small connector each, or a stated reason why not | U2 or later |

**Stated limits rather than gaps:** a download that needs something the app can't open yet says so plainly. For example 3DEP lidar point clouds (LAZ / EPT): the app can list them and save the files, but has no point-cloud support to open them (`docs/PUBLIC_DATA_CATALOG.md` P2).

**Every download goes through the same machinery:**
- the background downloads of UR-9, with cancel;
- citations with the service, layer and access date;
- the Public Data node (`docs/PUBLIC_DATA_NODE_PLAN.md`), so any USGS service can also be a model input;
- the CRS rule: a service's CRS is read from its metadata, and a service without one is refused, never guessed.

| WP | Work | Done when | Sessions |
|---|---|---|---|
| **U0** | Opus spike: inventory the USGS ArcGIS servers and other USGS REST APIs from USGS's own listings; check each answers, its CRS and query support; decide the crawl list and which APIs need a connector | A list in this section with each host, its service count and the download path, signed off by the lead | 0.5 |
| **U1** | Crawler and health-check scripts; bundled directory; Catalog USGS section with themes and search (shared with the Census search code) | Every crawled service appears with the right actions; the search query list passes; a dead service shows as unavailable | 1 |
| **U2** | Connectors: TNM Access products, ScienceBase, the Water Data preset, plus any U0 adds | Each works against a local fake server, and once against the real service in an ignored network test | 1–1.5 |

**Order:** U0–U2 come after S2 and S3 (they use the browser, ImageServer clip and save view) and after the Public Data node, so every USGS entry can be "Add as a node" from the start. They come before planetary, which uses the Astrogeology entries.

---

## 5. Decisions

**Defaults, unless the user changes them:**
| Topic | Default | Why |
|---|---|---|
| WFS | **Included** (user's USGS rule, 2026-11-06: some USGS services, such as mineral resources, are WFS) | Same dialog, but downloads rather than draws; GDAL's WFS driver reads it |
| WCS | Leave out | Rarely needed; ImageServer and STAC cover most raster downloads |
| OGC API – Maps and Tiles | Leave out for now | Not widely deployed yet |

**Decided by the user (2026-11-04):** sign-in to private servers is in the first release (§3a).
