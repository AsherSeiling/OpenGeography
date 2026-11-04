// Every ribbon command lives in this one registry. The ribbon layout, menus
// and the "Tell me" search all read from it.
import { createElement } from "react";
import { toolIcon } from "../nodes/toolIcon";
import { openToolMenu } from "../nodes/toolMenuStore";
import { PUBLIC_DATA } from "../panes/CatalogPane";
import { toggleAppearance, useAppearance } from "../theme/appearance";
import { addToolFromUi, focusPublicData } from "./addTool";
import {
  BorderAllRegular,
  AddCircleRegular,
  MyLocationRegular,
  LocationRegular,
  BookRegular,
  FolderOpenRegular,
  MapRegular,
  CopyRegular,
  ShapesRegular,
  TargetArrowRegular,
  SettingsRegular,
  FilterRegular,
  CompassNorthwestRegular,
  LayerDiagonalAddRegular,
  CalculatorRegular,
  DatabaseRegular,
  EarthRegular,
  FullScreenMaximizeRegular,
  GlobeLocationRegular,
  GridRegular,
  LocationTargetSquareRegular,
  Map16Regular,
  ScanObjectRegular,
  RulerRegular,
  TableAddRegular,
  ArrowExportRegular,
  ArrowResetRegular,
  CursorClickRegular,
  DarkThemeRegular,
  DeleteRegular,
  DismissCircleRegular,
  DocumentAddRegular,
  DocumentPdfRegular,
  FlowchartRegular,
  GlobeRegular,
  HandLeftRegular,
  InfoRegular,
  LockClosedRegular,
  GroupRegular,
  ZoomInRegular,
  ZoomOutRegular,
  TaskListSquareLtrRegular,
  ArrowMaximizeRegular,
  PanelLeftContractRegular,
  MountainTrailRegular,
  PaintBrushRegular,
  PanelLeftRegular,
  PanelRightRegular,
  PlayRegular,
  RectangleLandscapeRegular,
  SaveEditRegular,
  SaveRegular,
  SelectAllOnRegular,
  TableRegular,
  TopSpeedRegular,
  DataUsageRegular,
  ZoomFitRegular,
  TextTRegular,
  TextBulletListRegular,
  NavigationRegular,
  BookInformationRegular,
  ImageRegular,
  CircleRegular,
  ChartMultipleRegular,
  TriangleRegular,
  LineHorizontal1Regular,
  ArrowRightRegular,
  ImageAddRegular,
  AddSquareRegular,
  ColorRegular,
} from "@fluentui/react-icons";
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
import { exportLayout, insertMapFrame, insertOverview, insertPicture, insertSurround, newLayout, pickFrameCrs, setFrameFromMapView } from "../layout/actions";
import { addChart, addShape, addTable, addText, useLayouts } from "../store/layout";
import { toggleLockSelected } from "../layout/clipboard";
import { useLayoutView } from "../layout/view";
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
const hasFrame = () => !!useLayouts.getState().doc?.elements.some((e) => e.kind === "map_frame");

/** A Windows button: pressed while the pane is open, a click opens or closes it. */
function paneToggle(id: string, pane: PaneId, label: string, icon: ReactElement, tip: string): Command {
  return { id, label, icon, tip, run: () => togglePane(pane), checked: () => isPaneOpen(pane) };
}

