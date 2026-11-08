// What is on each ribbon tab. Commands are looked up in commands.tsx; custom controls live in controls.tsx and shelf.tsx.
import { AlignMenu, BasemapGallery, DistributeMenu, DpiBox, MapFrameInsert, NewLayoutGallery, OrderMenu, SameSizeMenu, ScaleBox, SeriesExport, SeriesGroup, ShapeMenu, TemplateGroup, ThemeMenu, WhichMapMenu } from "./controls";
// PageMenu lives in Ribbon.tsx (a hoisted function declaration, so the import cycle is safe).
import { PageMenu } from "./Ribbon";
import { Shelf } from "./shelf";
import type { Group } from "./tabs";

export const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }] },
    { label: "Layer", items: [{ cmd: "addData" }, { cmd: "newTable" }, { cmd: "catalog" }, { custom: <BasemapGallery /> }] },
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
    { label: "Zoom", items: [{ cmd: "layoutFit" }, { cmd: "layoutActual" }, { cmd: "layoutZoomIn", size: "small" }, { cmd: "layoutZoomOut", size: "small" }] },
    {
      label: "Windows",
      items: [
        { cmd: "paneItems", size: "small" },
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
    { label: "Program", items: [{ cmd: "settings" }, { cmd: "saveStartup", size: "small" }, { cmd: "resetStartup", size: "small" }] },
    { label: "Performance", items: [{ cmd: "fps", size: "small" }, { cmd: "benchmark", size: "small" }] },
    { label: "Testing", items: [{ cmd: "testData" }] },
  ],
  Analysis: [
    { label: "Geoprocessing", items: [{ cmd: "addTool" }, { cmd: "runModel" }, { cmd: "paneModel" }, { cmd: "paneTable" }] },
    { label: "Favourites", items: [{ custom: <Shelf /> }] },
  ],
  // The layout workspace: Page, Insert (the same tab as in the map), Arrange, View (the same tab as in the map, with Zoom active) and Export.
  // Elements are inserted from the rail beside the page (RAIL below); Map Frame appears only while a frame is selected.
  Page: [
    { label: "Sheet", items: [{ custom: <PageMenu /> }] },
    { label: "Guides", items: [{ cmd: "layoutGuideV", size: "small" }, { cmd: "layoutGuideH", size: "small" }, { cmd: "layoutGuidesClear", size: "small" }] },
    { label: "Look", items: [{ custom: <ThemeMenu /> }] },
    { label: "Template", items: [{ custom: <TemplateGroup /> }] },
    { label: "Map Series", items: [{ custom: <SeriesGroup /> }] },
  ],
  Arrange: [
    { label: "Order", items: [{ custom: <OrderMenu /> }] },
    { label: "Line Up", items: [{ custom: <AlignMenu /> }, { custom: <DistributeMenu /> }, { custom: <SameSizeMenu /> }] },
    { label: "Group", items: [{ cmd: "layoutGroup", size: "small" }, { cmd: "layoutUngroup", size: "small" }, { cmd: "layoutLock", size: "small" }] },
  ],
  Export: [
    { label: "This Page", items: [{ cmd: "layoutPdf" }, { cmd: "layoutPng" }, { cmd: "layoutSvg" }, { cmd: "layoutPreflight" }] },
    { label: "Quality", items: [{ custom: <DpiBox /> }] },
    { label: "Map Series", items: [{ custom: <SeriesExport /> }] },
  ],
  "Map Frame": [
    { label: "Map", items: [{ custom: <WhichMapMenu /> }, { cmd: "layoutFromMap" }] },
    { label: "Scale and CRS", items: [{ custom: <ScaleBox /> }, { cmd: "layoutFrameCrs" }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }] },
    { label: "Fields", items: [{ cmd: "addField", size: "small" }, { cmd: "calcField", size: "small" }, { cmd: "calcGeometry", size: "small" }, { cmd: "census" }] },
    { label: "Filter", items: [{ cmd: "definitionQuery" }] },
    { label: "Data", items: [{ cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "exportTable" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }] },
    { label: "Remove", items: [{ cmd: "removeLayer" }] },
  ],
};


/** The Insert rail beside the layout page (option B): what goes on the page, in the doc's four groups. Ribbon-style groups, shown as a column. */
export const RAIL: Group[] = [
  { label: "Maps", items: [{ custom: <MapFrameInsert />, covers: ["layoutMapFrame"] }, { cmd: "layoutOverview" }] },
  { label: "Map Furniture", items: [{ cmd: "layoutLegend" }, { cmd: "layoutScaleBar" }, { cmd: "layoutNorthArrow" }, { cmd: "layoutDeclination" }] },
  { label: "Text and Data", items: [{ cmd: "layoutText" }, { cmd: "layoutSources" }, { cmd: "layoutTable" }, { cmd: "layoutChart" }] },
  { label: "Graphics", items: [{ custom: <ShapeMenu />, covers: ["layoutRect", "layoutEllipse", "layoutTriangle", "layoutLine", "layoutArrow"] }, { cmd: "layoutPicture" }] },
];

/** The tabs only a layout has (Map Frame only while a frame is selected). The Insert and View tabs are the main ones. */
export const LAYOUT_TABS = ["Page", "Arrange", "Export", "Map Frame"];
/** The tabs shown, in order, while a layout is open: Map and Analysis leave, the rest do not move. */
export const LAYOUT_ORDER = ["Page", "Insert", "Arrange", "View", "Export"];
export const MAIN_TABS = ["Map", "Insert", "View", "Analysis"];
export const KEY_TIPS: Record<string, string> = { Map: "M", Insert: "I", View: "V", Analysis: "A" };

/** Every command id a tab (or a custom control on it) offers. */
export function commandsOnTabs(tabs: Record<string, Group[]> = TABS): Set<string> {
  const ids = new Set<string>();
  for (const groups of [...Object.values(tabs), ...(tabs === TABS ? [RAIL] : [])]) for (const g of groups) for (const it of g.items) if ("cmd" in it) ids.add(it.cmd); else for (const c of it.covers ?? []) ids.add(c);
  return ids;
}
