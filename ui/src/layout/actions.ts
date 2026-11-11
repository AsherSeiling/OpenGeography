// Layout actions shared by the ribbon, Contents and the layout view.
import { open as openDialog, save } from "../ipc/dialogs";
import { useDialog } from "../actions";
import { reportError } from "../errors/store";
import { api, errorText, IN_TAURI, type TopoBuild, type TopoSpec } from "../ipc/api";
import { todayIso } from "./topoArea";
import { getCenterInMapCrs, getGroundPerPixel, getMap } from "../map/mapController";
import { anchorMapPanel, dockIsRebuilding, getDock, PANE_TITLES, showPane, type PaneId } from "../panes/panes";
import { isCancelled } from "../progress/progress";
import { useExportDpi } from "./exportDpi";
import { useSeriesView } from "./series";
import { useLayoutView } from "./view";
import { addMapFrame, addPicture, addSurround, printBasemap, flushLayoutSaves, newLayoutId, useLayouts } from "../store/layout";
import { effectiveStyle, useApp, type LayerStyle } from "../store/store";
import { recolour } from "./themes";
import { followedMap, frames, newLayoutDoc, normalizeLayout, roundScaleUp, type Layout, type MapFrame, type Orientation, type RenderRequest } from "./types";
import { isMapPanelId, mapOf } from "../store/maps";
import { printRefusal } from "../map/basemaps";
import { extentIn, scaleToFit, useBookmarks } from "../map/bookmarks";

const status = (s: string) => useApp.getState().setStatus(s);

/** What the engine needs besides the layout: styles, order and visibility from the map (a node result replaces its layer, as on screen). */
export function layoutRequest(layout: Layout): RenderRequest {
  const s = useApp.getState();
  const styles: Record<string, LayerStyle> = {};
  const visible: Record<string, boolean> = {};
  const order: string[] = [];
  const shownId = (id: string) => (s.results[id] && !s.showOriginal[id] ? s.results[id].id : id);
  for (const id of s.order) {
    const shown = shownId(id);
    order.push(shown);
    visible[shown] = s.visible[id] ?? true;
  }
  // Style is shared across maps, and a frame can draw any project layer: one on another map, or in a fixed list and on no map at all.
  for (const id of Object.keys(s.layers)) if (s.styles[id]) styles[shownId(id)] = effectiveStyle(s, id)!;
  // A frame that names a map gets that map's layers, order and visibility as an explicit list; the engine never sees the map id.
  // A frame whose map was deleted draws no layers (and export says so) rather than drawing another map's.
  layout = {
    ...layout,
    elements: layout.elements.map((e) => {
      const id = e.kind === "map_frame" ? followedMap((e as MapFrame).layers) : null;
      if (id === null) return e;
      const m = mapOf(s, id);
      return { ...e, layers: (m?.order ?? []).map((l) => ({ id: shownId(l), visible: m?.visible[l] ?? true })) } as typeof e;
    }),
  };
  const project = s.projectDir?.split(/[\\/]/).pop()?.replace(/\.gproj$/, "") || "Untitled";
  // The Layout view shows one page of the map series when asked to (and exports of the open page match it).
  const shown = useSeriesView.getState().page;
  const seriesPage = layout.series && shown !== null && useLayouts.getState().id === layout.id ? { seriesPage: shown } : {};
  return { layout, styles, order, visible, project, date: new Date().toISOString().slice(0, 10), ...seriesPage };
}

const panelId = (id: string) => `layout:${id}`;

/** Show a layout's tab, opening it beside the Map if needed. */
export function openLayout(id: string) {
  const dock = getDock();
  const layout = useLayouts.getState().list().find((l) => l.id === id);
  if (!dock || !layout) return;
  const existing = dock.getPanel(panelId(id));
  if (existing) return existing.api.setActive();
  dock.addPanel({
    id: panelId(id),
    component: "layout",
    title: layout.name,
    params: { layoutId: id },
    position: anchorMapPanel() ? { referencePanel: anchorMapPanel()!.id, direction: "within" } : undefined,
  });
}

