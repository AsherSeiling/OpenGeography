// Every ribbon command lives in this one registry. The ribbon layout, menus
// and the "Tell me" search all read from it.
import { createElement } from "react";
import { toolIcon } from "../nodes/toolIcon";
import { openToolMenu } from "../nodes/toolMenuStore";
import { PUBLIC_DATA } from "../panes/CatalogPane";
import { toggleAppearance, useAppearance } from "../theme/appearance";
import { addToolFromUi, focusPublicData } from "./addTool";
import { RIcon } from "./icons/RIcon";
import { useGeoref } from "../georef/store";
import { useAdjustWindow } from "../georef/AdjustWindow";
import { editCommandList } from "../edit/editCommands";
import { resetStartupLayout, saveStartupLayout, useSettings } from "../store/settings";
import type { ReactElement } from "react";
import {
  addData,
  exportLayer,
  splitByAttribute,
  exportTable,
  layerFromSelection,
  openDefinitionQuery,
  newProject,
  openProject,
  pickMapCrs,
  addTissotCircles,
  addWorldOutline,
  addWorldReference,
  addUsngGrid,
  toggleWorldView,
  removeLayer,
  runModel,
  saveProject,
  useDialog,
  zoomToLayer,
  zoomToSelection,
} from "../actions";
import { zoomToExtent } from "../map/mapController";
import { duplicateMap, newMap } from "../maps/mapActions";
import { openLayerProperties } from "../dialogs/LayerPropertiesWindow";
import { useNav } from "../nav/navStore";
import { useGoTo } from "../goto/gotoStore";
import { exportLayout, insertMapFrame, insertOverview, insertPicture, insertSurround, newLayout, pickFrameCrs, setFrameFromMapView } from "../layout/actions";
import { addChart, addGuide, addShape, addTable, addText, clearGuides, useLayouts } from "../store/layout";
import { toggleLockSelected } from "../layout/clipboard";
import { useLayoutView } from "../layout/view";
import { deleteStatsSheet, newStatsSheet, renameStatsSheet, runAllStatCards, runStatCard } from "../stats/actions";
import { exportCardChart, exportCardTable, exportSheetReport } from "../stats/share";
import { useStats } from "../stats/store";
import { defaultLayout, isPaneOpen, showPane, togglePane, type PaneId } from "../panes/panes";
import { useApp, type MapTool } from "../store/store";

export interface Command {
  id: string;
  label: string;
  icon: ReactElement;
  run: () => void;
  tip?: string;
  keys?: string;
  enabled?: () => boolean;
  checked?: () => boolean;
}

const app = useApp.getState;
const active = () => {
  const s = app();
  return s.activeLayer && s.layers[s.activeLayer] ? s.layers[s.activeLayer] : null;
};
/** The active layer's own data is shown (not a read-only node result). */
const canEditActive = () => {
  const a = active();
  return !!a && (!app().results[a.id] || !!app().showOriginal[a.id]);
};
const hasSelection = () => Object.values(app().selection).some((f) => f.length > 0);
const setTool = (t: MapTool) => () => app().setTool(t);
const tool = (t: MapTool) => () => app().tool === t;
const inLayout = () => !!useLayouts.getState().doc;
/** The open statistics sheet and its selected card (the workspace's commands act on these). */
const openSheet = () => (useStats.getState().id ? (useStats.getState().docs[useStats.getState().id!] ?? null) : null);
const pickedCard = () => {
  const st = useStats.getState();
  const sheet = openSheet();
  return sheet && st.selected ? (sheet.cards.find((c) => c.id === st.selected) ?? null) : null;
};
const hasFrame = () => !!useLayouts.getState().doc?.elements.some((e) => e.kind === "map_frame");

/** A Windows button: pressed while the pane is open, a click opens or closes it. */
function paneToggle(id: string, pane: PaneId, label: string, icon: ReactElement, tip: string): Command {
  return { id, label, icon, tip, run: () => togglePane(pane), checked: () => isPaneOpen(pane) };
}

