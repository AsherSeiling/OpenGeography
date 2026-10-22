// Every ribbon command lives in this one registry. The ribbon layout, menus
// and the "Tell me" search all read from it.
import {
  AddCircleRegular,
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
  FolderOpenRegular,
  GlobeRegular,
  HandLeftRegular,
  InfoRegular,
  MapRegular,
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
} from "@fluentui/react-icons";
import type { ReactElement } from "react";
import {
  addData,
  exportLayer,
  layerFromSelection,
  openDefinitionQuery,
  newProject,
  openProject,
  pickMapCrs,
  removeLayer,
  runModel,
  saveProject,
  useDialog,
  zoomToLayer,
  zoomToSelection,
} from "../actions";
import { zoomToExtent } from "../map/mapController";
import { openLayerProperties } from "../dialogs/LayerPropertiesWindow";
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
  { id: "selectByAttributes", label: "Select By Attributes", icon: <SelectAllOnRegular />, run: () => active() && useDialog.getState().show({ kind: "selectByAttributes", layer: active()!.id }), enabled: () => !!active() },
  { id: "zoomSelection", label: "Zoom To Selection", icon: <ZoomFitRegular />, run: zoomToSelection, enabled: hasSelection },
  { id: "clearSelection", label: "Clear", icon: <DismissCircleRegular />, run: () => app().clearSelection(), enabled: hasSelection, tip: "Clear the selection in all layers" },
  { id: "identify", label: "Identify", icon: <InfoRegular />, run: setTool("identify"), checked: tool("identify"), tip: "Click a feature to see its attributes" },
  { id: "mapCrs", label: "Coordinate System", icon: <GlobeRegular />, run: pickMapCrs, tip: "Set the map's coordinate system (used for coordinates, measurements and layouts)" },

  // Data tab
  { id: "defineProjection", label: "Define Projection", icon: <GlobeRegular />, run: () => active() && useDialog.getState().show({ kind: "defineProjection", layer: active()!.id }), enabled: () => !!active() },
  { id: "definitionQuery", label: "Definition Query", icon: <FilterRegular />, run: () => active() && openDefinitionQuery(active()!.id), enabled: () => !!active(), tip: "Limit the layer to features matching an expression" },
  { id: "layerFromSelection", label: "Layer From Selection", icon: <LayerDiagonalAddRegular />, run: () => active() && layerFromSelection(active()!.id), enabled: () => !!active() && (app().selection[active()!.id]?.length ?? 0) > 0, tip: "Copy the selected features into a new layer" },
  { id: "exportLayer", label: "Export Features", icon: <ArrowExportRegular />, run: () => active() && exportLayer(active()!.id), enabled: () => !!active() },
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
  { id: "newLayout", label: "New Layout", icon: <DocumentPdfRegular />, run: comingSoon("The layout view", "Milestone 2") },
  { id: "topoMap", label: "Topo Map", icon: <MountainTrailRegular />, run: comingSoon("The Topo Map Builder", "Milestone 4") },

  // View tab
  { id: "paneContents", label: "Contents", icon: <PanelLeftRegular />, run: () => togglePane("contents") },
  { id: "paneTable", label: "Attribute Table", icon: <TableRegular />, run: () => togglePane("table") },
  { id: "paneModel", label: "Model", icon: <PanelBottomRegular />, run: () => togglePane("model") },
  { id: "paneProperties", label: "Properties", icon: <PanelRightRegular />, run: () => togglePane("properties") },
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