/**
 * The dock's active panel changed. A layout tab opens its layout; any other panel
 * (Properties, Contents, Map) leaves the layout open. Closing the layout here made
 * a click into the Properties pane drop the selection and every later edit (bug U-5).
 */
export function followActivePanel(layoutId: string | undefined) {
  if (dockIsRebuilding()) return; // the dock is being rebuilt (Reset Panes, open project): activations on the way mean nothing
  if (layoutId) {
    useLayouts.getState().open(layoutId);
    enterWorkspace();
  } else if (!rearranging && workspace && isMapPanelId(getDock()?.activePanel?.id ?? "")) leaveWorkspace();
}

/**
 * Option B of docs/LAYOUT_UI_OPTIONS.md: while a layout is open the left column is Items (the page's own list; the Maps are on the Insert
 * rail) with Catalog stacked under it, instead of Map Layers, and the bottom panel (Node Editor, Attribute Table) is closed so the page gets the width and the
 * height. Items used to be a tab in the narrow Map Layers column, which cut its name off. What was there is remembered and put back on
 * Back to Map. Shift+A and a right-click still work: Shift+A on the page inserts an element, a tool picked elsewhere re-opens the Node Editor.
 */
interface Saved {
  bottom: { ids: PaneId[]; active: PaneId | null; height: number | null };
  /** Map Layers / Catalog were open (and Items was a tab beside them). */
  /** Width of the Properties column, put back after the columns beside it change (the dock would otherwise hand it the freed width). */
  propsWidth: number | null;
  left: { contents: boolean; catalog: boolean; items: boolean; width: number | null; contentsHeight: number | null };
}
let workspace: Saved | null = null;
const ITEMS_WIDTH = 240;
const BOTTOM_IDS: PaneId[] = ["table", "model"];
/** True while enterWorkspace closes and moves panels: the dock then activates some other group's panel, which must not read as "left the layout". */
let rearranging = false;

/** LW1: a layout tab is active, so the window becomes a layout workspace: Items beside the page, Properties on the right, no bottom panel. */
export function enterWorkspace() {
  const dock = getDock();
  useLayoutView.getState().setWorkspace(true);
  if (!dock || workspace) return;
  const group = (dock.getPanel("model") ?? dock.getPanel("table"))?.group ?? null;
  const ordered = group ? (group.panels.map((p) => p.id).filter((id) => BOTTOM_IDS.includes(id as PaneId)) as PaneId[]) : [];
  const active = group?.activePanel?.id as PaneId | undefined;
  const contents = dock.getPanel("contents");
  workspace = {
    bottom: { ids: ordered, active: active && ordered.includes(active) ? active : null, height: group ? group.api.height : null },
    propsWidth: dock.getPanel("properties")?.group.api.width ?? null,
    left: {
      contents: !!contents,
      catalog: !!dock.getPanel("catalog"),
      items: !!dock.getPanel("items"),
      width: contents ? contents.group.api.width : null,
      contentsHeight: contents ? contents.group.api.height : null,
    },
  };
  const layoutPanel = dock.panels.find((p) => p.id === panelId(useLayouts.getState().id ?? ""));
  rearranging = true;
  try {
    for (const id of [...ordered, "contents", "catalog", "items"]) dock.getPanel(id)?.api.close();
    // Items in a column of its own, whole height, at the far left.
    dock.addPanel({ id: "items", component: "items", title: PANE_TITLES.items, position: { direction: "left" } }).group.api.setSize({ width: ITEMS_WIDTH });
    // Catalog stays (layouts are renamed, copied and deleted there), stacked under Items rather than tabbed with it.
    if (workspace.left.catalog) {
      const p = dock.addPanel({ id: "catalog", component: "catalog", title: PANE_TITLES.catalog, position: { referencePanel: "items", direction: "below" } });
      p.group.api.setSize({ height: Math.round(dock.height * 0.34) });
    }
    showPane("properties");
    if (workspace.propsWidth) dock.getPanel("properties")?.group.api.setSize({ width: workspace.propsWidth });
    layoutPanel?.api.setActive();
  } finally {
    rearranging = false;
  }
}

