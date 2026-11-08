// Vector basemaps in every map CRS (UR-19). OpenLayers can't reproject vector tiles, so the style is drawn in Web Mercator on an
// off-screen map over the Web Mercator rectangle the engine names for the view, that picture is sent to the engine, and the engine warps it
// (PROJ) to the view's rectangle in the map's own CRS. From the warp on it is a raster, like the raster tile basemaps, so it is placed
// the same way and blank where the CRS has no answer. Cost: the picture is one image per view, drawn when the view settles.
import { apply as applyMapboxStyle } from "ol-mapbox-style";
import OlMap from "ol/Map";
import View from "ol/View";
import LayerGroup from "ol/layer/Group";
import { api } from "../ipc/api";

/** Largest side of the off-screen picture (pixels): a whole-world polar view is coarse rather than huge. */
export const MAX_PICTURE_PX = 2048;
/** How long to wait for the off-screen map's tiles before using what is drawn. */
export const RENDER_TIMEOUT_MS = 25_000;

/** Fetch a style and put its layers in a new group (not on any visible map). `webfonts` is an empty stylesheet: no font files come from anywhere. */
export async function loadStyleGroup(style: unknown, styleUrl: string): Promise<LayerGroup> {
  const group = new LayerGroup();
  await applyMapboxStyle(group, style as object, { styleUrl, webfonts: "data:text/css;charset=utf-8," });
  return group;
}

/** The colour of the style's first `background` layer when it is a plain colour (what the sea or the land is before anything is drawn), else null. */
export function backgroundOf(style: { layers?: { type?: string; paint?: Record<string, unknown> }[] }): string | null {
  const bg = style.layers?.find((l) => l.type === "background");
  const c = bg?.paint?.["background-color"];
  const opacity = bg?.paint?.["background-opacity"];
  if (typeof c !== "string") return null;
  return typeof opacity === "number" && opacity < 1 ? `color-mix(in srgb, ${c} ${Math.round(opacity * 100)}%, transparent)` : c;
}

/** The size of the Web Mercator picture for a rectangle, and of the output, in pixels. */
export function pictureSize(merc: [number, number, number, number], outW: number, outH: number): { mw: number; mh: number } {
  const [w, h] = [merc[2] - merc[0], merc[3] - merc[1]];
  let mw = Math.min(MAX_PICTURE_PX, Math.max(64, Math.ceil(Math.max(outW, outH) * 1.4)));
  let mh = Math.max(8, Math.round((mw * h) / w));
  if (mh > MAX_PICTURE_PX) {
    mw = Math.max(8, Math.round((mw * MAX_PICTURE_PX) / mh));
    mh = MAX_PICTURE_PX;
  }
  return { mw, mh };
}

/** A transparent canvas, for a view the CRS can't place anywhere in Web Mercator. */
export function blank(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = Math.max(1, w);
  c.height = Math.max(1, h);
  return c;
}

/** One off-screen Web Mercator map around `group`, reused for every picture (so its tiles stay cached). */
export class MercatorRenderer {
  private map: OlMap;
  private gone = false;
  private disposers: (() => void)[] = [];
  private host: HTMLDivElement;
  /** `background`: the style's background colour, painted under the layers (OpenLayers puts it on a map's element, not in its canvases). */
  constructor(group: LayerGroup, private background: string | null = null) {
    this.host = document.createElement("div");
    this.host.style.cssText = "position:fixed;left:-20000px;top:0;width:256px;height:256px;pointer-events:none;visibility:hidden";
    document.body.appendChild(this.host);
    this.map = new OlMap({ target: this.host, layers: [group], controls: [], interactions: [], pixelRatio: 1, view: new View({ projection: "EPSG:3857", center: [0, 0], resolution: 10000, constrainResolution: false }) });
  }

  /** True once the renderer has been disposed (the basemap, CRS or view changed): it draws nothing more. */
  get disposed(): boolean {
    return this.gone;
  }

