import { useShallow } from "zustand/react/shallow";
import {
  Button,
  Checkbox,
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
import { defineProjection, generateTestData, selectByAttributes, selectByLocation, useDialog } from "../actions";
import { reportError } from "../errors/store";
import { crsName, FAVORITES, loadCatalog, searchCatalog, statePlaneFor, utmFor, type CrsEntry } from "../crs/crs";
import { STATES } from "./TigerDialog";
import { api, errorText, type CrsInfo } from "../ipc/api";
import { ElevationDialog } from "./ElevationDialog";
import { ShortcutsDialog } from "../nodes/ShortcutsDialog";
import { ProjectionBuilderDialog } from "./ProjectionBuilderDialog";
import { ArcGisDialog } from "./ArcGisDialog";
import { StacDialog } from "./StacDialog";
import { HazardsSoilsDialog } from "./HazardsSoilsDialog";
import { OgcDialog } from "./OgcDialog";
import { DatumGridsDialog } from "./DatumGridsDialog";
import { OvertureDialog } from "./OvertureDialog";
import { CensusDialog } from "./CensusDialog";
import { NationalMapDialog } from "./NationalMapDialog";
import { ExportDialog } from "./ExportDialog";
import { ExportLayersDialog } from "./ExportLayersDialog";
import { ExportTableDialog } from "./ExportTableDialog";
import { SeriesDialog } from "./SeriesDialog";
import { SplitDialog } from "./SplitDialog";
import { NodeQueryDialog } from "./NodeQueryDialog";
import { TigerDialog } from "./TigerDialog";
import { AddFieldDialog, CalculateFieldDialog, CalculateGeometryDialog, RenameFieldDialog } from "./AttributeDialogs";
import { QueryEditor } from "../query/QueryEditor";
import { benchmarkFlight, getCenterLonLat, type FlightResult } from "../map/mapController";
import { displayInfo, useApp, type SelectMode } from "../store/store";

export function Dialogs() {
  const dialog = useDialog((s) => s.dialog);
  const close = useDialog((s) => s.close);
  if (!dialog) return null;
  switch (dialog.kind) {
    case "crs":
      return <CrsPickerDialog title={dialog.title} initial={dialog.initial ?? null} onPick={(id, n) => { dialog.onPick(id, n); close(); }} onClose={close} />;
    case "census":
      return <CensusDialog layer={dialog.layer} onClose={close} />;
    case "overture":
      return <OvertureDialog onClose={close} />;
    case "ogc":
      return <OgcDialog onClose={close} />;
    case "datumGrids":
      return <DatumGridsDialog onClose={close} />;
    case "hazardsSoils":
      return <HazardsSoilsDialog onClose={close} />;
    case "stac":
      return <StacDialog onClose={close} />;
    case "arcgis":
      return <ArcGisDialog onClose={close} />;
    case "projectionBuilder":
      return <ProjectionBuilderDialog onClose={close} />;
    case "defineProjection":
      return <DefineProjectionDialog layer={dialog.layer} onClose={close} />;
    case "selectByAttributes":
      return <SelectByAttributesDialog layer={dialog.layer} onClose={close} />;
    case "selectByLocation":
      return <SelectByLocationDialog layer={dialog.layer} onClose={close} />;
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
    case "layoutSeries":
      return <SeriesDialog onClose={close} />;
    case "exportLayers":
      return <ExportLayersDialog ids={dialog.ids} onClose={close} />;
    case "exportTable":
      return <ExportTableDialog owner={dialog.owner} onClose={close} />;
    case "splitByAttribute":
      return <SplitDialog owner={dialog.owner} onClose={close} />;
    case "elevation":
      return <ElevationDialog onClose={close} />;
    case "nationalMap":
      return <NationalMapDialog onClose={close} />;
    case "nodeShortcuts":
      return <ShortcutsDialog onClose={close} />;
    case "nodeQuery":
      return <NodeQueryDialog nodeId={dialog.nodeId} param={dialog.param} onClose={close} />;
  }
}

function mapCenter(): [number, number] | null {
  return getCenterLonLat();
}

// ---------- CRS picker ----------

/** The map centre, or null when the map isn't ready or its centre isn't a valid lon/lat. */
function safeCenter(): [number, number] | null {
  try {
    const c = mapCenter();
    return c && Number.isFinite(c[0]) && Number.isFinite(c[1]) ? c : null;
  } catch {
    return null;
  }
}

/** One line for the custom-definition preview: name, kind, unit, datum, area of use. */
export function describeCrs(i: CrsInfo): string {
  const area = i.areaOfUse ? `valid ${i.areaOfUse[1].toFixed(1)}°–${i.areaOfUse[3].toFixed(1)}° N, ${i.areaOfUse[0].toFixed(1)}°–${i.areaOfUse[2].toFixed(1)}° E` : "area of use unknown";
  return [i.code ? `${i.name} (${i.code})` : i.name, i.geographic ? "geographic" : "projected", i.unit, i.datum, area].filter(Boolean).join(" · ");
}

const RECENT_KEY = "gis.crs.recent";
/** The last coordinate systems picked, newest first (kept in this browser/app profile only). */
export function recentCrs(): { id: string; name: string }[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((e) => e && typeof e.id === "string" && typeof e.name === "string").slice(0, 6) : [];
  } catch {
    return [];
  }
}
export function rememberCrs(id: string, name: string, list = recentCrs()): { id: string; name: string }[] {
  const next = [{ id, name }, ...list.filter((e) => e.id !== id)].slice(0, 6);
  try { localStorage.setItem(RECENT_KEY, JSON.stringify(next)); } catch { /* storage may be unavailable */ }
  return next;
}

