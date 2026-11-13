// What is on each ribbon tab. Commands are looked up in commands.tsx; custom controls live in controls.tsx and shelf.tsx.
import { AlignMenu, BasemapGallery, DistributeMenu, DpiBox, MapFrameInsert, NewLayoutGallery, OrderMenu, SameSizeMenu, ScaleBox, SeriesExport, SeriesGroup, ShapeMenu, TemplateGroup, ThemeMenu, WhichMapMenu } from "./controls";
// PageMenu lives in Ribbon.tsx (a hoisted function declaration, so the import cycle is safe).
import { PageMenu } from "./Ribbon";
import { Shelf } from "./shelf";
import { ToolMenus } from "./toolDropdowns";
import { SwipeMenu } from "../map/SwipeUi";
import { BookmarksMenu, FrameBookmarkMenu } from "../map/BookmarkMenu";
import { AddCardMenu, ComingNote } from "../stats/ribbonControls";
import type { Group } from "./tabs";

export const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }, { cmd: "goToCoordinates" }, { custom: <BookmarksMenu /> }, { custom: <SwipeMenu /> }] },
    { label: "Layer", items: [{ cmd: "addData" }, { custom: <BasemapGallery /> }] },
    {
      label: "Selection",
      items: [{ cmd: "select" }, { cmd: "selectBox", size: "small" }, { cmd: "selectByAttributes", size: "small" }, { cmd: "selectByLocation", size: "small" }, { cmd: "zoomSelection", size: "small" }, { cmd: "clearSelection" }],
    },
    { label: "Inquiry", items: [{ cmd: "identify" }, { cmd: "measure" }, { cmd: "measureArea", size: "small" }] },
    { label: "Edit", items: [{ cmd: "startEditing" }, { cmd: "undoLastSave", size: "small" }] },
  ],
  // Getting data in and describing where it came from. Server connections and Census go here when they are built.
  Data: [
    { label: "Add", items: [{ cmd: "addData" }, { cmd: "catalog" }] },
    { label: "Create", items: [{ cmd: "newLayer" }, { cmd: "newTable" }] },
    { label: "Georeference", items: [{ cmd: "georeference" }, { cmd: "adjustShapes" }] },
    { label: "Sources", items: [{ cmd: "paneSources" }] },
  ],
  Coordinates: [
    { label: "Coordinate System", items: [{ cmd: "mapCrs" }, { cmd: "projectionBuilder" }, { cmd: "datumGrids" }, { cmd: "tissot" }, { cmd: "responsibleUse" }] },
    { label: "World", items: [{ cmd: "worldView" }, { cmd: "worldGraticule" }, { cmd: "worldOutline" }] },
    { label: "Land Navigation", items: [{ cmd: "resection" }, { cmd: "intersection" }, { cmd: "usngGrid" }] },
  ],
  Insert: [
    { label: "Layouts", items: [{ custom: <NewLayoutGallery />, covers: ["newLayout"] }] },
    { label: "Maps", items: [{ cmd: "newMap" }, { cmd: "duplicateMap" }, { cmd: "topoMap" }] },
    { label: "Statistics", items: [{ cmd: "statNewSheet" }] },
  ],
  View: [
    { label: "Zoom", when: "layout", items: [{ cmd: "layoutFit" }, { cmd: "layoutActual" }, { cmd: "layoutZoomIn", size: "small" }, { cmd: "layoutZoomOut", size: "small" }] },
    {
      label: "Windows",
      items: [
        { cmd: "paneItems", size: "small" },
        { cmd: "paneContents", size: "small" },
        { cmd: "paneCatalog", size: "small" },
        { cmd: "paneModel", size: "small" },
        { cmd: "paneTable", size: "small" },
        { cmd: "paneProperties", size: "small" },
        { cmd: "paneMap", size: "small" },
        { cmd: "resetPanes" },
      ],
    },
    { label: "Appearance", items: [{ cmd: "theme" }, { cmd: "colours" }] },
    { label: "Program", items: [{ cmd: "settings" }, { cmd: "saveStartup", size: "small" }, { cmd: "resetStartup", size: "small" }] },
    { label: "Developer", when: "developer", items: [{ cmd: "fps", size: "small" }, { cmd: "benchmark", size: "small" }, { cmd: "testData", size: "small" }] },
  ],
  Analysis: [
    { label: "Geoprocessing", items: [{ cmd: "addTool" }, { cmd: "runModel" }] },
    { label: "Tools", items: [{ custom: <ToolMenus /> }] },
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
    { label: "Map", items: [{ custom: <WhichMapMenu /> }, { cmd: "layoutFromMap" }, { custom: <FrameBookmarkMenu /> }] },
    { label: "Scale and CRS", items: [{ custom: <ScaleBox /> }, { cmd: "layoutFrameCrs" }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }, { cmd: "removeLayer" }] },
    { label: "Fields", items: [{ cmd: "addField", size: "small" }, { cmd: "calcField", size: "small" }, { cmd: "calcGeometry", size: "small" }] },
    { label: "Filter", items: [{ cmd: "definitionQuery" }] },
    { label: "Data", items: [{ cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "exportTable" }, { cmd: "layerFromSelection" }, { cmd: "splitByAttribute" }, { cmd: "census" }] },
  ],
};


