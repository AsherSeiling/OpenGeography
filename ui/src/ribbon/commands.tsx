// Every ribbon command lives in this one registry. The ribbon layout, menus
// and the "Tell me" search all read from it.
import {
  AddCircleRegular,
  MyLocationRegular,
  LocationRegular,
  BookRegular,
  FolderOpenRegular,
  MapRegular,
  ShapesRegular,
  TargetArrowRegular,
  SettingsRegular,
  FilterRegular,
  LayerDiagonalAddRegular,
  CalculatorRegular,
  GlobeDesktopRegular,
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
  MountainTrailRegular,
  PaintBrushRegular,
  PanelBottomRegular,
  PanelLeftRegular,
  PanelRightRegular,
  PlayRegular,
  RectangleLandscapeRegular,
  SaveEditRegular,
  SaveRegular,
  SelectAllOnRegular,
  TableRegular,
  TopSpeedRegular,
  WrenchRegular,
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
  toggleWorldView,
  removeLayer,
  runModel,
  saveProject,
  useDialog,
  zoomToLayer,
  zoomToSelection,
} from "../actions";
import { zoomToExtent } from "../map/mapController";
import { openLayerProperties } from "../dialogs/LayerPropertiesWindow";
import { useNav } from "../nav/navStore";
import { exportLayout, insertMapFrame, insertOverview, insertPicture, insertSurround, newLayout, pickFrameCrs, setFrameFromMapView } from "../layout/actions";
import { addChart, addShape, addTable, addText, useLayouts } from "../store/layout";
import { addToolToModel } from "../panes/ModelPane";
import { defaultLayout, showPane, togglePane } from "../panes/panes";
import { useApp, type Basemap, type MapTool } from "../store/store";

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
const comingSoon = (what: string, milestone: string) => () =>
  app().setStatus(`${what} is planned for ${milestone}. See the project plan for details.`);

