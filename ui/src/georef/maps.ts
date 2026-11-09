// The two OpenLayers maps of the Georeference window: the image in its own pixel space, and a reference map in the target CRS.
// proj4 (inside OL) only turns screen pixels into map coordinates; every figure shown comes from the engine (M-D5).
import OlMap from "ol/Map";
import View from "ol/View";
import Feature from "ol/Feature";
import Point from "ol/geom/Point";
import ImageLayer from "ol/layer/Image";
import ImageSource from "ol/source/Image";
import StaticImage from "ol/source/ImageStatic";
import VectorLayer from "ol/layer/Vector";
import VectorTileLayer from "ol/layer/VectorTile";
import TileLayer from "ol/layer/Tile";
import VectorSource from "ol/source/Vector";
import VectorTileSource from "ol/source/VectorTile";
import XYZ from "ol/source/XYZ";
import MVT from "ol/format/MVT";
import Projection from "ol/proj/Projection";
import Translate from "ol/interaction/Translate";
import { defaults as defaultInteractions } from "ol/interaction/defaults";
import { Circle as CircleStyle, Fill, Stroke, Style, Text } from "ol/style";
import type { Coordinate } from "ol/coordinate";
import { api, naturalEarthTileUrl, tileUrl, type CrsDisplayInfo } from "../ipc/api";
import { MERCATOR_KEY, registerCrs, tileGridFor } from "../map/crs";
import { layerStyleFunction } from "../map/olStyle";
import { displayInfo, effectiveStyle, useApp } from "../store/store";
import type { ControlPoint } from "./types";

export interface PointMarker {
  id: number | "pending";
  enabled: boolean;
  at: Coordinate;
  selected: boolean;
}

const COLORS = { on: "#0f6cbd", off: "#8a8886", pending: "#d83b01" };

function markerStyle(f: Feature): Style {
  const id = f.get("pid") as number | "pending";
  const enabled = f.get("enabled") as boolean;
  const sel = f.get("selected") as boolean;
  const color = id === "pending" ? COLORS.pending : enabled ? COLORS.on : COLORS.off;
  return new Style({
    image: new CircleStyle({ radius: sel ? 8 : 6, fill: new Fill({ color: "rgba(255,255,255,0.35)" }), stroke: new Stroke({ color, width: sel ? 3 : 2 }) }),
    text: id === "pending" ? undefined : new Text({ text: String(id), offsetX: 11, offsetY: -9, font: "600 12px sans-serif", fill: new Fill({ color }), stroke: new Stroke({ color: "#fff", width: 3 }) }),
  });
}

function markerLayer(): { source: VectorSource; layer: VectorLayer<VectorSource> } {
  const source = new VectorSource();
  return { source, layer: new VectorLayer({ source, style: (f) => markerStyle(f as Feature), zIndex: 1000, updateWhileAnimating: true, updateWhileInteracting: true }) };
}

/** Redraw a map's markers from the control points; `coordOf` gives each point's position in that map's coordinates. */
export function setMarkers(source: VectorSource, markers: PointMarker[]) {
  source.clear();
  for (const m of markers) {
    const f = new Feature({ geometry: new Point(m.at), pid: m.id, enabled: m.enabled, selected: m.selected });
    source.addFeature(f);
  }
}

export interface MapHandle {
  map: OlMap;
  markers: VectorSource;
  dispose: () => void;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("The picture could not be drawn."));
    img.src = src;
  });
}

interface Handlers {
  /** A click on the map, in the map's own coordinates. */
  click: (c: Coordinate) => void;
  /** A marker dragged to a new place. */
  moved: (id: number, c: Coordinate) => void;
  select: (id: number) => void;
}

function wire(map: OlMap, markers: { source: VectorSource; layer: VectorLayer<VectorSource> }, h: Handlers) {
  const translate = new Translate({ layers: [markers.layer], hitTolerance: 6 });
  translate.on("translateend", (e) => {
    const f = e.features.item(0);
    const id = f?.get("pid");
    const g = f?.getGeometry() as Point | undefined;
    if (typeof id === "number" && g) h.moved(id, g.getCoordinates());
  });
  map.addInteraction(translate);
  map.on("singleclick", (e) => {
    const hit = map.forEachFeatureAtPixel(e.pixel, (f) => f, { layerFilter: (l) => l === markers.layer, hitTolerance: 6 });
    const id = hit?.get("pid");
    if (typeof id === "number") return h.select(id);
    h.click(e.coordinate);
  });
}

