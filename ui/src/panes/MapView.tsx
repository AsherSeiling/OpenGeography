import type { FeatureCollection, Feature } from "geojson";
import { dismissNotices, visibleNotices, type MapNoticeLine } from "./mapNotices";
import OlMap from "ol/Map";
import { defaults as defaultControls } from "ol/control/defaults";
import GeoJSON from "ol/format/GeoJSON";
import MVT from "ol/format/MVT";
import { DoubleClickZoom, DragPan } from "ol/interaction";
import TileLayer from "ol/layer/Tile";
import VectorLayer from "ol/layer/Vector";
import VectorTileLayer from "ol/layer/VectorTile";
import type BaseLayer from "ol/layer/Base";
import type { Coordinate } from "ol/coordinate";
import XYZ from "ol/source/XYZ";
import VectorSource from "ol/source/Vector";
import VectorTileSource from "ol/source/VectorTile";
import "ol/ol.css";
import { useCallback, useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { api, errorText, tileUrl, type LayerInfo } from "../ipc/api";
import { addData } from "../actions";
import type { DropDetail } from "../dnd";
import { MERCATOR_KEY, PX_PER_M, WEB_MERCATOR, carryResolution, displayCrsId, registerCrs, scaleOf, tileGridFor } from "../map/crs";
import { serialQueue } from "../map/serialQueue";
import { getActiveCrs, makeView, refreshViewCache, scaleFactorAt, setActiveCrs, setMap, type ActiveCrs } from "../map/mapController";
import { labelExtraFields, labelStyleFunction, labelTileFields } from "../map/labels";
import { layerStyleFunction, sketchStyle } from "../map/olStyle";
import { createReadout, type Readout } from "../map/readout";
import { BASEMAPS, labelField, styleFields } from "../map/style";
import { displayInfo, drawableLayerIds, ownerOf, useApp } from "../store/store";
import { formatArea, formatLength, geodesicLine, toMgrs, type LonLat } from "../nav/geodesy";
import { MeasurePanel } from "../nav/MeasurePanel";
import { NavPanel } from "../nav/NavPanel";
import { useNav } from "../nav/navStore";

/** The pointer position as the engine reports it: lon/lat and the map-CRS coordinates they came from. */
export interface PointerReadout {
  x: number;
  y: number;
  lon: number;
  lat: number;
}

/** Live view state for the status bar (kept apart so the app doesn't re-render on mouse move). */
export const useMapView = create<{
  pointer: PointerReadout | null;
  /** Map-CRS units per screen pixel. */
  resolution: number | null;
  /** True `1:N` scale at the view centre (engine scale factor), or null until known. */
  scale: number | null;
  /** PROJ's accuracy (m) of the map CRS → WGS 84 transform, when it is large enough to matter for online basemaps (R1). */
  basemapOffsetM: number | null;
  /** Key of the CRS the map is drawn in; notices and "projecting" entries for other CRSs are hidden. */
  crsKey: string | null;
  /** Layers being projected into the map CRS (id → name). */
  projecting: Record<string, string>;
  /** Plain-language notes about what the map isn't drawing, e.g. features outside the CRS's area of use. */
  notices: Record<string, string>;
  /** Warnings the user cleared (id → the text that was cleared): they stay hidden until the text changes. */
  dismissed: Record<string, string>;
}>(() => ({ pointer: null, resolution: null, scale: null, basemapOffsetM: null, crsKey: null, projecting: {}, notices: {}, dismissed: {} }));

/** A basemap datum offset above this many metres gets a warning chip (R1). */
const BASEMAP_OFFSET_WARN_M = 5;

interface IdentifyResult {
  x: number;
  y: number;
  layer: string;
  fid: number;
  attrs: [string, string | null][];
  more: number;
}

interface Drawn {
  layer: BaseLayer;
  key: string;
  /** L1: the layer's feature labels, on their own declutter layer above it. */
  labels?: { layer: BaseLayer; key: string };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const VECTOR_MAX_LEVEL = 16;
const RASTER_MAX_LEVEL = 18;

export function MapView() {
  const el = useRef<HTMLDivElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  // Datasets dragged from the Catalog pane are added to the map.
  useEffect(() => {
    const w = wrapRef.current;
    if (!w) return;
    const onDrop = (e: Event) => {
      const { payload } = (e as CustomEvent<DropDetail>).detail;
      if (payload.kind === "file") void addData([payload.id]);
    };
    w.addEventListener("gis-drop", onDrop);
    return () => w.removeEventListener("gis-drop", onDrop);
  }, []);
  const mapRef = useRef<OlMap | null>(null);
  const [active, setActive] = useState<ActiveCrs | null>(null);
  const [identify, setIdentify] = useState<IdentifyResult | null>(null);
  const [box, setBox] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const [tick, setTick] = useState(0);
  const tool = useApp((s) => s.tool);
  const showFps = useApp((s) => s.showFps);
  const mapCrs = useApp((s) => s.mapCrs);

  // ---------- map ----------
  const readoutRef = useRef<Readout | null>(null);
  const lastK = useRef<number | null>(null);
  useEffect(() => {
    const map = new OlMap({
      target: el.current!,
      controls: defaultControls({ rotate: false, attributionOptions: { collapsible: true } }),
      layers: [],
    });
    // Web Mercator until the project has a coordinate system: the same US view the map always opened on.
    mapRef.current = map;
    setMap(map);
    const ro = new ResizeObserver(() => map.updateSize());
    ro.observe(el.current!);

    // Pointer readout: pixels → map CRS (view maths), then the engine for lon/lat (R2).
    const readout = createReadout(
      (x, y) => api.transformPoint(x, y, getActiveCrs()!.id, "EPSG:4326"),
      (v) => useMapView.setState({ pointer: v ? { x: v.x, y: v.y, lon: v.result[0], lat: v.result[1] } : null }),
    );
    readoutRef.current = readout;
    map.on("pointermove", (e) => {
      if (!e.dragging && getActiveCrs()) readout.move(e.coordinate[0], e.coordinate[1]);
    });
    const leave = () => readout.leave();
    el.current!.addEventListener("mouseleave", leave);

    map.on("moveend", () => {
      void refreshScale();
    });
    return () => {
      ro.disconnect();
      el.current?.removeEventListener("mouseleave", leave);
      setMap(null);
      map.setTarget(undefined);
    };
  }, []);

  /** Refresh the engine-derived view cache and the true scale at the view centre. */
  const refreshScale = useCallback(async () => {
    const cache = await refreshViewCache();
    const crs = getActiveCrs();
    const res = mapRef.current?.getView().getResolution();
    if (!cache || !crs || !res) return;
    lastK.current = cache.k;
    useMapView.setState({ resolution: res, scale: scaleOf(res, crs.info.unitM, cache.k) });
  }, []);

  // ---------- coordinate system (M-D7) ----------
  const switching = useRef(0);
  const worldView = useApp((s) => s.worldView);
  const prevCrs = useRef<{ id: string; name: string } | null>(null);
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    const id = displayCrsId(mapCrs?.id ?? WEB_MERCATOR, worldView);
    if (getActiveCrs()?.id === id) return;
    const my = ++switching.current;
    void (async () => {
      try {
        const layers = useApp.getState().layers;
        const exts = Object.values(layers).map((l) => l.extent).filter((e): e is NonNullable<typeof e> => !!e);
        const fallback: [number, number, number, number] | undefined = exts.length
          ? [Math.min(...exts.map((e) => e[0])), Math.min(...exts.map((e) => e[1])), Math.max(...exts.map((e) => e[2])), Math.max(...exts.map((e) => e[3]))]
          : undefined;
        const info = await api.crsDisplayInfo(id, fallback);
        if (my !== switching.current) return;
        const projection = registerCrs(info);
        const next: ActiveCrs = { id, info, projection };
        const prev = getActiveCrs();
        const view = map.getView();
        const oldCenter = view.getCenter();
        const oldRes = view.getResolution();
        let center: [number, number];
        let resolution: number;
        if (info.world) {
          // World view: frame the whole Earth.
          const ex = info.grid.extent;
          center = [(ex[0] + ex[2]) / 2, (ex[1] + ex[3]) / 2];
          const size = map.getSize() ?? [800, 600];
          resolution = (ex[2] - ex[0]) / Math.min(size[0], size[1]) * 1.05;
        } else if (prev && oldCenter && oldRes) {
          // Keep the centre and the ground scale at the centre.
          center = (await api.transformPoint(oldCenter[0], oldCenter[1], prev.id, id)) as [number, number];
          const [kOld, kNew] = await Promise.all([scaleFactorAt(prev, oldCenter[0], oldCenter[1]), scaleFactorAt(next, center[0], center[1])]);
          resolution = carryResolution(oldRes, prev.info.unitM, kOld, info.unitM, kNew);
        } else {
          // First view: the contiguous US at about 1:20 million.
          center = (await api.transformPoint(-98.5, 39.5, "EPSG:4326", id)) as [number, number];
          const k = await scaleFactorAt(next, center[0], center[1]);
          resolution = (20_000_000 / PX_PER_M) * (k / info.unitM);
        }
        if (my !== switching.current) return;
        if (!center.every(Number.isFinite) || !Number.isFinite(resolution)) throw new Error("The view centre can't be shown in this coordinate system.");
        const nv = makeView(next, center, resolution);
        nv.on("change:resolution", () => {
          const r = nv.getResolution();
          const last = lastK.current;
          if (r && last) useMapView.setState({ resolution: r, scale: scaleOf(r, getActiveCrs()!.info.unitM, last) });
        });
        map.setView(nv);
        prevCrs.current = mapCrs;
        setActive(next);
        setActiveCrs(next);
        useMapView.setState({ pointer: null, crsKey: info.crsKey, basemapOffsetM: info.basemapOffsetM !== null && info.basemapOffsetM > BASEMAP_OFFSET_WARN_M ? info.basemapOffsetM : null });
        void refreshScale();
      } catch (e) {
        if (my !== switching.current) return;
        const s = useApp.getState();
        s.setStatus(`The map can't be drawn in ${mapCrs?.name ?? id}: ${errorText(e)}`);
        // Keep the previous coordinate system (a world view that cannot be made just goes back to the normal view).
        if (worldView) s.setWorldView(false);
        else if (prevCrs.current && prevCrs.current.id !== id) s.setMapCrs(prevCrs.current);
      }
    })();
  }, [mapCrs, worldView, refreshScale]);

  // ---------- basemap ----------
  const basemap = useApp((s) => s.basemap);
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !active) return;
    const bm = BASEMAPS[basemap];
    if (!bm) return;
    // OL reprojects these Web Mercator tiles to the map CRS in the client (proj4, no datum grids: R1).
    const layer = new TileLayer({
      source: new XYZ({ urls: bm.tiles, maxZoom: bm.maxzoom, attributions: bm.attribution, crossOrigin: "anonymous", transition: 0 }),
      zIndex: -1,
    });
    map.addLayer(layer);
    return () => {
      map.removeLayer(layer);
    };
  }, [basemap, active]);

  // ---------- data layers ----------
  const layers = useApp((s) => s.layers);
  const order = useApp((s) => s.order);
  const visible = useApp((s) => s.visible);
  const styles = useApp((s) => s.styles);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const selection = useApp((s) => s.selection);
  const drawn = useRef<Record<string, Drawn>>({});
  const removeDrawn = (map: OlMap, id: string) => {
    const d = drawn.current[id];
    if (!d) return;
    map.removeLayer(d.layer);
    if (d.labels) map.removeLayer(d.labels.layer);
    delete drawn.current[id];
  };
  const selRef = useRef<Record<string, Set<number>>>({});
  const displayReady = useRef<Set<string>>(new Set());
  const building = useRef<Set<string>>(new Set());

  /** Project a layer for display in the map CRS, then redraw (the engine's tiles 404 until it's built). */
  const buildFor = useCallback(async (info: LayerInfo, crs: ActiveCrs, name: string, key: string) => {
    if (building.current.has(key)) return;
    building.current.add(key);
    useMapView.setState((s) => ({ projecting: { ...s.projecting, [key]: info.name } }));
    try {
      let st = await api.buildDisplay(info.id, crs.id);
      while (!st.ready && !st.error) {
        await sleep(400);
        st = await api.displayStatus(info.id, crs.id);
        if (!st.building && !st.ready && !st.error) st = await api.buildDisplay(info.id, crs.id);
      }
      if (st.error) {
        useMapView.setState((s) => ({ notices: { ...s.notices, [key]: `${info.name} can't be drawn in ${name}: ${st.error}` } }));
      } else {
        displayReady.current.add(key);
        useMapView.setState((s) => {
          const notices = { ...s.notices };
          if (st.dropped > 0) notices[key] = `${st.dropped.toLocaleString()} feature${st.dropped === 1 ? "" : "s"} of ${info.name} ${st.dropped === 1 ? "is" : "are"} outside the area of use of ${name} and ${st.dropped === 1 ? "isn't" : "aren't"} drawn`;
          else delete notices[key];
          return { notices };
        });
      }
    } catch (e) {
      useMapView.setState((s) => ({ notices: { ...s.notices, [key]: `${info.name} can't be drawn in ${name}: ${errorText(e)}` } }));
    } finally {
      building.current.delete(key);
      useMapView.setState((s) => {
        const projecting = { ...s.projecting };
        delete projecting[key];
        return { projecting };
      });
      setTick((t) => t + 1);
    }
  }, []);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !active) return;
    const crsName = mapCrs?.name ?? "Web Mercator";
    const mercator = active.info.crsKey === MERCATOR_KEY;
    // Each Contents layer draws what it currently shows: its node-tree result or its original data.
    const shown = (id: string) => displayInfo({ layers, results, showOriginal }, id);
    const wanted = new Set(order.filter((id) => { const l = shown(id); return l && l.geomType !== "Table" && l.crs; }));
    for (const id of Object.keys(drawn.current)) {
      if (!wanted.has(id)) {
        removeDrawn(map, id);
      }
    }
    order.forEach((id, i) => {
      if (!wanted.has(id)) return;
      const info = shown(id)!;
      const style = styles[id];
      const zIndex = order.length - i; // first in Contents is on top
      const raster = info.geomType === "Raster";
      const dispKey = `${info.id}|${info.version}|${active.info.crsKey}`;
      if (!raster && !mercator && !displayReady.current.has(dispKey)) {
        void buildFor(info, active, crsName, dispKey);
        // Keep drawing what is there (if anything) until the new display table is ready.
        if (drawn.current[id]) drawn.current[id].layer.setZIndex(zIndex);
        return;
      }
      const fields = styleFields(style);
      const key = JSON.stringify([active.info.crsKey, info.id, info.version, info.geomType, fields, { ...style, label: undefined }, raster ? 0 : 1]);
      let d = drawn.current[id];
      if (!d || d.key !== key) {
        if (d) removeDrawn(map, id);
        const url = tileUrl(info, fields, active.info.crsKey);
        let layer: BaseLayer;
        if (raster) {
          layer = new TileLayer({
            source: new XYZ({ url, projection: active.projection, tileGrid: tileGridFor(active.info.grid, RASTER_MAX_LEVEL), crossOrigin: "anonymous", transition: 0 }),
            opacity: style.opacity,
          });
        } else {
          const grid = tileGridFor(active.info.grid, VECTOR_MAX_LEVEL);
          const vt = new VectorTileLayer({
            source: new VectorTileSource({ url, projection: active.projection, tileGrid: grid, format: new MVT(), transition: 0 }),
            updateWhileAnimating: true,
            updateWhileInteracting: true,
          });
          vt.setStyle(layerStyleFunction(vt, id, info.geomType, style, grid.getResolutions(), () => selRef.current[id]));
          layer = vt;
        }
        d = { layer, key };
        drawn.current[id] = d;
        map.addLayer(layer);
      }
      d.layer.setZIndex(zIndex);
      d.layer.setVisible(!!visible[id]);

      // Feature labels (L1): their own declutter layer just above the symbols of this layer.
      const lf = raster ? null : labelField(style);
      const lkey = lf ? JSON.stringify([key, style.label]) : "";
      if (d.labels && d.labels.key !== lkey) {
        map.removeLayer(d.labels.layer);
        d.labels = undefined;
      }
      if (lf && style.label && !d.labels) {
        const grid = tileGridFor(active.info.grid, VECTOR_MAX_LEVEL);
        const lt = new VectorTileLayer({
          source: new VectorTileSource({ url: tileUrl(info, labelTileFields(info.geomType, lf, !!style.label?.indexOnly, labelExtraFields(style.label)), active.info.crsKey), projection: active.projection, tileGrid: grid, format: new MVT(), transition: 0 }),
          declutter: !style.label.allowOverlap,
          updateWhileAnimating: true,
          updateWhileInteracting: true,
          style: labelStyleFunction(info.geomType, style.label, () => useMapView.getState().scale),
        });
        // Declutter layers don't repaint by themselves when their tiles arrive: repaint once the burst of loads settles.
        let settle: ReturnType<typeof setTimeout> | undefined;
        lt.getSource()?.on("tileloadend", () => {
          clearTimeout(settle);
          settle = setTimeout(() => lt.changed(), 120);
        });
        d.labels = { layer: lt, key: lkey };
        map.addLayer(lt);
      }
      d.labels?.layer.setZIndex(zIndex + 0.5);
      d.labels?.layer.setVisible(!!visible[id]);
    });
  }, [layers, results, showOriginal, order, visible, styles, active, mapCrs, tick, buildFor]);

  // Selection highlight: the style function reads the sets, so a selection change only redraws.
  useEffect(() => {
    const touched = new Set([...Object.keys(selRef.current), ...Object.keys(selection)]);
    selRef.current = Object.fromEntries(Object.entries(selection).map(([k, v]) => [k, new Set(v)]));
    for (const id of touched) (drawn.current[id]?.layer as VectorTileLayer | undefined)?.changed();
  }, [selection]);

  // ---------- measure + land-nav sketch (drawn above all layers) ----------
  const nav = useNav();
  const sketch = useRef<VectorSource | null>(null);
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !active) return;
    const source = new VectorSource();
    const layer = new VectorLayer({ source, style: sketchStyle, zIndex: 10_000, updateWhileInteracting: true });
    map.addLayer(layer);
    sketch.current = source;
    return () => {
      map.removeLayer(layer);
      sketch.current = null;
    };
  }, [active]);
  useEffect(() => {
    const source = sketch.current;
    if (!source || !active) return;
    const feats: Feature[] = [];
    const { points, measureMode, done, rays, fix, triangle, known } = nav;
    if (tool === "measure" && points.length) {
      const ring = measureMode === "area" && points.length > 2 ? [...points, points[0]] : points;
      const line: LonLat[] = [];
      for (let i = 1; i < ring.length; i++) line.push(...geodesicLine(ring[i - 1], ring[i], 48));
      if (line.length) feats.push({ type: "Feature", properties: { kind: "measure" }, geometry: { type: "LineString", coordinates: line } });
      if (measureMode === "area" && points.length > 2) feats.push({ type: "Feature", properties: { kind: "area" }, geometry: { type: "Polygon", coordinates: [line] } });
      for (const p of points) feats.push({ type: "Feature", properties: { kind: done ? "vertex-done" : "vertex" }, geometry: { type: "Point", coordinates: p } });
    }
    for (const [a, b] of rays) feats.push({ type: "Feature", properties: { kind: "ray" }, geometry: { type: "LineString", coordinates: [a, b] } });
    if (triangle.length > 2) feats.push({ type: "Feature", properties: { kind: "triangle" }, geometry: { type: "Polygon", coordinates: [[...triangle, triangle[0]]] } });
    for (const k of known) feats.push({ type: "Feature", properties: { kind: "known" }, geometry: { type: "Point", coordinates: k } });
    if (fix) feats.push({ type: "Feature", properties: { kind: "fix" }, geometry: { type: "Point", coordinates: fix } });
    const data: FeatureCollection = { type: "FeatureCollection", features: feats };
    source.clear();
    // Display only: the sketch's lon/lat vertices are placed on screen with proj4; no number comes from this.
    source.addFeatures(new GeoJSON().readFeatures(data, { dataProjection: "EPSG:4326", featureProjection: active.projection }));
  }, [nav.points, nav.done, nav.measureMode, nav.rays, nav.fix, nav.triangle, nav.known, tool, active]);

  // Esc clears the measurement.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && useApp.getState().tool === "measure" && !(e.target as HTMLElement)?.closest?.("input, textarea, select")) useNav.getState().clearMeasure();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  /** Identify click tolerance, in screen pixels (times the view resolution gives map-CRS units). */
