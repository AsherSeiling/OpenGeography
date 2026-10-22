import { Map as MlMap, NavigationControl, ScaleControl, setWorkerUrl, type MapMouseEvent, type VectorTileSource } from "maplibre-gl";
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url";
import "maplibre-gl/dist/maplibre-gl.css";
import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { api, errorText, tileUrl } from "../ipc/api";
import { setMap } from "../map/mapController";
import { BASEMAPS, buildLayers, layerIds, styleFields } from "../map/style";
import { displayInfo, drawableLayerIds, ownerOf, useApp } from "../store/store";

// MapLibre 6 ships its tile-decoding worker as a separate module; Vite bundles it.
setWorkerUrl(workerUrl);

/** Live view state for the status bar (kept apart so the app doesn't re-render on mouse move). */
export const useMapView = create<{ lon: number | null; lat: number | null; zoom: number; centerLat: number }>(() => ({
  lon: null,
  lat: null,
  zoom: 4,
  centerLat: 39,
}));

interface IdentifyResult {
  x: number;
  y: number;
  layer: string;
  fid: number;
  attrs: [string, string | null][];
  more: number;
}

export function MapView() {
  const el = useRef<HTMLDivElement>(null);
  const mapRef = useRef<MlMap | null>(null);
  const [ready, setReady] = useState(false);
  const [identify, setIdentify] = useState<IdentifyResult | null>(null);
  const [box, setBox] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const tool = useApp((s) => s.tool);
  const showFps = useApp((s) => s.showFps);

  // Create the map once.
  useEffect(() => {
    const map = new MlMap({
      container: el.current!,
      style: { version: 8, sources: {}, layers: [{ id: "bg", type: "background", paint: { "background-color": "#f2efe9" } }] },
      center: [-98.5, 39.5],
      zoom: 3.6,
      attributionControl: { compact: true },
      boxZoom: false,
      fadeDuration: 0,
    });
    map.addControl(new NavigationControl({ visualizePitch: false }), "top-left");
    map.addControl(new ScaleControl({ unit: "imperial", maxWidth: 120 }), "bottom-left");
    map.addControl(new ScaleControl({ unit: "metric", maxWidth: 120 }), "bottom-left");
    map.on("load", () => setReady(true));
    map.on("mousemove", (e) => useMapView.setState({ lon: e.lngLat.lng, lat: e.lngLat.lat }));
    map.on("mouseout", () => useMapView.setState({ lon: null, lat: null }));
    const onView = () => useMapView.setState({ zoom: map.getZoom(), centerLat: map.getCenter().lat });
    map.on("move", onView);
    onView();
    map.on("error", (e) => {
      const msg = errorText((e as { error?: unknown }).error ?? e);
      if (!msg.includes("basemap.nationalmap.gov") && !msg.includes("openstreetmap")) console.warn("map:", msg);
    });
    mapRef.current = map;
    setMap(map);
    const ro = new ResizeObserver(() => map.resize());
    ro.observe(el.current!);
    return () => {
      ro.disconnect();
      setMap(null);
      map.remove();
    };
  }, []);

  // Basemap.
  const basemap = useApp((s) => s.basemap);
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    if (map.getLayer("basemap")) map.removeLayer("basemap");
    if (map.getSource("basemap")) map.removeSource("basemap");
    const bm = BASEMAPS[basemap];
    if (!bm) return;
    map.addSource("basemap", { type: "raster", tiles: bm.tiles, tileSize: 256, maxzoom: bm.maxzoom, attribution: bm.attribution });
    map.addLayer({ id: "basemap", type: "raster", source: "basemap" }, map.getStyle().layers[1]?.id);
  }, [basemap, ready]);

  // Keep MapLibre sources/layers in sync with the Contents pane.
  const layers = useApp((s) => s.layers);
  const order = useApp((s) => s.order);
  const visible = useApp((s) => s.visible);
  const styles = useApp((s) => s.styles);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const applied = useRef<Record<string, string>>({});
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    // Each Contents layer draws what it currently shows: its node-tree result or its original data.
    const shown = (id: string) => displayInfo({ layers, results, showOriginal }, id);
    const wanted = new Set(order.filter((id) => { const l = shown(id); return l && l.geomType !== "Table" && l.crs; }));
    // Remove layers that are gone.
    for (const id of Object.keys(applied.current)) {
      if (!wanted.has(id)) {
        for (const lid of layerIds(id)) if (map.getLayer(lid)) map.removeLayer(lid);
        if (map.getSource(`src-${id}`)) map.removeSource(`src-${id}`);
        delete applied.current[id];
      }
    }
    // Add/update bottom-up so order matches the Contents pane.
    for (const id of [...order].reverse()) {
      if (!wanted.has(id)) continue;
      const info = shown(id)!;
      const style = styles[id];
      const url = tileUrl(info, styleFields(style));
      const src = map.getSource(`src-${id}`) as VectorTileSource | undefined;
      if (!src) {
        map.addSource(`src-${id}`, { type: "vector", tiles: [url], minzoom: 0, maxzoom: 16 });
      } else if ((src.tiles ?? [])[0] !== url) {
        src.setTiles([url]);
      }
      const key = JSON.stringify([info.geomType, style]);
      if (applied.current[id] !== key) {
        for (const lid of layerIds(id)) if (map.getLayer(lid)) map.removeLayer(lid);
        for (const spec of buildLayers(id, info.geomType, style)) map.addLayer(spec);
        applied.current[id] = key;
      }
      for (const lid of layerIds(id)) {
        if (!map.getLayer(lid)) continue;
        map.setLayoutProperty(lid, "visibility", visible[id] ? "visible" : "none");
        map.moveLayer(lid);
      }
    }
  }, [layers, results, showOriginal, order, visible, styles, ready]);

  // Selection highlight via feature-state (no tile rebuilds).
  const selection = useApp((s) => s.selection);
  const shown = useRef<Record<string, number[]>>({});
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    for (const [id, fids] of Object.entries(shown.current)) {
      if (!map.getSource(`src-${id}`)) continue;
      const keep = new Set(selection[id] ?? []);
      for (const f of fids) if (!keep.has(f)) map.setFeatureState({ source: `src-${id}`, sourceLayer: "layer", id: f }, { selected: false });
    }
    for (const [id, fids] of Object.entries(selection)) {
      if (!map.getSource(`src-${id}`)) continue;
      for (const f of fids) map.setFeatureState({ source: `src-${id}`, sourceLayer: "layer", id: f }, { selected: true });
    }
    shown.current = Object.fromEntries(Object.entries(selection).map(([k, v]) => [k, [...v]]));
  }, [selection, ready]);

  // Click: identify or select, depending on the active tool.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    map.getCanvas().style.cursor = tool === "pan" ? "" : "crosshair";
    if (tool === "selectBox") map.dragPan.disable();
    else map.dragPan.enable();
    const onClick = async (e: MapMouseEvent) => {
      const s = useApp.getState();
      if (tool === "pan" || tool === "selectBox") return;
      const ids = drawableLayerIds();
      try {
        const hits = await api.identify(ids, e.lngLat.lng, e.lngLat.lat, map.getZoom());
        if (tool === "select") {
          const mode = e.originalEvent.shiftKey ? "add" : e.originalEvent.ctrlKey || e.originalEvent.metaKey ? "toggle" : "new";
          if (mode === "new") s.clearSelection();
          for (const h of hits) s.setSelection(ownerOf(h.layer), [h.fids[0]], mode === "new" ? "add" : mode);
          s.setStatus(hits.length ? `Selected ${hits.length} feature(s)` : "No features here");
        } else {
          const top = hits[0];
          if (!top) {
            setIdentify(null);
            s.setStatus("No features here");
            return;
          }
          const attrs = await api.getFeature(top.layer, top.fids[0]);
          s.setSelection(ownerOf(top.layer), [top.fids[0]]);
          setIdentify({ x: e.point.x, y: e.point.y, layer: top.layer, fid: top.fids[0], attrs, more: top.fids.length - 1 });
        }
      } catch (err) {
        s.setStatus(`Identify failed: ${errorText(err)}`);
      }
    };
    map.on("click", onClick);
    return () => {
      map.off("click", onClick);
    };
  }, [tool]);

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
      const a = map.unproject([x0, y0]);
      const b = map.unproject([ev.clientX - r.left, ev.clientY - r.top]);
      const bbox: [number, number, number, number] = [Math.min(a.lng, b.lng), Math.min(a.lat, b.lat), Math.max(a.lng, b.lng), Math.max(a.lat, b.lat)];
      const s = useApp.getState();
      try {
        const hits = await api.selectBox(drawableLayerIds(), bbox);
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
    <div className="map-wrap" onMouseDown={onMouseDown}>
      <div ref={el} className="map" />
      {box && (
        <div
          className="select-box"
          style={{ left: Math.min(box.x0, box.x1), top: Math.min(box.y0, box.y1), width: Math.abs(box.x1 - box.x0), height: Math.abs(box.y1 - box.y0) }}
        />
      )}
      {identify && <IdentifyPopup result={identify} onClose={() => setIdentify(null)} />}
      {showFps && <FpsMeter />}
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