/** The image in its own pixel space: x = pixel, y = -line, so OpenLayers' y-up map shows the image upright. */
export function createImageMap(el: HTMLElement, path: string, width: number, height: number, h: Handlers): MapHandle {
  const projection = new Projection({ code: "georef-image", units: "pixels", extent: [0, -height, width, 0] });
  const source = new ImageSource({
    projection,
    interpolate: false,
    loader: async (extent, resolution, pixelRatio) => {
      const [minx, miny, maxx, maxy] = extent;
      const outW = Math.max(1, Math.round(((maxx - minx) / resolution) * pixelRatio));
      const outH = Math.max(1, Math.round(((maxy - miny) / resolution) * pixelRatio));
      const canvas = document.createElement("canvas");
      canvas.width = outW;
      canvas.height = outH;
      // The part of the request that is inside the image, on whole image pixels; the rest of the canvas stays transparent.
      const cx0 = Math.max(0, Math.floor(minx));
      const cx1 = Math.min(width, Math.ceil(maxx));
      const cy0 = Math.max(0, Math.floor(-maxy));
      const cy1 = Math.min(height, Math.ceil(-miny));
      if (cx1 <= cx0 || cy1 <= cy0) return canvas;
      const sx = outW / (maxx - minx);
      const sy = outH / (maxy - miny);
      const rw = cx1 - cx0;
      const rh = cy1 - cy0;
      const png = await api.georefImageRegion(path, cx0, cy0, rw, rh, Math.max(1, Math.round(rw * sx)), Math.max(1, Math.round(rh * sy)));
      const img = await loadImage(`data:image/png;base64,${png}`);
      const ctx = canvas.getContext("2d")!;
      ctx.imageSmoothingEnabled = sx < 1;
      ctx.drawImage(img, (cx0 - minx) * sx, (maxy + cy0) * sy, rw * sx, rh * sy);
      return canvas;
    },
  });
  const markers = markerLayer();
  const view = new View({ projection, center: [width / 2, -height / 2], resolution: Math.max(width / Math.max(el.clientWidth, 200), height / Math.max(el.clientHeight, 200)) * 1.05, minResolution: 1 / 32, maxResolution: Math.max(width, height) });
  const map = new OlMap({ target: el, layers: [new ImageLayer({ source }), markers.layer], view, controls: [], interactions: defaultInteractions({ doubleClickZoom: false }) });
  wire(map, markers, h);
  return { map, markers: markers.source, dispose: () => map.setTarget(undefined) };
}

export interface ReferenceMap extends MapHandle {
  /** Show (or with null, remove) the preview picture of the warped image. */
  setPreview: (p: { png: string; extent: [number, number, number, number]; opacity: number } | null) => void;
  setPreviewOpacity: (o: number) => void;
  /** Fit the view to [west, south, east, north] given in this CRS's units. */
  fit: (extent: [number, number, number, number]) => void;
  crs: CrsDisplayInfo;
}