const IDENTIFY_TOLERANCE_PX = 4;

/** A map-CRS position as engine lon/lat (every number we store or show goes through here). */
  const toLonLat = (c: Coordinate): Promise<[number, number]> => api.transformPoint(c[0], c[1], getActiveCrs()!.id, "EPSG:4326");

  // ---------- tools: identify / select / measure ----------
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !active) return;
    (map.getTargetElement() as HTMLElement).style.cursor = tool === "pan" ? "" : "crosshair";
    map.getInteractions().forEach((i) => {
      // Double-click finishes a measurement instead of zooming.
      if (i instanceof DoubleClickZoom) i.setActive(tool !== "measure");
      if (i instanceof DragPan) i.setActive(tool !== "selectBox");
    });
    // Vertices go through the engine; apply them (and the double-click's "finish") in click order.
    const queue = serialQueue();
    const onDbl = (e: { preventDefault(): void }) => {
      if (tool !== "measure") return;
      e.preventDefault();
      queue.then(() => useNav.getState().set({ done: true }));
    };
    map.on("dblclick", onDbl);
    let lastClick: { pixel: number[]; t: number } | null = null;
    const onClick = async (e: { coordinate: Coordinate; pixel: number[]; originalEvent: MouseEvent }) => {
      const s = useApp.getState();
      const nav = useNav.getState();
      const orig = e.originalEvent;
      // A land-nav panel asked for a point.
      if (nav.pick) {
        const p = await toLonLat(e.coordinate);
        nav.pick(p);
        nav.set({ pick: null });
        return;
      }
      if (tool === "measure") {
        // The two clicks of a finishing double-click land on the same spot.
        if (lastClick && Math.hypot(lastClick.pixel[0] - e.pixel[0], lastClick.pixel[1] - e.pixel[1]) < 4 && performance.now() - lastClick.t < 500) return;
        lastClick = { pixel: e.pixel, t: performance.now() };
        if (nav.done) return;
        queue.add(toLonLat(e.coordinate), (p) => useNav.getState().addPoint(p));
        return;
      }
      if (tool === "pan" || tool === "selectBox") return;
      const ids = drawableLayerIds();
      try {
        const crs = getActiveCrs()!;
        const res = map.getView().getResolution()!;
        // The click and the 4 px tolerance are in map-CRS units; the engine carries the disc into each layer's CRS.
        const hits = await api.identify(ids, e.coordinate[0], e.coordinate[1], crs.id, IDENTIFY_TOLERANCE_PX * res);
        if (tool === "select") {
          const mode = orig.shiftKey ? "add" : orig.ctrlKey || orig.metaKey ? "toggle" : "new";
          if (mode === "new") s.clearSelection();
          for (const h of hits) s.setSelection(ownerOf(h.layer), [h.fids[0]], mode === "new" ? "add" : mode);
          s.setStatus(hits.length ? `Selected ${hits.length} feature(s)` : "No features here");
        } else {
          const top = hits[0];
          if (!top) {
            // No features here: report raster values under the click instead.
            const rasters = s.order.filter((id) => s.visible[id]).map((id) => displayInfo(s, id)).filter((l) => l?.geomType === "Raster");
            const [lon, lat] = rasters.length ? await toLonLat(e.coordinate) : [0, 0];
            for (const r of rasters) {
              const v = await api.rasterValue(r!.id, lon, lat);
              if (v !== null) {
                setIdentify({ x: e.pixel[0], y: e.pixel[1], layer: r!.id, fid: -1, attrs: [["Value", v.toFixed(2)], ["Band 1 range", `${r!.raster!.min.toFixed(1)} – ${r!.raster!.max.toFixed(1)}`]], more: 0 });
                return;
              }
            }
            setIdentify(null);
            s.setStatus("No features here");
            return;
          }
          const attrs = await api.getFeature(top.layer, top.fids[0]);
          try {
            const m = await api.measureFeature(top.layer, top.fids[0]);
            if (m.area !== null) attrs.push(["Area (geodesic)", `${formatArea(m.area, "acres")} · ${formatArea(m.area, "sqkm")}`]);
            if (m.perimeter !== null) attrs.push(["Perimeter (geodesic)", `${formatLength(m.perimeter, "mi")} · ${formatLength(m.perimeter, "km")}`]);
            if (m.length !== null) attrs.push(["Length (geodesic)", `${formatLength(m.length, "mi")} · ${formatLength(m.length, "km")}`]);
            if (m.centroid) attrs.push(["Centroid (MGRS)", toMgrs(m.centroid)]);
          } catch {
            /* measurements are extra information */
          }
          s.setSelection(ownerOf(top.layer), [top.fids[0]]);
          setIdentify({ x: e.pixel[0], y: e.pixel[1], layer: top.layer, fid: top.fids[0], attrs, more: top.fids.length - 1 });
        }
      } catch (err) {
        s.setStatus(`Identify failed: ${errorText(err)}`);
      }
    };
    map.on("click", onClick as never);
    return () => {
      map.un("click", onClick as never);
      map.un("dblclick", onDbl);
    };
  }, [tool, active]);

  // Rectangle select.
  const onMouseDown = (e: React.MouseEvent) => {
    if (tool !== "selectBox" || e.button !== 0) return;
    const r = el.current!.getBoundingClientRect();
    const x0 = e.clientX - r.left;
    const y0 = e.clientY - r.top;
    setBox({ x0, y0, x1: x0, y1: y0 });
    const move = (ev: MouseEvent) => setBox({ x0, y0, x1: ev.clientX - r.left, y1: ev.clientY - r.top });
    const up = async (ev: MouseEvent) => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      setBox(null);
      const map = mapRef.current!;
      const x1 = ev.clientX - r.left;
      const y1 = ev.clientY - r.top;
      const s = useApp.getState();
      try {
        // The dragged box in map-CRS coordinates; the engine densifies it and follows its curved edges in each layer's CRS.
        const a = map.getCoordinateFromPixel([x0, y0]);
        const b = map.getCoordinateFromPixel([x1, y1]);
        const bbox: [number, number, number, number] = [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
        const hits = await api.selectRect(drawableLayerIds(), bbox, getActiveCrs()!.id);
        const mode = ev.shiftKey ? "add" : "new";
        if (mode === "new") s.clearSelection();
        let n = 0;
        for (const h of hits) {
          s.setSelection(ownerOf(h.layer), h.fids, "add");
          n += h.fids.length;
        }
        s.setStatus(`Selected ${n.toLocaleString()} feature(s)`);
      } catch (err) {
        s.setStatus(`Select failed: ${errorText(err)}`);
      }
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  return (
    <div className="map-wrap" onMouseDown={onMouseDown} data-drop="map" ref={wrapRef}>
      <div ref={el} className="map" />
      {box && (
        <div
          className="select-box"
          style={{ left: Math.min(box.x0, box.x1), top: Math.min(box.y0, box.y1), width: Math.abs(box.x1 - box.x0), height: Math.abs(box.y1 - box.y0) }}
        />
      )}
      <MapNotices />
      <ScaleBar />
      {identify && <IdentifyPopup result={identify} onClose={() => setIdentify(null)} />}
      {tool === "measure" && <MeasurePanel />}
      {nav.navPanel && <NavPanel />}
      {nav.pick && <div className="map-hint">Click the map to pick the point · Esc to cancel</div>}
      {showFps && <FpsMeter />}
    </div>
  );
}

/** "Projecting…", dropped-feature and basemap-offset messages. */
function MapNotices() {
  const projecting = useMapView((s) => s.projecting);
  const notices = useMapView((s) => s.notices);
  const offset = useMapView((s) => s.basemapOffsetM);
  const basemap = useApp((s) => s.basemap);
  const crsKey = useMapView((s) => s.crsKey);
  // Keys end in the CRS key they were made for ("rasters" has none and is always current).
  const here = (k: string) => !k.includes("|") || k.endsWith(`|${crsKey}`);
  const dismissed = useMapView((s) => s.dismissed);
  const all: MapNoticeLine[] = [
    ...Object.entries(projecting).filter(([k]) => here(k)).map(([k, n]) => ({ id: `projecting:${k}`, text: `Projecting ${n}…`, kind: "busy" as const })),
    ...Object.entries(notices).filter(([k]) => here(k)).map(([k, text]) => ({ id: `notice:${k}`, text, kind: "warn" as const })),
  ];
  if (offset !== null && basemap !== "none") all.push({ id: "basemap-offset", text: `Basemap may be offset ≈${Math.round(offset)} m in this coordinate system (datum shift)`, kind: "warn" });
  const lines = visibleNotices(all, dismissed);
  if (!lines.length) return null;
  const warnings = lines.filter((l) => l.kind === "warn");
  const clear = (which: MapNoticeLine[]) => useMapView.setState((s) => ({ dismissed: dismissNotices(s.dismissed, which) }));
  return (
    <div className="map-notices" data-testid="map-notices">
      {lines.map((l) => (
        <div key={l.id} className={`map-notice ${l.kind}`}>
          <span>{l.text}</span>
          {l.kind === "warn" && (
            <button className="map-notice-x" title="Clear this message" aria-label={`Clear: ${l.text}`} onClick={() => clear([l])}>×</button>
          )}
        </div>
      ))}
      {warnings.length > 1 && (
        <button className="map-notice-all" onClick={() => clear(warnings)}>Clear all messages</button>
      )}
    </div>
  );
}

/** Scale bar from the engine's true ground scale at the view centre. */
function ScaleBar() {
  const scale = useMapView((s) => s.scale);
  const res = useMapView((s) => s.resolution);
  if (!scale) return null;
  // Ground metres per CSS pixel at this scale, 96 dpi.
  const gpp = scale / PX_PER_M;
  void res;
  const nice = (maxM: number) => {
    const p = 10 ** Math.floor(Math.log10(maxM));
    return [5, 2, 1].map((m) => m * p).find((v) => v <= maxM) ?? p;
  };
  const metric = nice(gpp * 120);
  const feetMax = (gpp * 120) / 0.3048;
  const imperial = feetMax >= 5280 ? nice(feetMax / 5280) * 5280 : nice(feetMax);
  const label = (m: number) => (m >= 1000 ? `${m / 1000} km` : `${m} m`);
  const labelFt = (ft: number) => (ft >= 5280 ? `${ft / 5280} mi` : `${ft} ft`);
  return (
    <div className="scalebar">
      <div style={{ width: imperial * 0.3048 / gpp }}>{labelFt(imperial)}</div>
      <div style={{ width: metric / gpp }}>{label(metric)}</div>
    </div>
  );
}

function IdentifyPopup({ result, onClose }: { result: IdentifyResult; onClose: () => void }) {
  const layer = useApp((s) => s.layers[ownerOf(result.layer)]);
  return (
    <div className="identify" style={{ left: Math.min(result.x + 12, 9999), top: result.y + 12 }} onMouseDown={(e) => e.stopPropagation()}>
      <div className="identify-head">
        <span>{layer?.name ?? result.layer}</span>
        <button className="x" onClick={onClose} aria-label="Close">×</button>
      </div>
      <table>
        <tbody>
          {result.attrs.map(([k, v]) => (
            <tr key={k}>
              <th>{k}</th>
              <td>{v ?? <span className="null">&lt;Null&gt;</span>}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {result.more > 0 && <div className="identify-more">+{result.more} more feature(s) at this location</div>}
    </div>
  );
}

function FpsMeter() {
  const [fps, setFps] = useState(0);
  useEffect(() => {
    let frames = 0;
    let last = performance.now();
    let raf = 0;
    const loop = () => {
      frames++;
      const now = performance.now();
      if (now - last >= 500) {
        setFps(Math.round((frames * 1000) / (now - last)));
        frames = 0;
        last = now;
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);
  return <div className={`fps ${fps < 45 ? "low" : ""}`}>{fps} fps</div>;
}
