// The edit overlay on a map (SHAPE_EDITING_PLAN E2): the features of the layer being edited, with their pending edits, drawn on a vector
// layer in the VIEW's projection, plus the Draw, Modify and Snap tools on the map being edited. Every coordinate is in the map's CRS and
// comes from the engine (`edit_features_in`) or goes back to it (`edit_apply`); proj4 and OpenLayers only place them on screen.
import { useEffect, useRef } from "react";
import type OlMap from "ol/Map";
import Feature from "ol/Feature";
import WKB from "ol/format/WKB";
import { Draw, Modify, Snap } from "ol/interaction";
import { createBox } from "ol/interaction/Draw";
import { Circle as CircleGeom } from "ol/geom";
import VectorLayer from "ol/layer/Vector";
import VectorSource from "ol/source/Vector";
import { Circle as CircleStyle, Fill, Stroke, Style } from "ol/style";
import type { Geometry } from "ol/geom";
import { api, errorText, type EditOp } from "../ipc/api";
import { useApp } from "../store/store";
import { mapOf } from "../store/maps";
import { getActiveCrs } from "../map/mapController";
import { useEdit } from "./editStore";

const fmt = new WKB({ hex: true });
const TOLERANCE_PX = 10;

const stroke = (color: string, w = 2) => new Stroke({ color, width: w });
const styleFor = (f: { get(k: string): unknown }) => {
  const pending = f.get("pending");
  const picked = useEdit.getState().selected.includes(f.get("fid") as number);
  const c = picked ? "#c2185b" : pending ? "#e08a00" : "#1f6fd0";
  return new Style({
    stroke: stroke(c, 2.5),
    fill: new Fill({ color: pending ? "rgba(224,138,0,0.18)" : "rgba(31,111,208,0.14)" }),
    image: new CircleStyle({ radius: picked ? 7 : 5, fill: new Fill({ color: c }), stroke: stroke("#ffffff", 1.5) }),
  });
};

/** True when a shape drawn on a geographic map spans more than half the world: it crosses the antimeridian, which isn't handled yet. */
export function crossesAntimeridian(coords: number[][], mapCrs: string): boolean {
  if (mapCrs !== "EPSG:4326") return false;
  for (let i = 1; i < coords.length; i++) if (Math.abs(coords[i][0] - coords[i - 1][0]) > 180) return true;
  return false;
}

const flat = (g: Geometry): number[][] => {
  const out: number[][] = [];
  const walk = (c: unknown): void => {
    if (Array.isArray(c) && typeof c[0] === "number") out.push(c as number[]);
    else if (Array.isArray(c)) c.forEach(walk);
  };
  walk((g as unknown as { getCoordinates(): unknown }).getCoordinates?.());
  return out;
};