/** Back to the Map: restore the bottom group and the left column (Map Layers over Catalog, Items where it was) as they were. */
export function leaveWorkspace() {
  const dock = getDock();
  useLayoutView.getState().setWorkspace(false);
  const w = workspace;
  workspace = null;
  // Reset Panes (and opening a project) rebuild the whole dock, closing the layout tab on the way: nothing to put back then.
  if (!dock || !w || dockIsRebuilding()) return;
  rearranging = true;
  try {
    dock.getPanel("items")?.api.close();
    dock.getPanel("catalog")?.api.close();
    if (w.left.contents) showPane("contents");
    if (w.left.catalog) showPane("catalog");
    if (w.left.items) showPane("items");
    const left = dock.getPanel("contents")?.group;
    if (left && w.left.width) left.api.setSize({ width: w.left.width, ...(w.left.catalog && w.left.contentsHeight ? { height: w.left.contentsHeight } : {}) });
    if (w.propsWidth) dock.getPanel("properties")?.group.api.setSize({ width: w.propsWidth });
    for (const id of w.bottom.ids) if (!dock.getPanel(id)) showPane(id);
    const bottom = w.bottom.ids.length ? dock.getPanel(w.bottom.ids[0])?.group : null;
    if (bottom && w.bottom.height) bottom.api.setSize({ height: w.bottom.height });
    if (w.bottom.active) dock.getPanel(w.bottom.active)?.api.setActive();
    dock.getPanel("contents")?.api.setActive();
  } finally {
    rearranging = false;
  }
}

/** A dock panel was closed: if it was the open layout's tab, no layout is open any more. */
export function layoutPanelClosed(layoutId: string | undefined) {
  if (layoutId && useLayouts.getState().id === layoutId) {
    useLayouts.getState().open(null);
    if (!getDock()?.panels.some((p) => p.id.startsWith("layout:") && p.id !== `layout:${layoutId}`)) leaveWorkspace();
  }
}

export function closeLayoutPanel(id: string) {
  getDock()?.getPanel(panelId(id))?.api.close();
}

export function setLayoutPanelTitle(id: string, title: string) {
  getDock()?.getPanel(panelId(id))?.api.setTitle(title);
}

export function newLayout(size = "Letter", orientation: Orientation = "landscape") {
  const names = new Set(useLayouts.getState().list().map((l) => l.name));
  let n = 1;
  while (names.has(`Layout ${n}`)) n++;
  const layout = newLayoutDoc(newLayoutId(), `Layout ${n}`, size, orientation);
  useLayouts.getState().add(layout);
  openLayout(layout.id);
  status(`Created ${layout.name} (${size} ${orientation}). Pick a start below, or add a Map Frame from the Insert rail.`);
}

export function renameLayout(id: string) {
  const cur = useLayouts.getState().list().find((l) => l.id === id);
  const name = cur && window.prompt("Layout name", cur.name)?.trim();
  if (!name) return;
  useLayouts.getState().rename(id, name);
  setLayoutPanelTitle(id, name);
}

export function deleteLayout(id: string) {
  const cur = useLayouts.getState().list().find((l) => l.id === id);
  if (!cur || !window.confirm(`Delete layout '${cur.name}'? This cannot be undone.`)) return;
  closeLayoutPanel(id);
  useLayouts.getState().remove(id);
}

export function duplicateLayout(id: string) {
  const copy = useLayouts.getState().duplicate(id);
  if (copy) openLayout(copy.id);
}

/** The selected map frame of the open layout, else its first one. */
export function targetFrame(): MapFrame | null {
  const s = useLayouts.getState();
  if (!s.doc) return null;
  const all = frames(s.doc);
  return all.find((f) => s.selected.includes(f.id)) ?? all[0] ?? null;
}