export function CrsPicker({ value, onChange }: { value: string | null; onChange: (id: string, name: string) => void }) {
  const [recents] = useState(recentCrs);
  const [list, setList] = useState<CrsEntry[]>([]);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<"all" | "projected" | "geographic">("all");
  const [nearby, setNearby] = useState(false);
  const [spState, setSpState] = useState<string>("");
  const [custom, setCustom] = useState("");
  const [customErr, setCustomErr] = useState<string | null>(null);
  const [preview, setPreview] = useState<CrsInfo | null>(null);
  // Live check of a typed definition: what it is, its datum and where it is valid, or why it can't be used.
  useEffect(() => {
    if (!custom.trim()) { setPreview(null); setCustomErr(null); return; }
    let live = true;
    const t = setTimeout(() => api.crsInfo(custom.trim()).then((i) => { if (live) { setPreview(i); setCustomErr(null); } }).catch((e) => { if (live) { setPreview(null); setCustomErr(errorText(e)); } }), 350);
    return () => { live = false; clearTimeout(t); };
  }, [custom]);
  useEffect(() => {
    loadCatalog().then(setList).catch((e) => reportError(e, "dialog:crs", { title: "The coordinate system list could not be loaded" }));
  }, []);
  const center = safeCenter();
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
      const i = await api.crsInfo(custom.trim());
      await api.validateCrs(custom);
      setCustomErr(null);
      onChange(i.code ?? custom.trim(), i.name || "Custom");
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
        {recents.length > 0 && <span className="muted">Recent:</span>}
        {recents.map((e) => (
          <Button key={`r-${e.id}`} size="small" onClick={() => onChange(e.id, e.name)} title={e.id}>
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
      <Field
        label="Or enter a custom definition (WKT, PROJ string, or AUTH:CODE)"
        validationMessage={customErr ?? undefined}
        validationState={customErr ? "error" : "none"}
        hint={preview ? describeCrs(preview) : undefined}
      >
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
            <Button size="small" appearance="subtle" onClick={() => setSel({ id: "EPSG:3857", name: "WGS 84 / Pseudo-Mercator (Web Mercator)" })}>Web Mercator (EPSG:3857)</Button>
            <CrsPicker value={sel?.id ?? null} onChange={(id, name) => setSel({ id, name })} />
            {sel && <div className="crs-current">Selected: <b>{sel.name}</b> ({sel.id})</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!sel} onClick={() => { if (!sel) return; rememberCrs(sel.id, sel.name); onPick(sel.id, sel.name); }}>OK</Button>
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

const RELATIONS: [string, string][] = [
  ["intersects", "Intersect"],
  ["distance", "Are within a distance of"],
  ["within", "Are completely within"],
  ["contains", "Completely contain"],
  ["touches", "Touch the boundary of"],
  ["crosses", "Cross"],
];
const METHODS: [SelectMode, string][] = [
  ["new", "New selection"],
  ["add", "Add to current selection"],
  ["remove", "Remove from current selection"],
  ["subset", "Select from current selection"],
];

function SelectByLocationDialog({ layer: initialLayer, onClose }: { layer: string; onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.geomType !== "Table" && l.geomType !== "Raster")));
  const [target, setTarget] = useState(initialLayer);
  const [by, setBy] = useState(() => layers.find((l) => l.id !== initialLayer)?.id ?? initialLayer);
  const [relation, setRelation] = useState("intersects");
  const [distance, setDistance] = useState("1");
  const [units, setUnits] = useState("km");
  const [invert, setInvert] = useState(false);
  const [mode, setMode] = useState<SelectMode>("new");
  const bySelected = useApp((s) => s.selection[by]?.length ?? 0);
  const [onlySelected, setOnlySelected] = useState(bySelected > 0);
  const [busy, setBusy] = useState(false);
  const name = (id: string) => layers.find((l) => l.id === id)?.name ?? "";
  const pick = (value: string, set: (v: string) => void) => (
    <Dropdown value={name(value)} onOptionSelect={(_, d) => set(d.optionValue!)}>
      {layers.map((l) => (
        <Option key={l.id} value={l.id}>{l.name}</Option>
      ))}
    </Dropdown>
  );
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface className="dlg-mid">
        <DialogBody>
          <DialogTitle>Select By Location</DialogTitle>
          <DialogContent className="stack">
            <Field label="Select features from">{pick(target, setTarget)}</Field>
            <Field label="That">
              <Dropdown value={RELATIONS.find(([v]) => v === relation)![1]} onOptionSelect={(_, d) => setRelation(d.optionValue!)}>
                {RELATIONS.map(([v, l]) => (
                  <Option key={v} value={v}>{l}</Option>
                ))}
              </Dropdown>
            </Field>
            {relation === "distance" && (
              <div className="row2">
                <Field label="Distance (geodesic)">
                  <Input type="number" value={distance} onChange={(_, d) => setDistance(d.value)} />
                </Field>
                <Field label="Units">
                  <Dropdown value={units} onOptionSelect={(_, d) => setUnits(d.optionValue!)}>
                    {["m", "km", "ft", "mi", "nmi"].map((u) => (
                      <Option key={u} value={u}>{u}</Option>
                    ))}
                  </Dropdown>
                </Field>
              </div>
            )}
            <Field label="The features of">{pick(by, setBy)}</Field>
            <Checkbox
              checked={onlySelected && bySelected > 0}
              disabled={bySelected === 0}
              onChange={(_, d) => setOnlySelected(!!d.checked)}
              label={bySelected ? `Use only its ${bySelected.toLocaleString()} selected feature(s)` : "Use only its selected features (none selected)"}
            />
            <Checkbox checked={invert} onChange={(_, d) => setInvert(!!d.checked)} label="Invert: select the features that do NOT match" />
            <Field label="Selection method">
              <RadioGroup value={mode} onChange={(_, d) => setMode(d.value as SelectMode)}>
                {METHODS.map(([v, l]) => (
                  <Radio key={v} value={v} label={l} />
                ))}
              </RadioGroup>
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button
              appearance="primary"
              disabled={busy || !target || !by || (relation === "distance" && !(Number(distance) >= 0))}
              onClick={async () => {
                setBusy(true);
                const ok = await selectByLocation(target, by, { relation, distance: Number(distance) || 0, units, invert, onlySelected: onlySelected && bySelected > 0, mode });
                setBusy(false);
                if (ok) onClose();
              }}
            >
              {busy ? "Selecting…" : "Apply"}
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
