// Layout actions shared by the ribbon, Contents and the layout view.
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import { useDialog } from "../actions";
import { reportError } from "../errors/store";
import { api, errorText, IN_TAURI } from "../ipc/api";
import { getCenterInMapCrs, getGroundPerPixel, getMap } from "../map/mapController";
import { getDock } from "../panes/panes";
import { isCancelled } from "../progress/progress";
import { useExportDpi } from "./exportDpi";
import { useSeriesView } from "./series";
import { addMapFrame, addPicture, addSurround, printBasemap, flushLayoutSaves, newLayoutId, useLayouts } from "../store/layout";
import { useApp, type LayerStyle } from "../store/store";
import { frames, newLayoutDoc, normalizeLayout, roundScaleUp, type Layout, type MapFrame, type Orientation, type RenderRequest } from "./types";

const status = (s: string) => useApp.getState().setStatus(s);

/** What the engine needs besides the layout: styles, order and visibility from the map (a node result replaces its layer, as on screen). */
export function layoutRequest(layout: Layout): RenderRequest {
  const s = useApp.getState();
  const styles: Record<string, LayerStyle> = {};
  const visible: Record<string, boolean> = {};
  const order: string[] = [];
  for (const id of s.order) {
    const shown = s.results[id] && !s.showOriginal[id] ? s.results[id].id : id;
    order.push(shown);
    if (s.styles[id]) styles[shown] = s.styles[id];
    visible[shown] = s.visible[id] ?? true;
  }
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
    position: dock.getPanel("map") ? { referencePanel: "map", direction: "within" } : undefined,
  });
}

/**
 * The dock's active panel changed. A layout tab opens its layout; any other panel
 * (Properties, Contents, Map) leaves the layout open. Closing the layout here made
 * a click into the Properties pane drop the selection and every later edit (bug U-5).
 */
export function followActivePanel(layoutId: string | undefined) {
  if (layoutId) useLayouts.getState().open(layoutId);
}

/** A dock panel was closed: if it was the open layout's tab, no layout is open any more. */
export function layoutPanelClosed(layoutId: string | undefined) {
  if (layoutId && useLayouts.getState().id === layoutId) useLayouts.getState().open(null);
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
  status(`Created ${layout.name} (${size} ${orientation}). Insert a Map Frame from the Layout tab.`);
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

const SURROUND_NAMES = { legend: "legend", scale_bar: "scale bar", north_arrow: "north arrow", sources: "sources line" } as const;

/** Insert a legend, scale bar, north arrow or sources line for the selected (else first) map frame. */
export function insertSurround(kind: keyof typeof SURROUND_NAMES) {
  const frame = targetFrame();
  if (!frame) return status(`Insert a map frame first, then the ${SURROUND_NAMES[kind]} (Layout tab → Map Frame).`);
  addSurround(kind, frame.id);
}

export function insertMapFrame() {
  const id = addMapFrame();
  if (id) void setFrameFromMapView(id);
}

/** Centre the frame on the map view and pick the smallest standard scale that shows what the map shows. */
export async function setFrameFromMapView(frameId?: string) {
  const frame = frameById(frameId);
  const map = getMap();
  if (!frame || !map) return status("Insert a map frame first (Layout tab → Map Frame).");
  if (!frame.crs) return status("Set a coordinate system for the map frame.");
  const c = getCenterInMapCrs();
  const mpp = getGroundPerPixel();
  const size = map.getSize();
  if (!c || !mpp || !size) return status("The map view isn't ready yet.");
  try {
    // The view centre is already in the map CRS; only a frame in another CRS needs the engine.
    const center = c.crs === frame.crs ? c.center : await api.transformPoint(c.center[0], c.center[1], c.crs, frame.crs);
    const w = (size[0] * mpp) / (Math.max(frame.w, 1) / 1000);
    const h = (size[1] * mpp) / (Math.max(frame.h, 1) / 1000);
    if (!center.every(Number.isFinite)) return status("That map position can't be shown in the frame's coordinate system. Choose a different one.");
    const mapBasemap = useApp.getState().basemap;
    useLayouts.getState().update(frame.id, { center, scale: roundScaleUp(Math.max(w, h)), basemap: printBasemap(mapBasemap) });
    if (mapBasemap === "osm") status("OpenStreetMap can't be used in print layouts (its tile policy forbids bulk downloads), so the frame has no basemap. Choose a USGS basemap.");
  } catch (e) {
    status(`Could not place the frame: ${errorText(e)}`);
  }
}

export function pickFrameCrs(frameId?: string) {
  const frame = frameById(frameId);
  if (!frame) return status("Insert a map frame first (Layout tab → Map Frame).");
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
    status("This layout has no map frame yet. Insert one (Layout tab → Map Frame) before exporting.");
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
export async function newLayoutFromTemplate(id: string) {
  const crs = useApp.getState().mapCrs;
  if (!crs) {
    status("Set a coordinate system for the map (Map tab → Coordinate System), then create the layout from a template.");
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
    status("Set up a map series first (Layout tab → Map Series).");
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

async function pickPicture(title: string): Promise<string | null> {
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
export async function choosePicture(id: string) {
  try {
    const path = await pickPicture("Choose picture or logo");
    if (path) useLayouts.getState().update(id, { source: await api.readPicture(path) });
  } catch (e) {
    reportError(e, "action:Choose picture", { title: "The picture could not be added" });
  }
}
