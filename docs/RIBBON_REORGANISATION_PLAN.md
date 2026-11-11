# Sorting the top toolbar (ribbon)

Written 2026-11-09 from the user's request to "sort the top toolbar better". Tab layout approved by the user the same day. Planned, not built. Packages RB1–RB3; order in `docs/MASTER_PLAN.md`.

## What's wrong today

Tabs are defined in `ui/src/ribbon/tabsDef.tsx`.

- **The Map tab holds nearly everything:** 9 groups and 33 commands. It needs about 2,140 px at full size, so on most screens groups shrink to icons or fold into menus.
- **Specialist commands sit beside everyday ones.** Land Nav, World, datum grids and Tissot circles take up as much room as Select and Identify.
- **Analysis looks almost empty, though it's the app's largest area.** It shows Add Tool, Run and favourites. Well over 100 tools are reachable only through Shift+A or search.
- **Insert has only 2 groups.**
- **View mixes four unrelated things:** windows, appearance, program settings, and developer tools (Frame Rate, Benchmark, Generate Test Data).
- **View's Zoom group only works in layouts,** yet it takes space on the map too.
- **Duplicates:** Processing Nodes and Attribute Table are on both Analysis and View.
- **Feature Layer** has a group holding one button (Remove), and Census under Fields, though it brings in data.
- **Coming soon with no home:** the statistics workspace, server connections, Census.

## Rules for the new layout

1. **Tabs follow what you're doing:** look at the map, get data, set up coordinates, analyse, make outputs, arrange the window.
2. **Left to right by how often it's used,** inside each tab and each group.
3. **A tab should fit at full size on a 1440 px window:** about 5 groups. The existing adaptive shrinking stays for narrower windows.
4. **Each command appears once.** The one exception is Add Data, on both Map and Data, because it's the first thing anyone does and people will look in both places.
5. **A group only appears where it works.** Better hidden than permanently greyed out.
6. **No parallel catalogs.** Tool menus are built from `tool_specs`.

## The new tabs

Tab order: **Map, Data, Coordinates, Analysis, Insert, View**. Key tips M, D, C, A, I, V.

| Tab | Group | Commands |
|---|---|---|
| **Map** | Navigate | Explore, Full Extent, Go To Coordinates, Bookmarks, Swipe |
| | Layer | Add Data, Basemap |
| | Selection | Select; Select by Rectangle, by Attributes, by Location; Zoom To Selection; Clear |
| | Inquiry | Identify, Measure, Measure Area |
| | Edit | Start Editing, Undo Last Save |
| **Data** (new) | Add | Add Data, Public Data |
| | Create | New Layer, New Table |
| | Georeference | Georeference, Adjust Shapes |
| | Sources | Sources |
| | *(later)* | Server connections and Census go here when built |
| **Coordinates** (new) | Coordinate System | Coordinate System, Projection Builder, Datum Grids, Tissot Circles, Limits & Responsible Use |
| | World | World View, Graticule, World Outline |
| | Land Navigation | Resection, Intersection, MGRS Grid |
| **Analysis** | Geoprocessing | Add Tool, Run |
| | Tools | One dropdown each: Vector, Raster, Terrain and Hydrology, Spatial Statistics, Network, Tables |
| | Favourites | The existing shelf |
| | *(later)* | Statistics Sheet, from Phase ST |
| **Insert** | Layouts, Maps | Unchanged |
| **View** | Zoom | Only while a layout is open |
| | Windows | Unchanged, plus Reset Panes |
| | Appearance | Theme, Colours |
| | Program | Settings, Save and Reset Startup |
| | Developer | Frame Rate, Benchmark, Generate Test Data. Hidden unless Settings › Show developer tools is on |

**Feature Layer (contextual):** Layer gets Remove (the one-button group goes). Census moves from Fields to Data. Otherwise unchanged.

**Layout tabs and the Edit tab:** unchanged. `LAYOUT_ORDER` still applies; Map, Data, Coordinates and Analysis leave while a layout is open.