/**
 * The statistics workspace's tabs (docs/STATISTICS_WORKSPACE_PLAN.md). Ids are prefixed so "Export" here is not the layout's Export tab; the tab shows
 * `STATS_LABELS[id]`. Explore holds the sheet and card commands that work now. The other tabs say which package builds them (plain text, not a
 * button), so there is no control that does nothing.
 */
export const STATS_ORDER = ["stats-explore", "stats-charts", "stats-tests", "stats-regression", "stats-spatial", "stats-export"];
export const STATS_LABELS: Record<string, string> = { "stats-explore": "Explore", "stats-charts": "Charts", "stats-tests": "Tests", "stats-regression": "Regression", "stats-spatial": "Spatial", "stats-export": "Export" };
export const STATS_TABS: Record<string, Group[]> = {
  "stats-explore": [
    { label: "Sheet", items: [{ cmd: "statNewSheet" }, { cmd: "statRenameSheet", size: "small" }, { cmd: "statDeleteSheet", size: "small" }] },
    { label: "Cards", items: [{ custom: <AddCardMenu /> }, { cmd: "statRemoveCard", size: "small" }, { cmd: "statCardUp", size: "small" }, { cmd: "statCardDown", size: "small" }] },
    { label: "Run", items: [{ cmd: "statRun" }, { cmd: "statRunAll" }] },
  ],
  "stats-charts": [{ label: "Charts", items: [{ custom: <ComingNote>Histogram, box plot, scatter plot, Q–Q plot, bar and line charts and a correlation heat map. Coming in ST3.</ComingNote> }] }],
  "stats-tests": [{ label: "Tests", items: [{ custom: <ComingNote>t tests, ANOVA, chi-square, rank tests, normality and correlation. Coming in ST4.</ComingNote> }] }],
  "stats-regression": [{ label: "Regression", items: [{ custom: <ComingNote>Linear and logistic regression with residual checks. Coming in ST5.</ComingNote> }] }],
  "stats-spatial": [{ label: "Spatial", items: [{ custom: <ComingNote>Spatial weights, Moran scatter plot, LISA, GWR, spatial lag and error models. Coming in ST6.</ComingNote> }] }],
  "stats-export": [{ label: "Export", items: [{ custom: <ComingNote>Send a card to a layout, export tables and charts, and a report of the sheet. Coming in ST7.</ComingNote> }] }],
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
/** The tabs shown, in order, while a layout is open: Map, Data, Coordinates and Analysis leave, the rest do not move. */
export const LAYOUT_ORDER = ["Page", "Insert", "Arrange", "View", "Export"];
export const MAIN_TABS = ["Map", "Data", "Coordinates", "Analysis", "Insert", "View"];
export const KEY_TIPS: Record<string, string> = { Map: "M", Data: "D", Coordinates: "C", Analysis: "A", Insert: "I", View: "V" };

/** Every command id a tab (or a custom control on it) offers. */
/** The groups of the Edit tab, which is on the ribbon only while a shape-edit session is open (EditHost registers it). */
export const EDIT_GROUPS: Group[] = [
  { label: "Session", items: [{ cmd: "editSave" }, { cmd: "editDiscard" }, { cmd: "editUndo", size: "small" }, { cmd: "editRedo", size: "small" }, { cmd: "editFinish", size: "small" }] },
  { label: "Create", items: [{ cmd: "editPoint" }, { cmd: "editLine" }, { cmd: "editPolygon" }, { cmd: "editRect", size: "small" }, { cmd: "editCircle", size: "small" }, { cmd: "editCoords", size: "small" }] },
  { label: "Modify", items: [{ cmd: "editSelect" }, { cmd: "editModify" }, { cmd: "editDelete" }, { cmd: "editFix" }] },
  { label: "Reshape", items: [{ cmd: "editMerge" }, { cmd: "editSplit" }, { cmd: "editReshape" }, { cmd: "editHole" }, { cmd: "editFillHoles", size: "small" }, { cmd: "editExplode", size: "small" }, { cmd: "editMulti", size: "small" }] },
  { label: "Clipboard", items: [{ cmd: "editCopy" }, { cmd: "editPaste" }] },
  { label: "Snapping", items: [{ cmd: "editSnap" }] },
];

export function commandsOnTabs(tabs: Record<string, Group[]> = TABS): Set<string> {
  const ids = new Set<string>();
  for (const groups of [...Object.values(tabs), ...(tabs === TABS ? [RAIL, EDIT_GROUPS, ...Object.values(STATS_TABS)] : [])]) for (const g of groups) for (const it of g.items) if ("cmd" in it) ids.add(it.cmd); else for (const c of it.covers ?? []) ids.add(c);
  return ids;
}
