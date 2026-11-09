// Georeference (GR1, GR2): the image beside a reference map; pick matching points on both (or type their coordinates), see each
// point's error and the overall RMS from the same transform the warp uses, preview the result on the map, then write a new GeoTIFF.
// The image file is only ever read.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Slider, Tooltip } from "@fluentui/react-components";
import { DeleteRegular, DismissRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useRef, useState } from "react";
import { CrsPicker } from "../dialogs/Dialogs";
import { api, errorText, type CrsDisplayInfo } from "../ipc/api";
import { open as openFile, save as saveFile } from "../ipc/dialogs";
import { getCenterLonLat } from "../map/mapController";
import { useApp } from "../store/store";
import { createImageMap, createReferenceMap, groundMarkers, imageMarkers, setMarkers, type MapHandle, type ReferenceMap } from "./maps";
import { errorOf, fitKey, fmtErr, parseCell, pointsShort, removePoint, summaryLine, updatePoint, useGeoref } from "./store";
import { METHODS, newControlSet, nextPointId, type ControlPoint, type ControlSet, type ErrorReport, type ParsedControlFile, type Resampling } from "./types";

const IMAGE_FILTERS = [
  { name: "Images and rasters", extensions: ["png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp", "jp2", "img", "pdf"] },
  { name: "All files", extensions: ["*"] },
];
const RESAMPLING: { id: Resampling; label: string }[] = [
  { id: "nearest", label: "Nearest neighbour (keeps the original values)" },
  { id: "bilinear", label: "Bilinear" },
  { id: "cubic", label: "Cubic" },
  { id: "cubicspline", label: "Cubic spline" },
  { id: "lanczos", label: "Lanczos" },
];
const setStatus = (s: string) => useApp.getState().setStatus(s);

export function GeoreferenceWindow() {
  const open = useGeoref((s) => s.open);
  return open ? <GeorefBody /> : null;
}

/** A table cell holding a number: edited as text, applied when it is a number, otherwise put back. */
function NumCell({ value, onCommit, width = 92 }: { value: number; onCommit: (v: number) => void; width?: number }) {
  const [text, setText] = useState<string | null>(null);
  const shown = text ?? String(value);
  const done = () => {
    if (text != null) {
      const n = parseCell(text);
      if (n != null && n !== value) onCommit(n);
    }
    setText(null);
  };
  return (
    <input
      className="gr-num"
      style={{ width }}
      value={shown}
      inputMode="decimal"
      onFocus={(e) => e.target.select()}
      onChange={(e) => setText(e.target.value)}
      onBlur={done}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setText(null);
      }}
    />
  );
}