  /** Draw the Web Mercator rectangle `merc` at `w` × `h` pixels and return the finished canvas; null when the renderer is (or becomes) disposed. */
  render(merc: [number, number, number, number], w: number, h: number): Promise<HTMLCanvasElement | null> {
    if (this.gone) return Promise.resolve(null);
    const map = this.map;
    this.host.style.width = `${w}px`;
    this.host.style.height = `${h}px`;
    map.updateSize();
    const view = map.getView();
    view.setCenter([(merc[0] + merc[2]) / 2, (merc[1] + merc[3]) / 2]);
    view.setResolution(Math.max((merc[2] - merc[0]) / w, (merc[3] - merc[1]) / h));
    return new Promise((resolve) => {
      let done = false;
      const finish = () => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.disposers = this.disposers.filter((f) => f !== finish); // a finished wait must not keep its canvas alive
        resolve(this.gone ? null : this.composite(w, h));
      };
      const timer = setTimeout(finish, RENDER_TIMEOUT_MS);
      map.once("rendercomplete", finish);
      this.disposers.push(finish); // disposing while a picture is in flight ends the wait
      map.renderSync();
    });
  }

  /** The layer canvases of the map drawn into one canvas (OpenLayers' own recipe for exporting a map). */
  private composite(w: number, h: number): HTMLCanvasElement {
    const out = blank(w, h);
    const ctx = out.getContext("2d")!;
    if (this.background) {
      ctx.fillStyle = this.background;
      ctx.fillRect(0, 0, w, h);
    }
    this.map.getViewport()!.querySelectorAll<HTMLCanvasElement>(".ol-layer canvas, canvas.ol-layer").forEach((canvas) => {
      if (!canvas.width) return;
      const parent = canvas.parentElement as HTMLElement | null;
      const opacity = canvas.style.opacity || parent?.style.opacity || "1";
      ctx.globalAlpha = opacity === "" ? 1 : Number(opacity);
      const t = canvas.style.transform.match(/^matrix\(([^)]*)\)$/);
      const m = t ? t[1].split(",").map(Number) : [parseFloat(canvas.style.width) / canvas.width || 1, 0, 0, parseFloat(canvas.style.height) / canvas.height || 1, 0, 0];
      ctx.setTransform(m[0], m[1], m[2], m[3], m[4], m[5]);
      ctx.drawImage(canvas, 0, 0);
    });
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = 1;
    return out;
  }

  dispose() {
    if (this.gone) return;
    this.gone = true;
    const waiting = this.disposers.splice(0);
    waiting.forEach((f) => f());
    this.map.setTarget(undefined);
    this.map.getLayers().clear();
    this.host.remove();
  }
}

/** An image for the map-CRS rectangle `extent` (`resolution` CRS units per pixel): the style drawn in Web Mercator, warped by the engine. */
/** `isCurrent` says whether the answer is still wanted (not superseded, renderer not disposed); after every wait it is checked and a stale picture is null, never drawn. */
export async function vectorPictureFor(renderer: Pick<MercatorRenderer, "render">, crsKey: string, extent: [number, number, number, number], resolution: number, pixelRatio: number, isCurrent: () => boolean = () => true): Promise<HTMLCanvasElement | null> {
  if (!isCurrent()) return null;
  let w = Math.max(1, Math.round(((extent[2] - extent[0]) / resolution) * pixelRatio));
  let h = Math.max(1, Math.round(((extent[3] - extent[1]) / resolution) * pixelRatio));
  const scale = Math.min(1, Math.sqrt(4_000_000 / (w * h)));
  w = Math.max(1, Math.round(w * scale));
  h = Math.max(1, Math.round(h * scale));
  const merc = await api.basemapMercatorExtent(crsKey, extent);
  if (!isCurrent()) return null;
  if (!merc) return blank(w, h); // nothing of this view can be placed in Web Mercator
  const { mw, mh } = pictureSize(merc, w, h);
  const picture = await renderer.render(merc, mw, mh);
  if (!picture || !isCurrent()) return null;
  const b64 = picture.toDataURL("image/png").split(",")[1];
  const warped = await api.warpBasemapPicture(b64, merc, crsKey, extent, w, h);
  if (!isCurrent()) return null;
  const img = new Image();
  img.src = `data:image/png;base64,${warped}`;
  await img.decode();
  if (!isCurrent()) return null;
  const out = blank(w, h);
  out.getContext("2d")!.drawImage(img, 0, 0);
  return out;
}
