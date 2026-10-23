import { useShallow } from "zustand/react/shallow";
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  Option,
  Radio,
  RadioGroup,
  ProgressBar,
} from "@fluentui/react-components";
import { WaitBar } from "../progress/progress";
import { Globe20Regular, Location20Regular, Star20Filled } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { defineProjection, generateTestData, selectByAttributes, useDialog } from "../actions";
import { crsName, FAVORITES, loadCatalog, searchCatalog, statePlaneFor, utmFor, type CrsEntry } from "../crs/crs";
import { STATES } from "./TigerDialog";
import { api, errorText } from "../ipc/api";
import { ElevationDialog } from "./ElevationDialog";
import { NationalMapDialog } from "./NationalMapDialog";
import { ExportDialog } from "./ExportDialog";
import { NodeQueryDialog } from "./NodeQueryDialog";
import { TigerDialog } from "./TigerDialog";
import { AddFieldDialog, CalculateFieldDialog, CalculateGeometryDialog, RenameFieldDialog } from "./AttributeDialogs";
import { QueryEditor } from "../query/QueryEditor";
import { benchmarkFlight, getMap, type FlightResult } from "../map/mapController";
import { displayInfo, useApp } from "../store/store";

export function Dialogs() {
  const dialog = useDialog((s) => s.dialog);
  const close = useDialog((s) => s.close);
  if (!dialog) return null;
  switch (dialog.kind) {
    case "crs":
      return <CrsPickerDialog title={dialog.title} initial={dialog.initial ?? null} onPick={(id, n) => { dialog.onPick(id, n); close(); }} onClose={close} />;
    case "defineProjection":
      return <DefineProjectionDialog layer={dialog.layer} onClose={close} />;
    case "selectByAttributes":
      return <SelectByAttributesDialog layer={dialog.layer} onClose={close} />;
    case "testData":
      return <TestDataDialog onClose={close} />;
    case "benchmark":
      return <BenchmarkDialog onClose={close} />;
    case "rename":
      return <RenameDialog layer={dialog.layer} onClose={close} />;
    case "addField":
      return <AddFieldDialog layer={dialog.layer} onClose={close} />;
    case "calcField":
      return <CalculateFieldDialog layer={dialog.layer} owner={dialog.owner} field={dialog.field} onClose={close} />;
    case "calcGeometry":
      return <CalculateGeometryDialog layer={dialog.layer} owner={dialog.owner} field={dialog.field} onClose={close} />;
    case "renameField":
      return <RenameFieldDialog layer={dialog.layer} field={dialog.field} onClose={close} />;
    case "tiger":
      return <TigerDialog onClose={close} />;
    case "export":
      return <ExportDialog owner={dialog.owner} onClose={close} />;
    case "elevation":
      return <ElevationDialog onClose={close} />;
    case "nationalMap":
      return <NationalMapDialog onClose={close} />;
    case "nodeQuery":
      return <NodeQueryDialog nodeId={dialog.nodeId} param={dialog.param} onClose={close} />;
  }
}

function mapCenter(): [number, number] | null {
  const c = getMap()?.getCenter();
  return c ? [c.lng, c.lat] : null;
}

// ---------- CRS picker ----------

