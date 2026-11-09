// Spatial Adjustment (GR3): move a vector layer's shapes with control points (affine, similarity or rubber sheet).
// "From" is a position in the layer's coordinate system; "To" is where it should be, in the coordinate system chosen here (it is
// converted into the layer's with PROJ, never assumed). The result is a new layer in the layer's own coordinate system.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { DeleteRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { create } from "zustand";
import { CrsPicker } from "../dialogs/Dialogs";
import { api, errorText } from "../ipc/api";
import { useApp } from "../store/store";
import { NumCell } from "./GeoreferenceWindow";
import { fmtErr } from "./store";
import { ADJUST_METHODS, newControlSet, nextPointId, type AdjustMethod, type AdjustReport, type AdjustResult, type ControlSet } from "./types";

export const useAdjustWindow = create<{ open: boolean; show: () => void; hide: () => void }>((set) => ({ open: false, show: () => set({ open: true }), hide: () => set({ open: false }) }));

export function AdjustWindow() {
  const open = useAdjustWindow((s) => s.open);
  return open ? <Body /> : null;
}

function Body() {
  const hide = useAdjustWindow((s) => s.hide);
  const layers = useApp((s) => s.layers);
  const activeLayer = useApp((s) => s.activeLayer);
  const vector = useMemo(() => Object.values(layers).filter((l) => l.geomType !== "Table" && l.geomType !== "Raster" && l.crs), [layers]);
  const [layerId, setLayerId] = useState<string>(() => (activeLayer && vector.some((l) => l.id === activeLayer) ? activeLayer : (vector[0]?.id ?? "")));
  const layer = layers[layerId];
  const [cs, setCs] = useState<ControlSet>(() => newControlSet("Adjustment points", null));
  const [method, setMethod] = useState<AdjustMethod>("affine");
  const [report, setReport] = useState<AdjustReport | null>(null);
  const [msg, setMsg] = useState<string | null>("Add control points.");
  const [saved, setSaved] = useState<ControlSet[]>([]);
  const [crsOpen, setCrsOpen] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<AdjustResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const m = ADJUST_METHODS.find((x) => x.id === method)!;

  useEffect(() => void api.listControlSets().then(setSaved, () => undefined), []);
  // The to points start in the layer's own coordinate system; changing it later is the user's choice.
  useEffect(() => {
    if (layer?.crs && !cs.targetCrs) setCs((s) => ({ ...s, targetCrs: layer.crs! }));
  }, [layerId]);

  const key = JSON.stringify([layerId, cs.targetCrs, method, cs.points]);
  useEffect(() => {
    if (!layerId || !cs.targetCrs) {
      setReport(null);
      setMsg(!layerId ? "Choose a layer." : "Choose the coordinate system of the To points.");
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      api.adjustErrors(layerId, { ...cs, name: cs.name.trim() || "Adjustment points" }, method).then(
        (r) => live && (setReport(r), setMsg(null)),
        (e) => live && (setReport(null), setMsg(errorText(e))),
      );
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [key]);

  const edit = (fn: (s: ControlSet) => ControlSet) => setCs((s) => fn(s));
  const run = async () => {
    setBusy(true);
    setErr(null);
    setResult(null);
    try {
      const r = await api.adjustLayer({ layer: layerId, set: { ...cs, name: cs.name.trim() || "Adjustment points" }, method, name: name.trim() || null });
      useApp.getState().addLayers([r.layer]);
      useApp.getState().setStatus(`Adjusted ${layer?.name}: added "${r.layer.name}" (${r.moved} features${r.dropped.length ? `, ${r.dropped.length} left out` : ""}).`);
      setResult(r);
    } catch (e) {
      const t = errorText(e);
      setErr(/Cancelled/.test(t) ? "Adjustment was cancelled. Nothing was added." : t);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && hide()}>
      <DialogSurface className="gr-crs-surface" style={{ maxWidth: 980, width: 980 }}>
        <DialogBody>
          <DialogTitle>Spatial Adjustment</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small">Moves a layer's shapes so From positions land on To positions. The layer is not changed; a new layer is made in the layer's own coordinate system. Only x and y move; Z and M are kept.</div>
            <div className="gr-bar" style={{ padding: 0, border: 0 }}>
              <Field label="Layer" size="small">
                <Dropdown size="small" value={layer?.name ?? ""} selectedOptions={layerId ? [layerId] : []} onOptionSelect={(_, d) => setLayerId(d.optionValue ?? "")}>
                  {vector.map((l) => <Option key={l.id} value={l.id}>{`${l.name} (${(l.crsName ?? l.crs ?? "").slice(0, 40)})`}</Option>)}
                </Dropdown>
              </Field>
              <Field label="Method" size="small">
                <Dropdown size="small" value={m.label} selectedOptions={[method]} onOptionSelect={(_, d) => setMethod((d.optionValue ?? "affine") as AdjustMethod)}>
                  {ADJUST_METHODS.map((x) => <Option key={x.id} value={x.id}>{x.label}</Option>)}
                </Dropdown>
              </Field>
              <Field label="To points are in" size="small">
                <Button size="small" onClick={() => setCrsOpen(true)} data-testid="adj-crs">{cs.targetCrs ? cs.targetCrs.split(/\s+/).join(" ").slice(0, 40) : "Choose…"}</Button>
              </Field>
              <Field label="Saved set" size="small">
                <Dropdown size="small" value="" placeholder={saved.length ? "Load a saved set" : "None saved"} selectedOptions={[]} onOptionSelect={(_, d) => {
                  const s = saved.find((x) => x.id === d.optionValue);
                  if (s) setCs({ ...s });
                }}>
                  {saved.map((s) => <Option key={s.id} value={s.id}>{`${s.name} (${s.points.length})`}</Option>)}
                </Dropdown>
              </Field>
            </div>
            <div className="gr-table-wrap" style={{ maxHeight: 220 }}>
              <table className="gr-table" data-testid="adj-table">
                <thead>
                  <tr>
                    <th>Use</th><th>#</th><th>From X</th><th>From Y</th><th>To X</th><th>To Y</th>
                    <th title="Where the transform puts the From point, minus the To point (units of the layer's CRS)">dX</th><th>dY</th><th>Error (m)</th><th />
                  </tr>
                </thead>
                <tbody>
                  {cs.points.map((p) => {
                    const er = report?.points.find((e) => e.id === p.id);
                    const set = (patch: Partial<typeof p>) => edit((s) => ({ ...s, points: s.points.map((q) => (q.id === p.id ? { ...q, ...patch } : q)) }));
                    return (
                      <tr key={p.id} className={p.enabled ? "" : "off"}>
                        <td><Checkbox checked={p.enabled} onChange={(_, d) => set({ enabled: !!d.checked })} aria-label={`Use point ${p.id}`} /></td>
                        <td>{p.id}{er?.check ? <span className="gr-chk"> check</span> : null}</td>
                        <td><NumCell value={p.pixel} width={110} onCommit={(v) => set({ pixel: v })} /></td>
                        <td><NumCell value={p.line} width={110} onCommit={(v) => set({ line: v })} /></td>
                        <td><NumCell value={p.x} width={110} onCommit={(v) => set({ x: v })} /></td>
                        <td><NumCell value={p.y} width={110} onCommit={(v) => set({ y: v })} /></td>
                        <td className="num">{er ? fmtErr(er.dx, report!.metresKind === "geodesic" ? "deg" : "m") : "–"}</td>
                        <td className="num">{er ? fmtErr(er.dy, report!.metresKind === "geodesic" ? "deg" : "m") : "–"}</td>
                        <td className="num">{er ? fmtErr(er.lengthM, "m") : "–"}</td>
                        <td><Button size="small" appearance="subtle" icon={<DeleteRegular />} aria-label={`Delete point ${p.id}`} onClick={() => edit((s) => ({ ...s, points: s.points.filter((q) => q.id !== p.id) }))} /></td>
                      </tr>
                    );
                  })}
                  {cs.points.length === 0 && <tr><td colSpan={10} className="muted">No points yet.</td></tr>}
                </tbody>
              </table>
            </div>
            <div className="btn-row">
              <Button size="small" onClick={() => edit((s) => ({ ...s, points: [...s.points, { id: nextPointId(s.points), pixel: 0, line: 0, x: 0, y: 0, enabled: true }] }))}>Add a point</Button>
              <Button size="small" disabled={!cs.targetCrs || cs.points.length === 0} onClick={async () => {
                try {
                  await api.saveControlSet({ ...cs, name: cs.name.trim() || "Adjustment points" });
                  useApp.setState({ dirty: true });
                  setSaved(await api.listControlSets());
                } catch (e) { setErr(errorText(e)); }
              }}>Save these points as a set</Button>
              <Input size="small" value={cs.name} onChange={(_, d) => edit((s) => ({ ...s, name: d.value }))} aria-label="Set name" />
            </div>
            {report ? (
              <div data-testid="adj-rms" title={report.formula}>
                <strong>RMS {fmtErr(report.rmsM, "m")} m</strong> ({report.enabledCount} points in the fit
                {report.checkRmsM != null ? `; check points ${fmtErr(report.checkRmsM, "m")} m` : ""}
                {report.scale != null ? `; scale ${report.scale.toFixed(8)}, rotation ${report.rotationDeg!.toFixed(5)}°` : ""}). Output in {report.outputCrs.split(/\s+/)[0]}.
              </div>
            ) : (
              <div className="muted small">{msg}</div>
            )}
            {report?.warnings.map((w) => <MessageBar key={w} intent="warning"><MessageBarBody>{w}</MessageBarBody></MessageBar>)}
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
            {result && (
              <MessageBar intent={result.dropped.length ? "warning" : "success"}>
                <MessageBarBody>
                  Added "{result.layer.name}" with {result.moved} adjusted feature{result.moved === 1 ? "" : "s"}.
                  {result.dropped.length > 0 && ` ${result.dropped.length} left out (feature id: reason): ${result.dropped.slice(0, 10).map((d) => `${d.fid}: ${d.reason}`).join("; ")}${result.dropped.length > 10 ? "; …" : ""}.`}
                  {result.notes.length > 0 && ` ${result.notes.join(" ")}`}
                </MessageBarBody>
              </MessageBar>
            )}
            <Field label="New layer name" size="small">
              <Input size="small" value={name} placeholder={layer ? `${layer.name} (adjusted)` : ""} onChange={(_, d) => setName(d.value)} />
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" disabled={!report || busy || !layerId} onClick={() => void run()} data-testid="adj-run">{busy ? "Adjusting…" : "Adjust"}</Button>
            <Button onClick={hide} disabled={busy}>Close</Button>
          </DialogActions>
        </DialogBody>
        <Dialog open={crsOpen} onOpenChange={(_, d) => !d.open && setCrsOpen(false)}>
          <DialogSurface className="gr-crs-surface">
            <DialogBody>
              <DialogTitle>Coordinate system of the To points</DialogTitle>
              <DialogContent>
                <CrsPicker value={cs.targetCrs || null} onChange={(id) => { edit((s) => ({ ...s, targetCrs: id })); setCrsOpen(false); }} />
              </DialogContent>
              <DialogActions><Button onClick={() => setCrsOpen(false)}>Cancel</Button></DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      </DialogSurface>
    </Dialog>
  );
}