/** The frame with this id in the open layout (none if no layout is open or it was deleted), else the target frame. */
function frameById(id?: string): MapFrame | null {
  const doc = useLayouts.getState().doc;
  if (!id) return targetFrame();
  return doc ? (frames(doc).find((f) => f.id === id) ?? null) : null;
}

const SURROUND_NAMES = { legend: "legend", scale_bar: "scale bar", north_arrow: "north arrow", declination: "declination diagram", sources: "sources line" } as const;

/** Insert a legend, scale bar, north arrow or sources line for the selected (else first) map frame. */
export function insertSurround(kind: keyof typeof SURROUND_NAMES) {
  const frame = targetFrame();
  if (!frame) return status(`Insert a map frame first, then the ${SURROUND_NAMES[kind]} (Insert rail → Map Frame).`);
  addSurround(kind, frame.id);
}

/** Insert a map frame; `over` sets fields of the new frame (the map it follows, say). */
export function insertMapFrame(over: Partial<MapFrame> = {}) {
  const id = addMapFrame(over);
  if (id) void setFrameFromMapView(id);
}

/** Centre the frame on the map view and pick the smallest standard scale that shows what the map shows. */
export async function setFrameFromMapView(frameId?: string) {
  const frame = frameById(frameId);
  // A frame that follows a named map takes that map's view; otherwise the active map's.
  const followed = frame ? followedMap(frame.layers) : null;
  const src = followed ? mapOf(useApp.getState(), followed) : undefined;
  if (frame && followed && !src) return status("The map this frame follows was deleted. Choose another map in the frame's properties.");
  const map = getMap(src?.id);
  if (!frame || !map) return status(src ? `Open the map "${src.name}" (Catalog › Maps) to set the frame from its view.` : "Insert a map frame first (Insert rail → Map Frame).");
  if (!frame.crs) return status("Set a coordinate system for the map frame.");
  const c = getCenterInMapCrs(src?.id);
  const mpp = getGroundPerPixel(src?.id);
  const size = map.getSize();
  if (!c || !mpp || !size) return status("The map view isn't ready yet.");
  try {
    // The view centre is already in the map CRS; only a frame in another CRS needs the engine.
    const center = c.crs === frame.crs ? c.center : await api.transformPoint(c.center[0], c.center[1], c.crs, frame.crs);
    const w = (size[0] * mpp) / (Math.max(frame.w, 1) / 1000);
    const h = (size[1] * mpp) / (Math.max(frame.h, 1) / 1000);
    if (!center.every(Number.isFinite)) return status("That map position can't be shown in the frame's coordinate system. Choose a different one.");
    const mapBasemap = src?.basemap ?? useApp.getState().basemap;
    useLayouts.getState().update(frame.id, { center, scale: roundScaleUp(Math.max(w, h)), basemap: printBasemap(mapBasemap) });
    const refused = printRefusal(mapBasemap);
    if (refused) status(refused);
  } catch (e) {
    status(`Could not place the frame: ${errorText(e)}`);
  }
}

/** Point the frame at a bookmark: its extent is moved into the frame's CRS, the frame is centred on it and takes the smallest standard scale that shows all of it. */
export async function setFrameFromBookmark(bookmarkId: string, frameId?: string) {
  const frame = frameById(frameId);
  const bm = useBookmarks.getState().items.find((b) => b.id === bookmarkId);
  if (!frame) return status("Insert a map frame first (Insert rail → Map Frame).");
  if (!bm) return status("That bookmark no longer exists.");
  if (!frame.crs) return status("Set a coordinate system for the map frame.");
  try {
    const box = await extentIn(bm, frame.crs);
    const center: [number, number] = [(box[0] + box[2]) / 2, (box[1] + box[3]) / 2];
    // Ground per millimetre at a reference scale; the size shown grows in step with the scale number.
    const ref = 10000;
    const per = await api.layoutFrameUnitsPerMm(useLayouts.getState().doc?.body ?? "earth", frame.crs, center, ref, frame.w, frame.h);
    const scale = scaleToFit(box, [Math.max(frame.w, 1), Math.max(frame.h, 1)], per, ref);
    if (!Number.isFinite(scale) || scale <= 0) return status("That bookmark can't be fitted in this frame's coordinate system.");
    useLayouts.getState().update(frame.id, { center, scale: roundScaleUp(scale) });
    status(`Map frame set to the bookmark "${bm.name}".`);
  } catch (e) {
    status(`Could not place the frame: ${errorText(e)}`);
  }
}