const list: Command[] = [
  // Project
  { id: "new", label: "New", icon: <DocumentAddRegular />, run: newProject, tip: "Start a new project" },
  { id: "open", label: "Open", icon: <FolderOpenRegular />, run: openProject, keys: "Ctrl+O", tip: "Open a .gproj project folder" },
  { id: "save", label: "Save", icon: <SaveRegular />, run: () => saveProject(), keys: "Ctrl+S", tip: "Save the project" },
  { id: "saveAs", label: "Save As", icon: <SaveEditRegular />, run: () => saveProject(true), tip: "Save the project to a new folder" },

  // Map tab
  { id: "explore", label: "Explore", icon: <HandLeftRegular />, run: setTool("pan"), checked: tool("pan"), tip: "Pan and zoom the map" },
  { id: "fullExtent", label: "Full Extent", icon: <FullScreenMaximizeRegular />, run: zoomFull, tip: "Zoom to all layers" },
  { id: "addData", label: "Add Data", icon: <AddCircleRegular />, run: () => addData(), tip: "Add shapefiles, GeoPackages, GeoJSON, KML, CSV, GeoParquet…" },
  { id: "select", label: "Select", icon: <CursorClickRegular />, run: setTool("select"), checked: tool("select"), tip: "Click features to select (Shift adds, Ctrl toggles)" },
  { id: "selectBox", label: "Select by Rectangle", icon: <RectangleLandscapeRegular />, run: setTool("selectBox"), checked: tool("selectBox"), tip: "Drag a box to select features" },
  { id: "selectByLocation", label: "Select By Location", icon: <LocationRegular />, run: () => active() && useDialog.getState().show({ kind: "selectByLocation", layer: active()!.id }), enabled: () => !!active() && Object.keys(app().layers).length > 1, tip: "Select features by where they are relative to another layer's features" },
  { id: "selectByAttributes", label: "Select By Attributes", icon: <SelectAllOnRegular />, run: () => active() && useDialog.getState().show({ kind: "selectByAttributes", layer: active()!.id }), enabled: () => !!active() },
  { id: "zoomSelection", label: "Zoom To Selection", icon: <ScanObjectRegular />, run: zoomToSelection, enabled: hasSelection },
  { id: "clearSelection", label: "Clear", icon: <DismissCircleRegular />, run: () => app().clearSelection(), enabled: hasSelection, tip: "Clear the selection in all layers" },
  { id: "identify", label: "Identify", icon: <InfoRegular />, run: setTool("identify"), checked: tool("identify"), tip: "Click a feature to see its attributes" },
  { id: "measure", label: "Measure", icon: <RulerRegular />, run: () => { useNav.getState().set({ measureMode: "distance" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "distance", tip: "Geodesic distance and azimuths: click points, double-click to finish" },
  { id: "measureArea", label: "Measure Area", icon: <ShapesRegular />, run: () => { useNav.getState().set({ measureMode: "area" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "area", tip: "Geodesic area and perimeter" },
  { id: "resection", label: "Resection", icon: <MyLocationRegular />, run: () => useNav.getState().set({ navPanel: "resection" }), checked: () => useNav.getState().navPanel === "resection", tip: "Find your position from azimuths to known landmarks (MGRS, mils, G-M angle)" },
  { id: "intersection", label: "Intersection", icon: <TargetArrowRegular />, run: () => useNav.getState().set({ navPanel: "intersection" }), checked: () => useNav.getState().navPanel === "intersection", tip: "Locate an unseen point from azimuths taken at known positions" },
  { id: "census", label: "Census Data", icon: <TableAddRegular />, run: () => active() && useDialog.getState().show({ kind: "census", layer: active()!.id }), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Join American Community Survey or 2020 Census values to the active layer by GEOID" },
  { id: "worldView", label: "World View", icon: <EarthRegular />, run: toggleWorldView, checked: () => app().worldView, tip: "Show the whole Earth in this coordinate system (cut at the opposite side of its central meridian)" },
  { id: "usngGrid", label: "MGRS Grid", icon: <BorderAllRegular />, run: () => void addUsngGrid(), tip: "Add an MGRS (USNG) grid for the map view: squares sized for how far you are zoomed, each labelled with its reference" },
  { id: "worldGraticule", label: "Graticule", icon: <GridRegular />, run: addWorldReference, tip: "Add curved latitude/longitude lines and the coordinate system's own reference lines (central meridian, standard parallels, edge of the map)" },
  { id: "worldOutline", label: "World Outline", icon: <MapRegular />, run: addWorldOutline, tip: "Add the bundled Natural Earth land outline (public domain, works offline)" },
  { id: "projectionBuilder", label: "Projection Builder", icon: <SettingsRegular />, run: () => useDialog.getState().show({ kind: "projectionBuilder" }), tip: "Build a conic, cylindrical, azimuthal or world projection from its parameters" },
  { id: "tissot", label: "Tissot Circles", icon: <CircleRegular />, run: addTissotCircles, tip: "Add circles that are round on the ground; their shape in the map's coordinate system shows its distortion" },
  { id: "datumGrids", label: "Datum Grids", icon: <DatabaseRegular />, run: () => useDialog.getState().show({ kind: "datumGrids" }), tip: "See which datum-shift grids are installed (they make coordinate conversions between datums accurate), and download more" },
  { id: "responsibleUse", label: "Limits & Responsible Use", icon: <InfoRegular />, run: () => useDialog.getState().show({ kind: "responsibleUse" }), tip: "What this program can and cannot be relied on for: datum accuracy, heights, distortion, estimates, sensitive coordinates" },
  { id: "mapCrs", label: "Coordinate System", icon: <GlobeRegular />, run: pickMapCrs, tip: "Set the map's coordinate system (used for coordinates, measurements and layouts)" },

  // Data tab
  { id: "defineProjection", label: "Define Projection", icon: <GlobeLocationRegular />, run: () => active() && useDialog.getState().show({ kind: "defineProjection", layer: active()!.id }), enabled: () => !!active() },
  { id: "definitionQuery", label: "Definition Query", icon: <FilterRegular />, run: () => active() && openDefinitionQuery(active()!.id), enabled: () => !!active(), tip: "Limit the layer to features matching an expression" },
  { id: "layerFromSelection", label: "Layer From Selection", icon: <LayerDiagonalAddRegular />, run: () => active() && layerFromSelection(active()!.id), enabled: () => !!active() && (app().selection[active()!.id]?.length ?? 0) > 0, tip: "Copy the selected features into a new layer" },
  { id: "splitByAttribute", label: "Split by Attribute", icon: <LayerDiagonalAddRegular />, run: () => active() && splitByAttribute(active()!.id), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Make one layer or one file per value of a field" },
  { id: "exportTable", label: "Export Table", icon: <ArrowExportRegular />, run: () => active() && exportTable(active()!.id), enabled: () => active()?.geomType === "Point", tip: "Write a point layer's attributes and coordinates (in a coordinate system you choose) as CSV or Parquet" },
  { id: "exportLayer", label: "Export Features", icon: <ArrowExportRegular />, run: () => active() && exportLayer(active()!.id), enabled: () => !!active() },
  { id: "addField", label: "Add Field", icon: <TableAddRegular />, run: () => active() && useDialog.getState().show({ kind: "addField", layer: active()!.id }), enabled: canEditActive },
  { id: "calcField", label: "Calculate Field", icon: <CalculatorRegular />, run: () => active() && useDialog.getState().show({ kind: "calcField", layer: active()!.id, owner: active()!.id }), enabled: canEditActive },
  { id: "calcGeometry", label: "Calculate Geometry", icon: <RulerRegular />, run: () => active() && useDialog.getState().show({ kind: "calcGeometry", layer: active()!.id, owner: active()!.id }), enabled: () => canEditActive() && active()!.geomType !== "Table" },
  { id: "testData", label: "Generate Test Data", icon: <DataUsageRegular />, run: () => useDialog.getState().show({ kind: "testData" }), tip: "Random points or polygons, to try tools and check speed" },

  // Analysis tab

  // Map › Layer. Public Data goes to the Catalog's tree; its eight dialogs are the Catalog's rows (PUBLIC_DATA below).
  { id: "catalog", label: "Public Data", icon: <FolderOpenRegular />, run: focusPublicData, tip: "Open the Catalog at its Public Data tree: Census, USGS, Overture, ArcGIS services and more. Double-click a row to download for your area" },

  // Analysis tab
  { id: "addTool", label: "Add Tool", icon: <AddSquareRegular />, run: () => openToolMenu(), keys: "Shift+A", tip: "Find a tool by name or plain words and add it to the active layer's node tree" },
  { id: "runModel", label: "Run", icon: <PlayRegular />, run: () => runModel(), tip: "Run every step of the active layer's node tree" },
  paneToggle("paneModel", "model", "Node Editor", <FlowchartRegular />, "Show or hide the Node Editor in the bottom panel"),
  paneToggle("paneTable", "table", "Attribute Table", <TableRegular />, "Show or hide the Attribute Table in the bottom panel"),

  // Insert tab (later milestones)
  { id: "newLayout", label: "New Layout", icon: <DocumentPdfRegular />, run: () => newLayout(), tip: "Create a print layout (Letter landscape); more page sizes are in the Insert tab's gallery" },
  { id: "newMap", label: "New Map", icon: <MapRegular />, run: () => void newMap(), tip: "Add a map to the project. It starts empty with the last map's coordinate system; each map has its own layers, order, visibility, basemap and view" },
  { id: "duplicateMap", label: "Duplicate Map", icon: <CopyRegular />, run: () => void duplicateMap(), tip: "Copy the active map, with its layers, order, visibility, coordinate system, basemap and view, into a new map beside it" },
  { id: "topoMap", label: "Topo Map", icon: <MountainTrailRegular />, run: () => useDialog.getState().show({ kind: "topoMap" }), tip: "Build a land-navigation topographic sheet: UTM and MGRS grid, declination diagram, scale bars, grid-reference box and legend, for an area, scale and paper you choose" },

  // Layout contextual tab (shown while a layout tab is active)
  { id: "layoutMapFrame", label: "Map Frame", icon: <MapRegular />, run: insertMapFrame, enabled: inLayout, tip: "Insert a map frame in the map's coordinate system, centred on the map view" },
  { id: "layoutOverview", label: "Overview Map", icon: <Map16Regular />, run: insertOverview, enabled: hasFrame, tip: "Insert a small overview map of the selected frame with its extent outlined" },
  { id: "layoutText", label: "Text", icon: <TextTRegular />, run: () => void addText(), enabled: inLayout, tip: "Insert a text box ({layout_name}, {scale}, {date} … are replaced when drawn)" },
  { id: "layoutRect", label: "Rectangle", icon: <RectangleLandscapeRegular />, run: () => void addShape("rectangle"), enabled: inLayout, tip: "Insert a rectangle (fill, outline, rounded corners)" },
  { id: "layoutEllipse", label: "Ellipse", icon: <CircleRegular />, run: () => void addShape("ellipse"), enabled: inLayout, tip: "Insert an ellipse" },
  { id: "layoutTriangle", label: "Triangle", icon: <TriangleRegular />, run: () => void addShape("triangle"), enabled: inLayout, tip: "Insert a triangle" },
  { id: "layoutLine", label: "Line", icon: <LineHorizontal1Regular />, run: () => void addShape("line"), enabled: inLayout, tip: "Insert a line (rotate it for other directions; solid, dashed or dotted)" },
  { id: "layoutArrow", label: "Arrow", icon: <ArrowRightRegular />, run: () => void addShape("arrow"), enabled: inLayout, tip: "Insert an arrow" },
  { id: "layoutPicture", label: "Picture", icon: <ImageAddRegular />, run: () => void insertPicture(), enabled: inLayout, tip: "Insert a picture or logo (PNG, JPEG or SVG) embedded in the layout" },
  { id: "layoutTable", label: "Table", icon: <TableRegular />, run: () => void addTable(useApp.getState().activeLayer ?? ""), enabled: inLayout, tip: "Insert a table of the active layer's attributes (choose fields and sorting in Properties)" },
  { id: "layoutChart", label: "Chart", icon: <ChartMultipleRegular />, run: () => void addChart(useApp.getState().activeLayer ?? ""), enabled: inLayout, tip: "Insert a bar or pie chart of a layer's categories" },
  { id: "layoutLegend", label: "Legend", icon: <TextBulletListRegular />, run: () => insertSurround("legend"), enabled: hasFrame, tip: "Insert a legend listing the frame's visible layers and their classes" },
  { id: "layoutScaleBar", label: "Scale Bar", icon: <RulerRegular />, run: () => insertSurround("scale_bar"), enabled: hasFrame, tip: "Insert a scale bar that is true to ground distance at the frame centre" },
  { id: "layoutNorthArrow", label: "North Arrow", icon: <NavigationRegular />, run: () => insertSurround("north_arrow"), enabled: hasFrame, tip: "Insert a north arrow pointing to true north (follows meridian convergence and frame rotation)" },
  { id: "layoutDeclination", label: "Declination", icon: <CompassNorthwestRegular />, run: () => insertSurround("declination"), enabled: hasFrame, tip: "Insert a declination diagram (grid, true and magnetic north) from the World Magnetic Model at the frame's centre" },
  { id: "layoutSources", label: "Sources", icon: <BookInformationRegular />, run: () => insertSurround("sources"), enabled: hasFrame, tip: "Insert the data sources (citations) of the layers drawn in the frame" },
  { id: "layoutFromMap", label: "Set from Map View", icon: <LocationTargetSquareRegular />, run: () => void setFrameFromMapView(), enabled: hasFrame, tip: "Centre the selected frame on the map view and use the nearest standard scale that shows it" },
  { id: "layoutFrameCrs", label: "Frame CRS", icon: <GlobeRegular />, run: () => pickFrameCrs(), enabled: hasFrame, tip: "Choose the coordinate system the frame is drawn in (true scale needs a projected one)" },
  { id: "layoutPdf", label: "PDF", icon: <DocumentPdfRegular />, run: () => void exportLayout("pdf"), enabled: inLayout, tip: "Export the page as a vector PDF" },
  { id: "layoutPng", label: "PNG", icon: <ImageRegular />, run: () => void exportLayout("png"), enabled: inLayout, tip: "Export the page as a PNG at the resolution in the dpi box (300 by default)" },
  { id: "layoutSvg", label: "SVG", icon: <ShapesRegular />, run: () => void exportLayout("svg"), enabled: inLayout, tip: "Export the page as SVG for finishing in Inkscape or Illustrator" },

  // Arrange tab › Group, View tab › Zoom, Export tab › This Page (the layout workspace)
  { id: "layoutGroup", label: "Group", icon: <GroupRegular />, run: () => useLayouts.getState().group(), enabled: () => useLayouts.getState().selected.length > 1, tip: "Select and move the chosen elements together" },
  { id: "layoutUngroup", label: "Ungroup", icon: <GroupRegular />, run: () => useLayouts.getState().ungroup(), enabled: () => { const s = useLayouts.getState(); return s.selected.some((id) => s.doc?.elements.find((e) => e.id === id)?.group); }, tip: "Split the selected group into separate elements" },
  { id: "layoutLock", label: "Lock", icon: <LockClosedRegular />, run: () => void toggleLockSelected(), enabled: () => useLayouts.getState().selected.length > 0, checked: () => { const s = useLayouts.getState(); const els = s.doc?.elements.filter((e) => s.selected.includes(e.id)) ?? []; return els.length > 0 && els.every((e) => e.locked); }, tip: "Lock the selected elements so they cannot be moved or deleted by mistake; press again to unlock" },
  { id: "layoutFit", label: "Fit Page", icon: <ArrowMaximizeRegular />, run: () => useLayoutView.getState().fitPage(), enabled: inLayout, checked: () => inLayout() && useLayoutView.getState().fit, tip: "Show the whole page in the window" },
  { id: "layoutActual", label: "100%", icon: <ZoomFitRegular />, run: () => useLayoutView.getState().actualSize(), enabled: inLayout, tip: "Show the page at its printed size on screen" },
  { id: "layoutZoomIn", label: "Zoom In", icon: <ZoomInRegular />, run: () => useLayoutView.getState().zoomBy(1.25), enabled: inLayout },
  { id: "layoutZoomOut", label: "Zoom Out", icon: <ZoomOutRegular />, run: () => useLayoutView.getState().zoomBy(1 / 1.25), enabled: inLayout },
  { id: "layoutPreflight", label: "Check Page", icon: <TaskListSquareLtrRegular />, run: () => { const v = useLayoutView.getState(); v.setPreflight(!v.preflightOpen); }, enabled: inLayout, checked: () => useLayoutView.getState().preflightOpen, tip: "Before exporting: is the coordinate system named, are the sources listed, do the scale bar and legend belong to a frame on the page" },

  // View tab › Windows
  paneToggle("paneContents", "contents", "Map Layers", <PanelLeftRegular />, "Show or hide the list of layers"),
  paneToggle("paneCatalog", "catalog", "Catalog", <FolderOpenRegular />, "Show or hide the Catalog: this project, its layouts and Public Data"),
  paneToggle("paneItems", "items", "Items", <PanelLeftContractRegular />, "Show or hide the list of what is on the layout page"),
  paneToggle("paneProperties", "properties", "Properties", <PanelRightRegular />, "Show or hide Properties"),
  paneToggle("paneSources", "sources", "Sources", <BookRegular />, "Every data source used in this project, as an annotated bibliography you can export"),
  { id: "paneMap", label: "Map", icon: <MapRegular />, run: () => showPane("map") },
  { id: "resetPanes", label: "Reset Panes", icon: <ArrowResetRegular />, run: defaultLayout, tip: "Restore the default pane layout" },
  { id: "fps", label: "Frame Rate", icon: <TopSpeedRegular />, run: () => app().set({ showFps: !app().showFps }), checked: () => app().showFps, tip: "Show map frames per second" },
  { id: "benchmark", label: "Benchmark", icon: <TopSpeedRegular />, run: () => useDialog.getState().show({ kind: "benchmark" }), tip: "Fly the map and measure smoothness" },
  { id: "theme", label: "Dark Theme", icon: <DarkThemeRegular />, run: () => app().set({ dark: !app().dark }), checked: () => app().dark },
  { id: "colours", label: "Colours", icon: <ColorRegular />, run: toggleAppearance, checked: () => useAppearance.getState().open, tip: "Choose the accent, highlight, panel and tool colours" },

  // Feature Layer contextual tab
  { id: "zoomLayer", label: "Zoom To Layer", icon: <ZoomFitRegular />, run: () => active() && zoomToLayer(active()!.id), enabled: () => !!active()?.extent },
  { id: "symbology", label: "Symbology", icon: <PaintBrushRegular />, run: () => { const a = active(); if (a) openLayerProperties(a.id, "symbology"); }, enabled: () => !!active() && active()!.geomType !== "Table" },
  { id: "layerProps", label: "Properties", icon: <SettingsRegular />, run: () => { const a = active(); if (a) openLayerProperties(a.id); }, enabled: () => !!active(), tip: "Open the Layer Properties window" },
  { id: "openTable", label: "Attribute Table", icon: <TableRegular />, run: () => showPane("table"), enabled: () => !!active() },
  { id: "removeLayer", label: "Remove", icon: <DeleteRegular />, run: () => active() && removeLayer(active()!.id), enabled: () => !!active() },
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

export const commands: Record<string, Command> = Object.fromEntries([...list, ...publicData].map((c) => [c.id, c]));
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