const list: Command[] = [
  // Project
  { id: "new", label: "New", icon: <RIcon name="new" />, run: newProject, tip: "Start a new project" },
  { id: "startPage", label: "Start Page", icon: <RIcon name="new" />, run: () => useDialog.getState().show({ kind: "start" }), tip: "Recent projects, new project from a template, sample data" },
  { id: "open", label: "Open", icon: <RIcon name="open-project" />, run: openProject, keys: "Ctrl+O", tip: "Open a .gproj project folder" },
  { id: "save", label: "Save", icon: <RIcon name="save" />, run: () => saveProject(), keys: "Ctrl+S", tip: "Save the project" },
  { id: "saveAs", label: "Save As", icon: <RIcon name="save-as" />, run: () => saveProject(true), tip: "Save the project to a new folder" },

  // Map tab
  { id: "explore", label: "Explore", icon: <RIcon name="explore" />, run: setTool("pan"), checked: tool("pan"), tip: "Pan and zoom the map" },
  { id: "fullExtent", label: "Full Extent", icon: <RIcon name="full-extent" />, run: zoomFull, tip: "Zoom to all layers" },
  { id: "goToCoordinates", label: "Go To Coordinates", icon: <RIcon name="go-to-coordinates" />, run: () => useGoTo.getState().setOpen(true), checked: () => useGoTo.getState().open, keys: "Ctrl+G", tip: "Type a position (latitude and longitude, UTM, USNG/MGRS, map x and y, or another coordinate system) and go there. Drops a temporary marker; changes no data." },
  { id: "addData", label: "Add Data", icon: <RIcon name="add-data" />, run: () => addData(), tip: "Add shapefiles, GeoPackages, GeoJSON, KML, CSV, GeoParquet…" },
  { id: "select", label: "Select", icon: <RIcon name="select" />, run: setTool("select"), checked: tool("select"), tip: "Click features to select (Shift adds, Ctrl toggles)" },
  { id: "selectBox", label: "Select by Rectangle", icon: <RIcon name="select-box" />, run: setTool("selectBox"), checked: tool("selectBox"), tip: "Drag a box to select features" },
  { id: "selectByLocation", label: "Select By Location", icon: <RIcon name="select-location" />, run: () => active() && useDialog.getState().show({ kind: "selectByLocation", layer: active()!.id }), enabled: () => !!active() && Object.keys(app().layers).length > 1, tip: "Select features by where they are relative to another layer's features" },
  { id: "selectByAttributes", label: "Select By Attributes", icon: <RIcon name="select-attributes" />, run: () => active() && useDialog.getState().show({ kind: "selectByAttributes", layer: active()!.id }), enabled: () => !!active() },
  { id: "zoomSelection", label: "Zoom To Selection", icon: <RIcon name="zoom-selection" />, run: zoomToSelection, enabled: hasSelection },
  { id: "clearSelection", label: "Clear", icon: <RIcon name="clear-selection" />, run: () => app().clearSelection(), enabled: hasSelection, tip: "Clear the selection in all layers" },
  { id: "identify", label: "Identify", icon: <RIcon name="identify" />, run: setTool("identify"), checked: tool("identify"), tip: "Click a feature to see its attributes" },
  { id: "measure", label: "Measure", icon: <RIcon name="measure" />, run: () => { useNav.getState().set({ measureMode: "distance" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "distance", tip: "Geodesic distance and azimuths: click points, double-click to finish" },
  { id: "measureArea", label: "Measure Area", icon: <RIcon name="measure-area" />, run: () => { useNav.getState().set({ measureMode: "area" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "area", tip: "Geodesic area and perimeter" },
  { id: "resection", label: "Resection", icon: <RIcon name="resection" />, run: () => useNav.getState().set({ navPanel: "resection" }), checked: () => useNav.getState().navPanel === "resection", tip: "Find your position from azimuths to known landmarks (MGRS, mils, G-M angle)" },
  { id: "intersection", label: "Intersection", icon: <RIcon name="intersect" />, run: () => useNav.getState().set({ navPanel: "intersection" }), checked: () => useNav.getState().navPanel === "intersection", tip: "Locate an unseen point from azimuths taken at known positions" },
  { id: "census", label: "Census Data", icon: <RIcon name="people" />, run: () => active() && useDialog.getState().show({ kind: "census", layer: active()!.id }), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Join American Community Survey or 2020 Census values to the active layer by GEOID" },
  { id: "worldView", label: "World View", icon: <RIcon name="globe" />, run: toggleWorldView, checked: () => app().worldView, tip: "Show the whole Earth in this coordinate system (cut at the opposite side of its central meridian)" },
  { id: "usngGrid", label: "MGRS Grid", icon: <RIcon name="usng-grid" />, run: () => void addUsngGrid(), tip: "Add an MGRS (USNG) grid for the map view: squares sized for how far you are zoomed, each labelled with its reference" },
  { id: "worldGraticule", label: "Graticule", icon: <RIcon name="graticule" />, run: addWorldReference, tip: "Add curved latitude/longitude lines and the coordinate system's own reference lines (central meridian, standard parallels, edge of the map)" },
  { id: "worldOutline", label: "World Outline", icon: <RIcon name="world-outline" />, run: addWorldOutline, tip: "Add the bundled Natural Earth land outline (public domain, works offline)" },
  { id: "projectionBuilder", label: "Projection Builder", icon: <RIcon name="projection-builder" />, run: () => useDialog.getState().show({ kind: "projectionBuilder" }), tip: "Build a conic, cylindrical, azimuthal or world projection from its parameters" },
  { id: "tissot", label: "Tissot Circles", icon: <RIcon name="tissot" />, run: addTissotCircles, tip: "Add circles that are round on the ground; their shape in the map's coordinate system shows its distortion" },
  { id: "datumGrids", label: "Datum Grids", icon: <RIcon name="datum-grids" />, run: () => useDialog.getState().show({ kind: "datumGrids" }), tip: "See which datum-shift grids are installed (they make coordinate conversions between datums accurate), and download more" },
  { id: "responsibleUse", label: "Limits & Responsible Use", icon: <RIcon name="info" />, run: () => useDialog.getState().show({ kind: "responsibleUse" }), tip: "What this program can and cannot be relied on for: datum accuracy, heights, distortion, estimates, sensitive coordinates" },
  { id: "georeference", label: "Georeference", icon: <RIcon name="coordinate-system" />, run: () => useGeoref.getState().show(), tip: "Fit a scanned map or photo to the ground with control points, and write a new georeferenced GeoTIFF (the image file is not changed)" },
  { id: "adjustShapes", label: "Adjust Shapes", icon: <RIcon name="coordinate-system" />, run: () => useAdjustWindow.getState().show(), enabled: () => Object.values(app().layers).some((l) => l.geomType !== "Table" && l.geomType !== "Raster"), tip: "Move a vector layer's shapes with control points: affine, similarity (scale, rotation, shift) or rubber sheet. Makes a new layer; the layer is not changed" },
  { id: "mapCrs", label: "Coordinate System", icon: <RIcon name="coordinate-system" />, run: pickMapCrs, tip: "Set the map's coordinate system (used for coordinates, measurements and layouts)" },

  // Data tab
  { id: "defineProjection", label: "Define Projection", icon: <RIcon name="define-projection" />, run: () => active() && useDialog.getState().show({ kind: "defineProjection", layer: active()!.id }), enabled: () => !!active() },
  { id: "definitionQuery", label: "Definition Query", icon: <RIcon name="definition-query" />, run: () => active() && openDefinitionQuery(active()!.id), enabled: () => !!active(), tip: "Limit the layer to features matching an expression" },
  { id: "layerFromSelection", label: "Layer From Selection", icon: <RIcon name="layer-add" />, run: () => active() && layerFromSelection(active()!.id), enabled: () => !!active() && (app().selection[active()!.id]?.length ?? 0) > 0, tip: "Copy the selected features into a new layer" },
  { id: "splitByAttribute", label: "Split by Attribute", icon: <RIcon name="split" />, run: () => active() && splitByAttribute(active()!.id), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Make one layer or one file per value of a field" },
  { id: "exportTable", label: "Export Table", icon: <RIcon name="export-table" />, run: () => active() && exportTable(active()!.id), enabled: () => active()?.geomType === "Point", tip: "Write a point layer's attributes and coordinates (in a coordinate system you choose) as CSV or Parquet" },
  { id: "exportLayer", label: "Export Features", icon: <RIcon name="export" />, run: () => active() && exportLayer(active()!.id), enabled: () => !!active() },
  { id: "addField", label: "Add Field", icon: <RIcon name="table-add-col" />, run: () => active() && useDialog.getState().show({ kind: "addField", layer: active()!.id }), enabled: canEditActive },
  { id: "calcField", label: "Calculate Field", icon: <RIcon name="calc-field" />, run: () => active() && useDialog.getState().show({ kind: "calcField", layer: active()!.id, owner: active()!.id }), enabled: canEditActive },
  { id: "calcGeometry", label: "Calculate Geometry", icon: <RIcon name="calc-geometry" />, run: () => active() && useDialog.getState().show({ kind: "calcGeometry", layer: active()!.id, owner: active()!.id }), enabled: () => canEditActive() && active()!.geomType !== "Table" },
  { id: "newTable", label: "New Table", icon: <RIcon name="table-new" />, run: () => useDialog.getState().show({ kind: "newTable" }), tip: "Make an empty table with typed fields (text, numbers, dates, yes/no) and fill it in the Attribute Table. Tables have rows but no shapes" },
  { id: "settings", label: "Settings", icon: <RIcon name="settings" />, run: () => useDialog.getState().show({ kind: "settings" }), tip: "Your own choices: theme, units, the coordinate system of new maps and the startup panes. They are saved on this computer, not in a project" },
  { id: "saveStartup", label: "Save Startup Layout", icon: <RIcon name="startup-layout" />, run: () => void saveStartupLayout(), tip: "Keep the panes as they are now as the arrangement new projects and empty starts open with. A project keeps the layout it was saved with" },
  { id: "resetStartup", label: "Reset Startup Layout", icon: <RIcon name="reset-panes" />, run: () => void resetStartupLayout(), enabled: () => useSettings.getState().settings.startupLayout != null, tip: "Forget your startup layout: new projects open with the built-in panes again" },
  { id: "testData", label: "Generate Test Data", icon: <RIcon name="test-data" />, run: () => useDialog.getState().show({ kind: "testData" }), tip: "Random points or polygons, to try tools and check speed" },

  // Analysis tab

  // Map › Layer. Public Data goes to the Catalog's tree; its eight dialogs are the Catalog's rows (PUBLIC_DATA below).
  { id: "catalog", label: "Public Data", icon: <RIcon name="catalog" />, run: focusPublicData, tip: "Open the Catalog at its Public Data tree: Census, USGS, Overture, ArcGIS services and more. Double-click a row to download for your area" },

  // Analysis tab
  { id: "addTool", label: "Add Tool", icon: <RIcon name="add-tool" />, run: () => openToolMenu(), keys: "Shift+A", tip: "Find a tool by name or plain words and add it to the active layer's node tree" },
  { id: "runModel", label: "Run", icon: <RIcon name="run-model" />, run: () => runModel(), tip: "Run every step of the active layer's node tree" },
  paneToggle("paneModel", "model", "Processing nodes", <RIcon name="processing-nodes" />, "Show or hide the Processing nodes editor in the bottom panel"),
  paneToggle("paneTable", "table", "Attribute Table", <RIcon name="attribute-table" />, "Show or hide the Attribute Table in the bottom panel"),

  // Statistics workspace (docs/STATISTICS_WORKSPACE_PLAN.md, ST1)
  { id: "statNewSheet", label: "New Statistics Sheet", icon: <RIcon name="layout-chart" />, run: () => newStatsSheet(), tip: "Open a statistics sheet: cards that explore and test a layer's or table's numbers, kept in the project" },
  { id: "statRenameSheet", label: "Rename Sheet", icon: <RIcon name="page-setup" />, enabled: () => !!openSheet(), run: () => { const s = openSheet(); if (s) renameStatsSheet(s.id); }, tip: "Rename the open statistics sheet" },
  { id: "statDeleteSheet", label: "Delete Sheet", icon: <RIcon name="remove" />, enabled: () => !!openSheet(), run: () => { const s = openSheet(); if (s) deleteStatsSheet(s.id); }, tip: "Delete the open statistics sheet from the project" },
  { id: "statRun", label: "Run Card", icon: <RIcon name="run-model" />, enabled: () => !!pickedCard(), run: () => { const s = openSheet(), c = pickedCard(); if (s && c) void runStatCard(s.id, c.id); }, tip: "Run the selected card on its layer. Nothing runs by itself: a card whose data changed is marked out of date until you run it" },
  { id: "statRunAll", label: "Run All", icon: <RIcon name="run-model" />, enabled: () => (openSheet()?.cards.length ?? 0) > 0, run: () => void runAllStatCards(), tip: "Run every card of the sheet, top to bottom" },
  { id: "statRemoveCard", label: "Remove Card", icon: <RIcon name="remove" />, enabled: () => !!pickedCard(), run: () => { const s = openSheet(), c = pickedCard(); if (s && c) useStats.getState().removeCard(s.id, c.id); }, tip: "Take the selected card off the sheet" },
  { id: "statCardUp", label: "Move Up", icon: <RIcon name="order" />, enabled: () => { const s = openSheet(), c = pickedCard(); return !!s && !!c && s.cards[0]?.id !== c.id; }, run: () => { const s = openSheet(), c = pickedCard(); if (s && c) useStats.getState().moveCard(s.id, c.id, -1); }, tip: "Move the selected card one place up the sheet" },
  { id: "statCardDown", label: "Move Down", icon: <RIcon name="order" />, enabled: () => { const s = openSheet(), c = pickedCard(); return !!s && !!c && s.cards[s.cards.length - 1]?.id !== c.id; }, run: () => { const s = openSheet(), c = pickedCard(); if (s && c) useStats.getState().moveCard(s.id, c.id, 1); }, tip: "Move the selected card one place down the sheet" },

  { id: "statExportCsv", label: "Table as CSV", icon: <RIcon name="export-table" />, enabled: () => !!pickedCard()?.result?.tables.length, run: () => void exportCardTable(), tip: "Save the selected card's table as a CSV file: full precision, units in the column headings, empty cells for missing values" },
  { id: "statExportSvg", label: "Chart as SVG", icon: <RIcon name="svg" />, enabled: () => !!pickedCard()?.result?.charts.length, run: () => void exportCardChart("svg"), tip: "Save the selected card's chart as an SVG drawing (dark ink on white, editable in a drawing program)" },
  { id: "statExportPng", label: "Chart as PNG", icon: <RIcon name="export" />, enabled: () => !!pickedCard()?.result?.charts.length, run: () => void exportCardChart("png"), tip: "Save the selected card's chart as a PNG picture at twice its screen size" },
  { id: "statExportReport", label: "Sheet Report", icon: <RIcon name="layout-sources" />, enabled: () => (openSheet()?.cards.length ?? 0) > 0, run: () => void exportSheetReport(), tip: "Save the whole sheet as one report page: every card's method, rows used and left out, tables, charts, notes, and the references and data sources. Print it to PDF from a browser" },

  // Insert tab (later milestones)
  { id: "newLayout", label: "New Layout", icon: <RIcon name="new-layout" />, run: () => newLayout(), tip: "Create a print layout (Letter landscape); more page sizes are in the Insert tab's gallery" },
  { id: "newMap", label: "New Map", icon: <RIcon name="map-add" />, run: () => void newMap(), tip: "Add a map to the project. It starts empty with the last map's coordinate system; each map has its own layers, order, visibility, basemap and view" },
  { id: "duplicateMap", label: "Duplicate Map", icon: <RIcon name="duplicate" />, run: () => void duplicateMap(), tip: "Copy the active map, with its layers, order, visibility, coordinate system, basemap and view, into a new map beside it" },
  { id: "topoMap", label: "Topo Map", icon: <RIcon name="topo-map" />, run: () => useDialog.getState().show({ kind: "topoMap" }), tip: "Build a land-navigation topographic sheet: UTM and MGRS grid, declination diagram, scale bars, grid-reference box and legend, for an area, scale and paper you choose" },

  // Layout contextual tab (shown while a layout tab is active)
  { id: "layoutGuideV", label: "Vertical Guide", icon: <RIcon name="layout-guide-v" />, run: () => addGuide("x"), enabled: inLayout, tip: "Add a vertical guide in the middle of the page. Elements snap to guides; type its position in Properties › Guides. You can also drag from a ruler" },
  { id: "layoutGuideH", label: "Horizontal Guide", icon: <RIcon name="layout-guide-h" />, run: () => addGuide("y"), enabled: inLayout, tip: "Add a horizontal guide in the middle of the page. Elements snap to guides; you can also drag from a ruler" },
  { id: "layoutGuidesClear", label: "Clear Guides", icon: <RIcon name="clear-guides" />, run: clearGuides, enabled: () => !!useLayouts.getState().doc?.guides?.length, tip: "Remove every guide from this layout (one undo step)" },
  { id: "layoutMapFrame", label: "Map Frame", icon: <RIcon name="map-frame" />, run: insertMapFrame, enabled: inLayout, tip: "Insert a map frame in the map's coordinate system, centred on the map view" },
  { id: "layoutOverview", label: "Overview Map", icon: <RIcon name="overview" />, run: insertOverview, enabled: hasFrame, tip: "Insert a small overview map of the selected frame with its extent outlined" },
  { id: "layoutText", label: "Text", icon: <RIcon name="layout-text" />, run: () => void addText(), enabled: inLayout, tip: "Insert a text box ({layout_name}, {scale}, {date} … are replaced when drawn)" },
  { id: "layoutRect", label: "Rectangle", icon: <RIcon name="rectangle" />, run: () => void addShape("rectangle"), enabled: inLayout, tip: "Insert a rectangle (fill, outline, rounded corners)" },
  { id: "layoutEllipse", label: "Ellipse", icon: <RIcon name="ellipse" />, run: () => void addShape("ellipse"), enabled: inLayout, tip: "Insert an ellipse" },
  { id: "layoutTriangle", label: "Triangle", icon: <RIcon name="triangle" />, run: () => void addShape("triangle"), enabled: inLayout, tip: "Insert a triangle" },
  { id: "layoutLine", label: "Line", icon: <RIcon name="layout-line" />, run: () => void addShape("line"), enabled: inLayout, tip: "Insert a line (rotate it for other directions; solid, dashed or dotted)" },
  { id: "layoutArrow", label: "Arrow", icon: <RIcon name="layout-arrow" />, run: () => void addShape("arrow"), enabled: inLayout, tip: "Insert an arrow" },
  { id: "layoutPicture", label: "Picture", icon: <RIcon name="layout-picture-add" />, run: () => void insertPicture(), enabled: inLayout, tip: "Insert a picture or logo (PNG, JPEG or SVG) embedded in the layout" },
  { id: "layoutTable", label: "Table", icon: <RIcon name="attribute-table" />, run: () => void addTable(useApp.getState().activeLayer ?? ""), enabled: inLayout, tip: "Insert a table of the active layer's attributes (choose fields and sorting in Properties)" },
  { id: "layoutChart", label: "Chart", icon: <RIcon name="layout-chart" />, run: () => void addChart(useApp.getState().activeLayer ?? ""), enabled: inLayout, tip: "Insert a bar or pie chart of a layer's categories" },
  { id: "layoutLegend", label: "Legend", icon: <RIcon name="layout-legend" />, run: () => insertSurround("legend"), enabled: hasFrame, tip: "Insert a legend listing the frame's visible layers and their classes" },
  { id: "layoutScaleBar", label: "Scale Bar", icon: <RIcon name="scale-bar" />, run: () => insertSurround("scale_bar"), enabled: hasFrame, tip: "Insert a scale bar that is true to ground distance at the frame centre" },
  { id: "layoutNorthArrow", label: "North Arrow", icon: <RIcon name="north-arrow" />, run: () => insertSurround("north_arrow"), enabled: hasFrame, tip: "Insert a north arrow pointing to true north (follows meridian convergence and frame rotation)" },
  { id: "layoutDeclination", label: "Declination", icon: <RIcon name="declination" />, run: () => insertSurround("declination"), enabled: hasFrame, tip: "Insert a declination diagram (grid, true and magnetic north) from the World Magnetic Model at the frame's centre" },
  { id: "layoutSources", label: "Sources", icon: <RIcon name="layout-sources" />, run: () => insertSurround("sources"), enabled: hasFrame, tip: "Insert the data sources (citations) of the layers drawn in the frame" },
  { id: "layoutFromMap", label: "Set from Map View", icon: <RIcon name="frame-from-map" />, run: () => void setFrameFromMapView(), enabled: hasFrame, tip: "Centre the selected frame on the map view and use the nearest standard scale that shows it" },
  { id: "layoutFrameCrs", label: "Frame CRS", icon: <RIcon name="frame-crs" />, run: () => pickFrameCrs(), enabled: hasFrame, tip: "Choose the coordinate system the frame is drawn in (true scale needs a projected one)" },
  { id: "layoutPdf", label: "PDF", icon: <RIcon name="pdf" />, run: () => void exportLayout("pdf"), enabled: inLayout, tip: "Export the page as a vector PDF" },
  { id: "layoutPng", label: "PNG", icon: <RIcon name="image" />, run: () => void exportLayout("png"), enabled: inLayout, tip: "Export the page as a PNG at the resolution in the dpi box (300 by default)" },
  { id: "layoutSvg", label: "SVG", icon: <RIcon name="svg" />, run: () => void exportLayout("svg"), enabled: inLayout, tip: "Export the page as SVG for finishing in Inkscape or Illustrator" },

  // Arrange tab › Group, View tab › Zoom, Export tab › This Page (the layout workspace)
  { id: "layoutGroup", label: "Group", icon: <RIcon name="layout-group" />, run: () => useLayouts.getState().group(), enabled: () => useLayouts.getState().selected.length > 1, tip: "Select and move the chosen elements together" },
  { id: "layoutUngroup", label: "Ungroup", icon: <RIcon name="layout-ungroup" />, run: () => useLayouts.getState().ungroup(), enabled: () => { const s = useLayouts.getState(); return s.selected.some((id) => s.doc?.elements.find((e) => e.id === id)?.group); }, tip: "Split the selected group into separate elements" },
  { id: "layoutLock", label: "Lock", icon: <RIcon name="layout-lock" />, run: () => void toggleLockSelected(), enabled: () => useLayouts.getState().selected.length > 0, checked: () => { const s = useLayouts.getState(); const els = s.doc?.elements.filter((e) => s.selected.includes(e.id)) ?? []; return els.length > 0 && els.every((e) => e.locked); }, tip: "Lock the selected elements so they cannot be moved or deleted by mistake; press again to unlock" },
  { id: "layoutFit", label: "Fit Page", icon: <RIcon name="fit-page" />, run: () => useLayoutView.getState().fitPage(), enabled: inLayout, checked: () => inLayout() && useLayoutView.getState().fit, tip: "Show the whole page in the window" },
  { id: "layoutActual", label: "100%", icon: <RIcon name="zoom-actual" />, run: () => useLayoutView.getState().actualSize(), enabled: inLayout, tip: "Show the page at its printed size on screen" },
  { id: "layoutZoomIn", label: "Zoom In", icon: <RIcon name="zoom-in" />, run: () => useLayoutView.getState().zoomBy(1.25), enabled: inLayout },
  { id: "layoutZoomOut", label: "Zoom Out", icon: <RIcon name="zoom-out" />, run: () => useLayoutView.getState().zoomBy(1 / 1.25), enabled: inLayout },
  { id: "layoutPreflight", label: "Check Page", icon: <RIcon name="preflight" />, run: () => { const v = useLayoutView.getState(); v.setPreflight(!v.preflightOpen); }, enabled: inLayout, checked: () => useLayoutView.getState().preflightOpen, tip: "Before exporting: is the coordinate system named, are the sources listed, do the scale bar and legend belong to a frame on the page" },

  // View tab › Windows
  paneToggle("paneContents", "contents", "Map Layers", <RIcon name="pane-layers" />, "Show or hide the list of layers"),
  paneToggle("paneCatalog", "catalog", "Catalog", <RIcon name="catalog" />, "Show or hide the Catalog: this project, its layouts and Public Data"),
  paneToggle("paneItems", "items", "Items", <RIcon name="pane-items" />, "Show or hide the list of what is on the layout page"),
  paneToggle("paneProperties", "properties", "Properties", <RIcon name="pane-properties" />, "Show or hide Properties"),
  paneToggle("paneSources", "sources", "Sources", <RIcon name="book" />, "Every data source used in this project, as an annotated bibliography you can export"),
  { id: "paneMap", label: "Map", icon: <RIcon name="map" />, run: () => showPane("map") },
  { id: "resetPanes", label: "Reset Panes", icon: <RIcon name="reset-panes" />, run: defaultLayout, tip: "Restore your startup pane layout, or the built-in one if you have not saved one (Settings)" },
  { id: "fps", label: "Frame Rate", icon: <RIcon name="fps" />, run: () => app().set({ showFps: !app().showFps }), checked: () => app().showFps, tip: "Show map frames per second" },
  { id: "benchmark", label: "Benchmark", icon: <RIcon name="fps" />, run: () => useDialog.getState().show({ kind: "benchmark" }), tip: "Fly the map and measure smoothness" },
  { id: "theme", label: "Dark Theme", icon: <RIcon name="theme" />, run: () => app().set({ dark: !app().dark }), checked: () => app().dark },
  { id: "colours", label: "Colours", icon: <RIcon name="palette" />, run: toggleAppearance, checked: () => useAppearance.getState().open, tip: "Choose the accent, highlight, panel and tool colours" },

  // Feature Layer contextual tab
  { id: "zoomLayer", label: "Zoom To Layer", icon: <RIcon name="zoom-layer" />, run: () => active() && zoomToLayer(active()!.id), enabled: () => !!active()?.extent },
  { id: "symbology", label: "Symbology", icon: <RIcon name="symbology" />, run: () => { const a = active(); if (a) openLayerProperties(a.id, "symbology"); }, enabled: () => !!active() && active()!.geomType !== "Table" },
  { id: "layerProps", label: "Properties", icon: <RIcon name="properties" />, run: () => { const a = active(); if (a) openLayerProperties(a.id); }, enabled: () => !!active(), tip: "Open the Layer Properties window" },
  { id: "openTable", label: "Attribute Table", icon: <RIcon name="attribute-table" />, run: () => showPane("table"), enabled: () => !!active() },
  { id: "removeLayer", label: "Remove", icon: <RIcon name="remove" />, run: () => active() && removeLayer(active()!.id), enabled: () => !!active() },
];

export function toolCommand(toolId: string, name: string, description: string): Command {
  return {
    id: `tool:${toolId}`,
    label: name,
    icon: createElement(toolIcon(toolId)),
    tip: description,
    run: () => void addToolFromUi(toolId),
  };
}

export { BASEMAP_OPTIONS } from "../map/basemaps";

/** The Catalog's Public Data rows are also commands (search, tests); the list itself lives in CatalogPane. */
const publicData: Command[] = PUBLIC_DATA.map((p) => ({ id: p.id, label: p.label, icon: <>{p.icon}</>, tip: p.tip, run: () => useDialog.getState().show(p.dialog) }));

export const commands: Record<string, Command> = Object.fromEntries([...list, ...editCommandList, ...publicData].map((c) => [c.id, c]));
/** Ids of the commands that live in the Catalog's Public Data tree rather than on a ribbon tab. */
export const CATALOG_COMMANDS: string[] = PUBLIC_DATA.map((p) => p.id);

function zoomFull() {
  const s = app();
  let ext: [number, number, number, number] | null = null;
  for (const id of s.order) {
    const e = s.layers[id]?.extent;
    if (!e || !s.visible[id]) continue;
    ext = ext ? [Math.min(ext[0], e[0]), Math.min(ext[1], e[1]), Math.max(ext[2], e[2]), Math.max(ext[3], e[3])] : e;
  }
  zoomToExtent(ext ?? [-125, 24, -66, 50]);
}