export function CrsPicker({ value, onChange }: { value: string | null; onChange: (id: string, name: string) => void }) {
  const [list, setList] = useState<CrsEntry[]>([]);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<"all" | "projected" | "geographic">("all");
  const [nearby, setNearby] = useState(false);
  const [spState, setSpState] = useState<string>("");
  const [custom, setCustom] = useState("");
  const [customErr, setCustomErr] = useState<string | null>(null);
  useEffect(() => {
    loadCatalog().then(setList);
  }, []);
  const center = mapCenter();
  const results = useMemo(
    () =>
      spState
        ? statePlaneFor(list, spState).filter((e) => !query || `${e.id} ${e.name}`.toLowerCase().includes(query.toLowerCase()))
        : searchCatalog(list, query, { kind, at: nearby ? center : null, limit: 200 }),
    [list, query, kind, nearby, spState],
  );
  const favorites = list.filter((e) => FAVORITES.includes(e.id));
  const utm = center ? [utmFor(center[0], center[1]), utmFor(center[0], center[1], "NAD83")] : [];

  const useCustom = async () => {
    try {
      await api.validateCrs(custom);
      setCustomErr(null);
      onChange(custom.trim(), "Custom");
    } catch (e) {
      setCustomErr(errorText(e));
    }
  };

  return (
    <div className="crs-picker">
      <div className="crs-toolbar">
        <Input placeholder="Search by name, code or area (e.g. 26914, Texas, UTM 14N)" value={query} onChange={(_, d) => setQuery(d.value)} style={{ flex: 1 }} autoFocus />
        <Dropdown value={{ all: "All", projected: "Projected", geographic: "Geographic" }[kind]} onOptionSelect={(_, d) => setKind(d.optionValue as never)} style={{ minWidth: 130 }}>
          <Option value="all">All</Option>
          <Option value="projected">Projected</Option>
          <Option value="geographic">Geographic</Option>
        </Dropdown>
        <Button icon={<Location20Regular />} appearance={nearby ? "primary" : "secondary"} onClick={() => setNearby(!nearby)} title="Only coordinate systems valid at the map center">
          Current extent
        </Button>
        <Dropdown
          value={spState ? `State Plane: ${spState}` : "State Plane…"}
          onOptionSelect={(_, d) => setSpState(d.optionValue ?? "")}
          style={{ minWidth: 170 }}
          title="All State Plane zones for a state, newest datum first"
        >
          <Option value="">All coordinate systems</Option>
          {STATES.map(([, n]) => (
            <Option key={n} value={n}>{n}</Option>
          ))}
        </Dropdown>
      </div>
      <div className="crs-quick">
        <span className="muted">Quick picks:</span>
        {favorites.map((e) => (
          <Button key={e.id} size="small" icon={<Star20Filled />} onClick={() => onChange(e.id, e.name)}>
            {e.name}
          </Button>
        ))}
        {utm.map((id) => (
          <Button key={id} size="small" icon={<Globe20Regular />} onClick={() => onChange(id, crsName(id))} title="UTM zone at the map center">
            {crsName(id)}
          </Button>
        ))}
      </div>
      <div className="crs-list" role="listbox">
        {list.length === 0 && <WaitBar label="Loading coordinate systems…" />}
        {results.map((e) => (
          <div key={e.id} role="option" aria-selected={e.id === value} className={`crs-row ${e.id === value ? "sel" : ""}`} onClick={() => onChange(e.id, e.name)} onDoubleClick={() => onChange(e.id, e.name)}>
            <span className="crs-code">{e.id}</span>
            <span className="crs-name">{e.name}</span>
            <span className="crs-area">{e.area}</span>
          </div>
        ))}
        {list.length > 0 && results.length === 0 && <div className="muted pad">No matches.</div>}
      </div>
      <Field label="Or enter a custom definition (WKT, PROJ string, or AUTH:CODE)" validationMessage={customErr ?? undefined} validationState={customErr ? "error" : "none"}>
        <div style={{ display: "flex", gap: 8 }}>
          <Input value={custom} onChange={(_, d) => setCustom(d.value)} style={{ flex: 1 }} placeholder="+proj=utm +zone=14 +datum=NAD83 +units=us-ft" />
          <Button disabled={!custom.trim()} onClick={useCustom}>Use</Button>
        </div>
      </Field>
    </div>
  );
}