/** The reference map in the target CRS: the bundled world basemap, then the project's visible layers. The engine draws each in this CRS. */
export function createReferenceMap(el: HTMLElement, crsId: string, info: CrsDisplayInfo, center: [number, number] | null, resolution: number | null, h: Handlers): ReferenceMap {
  const projection = registerCrs(info);
  const grid = info.grid;
  const maxLevel = (n: number) => Math.min(n, grid.resolutions.length - 1);
  const base = new TileLayer({ source: new XYZ({ url: naturalEarthTileUrl(info.crsKey), projection, tileGrid: tileGridFor(grid, maxLevel(6)), crossOrigin: "anonymous", transition: 0 }), zIndex: -1 });
  const markers = markerLayer();
  const ext = grid.extent;
  const mid: [number, number] = center ?? [(ext[0] + ext[2]) / 2, (ext[1] + ext[3]) / 2];
  const res = grid.resolutions;
  const view = new View({ projection, center: mid, resolution: resolution ?? res[Math.min(3, res.length - 1)], maxResolution: res[0], minResolution: res[res.length - 1] / 2, constrainResolution: false });
  const map = new OlMap({ target: el, layers: [base, markers.layer], view, controls: [], interactions: defaultInteractions({ doubleClickZoom: false }) });
  wire(map, markers, h);

  let disposed = false;
  const s = useApp.getState();
  const mercator = info.crsKey === MERCATOR_KEY;
  const order = s.order.filter((id) => s.visible[id]);
  order.forEach((id, i) => {
    const shown = displayInfo(s, id);
    const style = effectiveStyle(s, id);
    if (!shown || !style || shown.geomType === "Table" || !shown.crs) return;
    const z = order.length - i;
    const add = () => {
      if (disposed) return;
      if (shown.geomType === "Raster") {
        const url = tileUrl(shown, [], info.crsKey);
        map.addLayer(new TileLayer({ source: new XYZ({ url, projection, tileGrid: tileGridFor(grid, maxLevel(18)), crossOrigin: "anonymous", transition: 0 }), opacity: style.opacity, zIndex: z }));
      } else {
        const g = tileGridFor(grid, maxLevel(16));
        const vt = new VectorTileLayer({ source: new VectorTileSource({ url: tileUrl(shown, [], info.crsKey), projection, tileGrid: g, format: new MVT(), transition: 0 }), zIndex: z, updateWhileAnimating: true, updateWhileInteracting: true });
        vt.setStyle(layerStyleFunction(vt, id, shown.geomType, { ...style, label: undefined }, g.getResolutions(), () => undefined));
        map.addLayer(vt);
      }
    };
    if (shown.geomType === "Raster" || mercator) return add();
    // Vector layers are projected into this CRS first (the engine's tiles answer 404 until then).
    void (async () => {
      try {
        let st = await api.buildDisplay(shown.id, crsId);
        for (let n = 0; !st.ready && !st.error && n < 150 && !disposed; n++) {
          await new Promise((r) => setTimeout(r, 400));
          st = await api.displayStatus(shown.id, crsId);
        }
        if (st.ready) add();
      } catch {
        /* a layer that can't be drawn here is left out of the reference map */
      }
    })();
  });

  let previewLayer: ImageLayer<StaticImage> | null = null;
  return {
    map,
    markers: markers.source,
    crs: info,
    dispose: () => {
      disposed = true;
      map.setTarget(undefined);
    },
    setPreview: (p) => {
      if (previewLayer) {
        map.removeLayer(previewLayer);
        previewLayer = null;
      }
      if (!p) return;
      previewLayer = new ImageLayer({ source: new StaticImage({ url: `data:image/png;base64,${p.png}`, imageExtent: p.extent, projection, interpolate: true }), opacity: p.opacity, zIndex: 500 });
      map.addLayer(previewLayer);
    },
    setPreviewOpacity: (o) => previewLayer?.setOpacity(o),
    fit: (e) => view.fit(e, { size: map.getSize() ?? undefined, maxZoom: 22, padding: [20, 20, 20, 20] }),
  };
}

/** A control point as it sits on the image map. */
export function imageMarkers(points: ControlPoint[], selected: number | null, pending: { pixel?: number; line?: number }): PointMarker[] {
  const out: PointMarker[] = points.map((p) => ({ id: p.id, enabled: p.enabled, at: [p.pixel, -p.line], selected: p.id === selected }));
  if (pending.pixel != null && pending.line != null) out.push({ id: "pending", enabled: true, at: [pending.pixel, -pending.line], selected: false });
  return out;
}

/** A control point as it sits on the reference map (its target coordinates are already in the map's CRS). */
export function groundMarkers(points: ControlPoint[], selected: number | null, pending: { x?: number; y?: number }): PointMarker[] {
  const out: PointMarker[] = points.map((p) => ({ id: p.id, enabled: p.enabled, at: [p.x, p.y], selected: p.id === selected }));
  if (pending.x != null && pending.y != null) out.push({ id: "pending", enabled: true, at: [pending.x, pending.y], selected: false });
  return out;
}