export function pickFrameCrs(frameId?: string) {
  const frame = frameById(frameId);
  if (!frame) return status("Insert a map frame first (Insert rail → Map Frame).");
  useDialog.getState().show({
    kind: "crs",
    title: "Map Frame Coordinate System",
    initial: frame.crs || useApp.getState().mapCrs?.id,
    onPick: async (crs) => {
      // Keep what the frame shows: move its centre into the new CRS.
      try {
        const center = frame.crs ? await api.transformPoint(frame.center[0], frame.center[1], frame.crs, crs) : frame.center;
        useLayouts.getState().update(frame.id, { crs, center });
      } catch {
        useLayouts.getState().update(frame.id, { crs });
        void setFrameFromMapView(frame.id);
      }
    },
  });
}

const FORMATS = { pdf: "PDF", png: "PNG", svg: "SVG" } as const;

export async function exportLayout(format: keyof typeof FORMATS, dpi = useExportDpi.getState().dpi) {
  const doc = useLayouts.getState().doc;
  if (!doc) return;
  const name = `${doc.name}.${format}`;
  if (!frames(doc).length) {
    status("This layout has no map frame yet. Insert one (Insert rail → Map Frame) before exporting.");
    return;
  }
  const gone = frames(doc).find((f) => {
    const id = followedMap(f.layers);
    return id !== null && !mapOf(useApp.getState(), id);
  });
  if (gone) {
    status(`"${gone.name || gone.id}" follows a map that was deleted, so it would draw no layers. Choose another map in the frame's properties, then export.`);
    return;
  }
  let path: string | null;
  try {
    if (IN_TAURI) {
      path = await save({ title: `Export ${doc.name} as ${FORMATS[format]}`, defaultPath: name, filters: [{ name: FORMATS[format], extensions: [format] }] });
    } else {
      // Browser build (dev server): no native dialog; the engine writes where the dev server runs.
      path = window.prompt(`Export ${FORMATS[format]} to this path (on the machine running the dev server)`, `/tmp/${name}`);
    }
  } catch (e) {
    reportError(e, "action:Export layout", { title: "Export could not start" });
    return;
  }
  if (!path) return;
  await flushLayoutSaves();
  status(`Exporting ${name}…`);
  try {
    const warnings = await api.exportLayout(layoutRequest(doc), format, dpi, path);
    status(`Exported ${path}${warnings.length ? ` · ${warnings.length} warning${warnings.length > 1 ? "s" : ""}: ${warnings[0]}` : ""}`);
  } catch (e) {
    status(isCancelled(e) ? "Export cancelled" : `Export failed: ${errorText(e)}`);
    if (!isCancelled(e)) reportError(e, "action:Export layout", { title: `Export of ${name} failed`, context: "exporting a layout" });
  }
}

// ---------- templates (C3.1)

/** A new layout from a bundled or user template: its map frames take the map's coordinate system and view. */
/** Topo Map Builder: build the sheet with the engine, add it as a new layout and open it. Returns the engine's self-check. */
export async function buildTopoSheet(spec: TopoSpec): Promise<TopoBuild> {
  const built = await api.buildTopoLayout(spec, todayIso());
  const names = new Set(useLayouts.getState().list().map((l) => l.name));
  let name = built.layout.name;
  for (let n = 2; names.has(name); n++) name = `${built.layout.name} ${n}`;
  const layout: Layout = { ...normalizeLayout(built.layout), id: newLayoutId(), name };
  useLayouts.getState().add(layout);
  useLayouts.getState().open(layout.id);
  openLayout(layout.id);
  status(`Built "${name}" at 1:${spec.scale.toLocaleString("en-US")} on UTM zone ${built.zone}. Add contours and hillshade for this area, then check the list in the Topo Map dialog.`);
  return built;
}

