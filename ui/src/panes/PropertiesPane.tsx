import { useShallow } from "zustand/react/shallow";
import {
  Button,
  Checkbox,
  Dropdown,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  Option,
  Slider,
  SpinButton,
  Tab,
  TabList,
  Textarea,
} from "@fluentui/react-components";
import { EyeRegular, FolderOpenRegular, GlobeRegular, Play16Regular } from "@fluentui/react-icons";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { previewNode, runModel, setDefinitionQuery, useDialog } from "../actions";
import { crsName } from "../crs/crs";
import { api, errorText, type CrsInfo as CrsInfoT, type ToolParam, type TransformReport } from "../ipc/api";
import { LayoutElementProperties } from "../layout/ElementProperties";
import { LayoutLabelProperties } from "../layout/LabelProperties";
import { CATEGORY_COLORS, RAMPS, rampColors } from "../map/style";
import { QueryEditor } from "../query/QueryEditor";
import { moveClass, reverseClasses } from "../map/categoryOrder";
import { RasterSymbology } from "../raster/RasterSymbology";
import { LabelSettings } from "./LabelSettings";
import { fieldsForPort, useModel, useRun } from "../store/model";
import { displayInfo, useApp, type LayerStyle } from "../store/store";

export function PropertiesPane() {
  const target = useApp((s) => s.properties);
  if (target?.kind === "node") return <NodeProperties id={target.id} />;
  if (target?.kind === "layer") return <LayerProperties id={target.id} />;
  if (target?.kind === "layoutElement") return <LayoutElementProperties layout={target.layout} id={target.id} />;
  if (target?.kind === "layoutLabel") return <LayoutLabelProperties layout={target.layout} frame={target.frame} layer={target.layer} fid={target.fid} />;
  return <div className="empty-hint pad">Select a layer in Contents or a step in the Model to see its properties.</div>;
}

// ---------- Layer ----------