export function useEditOverlay(mapRef: { current: OlMap | null }, ready: unknown, mapId: string) {
  const layerId = useEdit((s) => s.layerId);
  const editMapId = useEdit((s) => s.mapId);
  const tool = useEdit((s) => s.tool);
  const snapping = useEdit((s) => s.snapping);
  const epoch = useEdit((s) => s.epoch);
  const selected = useEdit((s) => s.selected);
  const shows = useApp((s) => !!layerId && !!mapOf(s, mapId)?.order.includes(layerId));
  const source = useRef<VectorSource | null>(null);
  /** Shapes of the map's other visible layers, for snapping to their vertices (never drawn, never edited). */
  const snapSource = useRef<VectorSource | null>(null);
  const seq = useRef(0);

  // The vector layer and the features: reloaded after every change, and when the map moves (only features in and near the view are loaded).
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready || !layerId || !shows) return;
    const src = new VectorSource();
    const layer = new VectorLayer({ source: src, style: (f) => styleFor(f), zIndex: 9000, updateWhileInteracting: true });
    map.addLayer(layer);
    source.current = src;
    const snapSrc = new VectorSource();
    snapSource.current = snapSrc;
    const load = async () => {
      const crs = getActiveCrs(mapId);
      if (!crs) return;
      const my = ++seq.current;
      try {
        const size = map.getSize();
        if (!size) return;
        const e = map.getView().calculateExtent(size);
        const [w, h] = [e[2] - e[0], e[3] - e[1]];
        const corners: [number, number][] = [[e[0] - w / 4, e[1] - h / 4], [e[2] + w / 4, e[1] - h / 4], [e[0] - w / 4, e[3] + h / 4], [e[2] + w / 4, e[3] + h / 4]];
        const ll = await Promise.all(corners.map(([x, y]) => api.transformPoint(x, y, crs.id, "EPSG:4326").catch(() => null)));
        const ok = ll.filter((p): p is [number, number] => !!p);
        const extent: [number, number, number, number] = ok.length ? [Math.min(...ok.map((p) => p[0])), Math.min(...ok.map((p) => p[1])), Math.max(...ok.map((p) => p[0])), Math.max(...ok.map((p) => p[1]))] : [-180, -85, 180, 85];
        const r = await api.editFeaturesIn(layerId, crs.id, extent);
        const st = useApp.getState();
        const doc = mapOf(st, mapId);
        const others = (doc?.order ?? []).filter((x) => x !== layerId && doc?.visible[x] && st.layers[x]?.crs && st.layers[x].geomType !== "Raster" && st.layers[x].geomType !== "Table");
        const snapHex = (await Promise.all(others.map((x) => api.snapFeaturesIn(x, crs.id, extent).catch(() => [] as string[])))).flat();
        if (my !== seq.current) return;
        snapSrc.clear();
        for (const h of snapHex) snapSrc.addFeature(new Feature({ geometry: fmt.readGeometry(h) as Geometry }));
        src.clear();
        for (const f of r.features) {
          if (!f.geometry) continue;
          const feat = new Feature({ geometry: fmt.readGeometry(f.geometry) as Geometry, fid: f.fid, pending: f.pending });
          feat.setId(f.fid);
          src.addFeature(feat);
        }
        if (r.truncated) useEdit.getState().set({ notice: "Only part of this layer is loaded for editing (the features in and near the view). Zoom in to edit the rest." });
      } catch (err) {
        useApp.getState().setStatus(errorText(err));
      }
    };
    void load();
    const onMove = () => void load();
    map.on("moveend", onMove);
    return () => {
      map.un("moveend", onMove);
      map.removeLayer(layer);
      source.current = null;
      snapSource.current = null;
    };
  }, [mapRef, ready, layerId, shows, mapId, epoch]);

  useEffect(() => {
    source.current?.changed();
  }, [selected]);

  // The tools, on the map being edited only.
  useEffect(() => {
    const map = mapRef.current;
    const src = source.current;
    if (!map || !src || !layerId || !shows || editMapId !== mapId) return;
    const crsId = () => getActiveCrs(mapId)?.id;
    const apply = async (ops: EditOp[]) => {
      const id = crsId();
      if (!id) return;
      try {
        const r = await api.editApply(layerId, id, ops);
        useEdit.getState().bump(r.status);
        if (r.invalid.length) useEdit.getState().set({ notice: `${r.invalid.length} shape(s) are not valid (they cross themselves or are degenerate). Save keeps them as drawn; use Fix on the Edit tab to repair.` });
        else useEdit.getState().set({ notice: null });
      } catch (err) {
        useEdit.getState().set({ notice: errorText(err) });
        useEdit.getState().bump();
      }
    };
    const reshape = async (op: Record<string, unknown>) => {
      const id = crsId();
      if (!id) return;
      try {
        const r = await api.editReshape(layerId, id, op);
        useEdit.getState().set({ selected: [], notice: null });
        useEdit.getState().bump(r.status);
      } catch (err) {
        useEdit.getState().set({ notice: errorText(err) });
        useEdit.getState().bump();
      }
    };
    const interactions: import("ol/interaction/Interaction").default[] = [];
    const cleanups: (() => void)[] = [];
    const el = map.getTargetElement() as HTMLElement;
    const onClick = (e: { pixel: number[] }) => {
      const f = map.forEachFeatureAtPixel(e.pixel, (x) => x as Feature, { layerFilter: (l) => (l as VectorLayer<VectorSource>).getSource() === src, hitTolerance: 6 });
      if (f) void apply([{ op: "delete", fid: f.get("fid") as number }]);
    };
    const drawn = (g: Geometry): boolean => {
      const id = crsId() ?? "";
      if (crossesAntimeridian(flat(g), id)) {
        useEdit.getState().set({ notice: "That shape crosses the antimeridian (180° longitude). Shapes crossing it aren't handled in this version; draw it in two parts." });
        return false;
      }
      return true;
    };
    if (tool === "modify") {
      const modify = new Modify({ source: src, pixelTolerance: TOLERANCE_PX });
      modify.on("modifyend", (e) => {
        const ops: EditOp[] = [];
        for (const f of e.features.getArray()) {
          const g = f.getGeometry();
          if (!g || !drawn(g)) continue;
          ops.push({ op: "update", fid: f.get("fid") as number, geometry: fmt.writeGeometry(g) as string });
        }
        if (ops.length) void apply(ops);
        else useEdit.getState().bump();
      });
      interactions.push(modify);
      el.style.cursor = "";
    } else if (tool === "delete") {
      map.on("click", onClick);
      el.style.cursor = "crosshair";
    } else if (tool === "select") {
      const pick = (e: { pixel: number[]; originalEvent: MouseEvent }) => {
        const f = map.forEachFeatureAtPixel(e.pixel, (x) => x as Feature, { layerFilter: (l) => (l as VectorLayer<VectorSource>).getSource() === src, hitTolerance: 6 });
        const cur = useEdit.getState().selected;
        const fid = f?.get("fid") as number | undefined;
        if (fid === undefined) return useEdit.getState().set({ selected: e.originalEvent.shiftKey ? cur : [] });
        useEdit.getState().set({ selected: e.originalEvent.shiftKey ? (cur.includes(fid) ? cur.filter((x) => x !== fid) : [...cur, fid]) : [fid] });
      };
      map.on("click", pick as never);
      el.style.cursor = "pointer";
      cleanups.push(() => map.un("click", pick as never));
    } else if (tool === "split" || tool === "hole") {
      const draw = new Draw({ type: tool === "split" ? "LineString" : "Polygon" });
      draw.on("drawend", (e) => {
        const g = e.feature.getGeometry();
        const sel = useEdit.getState().selected;
        if (!g) return;
        if (sel.length !== 1) return void useEdit.getState().set({ notice: tool === "split" ? "Select the one polygon to split first (Select tool), then draw the cutting line across it." : "Select the one polygon to cut a hole in first (Select tool), then draw the hole inside it." });
        void reshape(tool === "split" ? { kind: "split", fid: sel[0], line: fmt.writeGeometry(g) as string } : { kind: "cut_hole", fid: sel[0], hole: fmt.writeGeometry(g) as string });
      });
      interactions.push(draw);
      el.style.cursor = "crosshair";
    } else if (tool === "rect") {
      const draw = new Draw({ type: "Circle", geometryFunction: createBox() });
      draw.on("drawend", (e) => {
        const ex = e.feature.getGeometry()?.getExtent();
        if (!ex) return;
        void (async () => {
          try {
            const r = await api.editRectangle([ex[0], ex[1]], [ex[2], ex[3]]);
            await apply([{ op: "insert", geometry: r.geometry }]);
          } catch (err) {
            useEdit.getState().set({ notice: errorText(err) });
          }
        })();
      });
      interactions.push(draw);
      el.style.cursor = "crosshair";
    } else if (tool === "circle") {
      const draw = new Draw({ type: "Circle" });
      draw.on("drawend", (e) => {
        const g = e.feature.getGeometry() as CircleGeom | null;
        const id = crsId();
        if (!g || !id) return;
        const [cx, cy] = g.getCenter();
        const r = g.getRadius();
        void (async () => {
          try {
            // The radius is ground metres on the ellipsoid, measured by the engine, and the circle is drawn by it with 48 vertices.
            const metres = await api.editDistance(id, [cx, cy], [cx + r, cy]);
            const c = await api.editCircle(id, [cx, cy], metres, 48);
            await apply([{ op: "insert", geometry: c.geometry }]);
            useEdit.getState().set({ notice: c.note });
          } catch (err) {
            useEdit.getState().set({ notice: errorText(err) });
          }
        })();
      });
      interactions.push(draw);
      el.style.cursor = "crosshair";
    } else {
      const type = tool === "point" ? "Point" : tool === "line" ? "LineString" : "Polygon";
      const draw = new Draw({ type });
      draw.on("drawend", (e) => {
        const g = e.feature.getGeometry();
        if (g && drawn(g)) void apply([{ op: "insert", geometry: fmt.writeGeometry(g) as string }]);
      });
      interactions.push(draw);
      el.style.cursor = "crosshair";
    }
    if (snapping) {
      interactions.push(new Snap({ source: src, pixelTolerance: TOLERANCE_PX }));
      if (snapSource.current) interactions.push(new Snap({ source: snapSource.current, pixelTolerance: TOLERANCE_PX }));
    }
    interactions.forEach((i) => map.addInteraction(i));
    return () => {
      interactions.forEach((i) => map.removeInteraction(i));
      cleanups.forEach((f) => f());
      map.un("click", onClick);
      el.style.cursor = "";
    };
  }, [mapRef, ready, layerId, shows, editMapId, mapId, tool, snapping, epoch]);
}
