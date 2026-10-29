// The Layout document tab: the page on a grey canvas, an engine-rendered
// preview (the same renderer as the export), and an HTML overlay that selects,
// moves and resizes elements.
import { useLegendRows } from "./legendRows";
import { Button, MessageBar, MessageBarBody, Tooltip } from "@fluentui/react-components";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "../ipc/api";
import { beginStep, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { layoutRequest } from "./actions";
import { dragged, withFeatureEdit, withoutDropped } from "./labelEdits";
import { sameLabel, useLabelHits } from "./labelHits";
import { Ruler, RULER_PX } from "./Rulers";
import { useSeriesView } from "./series";
import type { Layout, LayoutElement, MapFrame, PlacedLabel } from "./types";

const MIN_MM = 2;
const HANDLES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;
type Handle = (typeof HANDLES)[number];

const round = (v: number) => Math.round(v * 10) / 10;

/** The element's new box when `handle` is dragged by (dx, dy) mm. */
export function resized(box: { x: number; y: number; w: number; h: number }, handle: Handle, dx: number, dy: number) {
  let { x, y, w, h } = box;
  if (handle.includes("e")) w = Math.max(MIN_MM, box.w + dx);
  if (handle.includes("s")) h = Math.max(MIN_MM, box.h + dy);
  if (handle.includes("w")) {
    w = Math.max(MIN_MM, box.w - dx);
    x = box.x + box.w - w;
  }
  if (handle.includes("n")) {
    h = Math.max(MIN_MM, box.h - dy);
    y = box.y + box.h - h;
  }
  return { x: round(x), y: round(y), w: round(w), h: round(h) };
}

const LABEL: Record<string, string> = { map_frame: "Map frame", text: "Text", legend: "Legend", scale_bar: "Scale bar", north_arrow: "North arrow", sources: "Sources" };

const NONE: string[] = [];

/** Selection of a layout tab; a stable empty array for tabs that aren't the open layout (a fresh [] per call made React loop forever). */
export const selectionOf = (layoutId: string) => (s: { id: string | null; selected: string[] }) => (s.id === layoutId ? s.selected : NONE);

export function LayoutView({ layoutId }: { layoutId: string }) {
  const layout = useLayouts((s) => (s.id === layoutId ? s.doc : (s.docs[layoutId] ?? null)));
  const selected = useLayouts(selectionOf(layoutId));
  if (!layout) return <div className="empty-hint pad">This layout no longer exists.</div>;
  return <Canvas layout={layout} selected={selected} />;
}

function Canvas({ layout, selected }: { layout: Layout; selected: string[] }) {
  const wrap = useRef<HTMLDivElement>(null);
  const pageEl = useRef<HTMLDivElement>(null);
  // Where the page's corner sits in the scroll area, for the rulers (C3.0).
  const [org, setOrg] = useState({ x: 0, y: 0, w: 0, h: 0 });
  const [fit, setFit] = useState(true);
  const [zoom, setZoom] = useState(2); // screen px per mm
  const [box, setBox] = useState({ w: 800, h: 600 });
  const [preview, setPreview] = useState<{ src: string; warnings: string[]; bytes: number; ms: number; labels: PlacedLabel[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  const { page } = layout;

  // Fit the page into the canvas.
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const fitRaw = Math.min((box.w - 48) / page.width, (box.h - 48) / page.height);
  const fitZoom = Number.isFinite(fitRaw) ? Math.max(0.2, fitRaw) : 2;
  const z = fit ? fitZoom : zoom;
  const zoomTo = (v: number) => {
    setFit(false);
    setZoom(Math.min(12, Math.max(0.3, v)));
  };

  const measureOrigin = () => {
    const c = wrap.current;
    const p = pageEl.current;
    if (!c || !p) return;
    const cr = c.getBoundingClientRect();
    const pr = p.getBoundingClientRect();
    const next = { x: Math.round(pr.left - cr.left), y: Math.round(pr.top - cr.top), w: c.clientWidth, h: c.clientHeight };
    setOrg((o) => (o.x === next.x && o.y === next.y && o.w === next.w && o.h === next.h ? o : next));
  };
  useLayoutEffect(measureOrigin);
  useEffect(() => {
    const c = wrap.current;
    c?.addEventListener("scroll", measureOrigin, { passive: true });
    return () => c?.removeEventListener("scroll", measureOrigin);
  }, []);

  // Ctrl/⌘ + wheel zooms the page; plain wheel scrolls.
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      e.preventDefault();
      setFit(false);
      setZoom((cur) => Math.min(12, Math.max(0.3, (fit ? fitZoom : cur) * (e.deltaY < 0 ? 1.1 : 1 / 1.1))));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [fit, fitZoom]);

  // Preview: the engine renders the page (debounced), the newest request wins.
  const styles = useApp((s) => s.styles);
  const order = useApp((s) => s.order);
  const visible = useApp((s) => s.visible);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const layers = useApp((s) => s.layers);
  // The map series: its pages are worked out again when the series, or the index layer's data, changes (C3.2).
  const seriesPage = useSeriesView((s) => s.page);
  const indexVersion = useApp((s) => (layout.series ? `${s.layers[layout.series.indexLayer]?.version}|${s.layers[layout.series.indexLayer]?.definitionQuery}` : ""));
  useEffect(() => {
    void useSeriesView.getState().refresh(layout);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layout.series, layout.id, indexVersion]);
  const dpi = Math.min(144, Math.max(96, Math.ceil((z * 25.4 * (window.devicePixelRatio || 1)) / 24) * 24));
  const seq = useRef(0);
  useEffect(() => {
    const mine = ++seq.current;
    const t = setTimeout(async () => {
      setRendering(true);
      const t0 = performance.now();
      try {
        const res = await api.renderLayoutPreview(layoutRequest(layout), dpi);
        if (mine !== seq.current) return;
        setPreview({ src: `data:image/png;base64,${res.png}`, warnings: res.warnings, bytes: res.pngBytes, ms: Math.round(performance.now() - t0), labels: res.labels ?? [] });
        useLabelHits.getState().set(layout.id, res.labels ?? []);
        useLegendRows.getState().set(layout.id, res.legendRows ?? []);
        // Edits for features that are gone are removed (the engine already ignored them).
        for (const f of new Set((res.droppedLabelEdits ?? []).map((d) => d.frame))) {
          const el = useLayouts.getState().doc?.elements.find((e) => e.id === f && e.kind === "map_frame") as MapFrame | undefined;
          if (el && useLayouts.getState().id === layout.id) useLayouts.getState().update(f, { labelOverrides: withoutDropped(el.labelOverrides, f, res.droppedLabelEdits) });
        }
        setError(null);
      } catch (e) {
        if (mine === seq.current) setError(errorText(e));
      } finally {
        if (mine === seq.current) setRendering(false);
      }
    }, 150);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layout, styles, order, visible, results, showOriginal, layers, dpi, seriesPage]);

  // Selecting one element shows it in Properties.
  useEffect(() => {
    const app = useApp.getState();
    if (selected.length === 1) app.setProperties({ kind: "layoutElement", layout: layout.id, id: selected[0] });
    else if (app.properties?.kind === "layoutElement" && app.properties.layout === layout.id) app.setProperties(null);
  }, [selected, layout.id]);

  const activate = () => useLayouts.getState().open(layout.id);
  const store = useLayouts.getState;

  // Drag to move or resize; the whole gesture is one undo step.
  const startDrag = (e: React.PointerEvent, el: LayoutElement, handle: Handle | null) => {
    e.stopPropagation();
    activate();
    wrap.current?.focus();
    const s = store();
    if (e.shiftKey) s.select(s.selected.includes(el.id) ? s.selected.filter((x) => x !== el.id) : [...s.selected, el.id]);
    else if (!s.selected.includes(el.id)) s.select([el.id]);
    if (el.locked || e.button !== 0) return;
    const ids = handle ? [el.id] : store().selected;
    const start = Object.fromEntries((store().doc?.elements ?? []).filter((x) => ids.includes(x.id) && !x.locked).map((x) => [x.id, { x: x.x, y: x.y, w: x.w, h: x.h }]));
    const x0 = e.clientX;
    const y0 = e.clientY;
    const end = beginStep();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => {
      const dx = (m.clientX - x0) / z;
      const dy = (m.clientY - y0) / z;
      for (const [id, b] of Object.entries(start)) {
        store().update(id, handle ? resized(b, handle, dx, dy) : { x: round(b.x + dx), y: round(b.y + dy) });
      }
    };
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      end();
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  };

  // Labels: click selects one (Properties edits it), a drag moves it; the edit is saved in this layout only (L3).
  const props = useApp((s) => s.properties);
  const startLabelDrag = (e: React.PointerEvent, l: PlacedLabel) => {
    e.stopPropagation();
    activate();
    wrap.current?.focus();
    store().select([]);
    useApp.getState().setProperties({ kind: "layoutLabel", layout: layout.id, frame: l.frame, layer: l.layer, fid: l.fid });
    if (e.button !== 0) return;
    const x0 = e.clientX;
    const y0 = e.clientY;
    const box = e.currentTarget as HTMLElement;
    box.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => (box.style.transform = `translate(${m.clientX - x0}px, ${m.clientY - y0}px)`);
    const up = (m: PointerEvent) => {
      box.removeEventListener("pointermove", move);
      box.removeEventListener("pointerup", up);
      box.style.transform = "";
      const frame = store().doc?.elements.find((el) => el.id === l.frame && el.kind === "map_frame") as MapFrame | undefined;
      const edit = frame && dragged(frame.labelOverrides?.[l.layer]?.features?.[String(l.fid)], (m.clientX - x0) / z, (m.clientY - y0) / z);
      if (frame && edit) store().update(frame.id, { labelOverrides: withFeatureEdit(frame.labelOverrides, l.layer, l.fid, edit) });
    };
    box.addEventListener("pointermove", move);
    box.addEventListener("pointerup", up);
  };

  const onKey = (e: React.KeyboardEvent) => {
    const s = store();
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      e.shiftKey ? s.redo() : s.undo();
    } else if (mod && e.key.toLowerCase() === "y") {
      e.preventDefault();
      s.redo();
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      s.removeElements(s.selected.filter((id) => !s.doc?.elements.find((x) => x.id === id)?.locked));
    } else if (e.key === "Escape") {
      s.select([]);
    } else if (e.key.startsWith("Arrow") && s.selected.length) {
      e.preventDefault();
      const d = e.shiftKey ? 0.1 : 1;
      const [dx, dy] = e.key === "ArrowLeft" ? [-d, 0] : e.key === "ArrowRight" ? [d, 0] : e.key === "ArrowUp" ? [0, -d] : [0, d];
      const end = beginStep();
      for (const el of s.doc?.elements ?? []) if (s.selected.includes(el.id) && !el.locked) s.update(el.id, { x: round(el.x + dx), y: round(el.y + dy) });
      end();
    }
  };

  const warnings = preview?.warnings ?? [];
  const sorted = useMemo(() => layout.elements.filter((e) => e.visible), [layout.elements]);

  return (
    <div className="layout-view">
      <div className="layout-toolbar">
        <Button size="small" appearance={fit ? "primary" : "subtle"} onClick={() => setFit(true)}>Fit page</Button>
        <Button size="small" appearance="subtle" onClick={() => zoomTo(96 / 25.4)}>100%</Button>
        <Button size="small" appearance="subtle" onClick={() => zoomTo(z / 1.25)}>−</Button>
        <Button size="small" appearance="subtle" onClick={() => zoomTo(z * 1.25)}>+</Button>
        <span className="muted">{Math.round(((z * 25.4) / 96) * 100)}%</span>
        <span className="layout-stat muted" title="Engine-rendered preview: the export uses the same renderer">
          {rendering ? "Rendering…" : preview ? `Preview ${dpi} dpi · ${Math.round(preview.bytes / 1024)} KB · ${preview.ms} ms` : ""}
        </span>
      </div>
      {error && (
        <MessageBar intent="error" className="layout-msg">
          <MessageBarBody>{error}</MessageBarBody>
        </MessageBar>
      )}
      {warnings.length > 0 && (
        <MessageBar intent="warning" className="layout-msg">
          <MessageBarBody>{warnings.join(" · ")}</MessageBarBody>
        </MessageBar>
      )}
      <div className="layout-stage" style={{ gridTemplateColumns: `${RULER_PX}px 1fr`, gridTemplateRows: `${RULER_PX}px 1fr` }}>
        <div className="ruler-corner" />
        <div className="ruler-box" data-testid="ruler-h"><Ruler vertical={false} length={org.w} origin={org.x} zoom={z} /></div>
        <div className="ruler-box" data-testid="ruler-v"><Ruler vertical length={org.h} origin={org.y} zoom={z} /></div>
      <div
        className="layout-canvas"
        ref={wrap}
        tabIndex={0}
        onKeyDown={onKey}
        onPointerDown={() => {
          activate();
          store().select([]);
          wrap.current?.focus();
        }}
      >
        <div className="layout-page" ref={pageEl} style={{ width: page.width * z, height: page.height * z, background: page.background }}>
          {preview && <img className="layout-img" src={preview.src} alt="" draggable={false} />}
          {layout.elements.length === 0 && <div className="layout-empty">Insert a Map Frame or Text from the Layout tab.</div>}
          {sorted.map((el) => {
            const on = selected.includes(el.id);
            return (
              <Tooltip key={el.id} content={`${LABEL[el.kind] ?? el.kind} ${el.id}${el.locked ? " (locked)" : ""}`} relationship="description" positioning="above-start">
                <div
                  className={`layout-el ${on ? "on" : ""} ${el.locked ? "locked" : ""}`}
                  data-el={el.id}
                  style={{ left: el.x * z, top: el.y * z, width: el.w * z, height: el.h * z, transform: el.rotation ? `rotate(${el.rotation}deg)` : undefined }}
                  onPointerDown={(e) => startDrag(e, el, null)}
                >
                  {on && !el.locked && HANDLES.map((h) => <span key={h} className={`layout-handle ${h}`} onPointerDown={(e) => startDrag(e, el, h)} />)}
                </div>
              </Tooltip>
            );
          })}
          {!rendering &&
            (preview?.labels ?? []).map((l) => {
              const chosen = props?.kind === "layoutLabel" && sameLabel(props, l);
              const [x0, y0, x1, y1] = l.boxMm;
              return (
                <div
                  key={`${l.frame}/${l.layer}/${l.fid}`}
                  className={`layout-label ${chosen ? "on" : ""} ${l.pinned ? "pinned" : ""} ${l.hidden ? "hidden" : ""}`}
                  data-label={`${l.layer}:${l.fid}`}
                  title={l.hidden ? `Hidden label “${l.text}”` : `Label “${l.text}”: drag to move`}
                  style={{ left: x0 * z, top: y0 * z, width: (x1 - x0) * z, height: (y1 - y0) * z }}
                  onPointerDown={(e) => startLabelDrag(e, l)}
                />
              );
            })}
        </div>
      </div>
      </div>
    </div>
  );
}