const list: Command[] = [
  // Project
  { id: "new", label: "New", icon: <DocumentAddRegular />, run: newProject, tip: "Start a new project" },
  { id: "open", label: "Open", icon: <FolderOpenRegular />, run: openProject, keys: "Ctrl+O", tip: "Open a .gproj project folder" },
  { id: "save", label: "Save", icon: <SaveRegular />, run: () => saveProject(), keys: "Ctrl+S", tip: "Save the project" },
  { id: "saveAs", label: "Save As", icon: <SaveEditRegular />, run: () => saveProject(true), tip: "Save the project to a new folder" },

  // Map tab
  { id: "explore", label: "Explore", icon: <HandLeftRegular />, run: setTool("pan"), checked: tool("pan"), tip: "Pan and zoom the map" },
  { id: "fullExtent", label: "Full Extent", icon: <ZoomFitRegular />, run: zoomFull, tip: "Zoom to all layers" },
  { id: "addData", label: "Add Data", icon: <AddCircleRegular />, run: () => addData(), tip: "Add shapefiles, GeoPackages, GeoJSON, KML, CSV, GeoParquet…" },
  { id: "select", label: "Select", icon: <CursorClickRegular />, run: setTool("select"), checked: tool("select"), tip: "Click features to select (Shift adds, Ctrl toggles)" },
  { id: "selectBox", label: "Select by Rectangle", icon: <RectangleLandscapeRegular />, run: setTool("selectBox"), checked: tool("selectBox"), tip: "Drag a box to select features" },
  { id: "selectByLocation", label: "Select By Location", icon: <LocationRegular />, run: () => active() && useDialog.getState().show({ kind: "selectByLocation", layer: active()!.id }), enabled: () => !!active() && Object.keys(app().layers).length > 1, tip: "Select features by where they are relative to another layer's features" },
  { id: "selectByAttributes", label: "Select By Attributes", icon: <SelectAllOnRegular />, run: () => active() && useDialog.getState().show({ kind: "selectByAttributes", layer: active()!.id }), enabled: () => !!active() },
  { id: "zoomSelection", label: "Zoom To Selection", icon: <ZoomFitRegular />, run: zoomToSelection, enabled: hasSelection },
  { id: "clearSelection", label: "Clear", icon: <DismissCircleRegular />, run: () => app().clearSelection(), enabled: hasSelection, tip: "Clear the selection in all layers" },
  { id: "identify", label: "Identify", icon: <InfoRegular />, run: setTool("identify"), checked: tool("identify"), tip: "Click a feature to see its attributes" },
  { id: "measure", label: "Measure", icon: <RulerRegular />, run: () => { useNav.getState().set({ measureMode: "distance" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "distance", tip: "Geodesic distance and azimuths: click points, double-click to finish" },
  { id: "measureArea", label: "Measure Area", icon: <ShapesRegular />, run: () => { useNav.getState().set({ measureMode: "area" }); app().setTool("measure"); }, checked: () => app().tool === "measure" && useNav.getState().measureMode === "area", tip: "Geodesic area and perimeter" },
  { id: "resection", label: "Resection", icon: <MyLocationRegular />, run: () => useNav.getState().set({ navPanel: "resection" }), checked: () => useNav.getState().navPanel === "resection", tip: "Find your position from azimuths to known landmarks (MGRS, mils, G-M angle)" },
  { id: "intersection", label: "Intersection", icon: <TargetArrowRegular />, run: () => useNav.getState().set({ navPanel: "intersection" }), checked: () => useNav.getState().navPanel === "intersection", tip: "Locate an unseen point from azimuths taken at known positions" },
  { id: "census", label: "Census Data", icon: <TableAddRegular />, run: () => active() && useDialog.getState().show({ kind: "census", layer: active()!.id }), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Join American Community Survey or 2020 Census values to the active layer by GEOID" },
  { id: "overture", label: "Overture Maps", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "overture" }), tip: "Download buildings, places, roads, water, land use or boundaries for your area from Overture Maps (open GeoParquet)" },
  { id: "ogc", label: "OGC API Features", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "ogc" }), tip: "Download features from USGS Water Data or any OGC API – Features service for your area" },
  { id: "hazardsSoils", label: "Earthquakes & Soils", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "hazardsSoils" }), tip: "Download USGS earthquakes or NRCS soil map units for your area" },
  { id: "stac", label: "Imagery & DEM", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "stac" }), tip: "Search Sentinel-2 imagery and the Copernicus DEM (STAC) and clip a scene to your area straight from the cloud" },
  { id: "arcgis", label: "ArcGIS Services", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "arcgis" }), tip: "Download PAD-US, FEMA flood zones, wetlands, wildfire perimeters or any ArcGIS REST layer for your area" },
  { id: "worldView", label: "World View", icon: <GlobeDesktopRegular />, run: toggleWorldView, checked: () => app().worldView, tip: "Show the whole Earth in this coordinate system (cut at the opposite side of its central meridian)" },
  { id: "worldGraticule", label: "Graticule", icon: <GlobeRegular />, run: addWorldReference, tip: "Add curved latitude/longitude lines and the coordinate system's own reference lines (central meridian, standard parallels, edge of the map)" },
  { id: "worldOutline", label: "World Outline", icon: <GlobeRegular />, run: addWorldOutline, tip: "Add the bundled Natural Earth land outline (public domain, works offline)" },
  { id: "projectionBuilder", label: "Projection Builder", icon: <SettingsRegular />, run: () => useDialog.getState().show({ kind: "projectionBuilder" }), tip: "Build a conic, cylindrical, azimuthal or world projection from its parameters" },
  { id: "tissot", label: "Tissot Circles", icon: <GlobeRegular />, run: addTissotCircles, tip: "Add circles that are round on the ground; their shape in the map's coordinate system shows its distortion" },
  { id: "datumGrids", label: "Datum Grids", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "datumGrids" }), tip: "See which datum-shift grids are installed (they make coordinate conversions between datums accurate), and download more" },
  { id: "responsibleUse", label: "Limits & Responsible Use", icon: <InfoRegular />, run: () => useDialog.getState().show({ kind: "responsibleUse" }), tip: "What this program can and cannot be relied on for: datum accuracy, heights, distortion, estimates, sensitive coordinates" },
  { id: "mapCrs", label: "Coordinate System", icon: <GlobeRegular />, run: pickMapCrs, tip: "Set the map's coordinate system (used for coordinates, measurements and layouts)" },

  // Data tab
  { id: "defineProjection", label: "Define Projection", icon: <GlobeRegular />, run: () => active() && useDialog.getState().show({ kind: "defineProjection", layer: active()!.id }), enabled: () => !!active() },
  { id: "definitionQuery", label: "Definition Query", icon: <FilterRegular />, run: () => active() && openDefinitionQuery(active()!.id), enabled: () => !!active(), tip: "Limit the layer to features matching an expression" },
  { id: "layerFromSelection", label: "Layer From Selection", icon: <LayerDiagonalAddRegular />, run: () => active() && layerFromSelection(active()!.id), enabled: () => !!active() && (app().selection[active()!.id]?.length ?? 0) > 0, tip: "Copy the selected features into a new layer" },
  { id: "splitByAttribute", label: "Split by Attribute", icon: <LayerDiagonalAddRegular />, run: () => active() && splitByAttribute(active()!.id), enabled: () => !!active() && active()!.geomType !== "Raster", tip: "Make one layer or one file per value of a field" },
  { id: "exportTable", label: "Export Table", icon: <ArrowExportRegular />, run: () => active() && exportTable(active()!.id), enabled: () => active()?.geomType === "Point", tip: "Write a point layer's attributes and coordinates (in a coordinate system you choose) as CSV or Parquet" },
  { id: "exportLayer", label: "Export Features", icon: <ArrowExportRegular />, run: () => active() && exportLayer(active()!.id), enabled: () => !!active() },
  { id: "nationalMap", label: "National Map", icon: <MapRegular />, run: () => useDialog.getState().show({ kind: "nationalMap" }), tip: "Download USGS streams, lakes, watersheds, roads, trails, structures, place names and federal lands for an area" },
  { id: "elevation", label: "Elevation (3DEP)", icon: <MountainTrailRegular />, run: () => useDialog.getState().show({ kind: "elevation" }), tip: "Download USGS 3DEP elevation for the map view or a layer's area" },
  { id: "tiger", label: "Sample Data (TIGER)", icon: <GlobeDesktopRegular />, run: () => useDialog.getState().show({ kind: "tiger" }), tip: "Download Census boundaries, roads and water for any state or county" },
  { id: "addField", label: "Add Field", icon: <TableAddRegular />, run: () => active() && useDialog.getState().show({ kind: "addField", layer: active()!.id }), enabled: canEditActive },
  { id: "calcField", label: "Calculate Field", icon: <CalculatorRegular />, run: () => active() && useDialog.getState().show({ kind: "calcField", layer: active()!.id, owner: active()!.id }), enabled: canEditActive },
  { id: "calcGeometry", label: "Calculate Geometry", icon: <RulerRegular />, run: () => active() && useDialog.getState().show({ kind: "calcGeometry", layer: active()!.id, owner: active()!.id }), enabled: () => canEditActive() && active()!.geomType !== "Table" },
  { id: "testData", label: "Generate Test Data", icon: <DataUsageRegular />, run: () => useDialog.getState().show({ kind: "testData" }), tip: "Random points or polygons, to try tools and check speed" },

  // Analysis tab
  { id: "modelBuilder", label: "Model", icon: <FlowchartRegular />, run: () => showPane("model"), tip: "Open the Model (node editor)" },
  { id: "runModel", label: "Run Model", icon: <PlayRegular />, run: () => runModel(), tip: "Run every step of the model" },
  { id: "tools", label: "Tools", icon: <WrenchRegular />, run: () => showPane("tools"), tip: "Open the Geoprocessing pane" },

  // Insert tab (later milestones)
  { id: "newLayout", label: "New Layout", icon: <DocumentPdfRegular />, run: () => newLayout(), tip: "Create a print layout (Letter landscape); more page sizes are in the Insert tab's gallery" },
  { id: "topoMap", label: "Topo Map", icon: <MountainTrailRegular />, run: comingSoon("The Topo Map Builder", "Milestone 4") },

  // Layout contextual tab (shown while a layout tab is active)
  { id: "layoutMapFrame", label: "Map Frame", icon: <MapRegular />, run: insertMapFrame, enabled: inLayout, tip: "Insert a map frame in the map's coordinate system, centred on the map view" },
  { id: "layoutOverview", label: "Overview Map", icon: <MapRegular />, run: insertOverview, enabled: hasFrame, tip: "Insert a small overview map of the selected frame with its extent outlined" },
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
  { id: "layoutSources", label: "Sources", icon: <BookInformationRegular />, run: () => insertSurround("sources"), enabled: hasFrame, tip: "Insert the data sources (citations) of the layers drawn in the frame" },
  { id: "layoutFromMap", label: "Set from Map View", icon: <ZoomFitRegular />, run: () => void setFrameFromMapView(), enabled: hasFrame, tip: "Centre the selected frame on the map view and use the nearest standard scale that shows it" },
  { id: "layoutFrameCrs", label: "Frame CRS", icon: <GlobeRegular />, run: () => pickFrameCrs(), enabled: hasFrame, tip: "Choose the coordinate system the frame is drawn in (true scale needs a projected one)" },
  { id: "layoutPdf", label: "PDF", icon: <DocumentPdfRegular />, run: () => void exportLayout("pdf"), enabled: inLayout, tip: "Export the page as a vector PDF" },
  { id: "layoutPng", label: "PNG", icon: <ImageRegular />, run: () => void exportLayout("png"), enabled: inLayout, tip: "Export the page as a PNG at the resolution in the dpi box (300 by default)" },
  { id: "layoutSvg", label: "SVG", icon: <ShapesRegular />, run: () => void exportLayout("svg"), enabled: inLayout, tip: "Export the page as SVG for finishing in Inkscape or Illustrator" },

  // View tab
  { id: "paneContents", label: "Contents", icon: <PanelLeftRegular />, run: () => togglePane("contents") },
  { id: "paneTable", label: "Attribute Table", icon: <TableRegular />, run: () => togglePane("table") },
  { id: "paneModel", label: "Model", icon: <PanelBottomRegular />, run: () => togglePane("model") },
  { id: "paneProperties", label: "Properties", icon: <PanelRightRegular />, run: () => togglePane("properties") },
  { id: "sources", label: "Sources & Citations", icon: <BookRegular />, run: () => showPane("sources"), tip: "Every data source used in this project, as an annotated bibliography you can export" },
  { id: "paneSources", label: "Sources", icon: <BookRegular />, run: () => togglePane("sources") },
  { id: "catalog", label: "Catalog", icon: <FolderOpenRegular />, run: () => showPane("catalog"), tip: "Browse folders and add GIS data (file manager)" },
  { id: "paneCatalog", label: "Catalog", icon: <FolderOpenRegular />, run: () => togglePane("catalog") },
  { id: "paneTools", label: "Geoprocessing", icon: <WrenchRegular />, run: () => togglePane("tools") },
  { id: "paneMap", label: "Map", icon: <MapRegular />, run: () => showPane("map") },
  { id: "resetPanes", label: "Reset Panes", icon: <ArrowResetRegular />, run: defaultLayout, tip: "Restore the default pane layout" },
  { id: "fps", label: "Frame Rate", icon: <TopSpeedRegular />, run: () => app().set({ showFps: !app().showFps }), checked: () => app().showFps, tip: "Show map frames per second" },
  { id: "benchmark", label: "Benchmark", icon: <TopSpeedRegular />, run: () => useDialog.getState().show({ kind: "benchmark" }), tip: "Fly the map and measure smoothness" },
  { id: "theme", label: "Dark Theme", icon: <DarkThemeRegular />, run: () => app().set({ dark: !app().dark }), checked: () => app().dark },

  // Feature Layer contextual tab
  { id: "zoomLayer", label: "Zoom To Layer", icon: <ZoomFitRegular />, run: () => active() && zoomToLayer(active()!.id), enabled: () => !!active()?.extent },
  { id: "symbology", label: "Symbology", icon: <PaintBrushRegular />, run: () => { const a = active(); if (a) openLayerProperties(a.id, "symbology"); }, enabled: () => !!active() && active()!.geomType !== "Table" },
  { id: "layerProps", label: "Properties", icon: <SettingsRegular />, run: () => { const a = active(); if (a) openLayerProperties(a.id); }, enabled: () => !!active(), tip: "Open the Layer Properties window" },
  { id: "openTable", label: "Attribute Table", icon: <TableRegular />, run: () => showPane("table"), enabled: () => !!active() },
  { id: "removeLayer", label: "Remove", icon: <DeleteRegular />, run: () => active() && removeLayer(active()!.id), enabled: () => !!active() },
];

export const TOOL_BUTTONS = ["buffer", "clip", "intersect", "dissolve", "spatial_join", "attribute_join", "project", "filter", "calculate_field", "calculate_geometry"];

export function toolCommand(toolId: string, name: string, description: string): Command {
  return {
    id: `tool:${toolId}`,
    label: name,
    icon: <WrenchRegular />,
    tip: description,
    run: () => {
      showPane("model");
      addToolToModel(toolId);
    },
  };
}

export const BASEMAP_OPTIONS: [Basemap, string][] = [
  ["usgsTopo", "USGS Topo"],
  ["usgsImagery", "USGS Imagery"],
  ["usgsRelief", "USGS Shaded Relief"],
  ["osm", "OpenStreetMap"],
  ["none", "None"],
];

export const commands: Record<string, Command> = Object.fromEntries(list.map((c) => [c.id, c]));

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
