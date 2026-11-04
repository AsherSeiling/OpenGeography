// What is on each ribbon tab. Commands are looked up in commands.tsx; custom controls live in controls.tsx and shelf.tsx.
import { ArrangeMenu, BasemapGallery, DpiBox, NewLayoutGallery, ScaleBox, SeriesGroup, TemplateGroup, ThemeMenu } from "./controls";
// PageMenu lives in Ribbon.tsx (a hoisted function declaration, so the import cycle is safe).
import { PageMenu } from "./Ribbon";
import { Shelf } from "./shelf";
import type { Group } from "./tabs";

export const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }] },
    { label: "Layer", items: [{ cmd: "addData" }, { cmd: "catalog" }, { custom: <BasemapGallery /> }] },
    {
      label: "Selection",
      items: [{ cmd: "select" }, { cmd: "selectBox", size: "small" }, { cmd: "selectByAttributes", size: "small" }, { cmd: "selectByLocation", size: "small" }, { cmd: "zoomSelection", size: "small" }, { cmd: "clearSelection" }],
    },
    { label: "Inquiry", items: [{ cmd: "identify" }, { cmd: "measure" }, { cmd: "measureArea", size: "small" }] },
    { label: "Land Nav", items: [{ cmd: "resection" }, { cmd: "intersection" }, { cmd: "usngGrid" }] },
    { label: "Coordinate System", items: [{ cmd: "mapCrs" }, { cmd: "datumGrids" }, { cmd: "projectionBuilder" }, { cmd: "tissot" }, { cmd: "responsibleUse" }] },
    { label: "World", items: [{ cmd: "worldView" }, { cmd: "worldGraticule" }, { cmd: "worldOutline" }] },
  ],
  Insert: [
    { label: "Layouts", items: [{ custom: <NewLayoutGallery />, covers: ["newLayout"] }] },
    { label: "Maps", items: [{ cmd: "newMap" }, { cmd: "duplicateMap" }, { cmd: "topoMap" }] },
  ],
  View: [
    {
      label: "Windows",
      items: [
        { cmd: "paneContents", size: "small" },
        { cmd: "paneCatalog", size: "small" },
        { cmd: "paneModel", size: "small" },
        { cmd: "paneTable", size: "small" },
        { cmd: "paneProperties", size: "small" },
        { cmd: "paneSources", size: "small" },
        { cmd: "paneMap", size: "small" },
        { cmd: "resetPanes" },
      ],
    },
    { label: "Appearance", items: [{ cmd: "theme" }, { cmd: "colours" }] },
    { label: "Performance", items: [{ cmd: "fps", size: "small" }, { cmd: "benchmark", size: "small" }] },
    { label: "Testing", items: [{ cmd: "testData" }] },
  ],
  Analysis: [
    { label: "Geoprocessing", items: [{ cmd: "addTool" }, { cmd: "runModel" }, { cmd: "paneModel" }, { cmd: "paneTable" }] },
    { label: "Favourites", items: [{ custom: <Shelf /> }] },
  ],
  Layout: [
    { label: "Page", items: [{ custom: <PageMenu /> }] },
    { label: "Template", items: [{ custom: <TemplateGroup /> }] },
    { label: "Theme", items: [{ custom: <ThemeMenu /> }] },
    { label: "Map Series", items: [{ custom: <SeriesGroup /> }] },
  ],
  Elements: [
    { label: "Insert", items: [{ cmd: "layoutMapFrame" }, { cmd: "layoutOverview" }, { cmd: "layoutText" }, { cmd: "layoutTable" }, { cmd: "layoutChart" }, { cmd: "layoutLegend" }, { cmd: "layoutScaleBar" }, { cmd: "layoutNorthArrow" }, { cmd: "layoutDeclination" }, { cmd: "layoutSources" }] },
    { label: "Graphics", items: [{ cmd: "layoutRect" }, { cmd: "layoutEllipse" }, { cmd: "layoutTriangle" }, { cmd: "layoutLine" }, { cmd: "layoutArrow" }, { cmd: "layoutPicture" }] },
  ],
  Format: [
    { label: "Map Frame", items: [{ cmd: "layoutFromMap" }, { cmd: "layoutFrameCrs" }, { custom: <ScaleBox /> }] },
    { label: "Arrange", items: [{ custom: <ArrangeMenu /> }] },
  ],
  Export: [
    { label: "Export", items: [{ cmd: "layoutPdf" }, { cmd: "layoutPng" }, { cmd: "layoutSvg" }, { custom: <DpiBox /> }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }] },
    { label: "Fields", items: [{ cmd: "addField", size: "small" }, { cmd: "calcField", size: "small" }, { cmd: "calcGeometry", size: "small" }, { cmd: "census" }] },
    { label: "Filter", items: [{ cmd: "definitionQuery" }] },
    { label: "Data", items: [{ cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "exportTable" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
    { label: "Remove", items: [{ cmd: "removeLayer" }] },
  ],
};

/** The layout workspace's contextual tabs (LW1), all under one "Layout" band. */
export const LAYOUT_TABS = ["Layout", "Elements", "Format", "Export"];
export const MAIN_TABS = ["Map", "Insert", "View", "Analysis"];
export const KEY_TIPS: Record<string, string> = { Map: "M", Insert: "I", View: "V", Analysis: "A" };

/** Every command id a tab (or a custom control on it) offers. */
export function commandsOnTabs(tabs: Record<string, Group[]> = TABS): Set<string> {
  const ids = new Set<string>();
  for (const groups of Object.values(tabs)) for (const g of groups) for (const it of g.items) if ("cmd" in it) ids.add(it.cmd); else for (const c of it.covers ?? []) ids.add(c);
  return ids;
}