function LayerProperties({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const [tab, setTab] = useState<"symbology" | "general" | "crs" | "query">("symbology");
  // Another part of the app asked for a specific tab (e.g. Definition Query).
  const requested = useApp((s) => s.propsTab);
  useEffect(() => {
    if (requested) {
      setTab(requested as never);
      useApp.getState().set({ propsTab: null });
    }
  }, [requested]);
  if (!info) return null;
  const isTable = info.geomType === "Table";
  return (
    <div className="props">
      <div className="props-title">{info.name}</div>
      <TabList size="small" selectedValue={isTable && tab === "symbology" ? "general" : tab} onTabSelect={(_, d) => setTab(d.value as never)}>
        {!isTable && <Tab value="symbology">Symbology</Tab>}
        <Tab value="general">General</Tab>
        <Tab value="query">Definition Query{info.definitionQuery ? " •" : ""}</Tab>
        {!isTable && <Tab value="crs">Coordinate System</Tab>}
      </TabList>
      <div className="props-body">
        {tab === "query" ? (
          <DefinitionQuery id={id} />
        ) : tab === "general" || isTable ? (
          <dl className="kv">
            <dt>Type</dt>
            <dd>{info.geomType === "Table" ? "Table" : `${info.geomType} features`}</dd>
            <dt>Records</dt>
            <dd>
              {info.definitionQuery ? `${(info.queryCount ?? 0).toLocaleString()} of ${info.featureCount.toLocaleString()} (definition query)` : info.featureCount.toLocaleString()}
            </dd>
            <dt>Fields</dt>
            <dd>{info.fields.map((f) => `${f.name} (${f.type.toLowerCase()})`).join(", ") || "—"}</dd>
            <dt>Source</dt>
            <dd className="wrap">{info.source ?? (info.temporary ? "Model preview" : "Model output")}</dd>
          </dl>
        ) : tab === "crs" ? (
          <CrsInfo id={id} />
        ) : (
          <Symbology id={id} />
        )}
      </div>
    </div>
  );
}

export function CrsInfo({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const mapCrs = useApp((s) => s.mapCrs);
  const [detail, setDetail] = useState<CrsInfoT | null>(null);
  const [mapKey, setMapKey] = useState<boolean | null>(null);
  const [shift, setShift] = useState<TransformReport | null>(null);
  const [strict, setStrict] = useState(false);
  const ext = info.nativeExtent;
  // What the engine knows about this layer's CRS (unit, datum, area of use) and whether the map has to reproject it.
  useEffect(() => {
    let live = true;
    setDetail(null);
    setMapKey(null);
    setShift(null);
    if (info.crs) {
      api.crsInfo(info.crs).then((d) => live && setDetail(d)).catch(() => {});
      if (mapCrs) {
        api.sameCrs(info.crs, mapCrs.id).then((same) => live && setMapKey(same)).catch(() => {});
        api.transformReport(info.crs, mapCrs.id, info.nativeExtent ?? null).then((r) => live && setShift(r)).catch(() => {});
      }
    }
    api.datumPolicy().then((v) => live && setStrict(v)).catch(() => {});
    return () => { live = false; };
  }, [info.crs, mapCrs?.id]);
  return (
    <div className="stack">
      {info.crs ? (
        <dl className="kv">
          <dt>Name</dt>
          <dd>{info.crsName ?? crsName(info.crs)}</dd>
          {detail && (
            <>
              {detail.code && (<><dt>Code</dt><dd>{detail.code}</dd></>)}
              <dt>Kind</dt>
              <dd>{detail.geographic ? "Geographic" : "Projected"}, unit {detail.unit}</dd>
              {detail.datum && (<><dt>Datum</dt><dd>{detail.datum}</dd></>)}
              {detail.areaOfUse && (<><dt>Valid in</dt><dd>{`${detail.areaOfUse[1].toFixed(1)}° to ${detail.areaOfUse[3].toFixed(1)}° N, ${detail.areaOfUse[0].toFixed(1)}° to ${detail.areaOfUse[2].toFixed(1)}° E`}</dd></>)}
            </>
          )}
          {mapKey !== null && (<><dt>On the map</dt><dd>{mapKey ? "drawn as is (same as the map CRS)" : "drawn in the map CRS (reprojected)"}</dd></>)}
          {shift && mapKey === false && (
            <>
              <dt>Datum shift</dt>
              <dd>
                <span className={shift.warn ? "warn-text" : undefined}>{shift.text}</span>
              </dd>
            </>
          )}
          <dt>Strict datum mode</dt>
          <dd>
            <Checkbox
              label="Refuse approximate datum shifts (otherwise warn and continue)"
              checked={strict}
              onChange={(_, d) => { setStrict(!!d.checked); api.setDatumPolicy(!!d.checked).catch(() => {}); }}
            />
          </dd>
          <dt>Definition</dt>
          <dd className="wrap mono">{info.crs.length > 80 ? `${info.crs.slice(0, 80)}…` : info.crs}</dd>
          {ext && (
            <>
              <dt>Extent</dt>
              <dd className="mono">
                {ext[0].toFixed(4)}, {ext[1].toFixed(4)}
                <br />
                {ext[2].toFixed(4)}, {ext[3].toFixed(4)}
              </dd>
            </>
          )}
        </dl>
      ) : (
        <MessageBar intent="warning">
          <MessageBarBody>No coordinate system. This layer can't be drawn or processed until one is defined.</MessageBarBody>
        </MessageBar>
      )}
      {info.warnings.map((w) => (
        <MessageBar key={w} intent="warning">
          <MessageBarBody>{w}</MessageBarBody>
        </MessageBar>
      ))}
      <Button icon={<GlobeRegular />} onClick={() => useDialog.getState().show({ kind: "defineProjection", layer: id })}>
        Define Projection…
      </Button>
      <div className="muted">To convert coordinates into a different system, use the Project tool in the Model.</div>
    </div>
  );
}

export function Symbology({ id }: { id: string }) {
  // Classify what the layer shows (its node-tree result may have new fields).
  const info = useApp((s) => displayInfo(s, id))!;
  const style = useApp((s) => s.styles[id]);
  const setStyle = (p: Partial<LayerStyle>) => useApp.getState().setStyle(id, { ...style, ...p });
  const [ramp, setRamp] = useState("Blues");
  const [classes, setClasses] = useState(5);
  const [err, setErr] = useState<string | null>(null);
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  if (!style) return null;
  if (info.geomType === "Raster") return <RasterSymbology id={id} />;
  const numericFields = info.fields.filter((f) => /INT|DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC/i.test(f.type));
  const fields = style.kind === "graduated" ? numericFields : info.fields;

  const classify = async (field: string, kind = style.kind) => {
    setErr(null);
    try {
      if (kind === "categorized") {
        const vals = await api.uniqueValues(info.id, field, 60);
        setStyle({ kind, field, classes: vals.map(([v, n], i) => ({ value: v, color: CATEGORY_COLORS[i % CATEGORY_COLORS.length], label: `${v || "<empty>"} (${n})` })) });
      } else if (kind === "graduated") {
        const breaks = await api.quantileBreaks(info.id, field, classes);
        setStyle({ kind, field, breaks, colors: rampColors(RAMPS[ramp], breaks.length + 1) });
      }
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <div className="stack">
      <Field label="Primary symbology">
        <Dropdown
          value={{ single: "Single symbol", categorized: "Unique values", graduated: "Graduated colors" }[style.kind]}
          onOptionSelect={(_, d) => {
            const kind = d.optionValue as LayerStyle["kind"];
            if (kind === "single") setStyle({ kind });
            else {
              const f = (kind === "graduated" ? numericFields : info.fields)[0]?.name;
              if (f) classify(f, kind);
              else setErr(kind === "graduated" ? "No numeric fields to classify." : "No fields to classify.");
            }
          }}
        >
          <Option value="single">Single symbol</Option>
          <Option value="categorized">Unique values</Option>
          <Option value="graduated">Graduated colors</Option>
        </Dropdown>
      </Field>
      {style.kind !== "single" && (
        <Field label="Field">
          <Dropdown value={style.field ?? ""} onOptionSelect={(_, d) => classify(d.optionValue!)}>
            {fields.map((f) => (
              <Option key={f.name} value={f.name}>{f.name}</Option>
            ))}
          </Dropdown>
        </Field>
      )}
      {style.kind === "graduated" && (
        <div className="row2">
          <Field label="Classes (quantile)">
            <SpinButton value={classes} min={2} max={9} onChange={(_, d) => setClasses(d.value ?? Number(d.displayValue) ?? 5)} />
          </Field>
          <Field label="Color scheme">
            <Dropdown value={ramp} onOptionSelect={(_, d) => setRamp(d.optionValue!)}>
              {Object.keys(RAMPS).map((r) => (
                <Option key={r} value={r} text={r}>
                  <span className="ramp">{RAMPS[r].map((c) => <i key={c} style={{ background: c }} />)}</span> {r}
                </Option>
              ))}
            </Dropdown>
          </Field>
        </div>
      )}
      {style.kind === "graduated" && style.field && <Button size="small" onClick={() => classify(style.field!)}>Reclassify</Button>}
      {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}

      {style.kind === "single" && (
        <div className="row2">
          <Field label={info.geomType === "Line" ? "Color" : "Fill"}>
            <input type="color" value={style.fill} onChange={(e) => setStyle({ fill: e.target.value, ...(info.geomType !== "Polygon" ? { stroke: e.target.value } : {}) })} />
          </Field>
          {info.geomType === "Polygon" && (
            <Field label="Outline">
              <input type="color" value={style.stroke} onChange={(e) => setStyle({ stroke: e.target.value })} />
            </Field>
          )}
        </div>
      )}
      {style.kind === "categorized" && style.classes && (
        <>
          <div className="classes" data-testid="classes">
            {style.classes.map((c, i) => (
              <div
                key={c.value}
                className={`class-row ${dragFrom === i ? "dragging" : ""}`}
                draggable
                onDragStart={(e) => {
                  setDragFrom(i);
                  e.dataTransfer.effectAllowed = "move";
                }}
                onDragOver={(e) => e.preventDefault()}
                onDrop={(e) => {
                  e.preventDefault();
                  if (dragFrom !== null) setStyle({ classes: moveClass(style.classes!, dragFrom, i) });
                  setDragFrom(null);
                }}
                onDragEnd={() => setDragFrom(null)}
              >
                <span className="class-grip" title="Drag to change the drawing order" aria-hidden>⠿</span>
                <input type="color" value={c.color} onChange={(e) => setStyle({ classes: style.classes!.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)) })} />
                <span className="grow">{c.label}</span>
                <Button size="small" appearance="subtle" aria-label={`Draw ${c.label} higher`} disabled={i === 0} onClick={() => setStyle({ classes: moveClass(style.classes!, i, i - 1) })}>▲</Button>
                <Button size="small" appearance="subtle" aria-label={`Draw ${c.label} lower`} disabled={i === style.classes!.length - 1} onClick={() => setStyle({ classes: moveClass(style.classes!, i, i + 1) })}>▼</Button>
              </div>
            ))}
          </div>
          <div className="btn-row">
            <Button size="small" onClick={() => setStyle({ classes: reverseClasses(style.classes!) })}>Reverse order</Button>
            <span className="muted small">The top of the list draws on top, on the map, in layouts and in the legend.</span>
          </div>
        </>
      )}
      {style.kind === "graduated" && style.breaks && style.colors && (
        <div className="classes">
          {style.colors.map((c, i) => (
            <div key={i} className="class-row">
              <input type="color" value={c} onChange={(e) => setStyle({ colors: style.colors!.map((x, j) => (j === i ? e.target.value : x)) })} />
              <span>
                {i === 0 ? "≤ " : `${fmt(style.breaks![i - 1])} – `}
                {i < style.breaks!.length ? fmt(style.breaks![i]) : "max"}
              </span>
            </div>
          ))}
        </div>
      )}
      <Field label={`Opacity ${Math.round(style.opacity * 100)}%`}>
        <Slider min={0} max={100} value={Math.round(style.opacity * 100)} onChange={(_, d) => setStyle({ opacity: d.value / 100 })} />
      </Field>
      {info.geomType !== "Point" && (
        <Field label={`${info.geomType === "Line" ? "Line" : "Outline"} width ${style.strokeWidth} pt`}>
          <Slider min={0} max={8} step={0.1} value={style.strokeWidth} onChange={(_, d) => setStyle({ strokeWidth: d.value })} />
        </Field>
      )}
      {info.geomType === "Point" && (
        <Field label={`Size ${style.radius} pt`}>
          <Slider min={1} max={14} step={0.5} value={style.radius} onChange={(_, d) => setStyle({ radius: d.value })} />
        </Field>
      )}
      <details>
        <summary>Print layout effect</summary>
        <div className="stack">
          <Checkbox label="Drop shadow in layouts" checked={!!style.effect} onChange={(_, d) => setStyle({ effect: d.checked ? { dx: 0.8, dy: 0.8, color: "#000000", opacity: 0.35 } : undefined })} />
          {style.effect && (
            <>
              <Field label={`Offset ${style.effect.dx} mm right, ${style.effect.dy} mm down`}>
                <Slider min={0} max={4} step={0.1} value={style.effect.dx} onChange={(_, d) => setStyle({ effect: { ...style.effect!, dx: d.value, dy: d.value } })} />
              </Field>
              <Field label={`Shadow strength ${Math.round(style.effect.opacity * 100)}%`}>
                <Slider min={5} max={100} value={Math.round(style.effect.opacity * 100)} onChange={(_, d) => setStyle({ effect: { ...style.effect!, opacity: d.value / 100 } })} />
              </Field>
              <div className="props-hint">A solid offset copy behind the layer; it appears in layouts and exports, not on the map.</div>
            </>
          )}
        </div>
      </details>
      <div className="props-title">Labels</div>
      <LabelSettings id={id} />
    </div>
  );
}

/** Limit a layer to features matching an expression, everywhere in the app. */
export function DefinitionQuery({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const hasResult = useApp((s) => !!s.results[id] && !s.showOriginal[id]);
  const [pending, setPending] = useState<{ sql: string; error: string | null }>({ sql: info.definitionQuery ?? "", error: null });
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editorKey, setEditorKey] = useState(0);
  const apply = async (q: string | null) => {
    setBusy(true);
    setErr(null);
    try {
      await setDefinitionQuery(id, q);
      if (!q) setEditorKey((k) => k + 1); // reset the editor after Clear
    } catch (e) {
      setErr(errorText(e));
    }
    setBusy(false);
  };
  return (
    <div className="stack">
      <div className="muted">
        Only features matching the query are part of the layer: drawn on the map, listed in the table, selectable, exported, and used as tool input. The data itself isn't changed.
      </div>
      {info.definitionQuery ? (
        <MessageBar intent="info">
          <MessageBarBody>
            Active: <code>{info.definitionQuery}</code> — {(info.queryCount ?? 0).toLocaleString()} of {info.featureCount.toLocaleString()} features.
          </MessageBarBody>
        </MessageBar>
      ) : null}
      {hasResult && (
        <MessageBar intent="warning">
          <MessageBarBody>This layer shows its node-tree result. The query filters the tree's Layer Input: run the tree to update the result.</MessageBarBody>
        </MessageBar>
      )}
      <QueryEditor key={editorKey} layerId={id} fields={info.fields} initialSql={info.definitionQuery} onChange={(sql, error) => setPending({ sql, error })} />
      {err && (
        <MessageBar intent="error">
          <MessageBarBody>{err}</MessageBarBody>
        </MessageBar>
      )}
      <div className="btn-row">
        <Button appearance="primary" size="small" disabled={busy || !!pending.error || !pending.sql} onClick={() => apply(pending.sql)}>Apply</Button>
        <Button size="small" disabled={busy || !info.definitionQuery} onClick={() => apply(null)}>Clear</Button>
      </div>
    </div>
  );
}

const fmt = (n: number) => (Math.abs(n) >= 1000 ? Math.round(n).toLocaleString() : Number(n.toPrecision(4)).toString());

// ---------- Model node ----------

function NodeProperties({ id }: { id: string }) {
  const node = useModel((s) => s.nodes.find((n) => n.id === id));
  const spec = useRun((s) => (node ? s.specs[node.data.tool] : undefined));
  const result = useRun((s) => s.results[id]);
  const running = useRun((s) => s.running);
  if (!node || !spec) return <div className="empty-hint pad">Step not found.</div>;
  return (
    <div className="props">
      <div className="props-title">{spec.name}</div>
      <div className="props-body stack">
        <div className="muted">{spec.description}</div>
        {result?.status === "error" && (
          <MessageBar intent="error"><MessageBarBody>{result.error}</MessageBarBody></MessageBar>
        )}
        {result?.status === "skipped" && (
          <MessageBar intent="warning"><MessageBarBody>{result.error}</MessageBarBody></MessageBar>
        )}
        {spec.params.map((p) => (
          <ParamField key={p.id} nodeId={id} param={p} value={node.data.params[p.id]} />
        ))}
        <div className="btn-row">
          <Button size="small" icon={<Play16Regular />} disabled={running} onClick={() => runModel(id)}>Run to here</Button>
          {spec.output && (
            <Button size="small" icon={<EyeRegular />} disabled={!result?.output} onClick={() => previewNode(id)}>Preview on map</Button>
          )}
        </div>
        {result && (result.status === "done" || result.status === "cached") && (
          <dl className="kv">
            {result.rows !== null && (<><dt>Rows</dt><dd>{result.rows.toLocaleString()}</dd></>)}
            {result.crs && (<><dt>Output CRS</dt><dd>{crsName(result.crs)}</dd></>)}
            <dt>Time</dt>
            <dd>{result.status === "cached" ? "reused from cache" : `${result.millis} ms`}</dd>
            {result.reprojected && (<><dt>Note</dt><dd>An input was reprojected to match the first input's coordinate system.</dd></>)}
            {result.message && (<><dt>Result</dt><dd>{result.message}</dd></>)}
          </dl>
        )}
      </div>
    </div>
  );
}

function ParamField({ nodeId, param: p, value }: { nodeId: string; param: ToolParam; value: unknown }) {
  const set = (v: unknown) => useModel.getState().setParam(nodeId, p.id, v);
  const layers = useApp(useShallow((s) => s.order.map((lid) => s.layers[lid]).filter((l) => l && !l.temporary)));
  const label = `${p.label}${p.optional ? " (optional)" : ""}`;
  const fields = p.fieldOf ? fieldsForPort(nodeId, p.fieldOf) : [];
  switch (p.kind) {
    case "number":
      return (
        <Field label={label} hint={p.help || undefined}>
          <Input type="number" value={value === undefined || value === null ? "" : String(value)} onChange={(_, d) => set(d.value === "" ? null : Number(d.value))} />
        </Field>
      );
    case "bool":
      return <Checkbox label={p.label} checked={!!value} onChange={(_, d) => set(!!d.checked)} />;
    case "color":
      return (
        <Field label={label} hint={p.help || undefined}>
          <input type="color" value={/^#[0-9a-fA-F]{6}$/.test(String(value ?? "")) ? String(value) : "#000000"} onChange={(e) => set(e.target.value)} />
        </Field>
      );
    case "enum":
      return (
        <Field label={label}>
          <Dropdown value={p.options.find(([v]) => v === value)?.[1] ?? ""} onOptionSelect={(_, d) => set(d.optionValue)}>
            {p.options.map(([v, l]) => (
              <Option key={v} value={v}>{l}</Option>
            ))}
          </Dropdown>
        </Field>
      );
    case "layer":
      return (
        <Field label={label}>
          <Dropdown value={layers.find((l) => l.id === value)?.name ?? ""} placeholder="Choose a layer" onOptionSelect={(_, d) => set(d.optionValue)}>
            {layers.map((l) => (
              <Option key={l.id} value={l.id}>{l.name}</Option>
            ))}
          </Dropdown>
        </Field>
      );
    case "field":
      return (
        <Field label={label} hint={fields.length ? undefined : "Connect an input to list its fields"}>
          <Dropdown value={String(value ?? "")} placeholder={p.optional ? "(none)" : "Choose a field"} onOptionSelect={(_, d) => set(d.optionValue || null)}>
            {p.optional && <Option value="">(none)</Option>}
            {fields.map((f) => (
              <Option key={f.name} value={f.name}>{f.name}</Option>
            ))}
          </Dropdown>
        </Field>
      );
    case "expression":
      return (
        <Field label={label} hint={p.help || undefined}>
          {fields.length > 0 && (
            <div className="field-chips">
              {fields.map((f) => (
                <button key={f.name} className="chip" onClick={() => set(`${value ?? ""}${value ? " " : ""}${/^[A-Za-z_][A-Za-z0-9_]*$/.test(f.name) ? f.name : `"${f.name}"`}`)}>
                  {f.name}
                </button>
              ))}
            </div>
          )}
          <Textarea value={String(value ?? "")} onChange={(_, d) => set(d.value)} rows={3} className="mono" />
        </Field>
      );
    case "query":
      return (
        <Field label={label}>
          <div className="stack">
            <code className="query-preview">{String(value ?? "") || "No query yet"}</code>
            <Button size="small" onClick={() => useDialog.getState().show({ kind: "nodeQuery", nodeId, param: p.id })}>{value ? "Edit query…" : "Build query…"}</Button>
          </div>
        </Field>
      );
    case "crs":
      return (
        <Field label={label}>
          <div className="crs-param">
            <span>{value ? crsName(String(value)) : "Not set"}</span>
            <Button
              size="small"
              icon={<GlobeRegular />}
              onClick={() => useDialog.getState().show({ kind: "crs", title: p.label, initial: (value as string) ?? null, onPick: (crsId) => set(crsId) })}
            >
              Choose…
            </Button>
          </div>
        </Field>
      );
    case "savePath":
      return (
        <Field label={label}>
          <div className="crs-param">
            <Input value={String(value ?? "")} onChange={(_, d) => set(d.value)} style={{ flex: 1 }} />
            <Button
              size="small"
              icon={<FolderOpenRegular />}
              onClick={async () => {
                const path = await save({ title: p.label });
                if (path) set(path);
              }}
            />
          </div>
        </Field>
      );
    default:
      return (
        <Field label={label} hint={p.help || undefined}>
          <Input value={String(value ?? "")} onChange={(_, d) => set(d.value)} />
        </Field>
      );
  }
}
