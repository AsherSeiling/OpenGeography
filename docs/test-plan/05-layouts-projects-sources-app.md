# 5. Layouts, projects, sources, secrets, IPC parity, UI shell, packaging

Existing: TP §9 (layouts), §11 (sources), §12 (projects), §14 manual cases. Conventions: [`../TEST_PLAN_FULL.md`](../TEST_PLAN_FULL.md#conventions).

## 5.1 Layouts (`list_layouts`, `save_layout`, `delete_layout`, `render_layout_preview`, `export_layout`; `crates/engine/src/layout/`, `ui/src/layout/`)

| ID | Pre | Steps | Expected (user-facing) | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| LAY-01 | — | Export PDF/PNG/SVG with no map frame; with a frame whose layer was deleted; with all layers hidden | Explained up front / "layer no longer exists" / empty frame with a notice; never a raw error | V+E | partial: crashes.test.ts (no frame); to automate deleted layer | P0 |
| LAY-02 | Layout open | Change the map CRS in the map tab | Layout frame CRS unchanged (frame has own CRS); map view update does not touch layout | 2E | to automate | P0 |
| LAY-03 | Layout with frame in UTM | Change layer's CRS (Define Projection) then export | Rendered in frame CRS from the new definition; stale preview refreshed | E+2E | to automate | P0 |
| LAY-04 | — | Frame CRS: Mercator at 80°N, polar frame, antimeridian-crossing frame, ortho (data beyond horizon), geographic at lat 90 | Graticule/scale-bar warnings; no NaN; features beyond horizon dropped with a note | E | partial: TP 9.9, 9.11; to automate antimeridian and ortho | P0 |
| LAY-05 | — | Scale 1:1, 1:1e9, 0, NaN, text "abc"; page 1×1 mm, 5 m × 5 m, custom units | Rejected/clamped with message (cleanPatch); export limits stated ("page too large for PNG at 300 dpi") | V+E | partial: crashes.test.ts, types.test.ts; to automate engine limits | P0 |
| LAY-06 | — | PNG at 72, 150, 300, 600 dpi on A0; 1 000-dpi | Memory guard "image would be N megapixels; limit M"; progress and cancel | E (perf) | partial: TP 9.5, 9.8 | P0 |
| LAY-07 | — | Layout with 100 k polygons, 1 M points | Renders < budget; PDF stays vector and size sane (< 200 MB) or rasterises dense layers with note | E (perf) | partial: TP 9.5 | P1 |
| LAY-08 | — | Fonts: unicode text (CJK, Arabic RTL, emoji, combining marks), very long text, `{scale}` tokens inside; bold/italic fonts missing | Falls back with a notice; embedded in PDF; no tofu silently | E | partial: TP 9.1 (embedded text); to automate unicode | P1 |
| LAY-09 | — | Legend with 0 layers, 200 classes, nested tiny frame | "Does not fit" reported (TP 9.10); multi-column | E | partial: TP 9.10 | P1 |
| LAY-10 | — | North arrow at various convergence; scale bar in feet / km / miles; scale bar 10 m (tiny) / 5 000 km | Correct units & round values | E | partial: TP 9.9 | P1 |
| LAY-11 | — | Graticule spacing 0, tiny (millions of lines), labels at edges, polar | Capped with message; no hang | E | partial: TP 9.11 | P1 |
| LAY-12 | — | Undo/redo per layout (insert, move, resize, delete, page change, frame set from map); undo after save and reopen | Per layout stack; redo cleared by new edit; empties after reopen | V | partial: TP 9.15 | P1 |
| LAY-13 | — | Duplicate / rename / delete layout with the same name, unicode, 300 chars; delete while its tab is open & exporting | Name uniqueness; export cancelled on delete | V+E | partial: TP 9.15; to automate name rules | P1 |
| LAY-14 | — | Two layouts open; the bug of cyclic updates (U-class) – open project with 3 layouts | No "Maximum update depth" | V | auto: crashes.test.ts | P0 |
| LAY-15 | — | Map frame "Set from Map View" when map has no layers / invalid CRS / OSM basemap | Message; basemap copied (U-1) | V | partial: layout.test.ts basemap test | P1 |
| LAY-16 | — | Export atomic: target in read-only folder / on a full disk / cancel | No partial file; readable message | E | partial: TP 9.8 | P0 |
| LAY-17 | — | Export while the layout is being edited | Snapshot at start; or edits blocked | 2E | to automate (CON-09) | P1 |
| LAY-18 | — | Layout in a project saved on macOS and opened elsewhere (fonts, paths, units) | Identical render hash within tolerance (golden per OS) | E+PK | partial: TP 9.1 golden (macOS); manual M-24 | P1 |
| LAY-19 | — | Layout view usability: zoom 10 %–800 %, snap, keyboard nudge, multi-select, lock | Behaves; no jank | M | manual M-19 | P2 |
| LAY-20 | — | Labels in layouts L2/L3 (collision, overrides, pinned) | — | E+V+2E | partial: tests/layout.rs l2_*, l3_*; ui labelEdits.test.ts; e2e/labels.spec.ts; to automate more than 50 000 features, labels over a basemap, rotated frame (TEST_PLAN 9.16g) | P1 |
| LAY-21 | — | Map series, templates, Topo Map, GeoPDF, rulers, dpi box | — | — | pending feature (C3, C4) | P2 |
| LAY-22 | — | Preview (`render_layout_preview`) debounce: 30 edits in 1 s | One render at the end; stale previews discarded | V+E | to automate | P1 |

## 5.2 Projects (`save_project`, `open_project`, `new_project`)

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| PRJ-01 | — | Save → quit → relaunch → Open (including the fixed U-4: ribbon Open with no args) | Opens; folder picker used; no JSON serialisation error | V | auto: openProject.test.ts | P0 |
| PRJ-02 | Cyclic state | Any command run from ribbon/menus passes no event objects | All ribbon `run()` called without args | V | to automate (iterate `commands`) | P0 |
| PRJ-03 | — | Open a project folder from a **newer** engine version / schema | "This project was made by a newer GIS Studio (v X)" and stays closed | E | to automate | P0 |
| PRJ-04 | — | Open a project with a missing raster file, missing source file (absolute path moved), missing layout reference | Opens; missing items flagged "file not found: …" in Contents/Sources; no crash | E | partial: TP 12.9 (gap); to automate | P0 |
| PRJ-05 | — | Corrupt DuckDB (truncate 50 %), zero-byte db, locked by another process, read-only folder | Readable error; current session untouched (TP 12.4 covers non-project / corrupt) | E | partial: TP 12.4; to automate truncation/lock/readonly | P0 |
| PRJ-06 | — | Save As to the same folder, into itself, into a subfolder of the project, onto a file, onto a path with spaces/unicode/long, onto external drive removed during save | Refused or atomic; original intact if the save fails | E | partial: TP 12.5; to automate | P0 |
| PRJ-07 | — | Save with 50 layers, 1 GB data: progress, cancel | Progress shown; cancel leaves previous save intact (write to temp then swap) | E (perf) | to automate | P0 |
| PRJ-08 | — | Interrupted save (kill process at 50 %) | Previous project opens | E | to automate (fault injection) | P0 |
| PRJ-09 | — | Recent projects list: add, reorder, missing folder entry, 20+ entries, clear, unicode | Missing entry removed on click with message; max N; persists | V+2E | pending feature: no recent-projects list exists in `ui/src` or the engine today (grep `recent` finds nothing); write these cases with the feature | P1 |
| PRJ-10 | — | Dirty marker: set by each of: edit, symbology, layout change, rename, define CRS, node edit; not set by pan/zoom/selection | Exact | 2E | partial: M-23 e2e | P1 |
| PRJ-11 | — | Unsaved-changes prompt on New / Open / Quit / window close / OS shutdown | Save / Discard / Cancel each work | 2E | partial: e2e M-23 | P0 |
| PRJ-12 | — | Project round trips: symbology, definition queries, selection (not saved), map CRS, camera, pane layout, basemap, layouts, tree with cache, sources | Equal | E+V | partial: TP 12.1 | P0 |
| PRJ-13 | — | Open a project on a different OS / user / path (macOS → Windows) | Layers, rasters, layouts load; absolute source paths flagged not failed | PK | manual M-24 | P1 |
| PRJ-14 | — | Project folder name with spaces/unicode; `.gproj` extension missing/doubled | Normalised | E | to automate | P1 |
| PRJ-15 | — | Two app instances open the same project | Second says "already open" (lock) or read-only | M | manual | P1 |
| PRJ-16 | — | Autosave/crash recovery | — | — | pending feature (none exists) – record behaviour | P2 |
| PRJ-17 | — | Large number of undo steps in project; ui state with 1e21 numbers, unicode (TP 12.1) | Saved verbatim | E | auto: TP 12.1 | P2 |
| PRJ-18 | — | Layer ids and counters after delete/reopen | No collisions | E | auto: TP 12.6 | P1 |

## 5.3 Sources and citations (`sources`, `update_source`, `remove_source`)

Existing: TP 11.1–11.10. Additional:

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| SRC-01 | — | Remove a source used by layers in a saved layout's Sources element | Layout shows remaining; no error | E | to automate | P1 |
| SRC-02 | — | Citation with 5 000-char title, HTML in title, `\u0000`, RTL; export formats | Escaped; BibTeX keys unique; no crash | V | partial: TP 11.8 | P1 |
| SRC-03 | — | Auto-export bibliography next to project when the folder is read-only / when project not saved yet | Message; nothing thrown | E+2E | to automate | P1 |
| SRC-04 | — | `update_source` on a missing key / concurrent edit from two panes | Ignored with message / last wins | E | partial: TP 11.2 | P2 |
| SRC-05 | — | Derived layer from 30 inputs | Each cited once; list readable | E | partial: TP 11.3 | P2 |
| SRC-06 | — | Date accessed uses local time zone and ISO date; leap day; year 0 | Correct | V | partial: cite tests | P2 |

## 5.4 Secrets (keychain)

The keychain UI is **not built** (Census join ⬜; Earthdata token for Phase D/E; `grep keyring` finds nothing today). Cases are recorded so they are written with the feature.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| SEC-01 | Feature built | Enter, save, retrieve a Census key; restart app | Stored in the OS keychain only; UI shows masked | M+PK | pending feature | P0 |
| SEC-02 | — | Search project folder, DB, logs, `frontend_log`, crash dumps, exports, bibliography, layouts for the key text | Not present anywhere | E+M | pending feature (grep-based test) | P0 |
| SEC-03 | — | Keychain locked/denied (macOS prompt cancelled, Linux no secret-service, Windows credential manager error) | "Could not access the system keychain: …"; app keeps working without the key; no plaintext fallback | M | pending feature | P0 |
| SEC-04 | — | Wrong/revoked key: request fails with 401/403 | "The service rejected your key" naming where to change it; key not echoed | E | pending feature | P0 |
| SEC-05 | — | Delete key; key with spaces/newline/unicode paste | Trimmed; removal verified | M | pending feature | P1 |
| SEC-06 | — | Error pop-up "Copy details" and bug reports never include the secret | Redacted (`****`) | V | pending feature (add redaction test now for URL query `key=`) | P0 |

## 5.5 IPC command parity (Tauri `commands.rs` ↔ devserver `crates/devserver/src/main.rs`)

73 `#[tauri::command]` functions; 71 devserver routes. Existing guard: none automated.

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| CMD-01 | — | Parse command names in `commands.rs` and `main.rs` (generate_handler list vs devserver match arms) | Every Tauri command has a devserver twin (known exceptions listed in the test: dialogs, `frontend_log`) | E (script) | to automate: `scripts/check_ipc_parity.py` | P0 |
| CMD-02 | — | Every `ui/src/ipc/api.ts` wrapper matches a command and its argument names (camelCase↔snake_case) | No dangling wrapper | V | to automate | P0 |
| CMD-03 | — | For each command, call with missing, extra, wrongly typed args | Error is a string sentence ("missing field `layer`") mapped to a friendly message; process stays up | E | to automate (devserver HTTP) | P0 |
| CMD-04 | — | For each command with a layer id: unknown / empty / `"; DROP"` / 1 000-char id | "Unknown layer …" | E | partial: TP 5.13 | P0 |
| CMD-05 | — | Serialisation: NaN/Infinity floats, 64-bit ints > 2^53, huge strings, cyclic input (U-4) | Round-trips or readable error | E+V | partial: api.test.ts | P0 |
| CMD-06 | — | Dev server bound to 127.0.0.1 only; CORS limited to localhost origins | Not reachable from LAN | E | to automate | P0 |
| CMD-07 | — | Commands that block > 1 s run off the UI thread (`blocking`) | UI stays responsive while `run_graph` runs | E+PK | partial: TP 13 manual | P1 |
| CMD-08 | — | `frontend_log` with 10 MB message, rapid 10 000 calls | Truncated; no memory blowup | E | to automate | P2 |

## 5.6 UI shell: ribbon, panes, dialogs

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| UI-01 | — | For every ribbon command (`commands.tsx`: 65) in each tab: with no project / no layer / Table layer / raster layer / in layout tab: enabled state correct; click does not throw | Disabled with tooltip reason, or runs; no pop-up crash | V | to automate (iterate `commands`, fake store) | P0 |
| UI-02 | — | Every ribbon tab renders; group labels present; keyboard shortcut list (Ctrl+O, Ctrl+S) | pass | 2E | partial: e2e M-8 | P1 |
| UI-03 | — | Each pane (contents, catalog, map, table, model, properties, tools, sources): open, close, float, dock, reset, restore after reopening; throw inside pane → PaneBoundary | Pane box with "Reopen"; other panes unaffected | 2E+V | partial: e2e M-8; crashes.test.ts | P0 |
| UI-04 | — | Contents: rename (blank, 300 chars, duplicate), reorder, context menu entries for each geometry type/Table/raster, "Show original" toggle | Rules per BUGS B-T2 | 2E | partial: TP 5.14 | P1 |
| UI-05 | — | Properties pane / Layer Properties window: Source, Symbology tabs for each layer type | No blank tabs; invalid input messaging | 2E | to automate | P1 |
| UI-06 | — | Geoprocessing pane (`ToolsPane`) lists all 51 tools, search, drag onto model canvas | Tool list = `tool_specs`; no parallel catalog | 2E | to automate (list equals specs) | P0 |
| UI-07 | — | Dialogs (Add Field, Calc Field, Calc Geometry, Select by Attributes/Location, Define Projection, Rename, Export, TIGER, National Map, Elevation, Benchmark, Query): Esc, Enter, double-click OK, close during busy | One submit; errors inline in the dialog; busy state | 2E | partial: M-11/M-12 e2e | P0 |
| UI-08 | — | Keyboard-only use: tab order, focus ring, Escape handling; screen reader names on ribbon buttons | Reasonable | M | manual | P2 |
| UI-09 | — | Window sizes 1024×600 → 4K; ribbon collapses; dark theme contrast | Usable | M | manual (extend M-8) | P2 |
| UI-10 | — | Status bar: coordinates, CRS name, scale, selection count, tasks; no project | Correct in each state | 2E | partial: M-7 | P2 |
| UI-11 | — | Test-data command "Generate Test Data" with extreme counts | See GEN-01 | 2E | to automate | P2 |
| UI-12 | — | Coming-soon commands (Topo Map) | Friendly "coming soon" dialog, not an error | V | to automate | P2 |

## 5.7 Cross-platform and paths

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| XPL-01 | macOS/Win/Linux | Path matrix: spaces, `é`, CJK, emoji, 200+ chars, trailing dot/space (Windows), `#`, `%`, `&`, leading `-`, case-only differences, symlinks, network share | Import, export, save, open, bibliography, layout export all succeed or fail with the path in the message | E+PK | partial: TP 1.11; to automate as one table-driven test run in CI on 3 OS | P0 |
| XPL-02 | Windows | `\\?\` long paths, drive-letter case, CRLF files, 8.3 names | Works | E | to automate on windows CI | P1 |
| XPL-03 | Linux | No display / minimal fonts / no secret-service; AppImage vs deb libs (`linux-private-libs.sh`) | Starts, `--self-test` passes | PK | manual M-26 | P0 |
| XPL-04 | All | No hard-coded paths or OS checks outside `resources.rs` / `raster.rs::store_dir` | grep guard test | E (script) | to automate: `scripts/check_no_os_checks.sh` | P1 |
| XPL-05 | All | Time zone / locale (decimal comma, 24 h) in dates, number parsing in table cells | Stable | V | to automate | P2 |
| XPL-06 | All | Project written on one OS opens on others (golden project fixtures checked into `crates/engine/tests/fixtures`) | pass | E | partial: manual M-24 | P1 |

## 5.8 Packaging, `--self-test`, installers

| ID | Pre | Steps | Expected | Lvl | Auto | Pri |
|---|---|---|---|---|---|---|
| PKG-01 | Packaged app | `--self-test` in a clean env (`env -i`) | `SELF-TEST OK` incl. spatial extension, GDAL, PROJ, State Plane transform, vector tile, DEM→PNG tile, contours | PK | auto: `run-test-plan.sh --package` (macOS) | P0 |
| PKG-02 | — | `--self-test` with resources dir renamed/missing, read-only HOME, no write to temp | `SELF-TEST FAILED: <sentence>` exit 1, never panic backtrace | PK | to automate | P0 |
| PKG-03 | — | Self-test extended coverage: CRS grid shift file present, TIGER base URL reachable (optional flag), keychain access (when built), write+read a project, export PDF layout | Report lists each | PK | to automate (extend `selftest.rs`) | P1 |
| PKG-04 | Win | NSIS + MSI install, run, `--self-test`, uninstall leaves no files; upgrade over old version keeps projects | Pass | PK | manual M-26 | P0 |
| PKG-05 | Linux | AppImage and `.deb` start and self-test on Ubuntu 22.04/24.04 and Fedora | Pass | PK | manual M-26 | P0 |
| PKG-06 | macOS | DMG opens; first launch Gatekeeper path; app works offline; arm64 vs Intel | Pass; unsigned state documented | PK | manual M-26 | P1 |
| PKG-07 | All | `third_party_notices.py` up to date after dependency change; license gate | `scripts/license-gate.sh` passes | CI | auto: `scripts/license-gate.sh` in CI | P1 |
| PKG-08 | All | Bundled resources size and file list stable (no dev files, no `.claude/`) | List matches | PK | to automate | P2 |
| PKG-09 | All | Version number in About/title equals tag | Equal | PK | to automate | P2 |
| PKG-10 | Fresh worktree | `npm run build && scripts/stage-resources.sh` then `cargo build -p gis-studio` | Builds (empty `resources/` fails) | CI/manual | manual (documented) | P2 |
