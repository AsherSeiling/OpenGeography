// The Layout document tab: the page on a grey canvas, an engine-rendered
// preview (the same renderer as the export), and an HTML overlay that selects,
// moves and resizes elements.
import { useLegendRows } from "./legendRows";
import { snapMove, snapTargets } from "./snap";
import { pointerMm } from "./items";
import { Button, MessageBar, MessageBarBody, Tooltip } from "@fluentui/react-components";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, errorText } from "../ipc/api";
import { beginStep, useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { layoutRequest } from "./actions";
import { dragged, withFeatureEdit, withoutDropped } from "./labelEdits";
import { sameLabel, useLabelHits } from "./labelHits";
import { Ruler, RULER_PX } from "./Rulers";
import { ContextBar, LayoutPopups, useLayoutMenus } from "./LayoutMenus";
import { PAGE_TARGET } from "./PageProperties";
import { PreflightPanel } from "./PreflightPanel";
import { Rail, MAP_DRAG } from "./Rail";
import { StartGallery } from "./StartGallery";
import { copySelected, cutSelected, duplicateSelected, pasteHeld } from "./clipboard";
import { insertMapFrame, setFrameFromMapView } from "./actions";
import { useLayoutView, zoomPercent } from "./view";
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

const LABEL: Record<string, string> = { map_frame: "Map frame", text: "Text", legend: "Legend", scale_bar: "Scale bar", north_arrow: "North arrow", declination: "Declination diagram", sources: "Sources" };

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
  const fit = useLayoutView((s) => s.fit); // View › Zoom on the ribbon and the buttons here share this
  const zoom = useLayoutView((s) => s.zoom);
  const preflightOpen = useLayoutView((s) => s.preflightOpen);
  const galleryClosed = useLayoutView((s) => !!s.galleryClosed[layout.id]);
  const [box, setBox] = useState({ w: 800, h: 600 });
  const [preview, setPreview] = useState<{ src: string; warnings: string[]; bytes: number; ms: number; labels: PlacedLabel[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rendering, setRendering] = useState(false);
  const [ptr, setPtr] = useState<{ x: number; y: number } | null>(null);
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
  const zoomTo = (v: number) => useLayoutView.getState().setZoom(v);
  // The ribbon's In and Out step from what is on screen.
  const active = useLayouts((s) => s.id === layout.id);
  useEffect(() => {
    if (active) useLayoutView.getState().setShown(z);
  }, [z, active]);

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
      useLayoutView.getState().setZoom((fit ? fitZoom : useLayoutView.getState().zoom) * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [fit, fitZoom]);

  // Preview: the engine renders the page (debounced), the newest request wins.
  const styles = useApp((s) => s.styles);
  const order = useApp((s) => s.order);
  const visible = useApp((s) => s.visible);
  const maps = useApp((s) => s.maps); // frames that follow a named map redraw when that map's layers change
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
  }, [layout, styles, order, visible, maps, results, showOriginal, layers, dpi, seriesPage]);

  // Selecting one element shows it in Properties; with nothing selected Properties shows the page.
  useEffect(() => {
    const app = useApp.getState();
    if (selected.length === 1) app.setProperties({ kind: "layoutElement", layout: layout.id, id: selected[0] });
    else if (selected.length === 0 && useLayouts.getState().id === layout.id) {
      // A label chosen on the page keeps Properties until something else is picked.
      if (!(app.properties?.kind === "layoutLabel" && app.properties.layout === layout.id)) app.setProperties({ kind: "layoutElement", layout: layout.id, id: PAGE_TARGET });
    }
    else if (app.properties?.kind === "layoutElement" && app.properties.layout === layout.id) app.setProperties(null);
  }, [selected, layout.id, active]);

  const activate = () => useLayouts.getState().open(layout.id);
  // A drag from a ruler onto the page drops a guide where it is released (CG6); dropping outside the page makes none.
  const addGuide = (axis: "x" | "y", cx: number, cy: number) => {
    const r = pageEl.current?.getBoundingClientRect();
    if (!r) return;
    const mm = Math.round(((axis === "x" ? cx - r.left : cy - r.top) / z) * 10) / 10;
    const max = axis === "x" ? layout.page.width : layout.page.height;
    if (mm < 0 || mm > max) return;
    activate();
    store().setMeta({ guides: [...(layout.guides ?? []), { axis, pos: mm }] });
  };
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
    // Snap the dragged box (all moved elements as one box) to guides, page lines and the other elements; Alt turns it off.
    const boxes = Object.values(start);
    const union = boxes.length
      ? { x: Math.min(...boxes.map((b) => b.x)), y: Math.min(...boxes.map((b) => b.y)), w: Math.max(...boxes.map((b) => b.x + b.w)) - Math.min(...boxes.map((b) => b.x)), h: Math.max(...boxes.map((b) => b.y + b.h)) - Math.min(...boxes.map((b) => b.y)) }
      : null;
    const targets = snapTargets(layout.page, layout.guides ?? [], (store().doc?.elements ?? []).filter((x) => x.visible && !(x.id in start)));
    const move = (m: PointerEvent) => {
      let dx = (m.clientX - x0) / z;
      let dy = (m.clientY - y0) / z;
      if (union && !m.altKey) ({ dx, dy } = snapMove(union, dx, dy, targets, 5 / z, handle));
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
    // Listen on the window, not the box: the boxes are removed while the preview redraws (any layout change, even one that
    // starts mid-drag), and a drag whose box vanished would otherwise be lost (E2E-5).
    const move = (m: PointerEvent) => (box.style.transform = `translate(${m.clientX - x0}px, ${m.clientY - y0}px)`);
    const done = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", done);
      box.style.transform = "";
    };
    const up = (m: PointerEvent) => {
      done();
      const frame = store().doc?.elements.find((el) => el.id === l.frame && el.kind === "map_frame") as MapFrame | undefined;
      const edit = frame && dragged(frame.labelOverrides?.[l.layer]?.features?.[String(l.fid)], (m.clientX - x0) / z, (m.clientY - y0) / z);
      if (frame && edit) store().update(frame.id, { labelOverrides: withFeatureEdit(frame.labelOverrides, l.layer, l.fid, edit) });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", done);
  };

  const openContext = (e: React.MouseEvent, target: "page" | "element" | "frame") => {
    e.preventDefault();
    e.stopPropagation();
    activate();
    wrap.current?.focus();
    const r = pageEl.current?.getBoundingClientRect();
    useLayoutMenus.getState().open({ kind: "context", x: e.clientX, y: e.clientY, target, at: r ? pointerMm(e.clientX, e.clientY, r, z, page) : null });
  };
  // A map dragged from the rail onto the page becomes a frame that draws that map, with its top-left corner where it was dropped.
  const dropMap = (e: React.DragEvent) => {
    const id = e.dataTransfer.getData(MAP_DRAG);
    if (!id) return;
    e.preventDefault();
    activate();
    const r = pageEl.current?.getBoundingClientRect();
    const at = r ? pointerMm(e.clientX, e.clientY, r, z, page) : null;
    const before = new Set(layout.elements.map((x) => x.id));
    insertMapFrame({ layers: { map: id } });
    const doc = store().doc;
    const f = doc?.elements.find((x) => x.kind === "map_frame" && !before.has(x.id));
    if (f && at) {
      const w = Math.min(f.w, page.width / 2);
      const h = Math.min(f.h, page.height / 2);
      store().update(f.id, { w, h, x: Math.max(0, Math.min(at.x, page.width - w)), y: Math.max(0, Math.min(at.y, page.height - h)) });
      for (const e2 of doc!.elements) if (e2.kind === "sources" && (e2 as { frame?: string }).frame === f.id) store().update(e2.id, { x: Math.max(0, Math.min(at.x, page.width - w)), y: Math.min(page.height - e2.h, Math.max(0, Math.min(at.y, page.height - h)) + h + 1.5), w });
      void setFrameFromMapView(f.id);
    }
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
    } else if (mod && e.key.toLowerCase() === "c") {
      e.preventDefault();
      copySelected();
    } else if (mod && e.key.toLowerCase() === "x") {
      e.preventDefault();
      cutSelected();
    } else if (mod && e.key.toLowerCase() === "v") {
      e.preventDefault();
      pasteHeld();
    } else if (mod && e.key.toLowerCase() === "d") {
      e.preventDefault();
      duplicateSelected();
    } else if ((e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) && wrap.current) {
      e.preventDefault();
      const c = wrap.current.getBoundingClientRect();
      const one = s.selected.length === 1 ? s.doc?.elements.find((x) => x.id === s.selected[0]) : undefined;
      useLayoutMenus.getState().open({ kind: "context", x: c.left + 60, y: c.top + 60, target: s.selected.length ? (one?.kind === "map_frame" ? "frame" : "element") : "page", at: null });
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
  const selEl = selected.length === 1 ? layout.elements.find((e) => e.id === selected[0]) : undefined;
  const sorted = useMemo(() => layout.elements.filter((e) => e.visible), [layout.elements]);

  return (
    <div className="layout-view">
      <div className="layout-toolbar">
        <Button size="small" appearance={fit ? "primary" : "subtle"} onClick={() => useLayoutView.getState().fitPage()}>Fit page</Button>
        <Button size="small" appearance="subtle" onClick={() => useLayoutView.getState().actualSize()}>100%</Button>
        <Button size="small" appearance="subtle" title="Zoom out" onClick={() => zoomTo(z / 1.25)}>−</Button>
        <Button size="small" appearance="subtle" title="Zoom in" onClick={() => zoomTo(z * 1.25)}>+</Button>
        <span className="muted">{zoomPercent(z)}%</span>
        <span className="layout-pointer muted" data-testid="layout-pointer" title="Pointer position on the page, mm from the top-left corner">
          {ptr ? `${ptr.x}, ${ptr.y} mm` : "— mm"}
        </span>
        {selEl && <span className="layout-pointer muted" data-testid="layout-selsize" title="Selected item: size in mm">{`${Math.round(selEl.w * 10) / 10} × ${Math.round(selEl.h * 10) / 10} mm`}</span>}
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
      <div className="layout-body">
      <Rail />
      <div className="layout-main">
      <div className="layout-stage" style={{ gridTemplateColumns: `${RULER_PX}px 1fr`, gridTemplateRows: `${RULER_PX}px 1fr` }}>
        <div className="ruler-corner" />
        <div className="ruler-box" data-testid="ruler-h"><Ruler vertical={false} length={org.w} origin={org.x} zoom={z} onGuide={(cx, cy) => addGuide("y", cx, cy)} /></div>
        <div className="ruler-box" data-testid="ruler-v"><Ruler vertical length={org.h} origin={org.y} zoom={z} onGuide={(cx, cy) => addGuide("x", cx, cy)} /></div>
      <div
        className="layout-canvas"
        ref={wrap}
        tabIndex={0}
        onKeyDown={onKey}
        onContextMenu={(e) => openContext(e, "page")}
        onDragOver={(e) => { if (e.dataTransfer.types.includes(MAP_DRAG)) { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; } }}
        onDrop={dropMap}
        onPointerMove={(e) => {
          const r = pageEl.current?.getBoundingClientRect();
          setPtr(r ? pointerMm(e.clientX, e.clientY, r, z, page) : null);
        }}
        onPointerLeave={() => setPtr(null)}
        onPointerDown={() => {
          activate();
          store().select([]);
          wrap.current?.focus();
        }}
      >
        <div className="layout-page" ref={pageEl} style={{ width: page.width * z, height: page.height * z, background: page.background }}>
          {(layout.guides ?? []).map((g, i) => (
            <div key={i} className={`layout-guide ${g.axis}`} data-guide={`${g.axis}:${g.pos}`} style={g.axis === "x" ? { left: g.pos * z } : { top: g.pos * z }} title={`Guide at ${g.pos} mm (double-click to remove)`} onDoubleClick={() => store().setMeta({ guides: (layout.guides ?? []).filter((_, j) => j !== i) })} />
          ))}
          {preview && <img className="layout-img" src={preview.src} alt="" draggable={false} />}
          {layout.elements.length === 0 && galleryClosed && (
            <div className="layout-empty" data-testid="empty-hint">
              <p>This page is empty.</p>
              <p>Add a map frame or text from the Insert rail on the left, press Shift+A to search for an element, or pick a start.</p>
              <button onClick={() => useLayoutView.getState().closeGallery(layout.id, false)}>Show the starts</button>
            </div>
          )}
          {sorted.map((el) => {
            const on = selected.includes(el.id);
            return (
              <Tooltip key={el.id} content={`${LABEL[el.kind] ?? el.kind} ${el.id}${el.locked ? " (locked)" : ""}`} relationship="description" positioning="below-start">
                <div
                  className={`layout-el ${on ? "on" : ""} ${el.locked ? "locked" : ""}`}
                  data-el={el.id}
                  style={{ left: el.x * z, top: el.y * z, width: el.w * z, height: el.h * z, transform: el.rotation ? `rotate(${el.rotation}deg)` : undefined }}
                  onPointerDown={(e) => startDrag(e, el, null)}
                  onContextMenu={(e) => {
                    const sel = store().selected;
                    const only = sel.length <= 1 || !sel.includes(el.id);
                    if (only) store().select([el.id]);
                    openContext(e, el.kind === "map_frame" && (only || sel.length === 1) ? "frame" : "element");
                  }}
                >
                  {on && !el.locked && HANDLES.map((h) => <span key={h} className={`layout-handle ${h}`} onPointerDown={(e) => startDrag(e, el, h)} />)}
                </div>
              </Tooltip>
            );
          })}
          {selEl && selected.length === 1 && (
            <ContextBar el={selEl} left={Math.max(0, selEl.x * z)} top={selEl.y * z >= 40 ? selEl.y * z - 38 : (selEl.y + selEl.h) * z + 6} />
          )}
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
      {layout.elements.length === 0 && !galleryClosed && <StartGallery layoutId={layout.id} />}
      {preflightOpen && <PreflightPanel layout={layout} />}
      </div>
      </div>
      <LayoutPopups />
    </div>
  );
}