function CrsPickerDialog({ title, initial, onPick, onClose }: { title: string; initial: string | null; onPick: (id: string, name: string) => void; onClose: () => void }) {
  const [sel, setSel] = useState<{ id: string; name: string } | null>(initial ? { id: initial, name: crsName(initial) } : null);
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface className="dlg-wide">
        <DialogBody>
          <DialogTitle>{title}</DialogTitle>
          <DialogContent>
            <CrsPicker value={sel?.id ?? null} onChange={(id, name) => setSel({ id, name })} />
            {sel && <div className="crs-current">Selected: <b>{sel.name}</b> ({sel.id})</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!sel} onClick={() => sel && onPick(sel.id, sel.name)}>OK</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ---------- Define Projection ----------

function DefineProjectionDialog({ layer, onClose }: { layer: string; onClose: () => void }) {
  const info = useApp((s) => s.layers[layer]);
  const [suggestions, setSuggestions] = useState<{ crs: string; reason: string }[]>([]);
  const [sel, setSel] = useState<{ id: string; name: string } | null>(null);
  const [, setCatalogReady] = useState(false);
  useEffect(() => {
    api.suggestCrs(layer).then(setSuggestions).catch(() => setSuggestions([]));
    loadCatalog().then(() => setCatalogReady(true)); // re-render so names replace codes
  }, [layer]);
  if (!info) return null;
  const ext = info.nativeExtent;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface className="dlg-wide">
        <DialogBody>
          <DialogTitle>Define Projection — {info.name}</DialogTitle>
          <DialogContent>
            <MessageBar intent={info.crs ? "info" : "warning"}>
              <MessageBarBody>
                {info.crs
                  ? `Current: ${info.crsName ?? info.crs}. Defining a projection changes how coordinates are interpreted; it does not move data. To convert coordinates, use the Project tool.`
                  : "This layer has no coordinate system, so it can't be drawn or processed. Choose the system the coordinates were recorded in. GIS Studio never guesses this for you."}
              </MessageBarBody>
            </MessageBar>
            {ext && (
              <div className="muted pad">
                Coordinate range: X {ext[0].toFixed(3)} to {ext[2].toFixed(3)}, Y {ext[1].toFixed(3)} to {ext[3].toFixed(3)}
              </div>
            )}
            {suggestions.length > 0 && (
              <div className="suggestions">
                <div className="muted">Suggestions from the coordinate values:</div>
                {suggestions.map((s) =>
                  s.crs.includes(":") ? (
                    <Button key={s.crs} size="small" onClick={() => setSel({ id: s.crs, name: crsName(s.crs) })} title={s.reason}>
                      {crsName(s.crs)} — {s.reason}
                    </Button>
                  ) : (
                    <div key={s.crs} className="muted">• {s.reason}</div>
                  ),
                )}
              </div>
            )}
            <CrsPicker value={sel?.id ?? null} onChange={(id, name) => setSel({ id, name })} />
            {sel && <div className="crs-current">Selected: <b>{sel.name}</b> ({sel.id})</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button
              appearance="primary"
              disabled={!sel}
              onClick={async () => {
                if (!sel) return;
                onClose();
                await defineProjection(layer, sel.id, sel.name);
              }}
            >
              Define
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ---------- Select By Attributes ----------

function SelectByAttributesDialog({ layer: initialLayer, onClose }: { layer: string; onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter(Boolean)));
  const [layer, setLayer] = useState(initialLayer);
  const [where, setWhere] = useState<{ sql: string; error: string | null }>({ sql: "", error: null });
  const [mode, setMode] = useState<"new" | "add">("new");
  const [err, setErr] = useState<string | null>(null);
  const info = useApp((s) => displayInfo(s, layer));
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface className="dlg-mid">
        <DialogBody>
          <DialogTitle>Select By Attributes</DialogTitle>
          <DialogContent className="stack">
            <Field label="Input rows">
              <Dropdown value={info?.name ?? ""} onOptionSelect={(_, d) => setLayer(d.optionValue!)}>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{l.name}</Option>
                ))}
              </Dropdown>
            </Field>
            <RadioGroup layout="horizontal" value={mode} onChange={(_, d) => setMode(d.value as never)}>
              <Radio value="new" label="New selection" />
              <Radio value="add" label="Add to current selection" />
            </RadioGroup>
            {info && <QueryEditor key={info.id} layerId={info.id} fields={info.fields} onChange={(sql, error) => { setWhere({ sql, error }); setErr(null); }} />}
            {err && (
              <MessageBar intent="error">
                <MessageBarBody>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button
              appearance="primary"
              disabled={!where.sql || !!where.error || !info}
              onClick={async () => {
                try {
                  await api.selectWhere(info!.id, `(${where.sql}) AND FALSE`);
                } catch (e) {
                  setErr(errorText(e).split("\n")[0]);
                  return;
                }
                if (await selectByAttributes(layer, where.sql, mode)) onClose();
              }}
            >
              Apply
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ---------- Test data / benchmark ----------

function TestDataDialog({ onClose }: { onClose: () => void }) {
  const [kind, setKind] = useState<"points" | "polygons">("points");
  const [count, setCount] = useState("1000000");
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Generate Test Data</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Random features across the current map extent, for trying tools and checking map speed.</div>
            <RadioGroup layout="horizontal" value={kind} onChange={(_, d) => setKind(d.value as never)}>
              <Radio value="points" label="Points" />
              <Radio value="polygons" label="Polygons" />
            </RadioGroup>
            <Field label="Number of features">
              <Input type="number" value={count} onChange={(_, d) => setCount(d.value)} />
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" onClick={() => { onClose(); generateTestData(kind, Math.max(1, Math.min(50_000_000, parseInt(count) || 1000))); }}>
              Generate
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

function BenchmarkDialog({ onClose }: { onClose: () => void }) {
  const [res, setRes] = useState<FlightResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [step, setStep] = useState(0);
  const run = async () => {
    const s = useApp.getState();
    const target = s.order.map((id) => s.layers[id]).find((l) => l?.extent);
    if (!target?.extent) {
      setErr("Add a layer (or Generate Test Data) first.");
      return;
    }
    setRunning(true);
    setErr(null);
    try {
      setStep(0);
      setRes(await benchmarkFlight(target.extent, setStep));
    } catch (e) {
      setErr(errorText(e));
    }
    setRunning(false);
  };
  const pass = res && res.avgFps >= 50 && res.p95FrameMs <= 25;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !running && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Map Benchmark</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Flies across the top layer at several zoom levels and records frame times. Target: ≥ 50 fps average, p95 frame ≤ 25 ms.</div>
            {running && (
              <div className="task-progress">
                <div className="tp-head"><span>Flying…</span><span className="tp-pct">{Math.round(step * 100)}%</span></div>
                <ProgressBar thickness="large" value={step} max={1} />
              </div>
            )}
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
            {res && (
              <MessageBar intent={pass ? "success" : "warning"}>
                <MessageBarBody>
                  {pass ? "PASS" : "BELOW TARGET"} — {res.avgFps.toFixed(1)} fps average · p95 frame {res.p95FrameMs.toFixed(1)} ms · worst {res.worstFrameMs.toFixed(0)} ms · {res.frames} frames in {res.seconds.toFixed(1)} s
                </MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={running} onClick={onClose}>Close</Button>
            <Button appearance="primary" disabled={running} onClick={run}>Run</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

function RenameDialog({ layer, onClose }: { layer: string; onClose: () => void }) {
  const info = useApp((s) => s.layers[layer]);
  const [name, setName] = useState(info?.name ?? "");
  const submit = async () => {
    const updated = await api.renameLayer(layer, name.trim());
    useApp.getState().updateLayer(updated);
    onClose();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Rename Layer</DialogTitle>
          <DialogContent>
            <Input value={name} onChange={(_, d) => setName(d.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && submit()} style={{ width: "100%" }} autoFocus />
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!name.trim()} onClick={submit}>OK</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