function GeorefBody() {
  const g = useGeoref();
  const layers = useApp((s) => s.layers);
  const [sets, setSets] = useState<ControlSet[]>([]);
  const [report, setReport] = useState<ErrorReport | null>(null);
  const [reportMsg, setReportMsg] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ intent: "error" | "info" | "success"; text: string } | null>(null);
  const [previewOn, setPreviewOn] = useState(true);
  const [opacity, setOpacity] = useState(0.7);
  const [previewing, setPreviewing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [outName, setOutName] = useState("");
  const [crsDialog, setCrsDialog] = useState<null | { next?: { id: string; name: string } }>(null);
  const [crsChoice, setCrsChoice] = useState<null | { id: string; name: string }>(null);
  const [importing, setImporting] = useState<null | { parsed: ParsedControlFile; crs: string | null }>(null);
  const imageEl = useRef<HTMLDivElement>(null);
  const refEl = useRef<HTMLDivElement>(null);
  const imageMap = useRef<MapHandle | null>(null);
  const refMap = useRef<ReferenceMap | null>(null);
  const [refReady, setRefReady] = useState(0);

  const method = METHODS.find((m) => m.id === g.methodId)!;
  const cs = g.set;
  const engineSet = useMemo(() => ({ ...cs, name: cs.name.trim() || "Control points" }), [cs]);
  const key = fitKey(engineSet, g.methodId, g.resampling, g.image?.path ?? null);
  const hasCrs = cs.targetCrs.trim() !== "";

  const refreshSets = () => api.listControlSets().then(setSets, (e) => setNotice({ intent: "error", text: errorText(e) }));
  useEffect(() => {
    void refreshSets();
    setOutName("");
  }, []);

  // ---------- image pane ----------
  useEffect(() => {
    imageMap.current?.dispose();
    imageMap.current = null;
    const el = imageEl.current;
    if (!el || !g.image) return;
    const { path, info } = g.image;
    const h = createImageMap(el, path, info.width, info.height, {
      click: ([x, y]) => {
        const line = -y;
        if (x < 0 || x > info.width || line < 0 || line > info.height) return;
        const st = useGeoref.getState();
        st.setPending({ ...st.pending, pixel: x, line });
        st.commitPending();
      },
      moved: (id, [x, y]) => useGeoref.getState().edit((s) => updatePoint(s, id, { pixel: x, line: -y })),
      select: (id) => useGeoref.getState().select(id),
    });
    imageMap.current = h;
    return () => {
      h.dispose();
      imageMap.current = null;
    };
  }, [g.image?.path]);

  // ---------- reference map (in the target CRS) ----------
  useEffect(() => {
    refMap.current?.dispose();
    refMap.current = null;
    setRefReady(0);
    const el = refEl.current;
    if (!el || !hasCrs) return;
    let live = true;
    (async () => {
      try {
        const info: CrsDisplayInfo = await api.crsDisplayInfo(cs.targetCrs);
        if (!live || !refEl.current) return;
        const pts = useGeoref.getState().set.points;
        let center: [number, number] | null = null;
        if (pts.length === 0) {
          const ll = getCenterLonLat();
          if (ll) {
            try {
              center = await api.transformPoint(ll[0], ll[1], "EPSG:4326", cs.targetCrs);
            } catch {
              center = null;
            }
          }
          if (!live || !refEl.current) return;
        }
        const handle = createReferenceMap(refEl.current, cs.targetCrs, info, center, null, {
          click: ([x, y]) => {
            const st = useGeoref.getState();
            st.setPending({ ...st.pending, x, y });
            st.commitPending();
          },
          moved: (id, [x, y]) => useGeoref.getState().edit((s) => updatePoint(s, id, { x, y })),
          select: (id) => useGeoref.getState().select(id),
        });
        refMap.current = handle;
        if (pts.length >= 2) {
          const xs = pts.map((p) => p.x);
          const ys = pts.map((p) => p.y);
          const padX = (Math.max(...xs) - Math.min(...xs)) * 0.25 || 1;
          const padY = (Math.max(...ys) - Math.min(...ys)) * 0.25 || 1;
          handle.fit([Math.min(...xs) - padX, Math.min(...ys) - padY, Math.max(...xs) + padX, Math.max(...ys) + padY]);
        } else if (pts.length === 0 && !center) handle.fit(info.grid.extent);
        setRefReady((n) => n + 1);
      } catch (e) {
        if (live) setNotice({ intent: "error", text: `The reference map can't be drawn in this coordinate system: ${errorText(e)}` });
      }
    })();
    return () => {
      live = false;
      refMap.current?.dispose();
      refMap.current = null;
    };
  }, [cs.targetCrs]);

  // ---------- markers on both maps ----------
  useEffect(() => {
    if (imageMap.current) setMarkers(imageMap.current.markers, imageMarkers(cs.points, g.selected, g.pending));
    if (refMap.current) setMarkers(refMap.current.markers, groundMarkers(cs.points, g.selected, g.pending));
  }, [cs.points, g.selected, g.pending, refReady, g.image?.path]);

  // ---------- errors (live) ----------
  useEffect(() => {
    if (!hasCrs) {
      setReport(null);
      setReportMsg("Choose the coordinate system the control points are in.");
      return;
    }
    const short = pointsShort(engineSet, g.methodId);
    let live = true;
    const t = setTimeout(() => {
      api.georefErrors(engineSet, method.method).then(
        (r) => live && (setReport(r), setReportMsg(null)),
        (e) => {
          if (!live) return;
          setReport(null);
          setReportMsg(short > 0 ? `Add ${short} more point${short === 1 ? "" : "s"} for a ${method.label.split(",")[0].toLowerCase()}.` : errorText(e));
        },
      );
    }, 120);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [key]);

  // ---------- preview (debounced; the engine cancels an older one when a newer starts) ----------
  const seq = useRef(0);
  useEffect(() => {
    const h = refMap.current;
    if (!previewOn || !g.image || !report || !h) {
      h?.setPreview(null);
      setPreviewing(false);
      return;
    }
    const mine = ++seq.current;
    setPreviewing(true);
    const t = setTimeout(() => {
      api.georefPreview(g.image!.path, engineSet, method.method, g.resampling, 512).then(
        (p) => {
          if (mine !== seq.current) return;
          setPreviewing(false);
          refMap.current?.setPreview({ png: p.png, extent: p.extent, opacity });
        },
        (e) => {
          if (mine !== seq.current) return;
          setPreviewing(false);
          if (!/^Cancelled/.test(errorText(e))) setNotice({ intent: "error", text: `Preview: ${errorText(e)}` });
        },
      );
    }, 450);
    return () => clearTimeout(t);
  }, [key, previewOn, report === null, refReady]);
  useEffect(() => refMap.current?.setPreviewOpacity(opacity), [opacity]);

  // ---------- actions ----------
  const pickImage = async () => {
    const p = await openFile({ title: "Image to georeference", filters: IMAGE_FILTERS });
    if (typeof p !== "string") return;
    await loadImage(p, null, p.split(/[\\/]/).pop() ?? p);
  };
  const loadImage = async (path: string, sourceLayer: string | null, label: string) => {
    try {
      const info = await api.georefImageInfo(path);
      g.setImage({ path, info, sourceLayer, label });
      setOutName(`${label.replace(/\.[^.]+$/, "")} (georeferenced)`);
      setNotice(info.hasGeoref ? { intent: "info", text: "This image already has georeferencing. The control points replace it in the new output; the image file itself is not changed." } : null);
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const newSet = () => {
    const n = sets.length + 1;
    g.setSet(newControlSet(`Control points ${n}`, g.image?.path ?? null), false);
  };
  const saveSet = async () => {
    try {
      await api.saveControlSet(engineSet);
      g.markSaved(engineSet.id);
      useApp.setState({ dirty: true });
      await refreshSets();
      setNotice({ intent: "success", text: `Saved "${engineSet.name}" in the project (${cs.points.length} points).` });
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const deleteSet = async () => {
    if (!g.savedId) return;
    try {
      await api.deleteControlSet(g.savedId);
      useApp.setState({ dirty: true });
      await refreshSets();
      g.setSet(newControlSet("Control points 1", g.image?.path ?? null), false);
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const exportSet = async () => {
    const path = await saveFile({ title: "Export control points", defaultPath: `${engineSet.name}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      await api.exportControlSet(engineSet, path);
      setNotice({ intent: "success", text: `Exported ${cs.points.length} points to ${path}. The file's header states the coordinate system.` });
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const importSet = async () => {
    const path = await openFile({ title: "Import control points", filters: [{ name: "Control points (CSV or QGIS .points)", extensions: ["csv", "points", "txt"] }, { name: "All files", extensions: ["*"] }] });
    if (typeof path !== "string") return;
    try {
      const parsed = await api.readControlFile(path, g.image ? [g.image.info.width, g.image.info.height] : null);
      setImporting({ parsed, crs: parsed.crs });
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const finishImport = async (mode: "replace" | "add") => {
    if (!importing || !importing.crs) return;
    try {
      let points = importing.parsed.points;
      let crs = importing.crs;
      if (mode === "add" && hasCrs) {
        // The new points are converted into this set's coordinate system through an explicit transform.
        const moved = await api.reprojectControlSet({ ...engineSet, targetCrs: importing.crs, points }, cs.targetCrs);
        points = moved.points;
        crs = cs.targetCrs;
        const first = nextPointId(cs.points);
        const min = Math.min(...points.map((p) => p.id));
        points = points.map((p) => ({ ...p, id: p.id - min + first }));
        g.edit((s) => ({ ...s, points: [...s.points, ...points] }));
      } else {
        g.setSet({ ...cs, targetCrs: crs, points }, false);
      }
      setImporting(null);
      setNotice({ intent: "info", text: `Imported ${points.length} points in ${crs}. ${importing.parsed.notes.join(" ")}` });
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const chooseCrs = async (next: { id: string; name: string }) => {
    setCrsDialog(null);
    if (cs.points.length === 0 || !hasCrs) return g.edit((s) => ({ ...s, targetCrs: next.id }));
    setCrsChoice(next);
  };
  const applyCrsChange = async (convert: boolean) => {
    const next = crsChoice;
    setCrsChoice(null);
    if (!next) return;
    try {
      if (convert) {
        const moved = await api.reprojectControlSet(engineSet, next.id);
        g.edit(() => moved);
      } else g.edit((s) => ({ ...s, targetCrs: next.id }));
    } catch (e) {
      setNotice({ intent: "error", text: errorText(e) });
    }
  };
  const run = async () => {
    if (!g.image) return;
    setBusy(true);
    setNotice(null);
    try {
      const info = await api.georefWarp({ set: engineSet, method: method.method, resampling: g.resampling, image: g.image.path, sourceLayer: g.image.sourceLayer, name: outName.trim() || null });
      useApp.getState().addLayers([info]);
      setStatus(`Georeferenced ${g.image.label}: added "${info.name}" in ${cs.targetCrs}.`);
      setNotice({ intent: "success", text: `Added "${info.name}" to the map. The image file was not changed.${report ? ` ${summaryLine(report)}.` : ""}` });
    } catch (e) {
      const t = errorText(e);
      setNotice(/Cancelled/.test(t) ? { intent: "info", text: "Georeferencing was cancelled. Nothing was added." } : { intent: "error", text: t });
    } finally {
      setBusy(false);
    }
  };

  const rasterLayers = Object.values(layers).filter((l) => l.geomType === "Raster" && l.raster?.path);
  const unitLabel = report ? (report.metresKind === "geodesic" ? "°" : ` ${report.unit}`) : "";
  const canRun = !!g.image && !!report && !busy && hasCrs;
  const pendingHint = g.pending.pixel != null ? "Now click the same place on the reference map (or type its coordinates in the table)." : g.pending.x != null ? "Now click the same place on the image." : "Click a point on the image, then the same place on the reference map.";

  return (
    <div className="georef" role="dialog" aria-label="Georeference" data-testid="georef">
      <div className="gr-head">
        <strong>Georeference</strong>
        <span className="muted small">Pick matching points on the image and on the map. The image file is never changed; the result is a new GeoTIFF layer.</span>
        <span className="grow" />
        <Button size="small" appearance="subtle" icon={<DismissRegular />} aria-label="Close Georeference" onClick={() => g.hide()} />
      </div>

      <div className="gr-bar">
        <Field label="Image" size="small">
          <div className="btn-row">
            <Button size="small" onClick={() => void pickImage()}>Open image…</Button>
            {rasterLayers.length > 0 && (
              <Dropdown size="small" placeholder="or a project raster" value="" selectedOptions={[]} onOptionSelect={(_, d) => {
                const l = rasterLayers.find((x) => x.id === d.optionValue);
                if (l) void loadImage(l.raster!.path, l.id, l.name);
              }}>
                {rasterLayers.map((l) => <Option key={l.id} value={l.id}>{l.name}</Option>)}
              </Dropdown>
            )}
            <span className="muted small">{g.image ? `${g.image.label} · ${g.image.info.width} × ${g.image.info.height} px` : "No image yet"}</span>
          </div>
        </Field>
        <Field label="Control point set" size="small">
          <div className="btn-row">
            <Dropdown size="small" value={g.savedId ? (sets.find((s) => s.id === g.savedId)?.name ?? cs.name) : "(not saved)"} selectedOptions={g.savedId ? [g.savedId] : []} onOptionSelect={(_, d) => {
              const s = sets.find((x) => x.id === d.optionValue);
              if (s) g.setSet(s, true);
            }}>
              {sets.map((s) => <Option key={s.id} value={s.id}>{s.name}</Option>)}
            </Dropdown>
            <Button size="small" onClick={newSet}>New</Button>
            <Button size="small" onClick={() => void importSet()}>Import…</Button>
            <Button size="small" disabled={!hasCrs || cs.points.length === 0} onClick={() => void exportSet()}>Export…</Button>
          </div>
        </Field>
        <Field label="Set name" size="small">
          <div className="btn-row">
            <Input size="small" value={cs.name} onChange={(_, d) => g.edit((s) => ({ ...s, name: d.value }))} />
            <Button size="small" appearance="primary" disabled={!hasCrs || !cs.name.trim() || !g.modified} onClick={() => void saveSet()} title="Keep this set in the project (nodes can use it by name)">Save set</Button>
            {g.savedId && <Button size="small" icon={<DeleteRegular />} onClick={() => void deleteSet()} aria-label="Delete this saved set" />}
          </div>
        </Field>
        <Field label="Coordinate system of the points (target)" size="small">
          <Button size="small" onClick={() => setCrsDialog({})} data-testid="gr-crs">{hasCrs ? cs.targetCrs.split(/\s+/).join(" ").slice(0, 48) : "Choose…"}</Button>
        </Field>
        <Field label="Method" size="small">
          <Dropdown size="small" value={method.label} selectedOptions={[g.methodId]} onOptionSelect={(_, d) => g.setMethod(d.optionValue ?? "p1")}>
            {METHODS.map((m) => <Option key={m.id} value={m.id}>{m.label}</Option>)}
          </Dropdown>
        </Field>
        <Field label="Resampling" size="small">
          <Dropdown size="small" value={RESAMPLING.find((r) => r.id === g.resampling)!.label} selectedOptions={[g.resampling]} onOptionSelect={(_, d) => g.setResampling((d.optionValue ?? "nearest") as Resampling)}>
            {RESAMPLING.map((r) => <Option key={r.id} value={r.id}>{r.label}</Option>)}
          </Dropdown>
        </Field>
      </div>

      {notice && (
        <MessageBar intent={notice.intent} className="gr-notice"><MessageBarBody>{notice.text}</MessageBarBody></MessageBar>
      )}

      <div className="gr-panes">
        <div className="gr-pane">
          <div className="gr-label">Image (pixel, line from the top-left corner)</div>
          {g.image ? <div ref={imageEl} className="gr-map" data-testid="gr-image-map" /> : <div className="gr-map gr-empty">Open an image to start.</div>}
        </div>
        <div className="gr-pane">
          <div className="gr-label">
            Reference map {hasCrs ? `in ${cs.targetCrs.split(/\s+/)[0].slice(0, 40)}` : ""}
            <label className="gr-prev">
              <Checkbox size="medium" label="Preview" checked={previewOn} onChange={(_, d) => setPreviewOn(!!d.checked)} />
              <Slider size="small" min={0.1} max={1} step={0.05} value={opacity} onChange={(_, d) => setOpacity(d.value)} aria-label="Preview opacity" style={{ width: 90 }} />
              {previewing && <span className="muted small">updating…</span>}
            </label>
          </div>
          {hasCrs ? <div ref={refEl} className="gr-map" data-testid="gr-ref-map" /> : <div className="gr-map gr-empty">Choose the coordinate system of the points to show the reference map.</div>}
        </div>
      </div>

      <div className="muted small gr-hint">{pendingHint} Drag a marker to adjust it; click a marker to select its row.</div>

      <div className="gr-lower">
        <div className="gr-table-wrap">
          <table className="gr-table" data-testid="gr-table">
            <thead>
              <tr>
                <th title="Use this point in the fit. A point turned off becomes a check point: its error is an independent accuracy figure.">Fit</th>
                <th>#</th>
                <th title="Image column (GDAL: the top-left corner of the top-left pixel is 0, 0)">Pixel</th>
                <th>Line</th>
                <th title="Easting or longitude in the target coordinate system">X</th>
                <th title="Northing or latitude in the target coordinate system">Y</th>
                <th title="Where the transform puts the point, minus where you said it is (target units). Ground errors (dX, dY, metres) come from the forward fit, image to ground. Pixel errors come from the separately fitted reverse fit, ground to image, which is what moves the pixels.">dX{unitLabel}</th>
                <th>dY{unitLabel}</th>
                <th title={report?.metresKind === "geodesic" ? "Geodesic distance on the ellipsoid" : "Grid distance in metres (not corrected for the projection's scale)"}>Error (m)</th>
                <th title="Error measured back in the source image, from the reverse fit (ground to image) that moves the pixels; the ground columns come from the forward fit">Error (px)</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {cs.points.map((p: ControlPoint) => {
                const er = errorOf(report, p.id);
                return (
                  <tr key={p.id} className={`${p.id === g.selected ? "sel" : ""} ${p.enabled ? "" : "off"}`} onClick={() => g.select(p.id)}>
                    <td><Checkbox checked={p.enabled} onChange={(_, d) => g.edit((s) => updatePoint(s, p.id, { enabled: !!d.checked }))} aria-label={`Use point ${p.id} in the fit`} /></td>
                    <td>{p.id}{er?.check ? <span className="gr-chk" title="Check point: left out of the fit, so this error is independent of it"> check</span> : null}</td>
                    <td><NumCell value={p.pixel} onCommit={(v) => g.edit((s) => updatePoint(s, p.id, { pixel: v }))} width={72} /></td>
                    <td><NumCell value={p.line} onCommit={(v) => g.edit((s) => updatePoint(s, p.id, { line: v }))} width={72} /></td>
                    <td><NumCell value={p.x} onCommit={(v) => g.edit((s) => updatePoint(s, p.id, { x: v }))} width={112} /></td>
                    <td><NumCell value={p.y} onCommit={(v) => g.edit((s) => updatePoint(s, p.id, { y: v }))} width={112} /></td>
                    <td className="num">{er ? fmtErr(er.dx, report!.metresKind === "geodesic" ? "deg" : "m") : "–"}</td>
                    <td className="num">{er ? fmtErr(er.dy, report!.metresKind === "geodesic" ? "deg" : "m") : "–"}</td>
                    <td className="num">{er ? fmtErr(er.lengthM, "m") : "–"}</td>
                    <td className="num">{er ? fmtErr(er.lengthPx, "px") : "–"}</td>
                    <td><Button size="small" appearance="subtle" icon={<DeleteRegular />} aria-label={`Delete point ${p.id}`} onClick={(e) => { e.stopPropagation(); g.edit((s) => removePoint(s, p.id)); }} /></td>
                  </tr>
                );
              })}
              {cs.points.length === 0 && <tr><td colSpan={11} className="muted">No points yet.</td></tr>}
            </tbody>
          </table>
          <Button size="small" onClick={() => g.edit((s) => ({ ...s, points: [...s.points, { id: nextPointId(s.points), pixel: 0, line: 0, x: 0, y: 0, enabled: false }] }))} title="Adds a row to fill in by typing; it stays off until you turn it on">Add a point by typing</Button>
        </div>

        <div className="gr-side">
          <Tooltip relationship="description" content={report?.formula ?? "RMS = sqrt( sum(dx² + dy²) / n ) over the enabled points, where (dx, dy) is where the fitted transform puts a control point minus where you said it is."} withArrow>
            <div className="gr-rms" data-testid="gr-rms">
              {report ? (
                <>
                  <div className="gr-rms-main">RMS {fmtErr(report.rmsM, "m")} m</div>
                  <div className="muted small">{fmtErr(report.rmsPx, "px")} px in the image · {report.enabledCount} point{report.enabledCount === 1 ? "" : "s"} in the fit</div>
                  <div className="muted small">{fmtErr(report.rms, report.metresKind === "geodesic" ? "deg" : "m")} {report.unit}{report.metresKind === "geodesic" ? "s" : "s"} in the target system</div>
                  {report.checkRmsM != null && <div className="small">Check points: {fmtErr(report.checkRmsM, "m")} m · {fmtErr(report.checkRmsPx, "px")} px</div>}
                </>
              ) : (
                <div className="muted small">{reportMsg ?? "Working…"}</div>
              )}
            </div>
          </Tooltip>
          {report?.warnings.map((w) => <MessageBar key={w} intent="warning"><MessageBarBody>{w}</MessageBarBody></MessageBar>)}
          {report && <div className="muted small">{report.metresKind === "geodesic" ? "Metres are geodesic distances on the ellipsoid." : "Metres are grid distances (the projection's scale is not removed)."} Hover the RMS for its formula.</div>}
          <Field label="New layer name" size="small">
            <Input size="small" value={outName} placeholder="Georeferenced image" onChange={(_, d) => setOutName(d.value)} />
          </Field>
          <Button appearance="primary" disabled={!canRun} onClick={() => void run()} data-testid="gr-run">{busy ? "Georeferencing…" : "Georeference"}</Button>
          <div className="muted small">Writes a new GeoTIFF in {hasCrs ? cs.targetCrs.split(/\s+/)[0] : "the target coordinate system"} with GDAL's suggested extent and cell size. Outside the image is transparent or NoData.</div>
        </div>
      </div>

      <Dialog open={!!crsDialog} onOpenChange={(_, d) => !d.open && setCrsDialog(null)}>
        <DialogSurface className="gr-crs-surface">
          <DialogBody>
            <DialogTitle>Coordinate system of the control points</DialogTitle>
            <DialogContent>
              <CrsPicker value={hasCrs ? cs.targetCrs : null} onChange={(id, name) => void chooseCrs({ id, name })} />
            </DialogContent>
            <DialogActions><Button onClick={() => setCrsDialog(null)}>Cancel</Button></DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog open={!!crsChoice} onOpenChange={(_, d) => !d.open && setCrsChoice(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>Change the coordinate system?</DialogTitle>
            <DialogContent>
              The {cs.points.length} points are in {cs.targetCrs.split(/\s+/)[0]}. Convert their X and Y into {crsChoice?.name}, or keep the numbers as typed and only relabel them (use that only if they were really measured in {crsChoice?.name}).
            </DialogContent>
            <DialogActions>
              <Button appearance="primary" onClick={() => void applyCrsChange(true)}>Convert the points</Button>
              <Button onClick={() => void applyCrsChange(false)}>Keep the numbers</Button>
              <Button appearance="subtle" onClick={() => setCrsChoice(null)}>Cancel</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog open={!!importing} onOpenChange={(_, d) => !d.open && setImporting(null)}>
        <DialogSurface className="gr-crs-surface">
          <DialogBody>
            <DialogTitle>Import control points</DialogTitle>
            <DialogContent>
              {importing && (
                <div className="stack">
                  <div>{importing.parsed.points.length} points from a {importing.parsed.format === "qgis" ? "QGIS .points" : "GIS Studio CSV"} file. {importing.parsed.notes.join(" ")}</div>
                  {importing.parsed.crs ? (
                    <div>The file names its coordinate system: <code>{importing.parsed.crs.split(/\s+/).join(" ").slice(0, 120)}</code></div>
                  ) : (
                    <MessageBar intent="warning"><MessageBarBody>This file doesn't say which coordinate system its X and Y are in, and it is never guessed. Choose it below.</MessageBarBody></MessageBar>
                  )}
                  {!importing.parsed.crs && <CrsPicker value={importing.crs} onChange={(id) => setImporting({ ...importing, crs: id })} />}
                  {!importing.parsed.crs && importing.crs && <div>Chosen: <code>{importing.crs}</code></div>}
                </div>
              )}
            </DialogContent>
            <DialogActions>
              <Button appearance="primary" disabled={!importing?.crs} onClick={() => void finishImport("replace")}>Replace the current points</Button>
              {cs.points.length > 0 && hasCrs && <Button disabled={!importing?.crs} onClick={() => void finishImport("add")}>Add to them (converted to {cs.targetCrs.split(/\s+/)[0]})</Button>}
              <Button appearance="subtle" onClick={() => setImporting(null)}>Cancel</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </div>
  );
}