**Expected result:** the Map tab drops from 9 groups and 33 commands to 5 groups and about 19, needing roughly 1,150 px (estimated from the current group widths).

## Packages

| Task | Effort | What |
|---|---|---|
| RB1 | 0.5 | **Re-sort.** Change `TABS`, `MAIN_TABS` and `KEY_TIPS` in `tabsDef.tsx`, remove the duplicates, tidy Feature Layer. Command ids don't change, so quick access and search keep working. Update the specs that open a tab by name. |
| RB2 | 0.5 | **Tool dropdowns on Analysis,** built from `tool_specs` and grouped by `ToolSpec.category` with the same `splitCategory` the Shift+A menu uses. Picking a tool does the same as picking it in Shift+A. Long menus use sub-groups. New tools appear with no ribbon change. |
| RB3 | 0.25 | **Conditional groups and the developer setting.** Zoom only in layouts. Developer group hidden by default, with a Settings checkbox stored per user. e2e turns it on through a dev hook. |
| RB4 | 0.5 | **Go To Coordinates** (below). |

**Total:** about 1.75 sessions.

**Order:** with the colour picker, after Wave 1 and before Wave 2, because Wave 2 adds ribbon search and favourites. The files don't overlap, so the two can be separate lanes.

## Go To Coordinates (RB4, user, 2026-11-09)

Type a position and the map goes there. Nothing does this today: typed coordinates exist only inside shape editing (`editParseCoords`, tied to a layer).

**Where:** Map › Navigate, opening a small panel that doesn't block the map. A keyboard shortcut if Ctrl/Cmd+G is free.

**Formats,** chosen from a list that starts on the status bar's format (UX14) and is remembered per computer:
- Latitude and longitude: decimal degrees, or degrees, minutes and seconds
- Map coordinates: x and y in the map's own coordinate system
- UTM: zone, hemisphere, easting, northing
- USNG / MGRS
- Any other coordinate system, from the existing picker

**Correctness:**
- Latitude and longitude are separate, labelled boxes, so their order can't be misread. Pasting "34.05, -118.25" fills them as latitude then longitude, and N, S, E or W letters override that.
- A latitude beyond 90° is refused. If the numbers look swapped, the message says so, but they are never swapped silently.
- The datum is always shown. Latitude and longitude default to WGS 84, with NAD 83 selectable; USNG uses the existing datum choice.
- The position moves to the map's coordinate system through an explicit engine transform.
- Before going, the panel shows how it read the input, in plain text, plus the same place in the other formats, with a copy button.
- A place the map's coordinate system can't show gets a readable message, never a jump to the wrong place. A map with no coordinate system yet says to set one first.

**What it does:** centres the map, keeping the current scale unless a scale is typed, and drops a temporary marker with a Clear button. It creates no layer and changes no data.

**How:** one new engine command, added to Tauri and the devserver, that reuses and extends the parser in `edit_tools.rs` rather than adding a second one, and returns the position in the map's CRS plus every format. Movement reuses the code behind bookmarks.

## Tests (add to `docs/TEST_PLAN.md`)

- `reach.test.ts` and the control audit: every command is still reachable, and none was lost in the move. Allow only Add Data as a duplicate.
- `ribbon-width.spec.ts`: add the new tabs. New check: the Map tab shows no collapsed group at 1440 px.
- Every `ToolSpec` appears in exactly one Analysis dropdown, except inputs and outputs, which stay in Shift+A.
- The Developer group is absent by default, and appears after the setting is turned on.
- Zoom appears on View in a layout and not on the map.
- An old project and a saved quick-access list work unchanged.
- Go To Coordinates: known answers for each format, including the southern and western hemispheres, UTM and polar USNG/MGRS (checked against the existing NGA GeoTrans values); swapped and out-of-range input refused; the marker lands at the right place on UTM and Web Mercator maps.

**Not in this plan:** a Raster Layer contextual tab, user-customisable ribbons, and changing the icons (the two new tabs reuse the existing ones).
