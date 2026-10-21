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
import { useState } from "react";
import { previewNode, runModel, useDialog } from "../actions";
import { crsName } from "../crs/crs";
import { api, errorText, type ToolParam } from "../ipc/api";
import { CATEGORY_COLORS, RAMPS, rampColors } from "../map/style";
import { fieldsForPort, useModel, useRun } from "../store/model";
import { useApp, type LayerStyle } from "../store/store";

export function PropertiesPane() {
  const target = useApp((s) => s.properties);
  if (target?.kind === "node") return <NodeProperties id={target.id} />;
  if (target?.kind === "layer") return <LayerProperties id={target.id} />;
  return <div className="empty-hint pad">Select a layer in Contents or a step in the Model to see its properties.</div>;
}

// ---------- Layer ----------

function LayerProperties({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const [tab, setTab] = useState<"symbology" | "general" | "crs">("symbology");
  if (!info) return null;
  const isTable = info.geomType === "Table";
  return (
    <div className="props">
      <div className="props-title">{info.name}</div>
      <TabList size="small" selectedValue={isTable && tab === "symbology" ? "general" : tab} onTabSelect={(_, d) => setTab(d.value as never)}>
        {!isTable && <Tab value="symbology">Symbology</Tab>}
        <Tab value="general">General</Tab>
        {!isTable && <Tab value="crs">Coordinate System</Tab>}
      </TabList>
      <div className="props-body">
        {tab === "general" || isTable ? (
          <dl className="kv">
            <dt>Type</dt>
            <dd>{info.geomType === "Table" ? "Table" : `${info.geomType} features`}</dd>
            <dt>Records</dt>
            <dd>{info.featureCount.toLocaleString()}</dd>
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

function CrsInfo({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const ext = info.nativeExtent;
  return (
    <div className="stack">
      {info.crs ? (
        <dl className="kv">
          <dt>Name</dt>
          <dd>{info.crsName ?? crsName(info.crs)}</dd>
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

function Symbology({ id }: { id: string }) {
  const info = useApp((s) => s.layers[id]);
  const style = useApp((s) => s.styles[id]);
  const setStyle = (p: Partial<LayerStyle>) => useApp.getState().setStyle(id, { ...style, ...p });
  const [ramp, setRamp] = useState("Blues");
  const [classes, setClasses] = useState(5);
  const [err, setErr] = useState<string | null>(null);
  if (!style) return null;
  const numericFields = info.fields.filter((f) => /INT|DOUBLE|FLOAT|DECIMAL|REAL|NUMERIC/i.test(f.type));
  const fields = style.kind === "graduated" ? numericFields : info.fields;

  const classify = async (field: string, kind = style.kind) => {
    setErr(null);
    try {
      if (kind === "categorized") {
        const vals = await api.uniqueValues(id, field, 60);
        setStyle({ kind, field, classes: vals.map(([v, n], i) => ({ value: v, color: CATEGORY_COLORS[i % CATEGORY_COLORS.length], label: `${v || "<empty>"} (${n})` })) });
      } else if (kind === "graduated") {
        const breaks = await api.quantileBreaks(id, field, classes);
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
        <div className="classes">
          {style.classes.map((c, i) => (
            <label key={i} className="class-row">
              <input type="color" value={c.color} onChange={(e) => setStyle({ classes: style.classes!.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)) })} />
              <span>{c.label}</span>
            </label>
          ))}
        </div>
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
  const layers = useApp((s) => s.order.map((lid) => s.layers[lid]).filter((l) => l && !l.temporary));
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