export async function newLayoutFromTemplate(id: string) {
  const crs = useApp.getState().mapCrs;
  if (!crs) {
    status("Set a coordinate system for the map (Coordinates tab → Coordinate System), then create the layout from a template.");
    return;
  }
  let template: Layout;
  try {
    template = normalizeLayout(await api.getLayoutTemplate(id));
  } catch (e) {
    status(`Could not open the template: ${errorText(e)}`);
    reportError(e, "action:Layout template", { title: "The template could not be opened", context: "creating a layout from a template" });
    return;
  }
  const names = new Set(useLayouts.getState().list().map((l) => l.name));
  let n = 1;
  while (names.has(`Layout ${n}`)) n++;
  const layout: Layout = { ...JSON.parse(JSON.stringify(template)), id: newLayoutId(), name: `Layout ${n}`, series: null };
  const basemap = printBasemap(useApp.getState().basemap);
  layout.elements = layout.elements.map((e) => (e.kind === "map_frame" ? { ...e, crs: crs.id, basemap } : e));
  useLayouts.getState().add(layout);
  useLayouts.getState().open(layout.id);
  openLayout(layout.id);
  for (const f of frames(layout)) await setFrameFromMapView(f.id);
  status(`Created ${layout.name} from the template "${template.name}".`);
}

/** Keep the open layout as a user template (a name that exists is replaced after asking). */
export async function saveLayoutAsTemplate() {
  const doc = useLayouts.getState().doc;
  if (!doc) return;
  const name = window.prompt("Template name", doc.name)?.trim();
  if (!name) return;
  try {
    const existing = (await api.listLayoutTemplates()).templates.find((t) => !t.builtin && t.name.toLowerCase() === name.toLowerCase());
    if (existing && !window.confirm(`Replace your template "${existing.name}"?`)) return;
    await flushLayoutSaves();
    await api.saveLayoutTemplate(name, doc);
    status(`Saved the template "${name}". It is in New Layout → From template.`);
  } catch (e) {
    status(`The template could not be saved: ${errorText(e)}`);
    reportError(e, "action:Save template", { title: "The template could not be saved", context: "saving a layout template" });
  }
}

export async function deleteLayoutTemplate(t: { id: string; name: string }) {
  if (!window.confirm(`Delete your template "${t.name}"? Layouts made from it are not changed.`)) return;
  try {
    await api.deleteLayoutTemplate(t.id);
    status(`Deleted the template "${t.name}".`);
  } catch (e) {
    status(`The template could not be deleted: ${errorText(e)}`);
    reportError(e, "action:Delete template", { title: "The template could not be deleted", context: "deleting a layout template" });
  }
}

// ---------- map series (C3.2)

/** Export every page of the open layout's series: one multi-page PDF, or one PNG per page in a folder. */
export async function exportLayoutSeries(format: "pdf" | "png") {
  const doc = useLayouts.getState().doc;
  if (!doc) return;
  if (!doc.series) {
    status("Set up a map series first (Page tab → Map Series).");
    return;
  }
  let path: string | null;
  try {
    if (IN_TAURI) {
      path =
        format === "pdf"
          ? await save({ title: `Export ${doc.name} series as PDF`, defaultPath: `${doc.name}.pdf`, filters: [{ name: "PDF", extensions: ["pdf"] }] })
          : ((await openDialog({ directory: true, title: `Folder for the ${doc.name} series (one PNG per page)` })) as string | null);
    } else {
      path = window.prompt(`Export the series ${format === "pdf" ? "PDF to this file" : "PNG files into this folder"} (on the machine running the dev server)`, format === "pdf" ? `/tmp/${doc.name}.pdf` : `/tmp/${doc.name}-series`);
    }
  } catch (e) {
    reportError(e, "action:Export series", { title: "Export could not start" });
    return;
  }
  if (!path) return;
  await flushLayoutSaves();
  status(`Exporting the ${doc.name} series…`);
  try {
    // Every page is drawn by the engine: no page is picked here.
    const req = { ...layoutRequest(doc), seriesPage: undefined };
    const r = await api.exportLayoutSeries(req, format, useExportDpi.getState().dpi, path);
    status(`Exported ${r.pages} page${r.pages === 1 ? "" : "s"} to ${format === "pdf" ? path : path}${r.warnings.length ? ` · ${r.warnings.length} warning${r.warnings.length > 1 ? "s" : ""}: ${r.warnings[0]}` : ""}`);
  } catch (e) {
    status(isCancelled(e) ? "Series export cancelled" : `Series export failed: ${errorText(e)}`);
    if (!isCancelled(e)) reportError(e, "action:Export series", { title: `Export of the ${doc.name} series failed`, context: "exporting a map series" });
  }
}

export async function pickPicture(title: string): Promise<string | null> {
  if (IN_TAURI) {
    const picked = await openDialog({ title, multiple: false, filters: [{ name: "Pictures", extensions: ["png", "jpg", "jpeg", "svg"] }] });
    return typeof picked === "string" ? picked : null;
  }
  // Browser build (dev server): no native dialog; the path is on the machine running the dev server.
  return window.prompt(`${title}: path of a PNG, JPEG or SVG file (on the machine running the dev server)`);
}

/** Insert a picture frame and fill it from a PNG, JPEG or SVG file. */
export async function insertPicture() {
  try {
    const path = await pickPicture("Insert picture or logo");
    if (path) addPicture(await api.readPicture(path));
  } catch (e) {
    reportError(e, "action:Insert picture", { title: "The picture could not be added" });
  }
}

/** Replace the picture of an existing Picture element. */
export const choosePicture = (id: string) => choosePictureFor(id);

/** Choose a picture file and store it in the `source` field of an element (Picture, custom North Arrow). */
export async function choosePictureFor(id: string) {
  try {
    const path = await pickPicture("Choose picture or logo");
    if (path) useLayouts.getState().update(id, { source: await api.readPicture(path) });
  } catch (e) {
    reportError(e, "action:Choose picture", { title: "The picture could not be added" });
  }
}

/** Insert a small overview map of the selected frame: same CRS and centre, about 10× smaller scale, with the frame's extent outlined. */
export function insertOverview() {
  const frame = targetFrame();
  const s = useLayouts.getState();
  if (!frame || !s.doc) return status("Insert a map frame first (Insert rail → Map Frame), then its overview map.");
  const w = Math.min(60, frame.w / 3);
  const id = addMapFrame({ x: frame.x + frame.w - w - 3, y: frame.y + 3, w, h: w, crs: frame.crs, center: frame.center, scale: roundScaleUp(frame.scale * 10), indicator: { frame: frame.id }, border: { width: 0.35, color: "#000000" }, decor: { background: "#ffffff" }, sources: false });
  if (id) s.select([id]);
}

/** Apply a layout theme: the page look from the engine, and (optionally) its palette on the single-symbol layers. */
export async function applyTheme(themeId: string, recolourLayers: boolean) {
  const doc = useLayouts.getState().doc;
  if (!doc) return;
  try {
    const themes = await api.layoutThemes();
    const theme = themes.find((t) => t.id === themeId);
    if (!theme) return status(`Unknown theme ${themeId}.`);
    useLayouts.getState().applyLook(await api.applyLayoutTheme(doc, themeId));
    if (recolourLayers) {
      const app = useApp.getState();
      const next = recolour(app.order, app.layers, app.styles, theme.palette);
      for (const [id, st] of Object.entries(next)) app.setStyle(id, st);
      status(`${theme.name} theme applied to the page and ${Object.keys(next).length} layer(s) (classified and raster layers keep their colours).`);
    } else {
      status(`${theme.name} theme applied to the page. Layer colours were not changed.`);
    }
  } catch (e) {
    reportError(e, "action:Apply theme", { title: "The theme could not be applied" });
  }
}
