// Raster symbology (RS3): renderer, band, stretch with a histogram, colour ramp, classes, transparency and image adjustments.
// The style is `LayerStyle.raster`; the engine draws it (raster_style.rs).
import { Button, Checkbox, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, OptionGroup, Slider, SpinButton } from "@fluentui/react-components";
import { open, save } from "../ipc/dialogs";
import { useEffect, useRef, useState } from "react";
import { api, errorText, type RampInfo, type RasterHistogram, type RasterUniqueValues } from "../ipc/api";
import { displayInfo, useApp, type LayerStyle, type RasterStyle } from "../store/store";
import { barHeights, dragHandle, fmtValue, rampCss, valueToX } from "./histogram";

const RENDERERS = { default: "Default (elevation + hillshade / RGB)", gray: "Singleband gray", pseudocolor: "Singleband pseudocolor", paletted: "Paletted / Unique values", multiband: "Multiband colour" } as const;
const STRETCH = { minmax: "Min – Max", percentile: "Percentile clip", stddev: "Standard deviations", manual: "Manual" } as const;
const METHODS = { equal: "Equal interval", quantile: "Quantile", jenks: "Natural breaks (Jenks)", stddev: "Standard deviation" } as const;
const GROUPS = ["Scientific", "Terrain", "Sequential", "Diverging", "Qualitative", "Gray"] as const;

// Band numbers for common composites. They assume the usual band order of the product; check it against your file.
const PRESETS: Record<string, [number, number, number]> = {
  "Natural colour (bands 1, 2, 3)": [1, 2, 3],
  "Landsat 8/9 natural colour (4, 3, 2)": [4, 3, 2],
  "Landsat 8/9 colour infrared (5, 4, 3)": [5, 4, 3],
  "Landsat 8/9 SWIR (7, 6, 4)": [7, 6, 4],
  "Landsat 8/9 agriculture (6, 5, 2)": [6, 5, 2],
  "NAIP colour infrared (4, 1, 2)": [4, 1, 2],
};

const W = 280;
const H = 70;

function Histogram({ hist, lo, hi, onChange }: { hist: RasterHistogram; lo: number; hi: number; onChange: (lo: number, hi: number) => void }) {
  const ref = useRef<SVGSVGElement>(null);
  const view = { min: hist.min, max: hist.max, width: W };
  const bars = barHeights(hist.counts);
  const bw = W / bars.length;
  const drag = (which: "min" | "max") => (e: React.PointerEvent) => {
    (e.target as Element).setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = ref.current!.getBoundingClientRect();
      const [a, b] = dragHandle(which, ((ev.clientX - r.left) / r.width) * W, lo, hi, view);
      onChange(a, b);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  const [x0, x1] = [valueToX(lo, view), valueToX(hi, view)];
  return (
    <svg ref={ref} viewBox={`0 0 ${W} ${H}`} width="100%" role="img" aria-label={`Histogram, stretch ${fmtValue(lo)} to ${fmtValue(hi)}`} style={{ touchAction: "none" }}>
      {bars.map((b, i) => (
        <rect key={i} x={i * bw} y={H - b * (H - 8)} width={Math.max(bw - 0.5, 0.5)} height={b * (H - 8)} fill="currentColor" opacity={0.45} />
      ))}
      <rect x={0} y={0} width={x0} height={H} fill="var(--colorNeutralBackground1, #fff)" opacity={0.6} />
      <rect x={x1} y={0} width={W - x1} height={H} fill="var(--colorNeutralBackground1, #fff)" opacity={0.6} />
      {([["min", x0], ["max", x1]] as const).map(([w, x]) => (
        <g key={w} onPointerDown={drag(w)} style={{ cursor: "ew-resize" }}>
          <rect x={x - 5} y={0} width={10} height={H} fill="transparent" />
          <line x1={x} x2={x} y1={0} y2={H} stroke="#d13438" strokeWidth={2} />
          <rect x={x - 3} y={H / 2 - 8} width={6} height={16} rx={2} fill="#d13438" />
        </g>
      ))}
    </svg>
  );
}

export function RasterSymbology({ id }: { id: string }) {
  const info = useApp((s) => displayInfo(s, id))!;
  const style = useApp((s) => s.styles[id])!;
  const r: RasterStyle = style.raster ?? { renderer: "default" };
  const set = (p: Partial<RasterStyle>) => useApp.getState().setStyle(id, { ...style, raster: { ...r, ...p } } as LayerStyle);
  const bands = info.raster?.bands ?? 1;
  const band = r.band ?? 1;
  const [ramps, setRamps] = useState<RampInfo[]>([]);
  const [hist, setHist] = useState<RasterHistogram | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [method, setMethod] = useState<keyof typeof METHODS>("quantile");
  const [nClasses, setNClasses] = useState(5);
  const [uniq, setUniq] = useState<RasterUniqueValues | null>(null);

  useEffect(() => {
    api.rasterRamps().then(setRamps).catch((e) => setErr(errorText(e)));
  }, []);
  useEffect(() => {
    if (r.renderer === "default" || r.renderer === "paletted" || r.renderer === "multiband") return;
    setHist(null);
    api.rasterHistogram(info.id, band, 64).then(setHist).catch((e) => setErr(errorText(e)));
  }, [info.id, info.version, band, r.renderer]);

  const loadClasses = async () => {
    setErr(null);
    try {
      const u = await api.rasterUniqueValues(info.id, band, 200);
      setUniq(u);
      const qual = ramps.find((x) => x.name === "Paired")?.colors ?? ["#1f78b4"];
      set({ entries: u.values.map((v, i) => ({ value: v.value, color: v.color ?? qual[i % qual.length], label: String(v.value), hidden: false })) });
    } catch (e) {
      setErr(errorText(e));
    }
  };
  const ramp = ramps.find((x) => x.name === (r.ramp ?? "viridis"));
  const sm = r.stretch?.mode ?? "minmax";
  const [lo, hi] = ((): [number, number] => {
    if (!hist) return [0, 1];
    const s = r.stretch;
    if (sm === "manual") return [s?.min ?? hist.min, s?.max ?? hist.max];
    if (sm === "percentile") return [hist.p2, hist.p98];
    if (sm === "stddev") return [Math.max(hist.min, hist.mean - (s?.sigma ?? 2) * hist.std), Math.min(hist.max, hist.mean + (s?.sigma ?? 2) * hist.std)];
    return [hist.min, hist.max];
  })();
  const classed = r.mode === "discrete" && !!r.breaks?.length;

  const classify = async () => {
    setErr(null);
    try {
      set({ mode: "discrete", breaks: await api.rasterClassify(info.id, band, method, nClasses) });
    } catch (e) {
      setErr(errorText(e));
    }
  };

  const saveStyle = async () => {
    setErr(null);
    try {
      const path = await save({ title: "Save raster style", defaultPath: `${info.name}.rasterstyle.json`, filters: [{ name: "Raster style", extensions: ["json"] }] });
      if (path) await api.saveRasterStyle(path, r);
    } catch (e) {
      setErr(errorText(e));
    }
  };
  const loadStyle = async () => {
    setErr(null);
    try {
      const path = await open({ title: "Load raster style", multiple: false, filters: [{ name: "Raster style", extensions: ["json"] }] });
      if (typeof path === "string") set(await api.loadRasterStyle(path));
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <div className="stack">
      <div className="class-row">
        <Button size="small" onClick={loadStyle}>Load style…</Button>
        <Button size="small" onClick={saveStyle} disabled={r.renderer === "default"}>Save style…</Button>
      </div>
      {err && (
        <MessageBar intent="error">
          <MessageBarBody>{err}</MessageBarBody>
        </MessageBar>
      )}
      <Field label="Render type">
        <Dropdown value={RENDERERS[r.renderer]} selectedOptions={[r.renderer]} onOptionSelect={(_, d) => set({ renderer: d.optionValue as RasterStyle["renderer"] })}>
          {Object.entries(RENDERERS).map(([k, v]) => (
            <Option key={k} value={k}>
              {v}
            </Option>
          ))}
        </Dropdown>
      </Field>
      {r.renderer === "paletted" && (
        <>
          <div className="class-row">
            <Button onClick={loadClasses}>{r.entries?.length ? "Reload classes" : "Load classes"}</Button>
          </div>
          {uniq?.sampled && (
            <MessageBar intent="warning">
              <MessageBarBody>This raster is large, so the classes come from a downsampled read: rare values may be missing. Add them by editing the value list.</MessageBarBody>
            </MessageBar>
          )}
          {uniq?.truncated && (
            <MessageBar intent="warning">
              <MessageBarBody>This band has more than 200 distinct values, so only the first 200 are listed. It may be continuous data; use pseudocolor instead.</MessageBarBody>
            </MessageBar>
          )}
          <div className="class-list">
            {(r.entries ?? []).map((e, i) => {
              const patch = (p: Partial<NonNullable<RasterStyle["entries"]>[number]>) => set({ entries: r.entries!.map((x, j) => (j === i ? { ...x, ...p } : x)) });
              return (
                <div key={i} className="class-row">
                  <Checkbox checked={!e.hidden} onChange={(_, d) => patch({ hidden: !d.checked })} aria-label={`Show class ${e.value}`} />
                  <input type="color" value={e.color} onChange={(ev) => patch({ color: ev.target.value })} aria-label={`Colour of ${e.value}`} />
                  <span style={{ width: 52 }}>{e.value}</span>
                  <Input size="small" value={e.label ?? ""} onChange={(_, d) => patch({ label: d.value })} aria-label={`Label of ${e.value}`} />
                </div>
              );
            })}
          </div>
        </>
      )}
      {r.renderer !== "default" && r.renderer !== "paletted" && (
        <>
          {r.renderer === "multiband" && (
            <>
              <Field label="Preset">
                <Dropdown placeholder="Choose a band combination" onOptionSelect={(_, d) => set({ rgbBands: PRESETS[d.optionValue!] })}>
                  {Object.keys(PRESETS)
                    .filter((k) => Math.max(...PRESETS[k]) <= bands)
                    .map((k) => (
                      <Option key={k} value={k}>
                        {k}
                      </Option>
                    ))}
                </Dropdown>
              </Field>
              <div className="class-row">
                {(["Red", "Green", "Blue"] as const).map((c, k) => (
                  <Field key={c} label={c}>
                    <Dropdown
                      value={`Band ${(r.rgbBands ?? [1, 2, 3])[k]}`}
                      selectedOptions={[String((r.rgbBands ?? [1, 2, 3])[k])]}
                      onOptionSelect={(_, d) => {
                        const b = [...(r.rgbBands ?? [1, 2, 3])] as [number, number, number];
                        b[k] = Number(d.optionValue);
                        set({ rgbBands: b });
                      }}
                    >
                      {Array.from({ length: bands }, (_, i) => (
                        <Option key={i} value={String(i + 1)}>{`Band ${i + 1}`}</Option>
                      ))}
                    </Dropdown>
                  </Field>
                ))}
              </div>
              <div className="props-hint">Each channel is stretched with its own band statistics.</div>
            </>
          )}
          {r.renderer !== "multiband" && bands > 1 && (
            <Field label="Band">
              <Dropdown value={`Band ${band}`} selectedOptions={[String(band)]} onOptionSelect={(_, d) => set({ band: Number(d.optionValue) })}>
                {Array.from({ length: bands }, (_, i) => (
                  <Option key={i} value={String(i + 1)}>{`Band ${i + 1}`}</Option>
                ))}
              </Dropdown>
            </Field>
          )}
          <Field label="Stretch">
            <Dropdown value={STRETCH[sm]} selectedOptions={[sm]} onOptionSelect={(_, d) => set({ stretch: { ...r.stretch, mode: d.optionValue as keyof typeof STRETCH } })}>
              {Object.entries(STRETCH).filter(([k]) => r.renderer !== "multiband" || k !== "manual").map(([k, v]) => (
                <Option key={k} value={k}>
                  {v}
                </Option>
              ))}
            </Dropdown>
          </Field>
          {hist && r.renderer !== "multiband" && (
            <>
              <Histogram hist={hist} lo={lo} hi={hi} onChange={(a, b) => set({ stretch: { ...r.stretch, mode: "manual", min: a, max: b } })} />
              <div className="class-row">
                <span>
                  Min <Input size="small" style={{ width: 90 }} value={fmtValue(lo)} onChange={(_, d) => Number.isFinite(Number(d.value)) && d.value !== "" && Number(d.value) < hi && set({ stretch: { ...r.stretch, mode: "manual", min: Number(d.value), max: hi } })} />
                </span>
                <span>
                  Max <Input size="small" style={{ width: 90 }} value={fmtValue(hi)} onChange={(_, d) => Number.isFinite(Number(d.value)) && d.value !== "" && Number(d.value) > lo && set({ stretch: { ...r.stretch, mode: "manual", min: lo, max: Number(d.value) } })} />
                </span>
              </div>
            </>
          )}
          {sm === "percentile" && (
            <div className="class-row">
              <SpinButton value={r.stretch?.pctLow ?? 2} min={0} max={49} onChange={(_, d) => d.value != null && set({ stretch: { ...r.stretch, mode: "percentile", pctLow: d.value } })} />
              <SpinButton value={r.stretch?.pctHigh ?? 98} min={51} max={100} onChange={(_, d) => d.value != null && set({ stretch: { ...r.stretch, mode: "percentile", pctHigh: d.value } })} />
            </div>
          )}
          {sm === "stddev" && (
            <Field label="Standard deviations">
              <SpinButton value={r.stretch?.sigma ?? 2} min={0.5} max={5} step={0.5} onChange={(_, d) => d.value != null && set({ stretch: { ...r.stretch, mode: "stddev", sigma: d.value } })} />
            </Field>
          )}
        </>
      )}
      {r.renderer === "pseudocolor" && (
        <>
          <Field label="Colour ramp">
            <Dropdown
              value={r.ramp ?? "viridis"}
              selectedOptions={[r.ramp ?? "viridis"]}
              onOptionSelect={(_, d) => set({ ramp: d.optionValue })}
              button={ramp ? <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}><span style={{ width: 80, height: 12, background: rampCss(ramp.colors, r.invert), border: "1px solid #888" }} />{ramp.name}</span> : undefined}
            >
              {GROUPS.map((g) => {
                const items = ramps.filter((x) => x.group === g);
                return items.length ? (
                  <OptionGroup key={g} label={g}>
                    {items.map((x) => (
                      <Option key={x.name} value={x.name} text={`${x.name}${x.cbSafe ? " ✓" : ""}`}>
                        <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                          <span style={{ width: 90, height: 12, background: rampCss(x.colors), border: "1px solid #888" }} />
                          {x.name}
                          {x.cbSafe ? " ✓" : ""}
                        </span>
                      </Option>
                    ))}
                  </OptionGroup>
                ) : null;
              })}
            </Dropdown>
          </Field>
          <div className="props-hint">✓ = readable with red–green colour blindness</div>
          <Checkbox label="Invert" checked={!!r.invert} onChange={(_, d) => set({ invert: !!d.checked })} />
          <Field label="Colour classes">
            <Dropdown value={classed ? "Classified" : "Continuous"} selectedOptions={[classed ? "c" : "l"]} onOptionSelect={(_, d) => (d.optionValue === "l" ? set({ mode: "linear", breaks: [] }) : void classify())}>
              <Option value="l">Continuous</Option>
              <Option value="c">Classified</Option>
            </Dropdown>
          </Field>
          {classed && (
            <>
              <div className="class-row">
                <Dropdown value={METHODS[method]} selectedOptions={[method]} onOptionSelect={(_, d) => setMethod(d.optionValue as keyof typeof METHODS)}>
                  {Object.entries(METHODS).map(([k, v]) => (
                    <Option key={k} value={k}>
                      {v}
                    </Option>
                  ))}
                </Dropdown>
                <SpinButton value={nClasses} min={2} max={64} onChange={(_, d) => d.value != null && setNClasses(d.value)} />
                <Button onClick={classify}>Classify</Button>
              </div>
              <Field label="Breaks (upper bounds, comma-separated)">
                <Input
                  key={(r.breaks ?? []).join(",")}
                  defaultValue={(r.breaks ?? []).map(fmtValue).join(", ")}
                  onBlur={(e) => {
                    const v = e.target.value.split(",").map((x) => Number(x.trim()));
                    if (v.length && v.every(Number.isFinite) && v.every((x, i) => i === 0 || x > v[i - 1])) set({ breaks: v });
                    else setErr("Class breaks must be numbers in increasing order.");
                  }}
                />
              </Field>
            </>
          )}
        </>
      )}
      <Field label={`Opacity ${Math.round((r.opacity ?? 1) * 100)}%`}>
        <Slider min={0} max={100} value={Math.round((r.opacity ?? 1) * 100)} onChange={(_, d) => set({ opacity: d.value / 100 })} />
      </Field>
      {r.renderer !== "default" && (
        <>
          <Field label="Resampling when zoomed">
            <Dropdown value={r.resampling || (r.renderer === "pseudocolor" && r.mode === "discrete" ? "nearest" : "bilinear")} selectedOptions={[r.resampling || (r.renderer === "pseudocolor" && r.mode === "discrete" ? "nearest" : "bilinear")]} onOptionSelect={(_, d) => set({ resampling: d.optionValue as RasterStyle["resampling"] })}>
              {["nearest", "bilinear", "cubic"].map((k) => (
                <Option key={k} value={k}>
                  {k}
                </Option>
              ))}
            </Dropdown>
          </Field>
          {(r.renderer === "gray" || r.renderer === "pseudocolor") && (
            <>
              <Checkbox label="Relief shading (for elevation data)" checked={!!r.shading} onChange={(_, d) => set({ shading: d.checked ? { mode: "hillshade" } : undefined })} />
              {r.shading && (
                <>
                  <Field label="Shading">
                    <Dropdown
                      value={{ hillshade: "Hillshade", multidirectional: "Multidirectional hillshade", slope: "Slope (darker = steeper)" }[r.shading.mode]}
                      selectedOptions={[r.shading.mode]}
                      onOptionSelect={(_, d) => set({ shading: { ...r.shading!, mode: d.optionValue as NonNullable<RasterStyle["shading"]>["mode"] } })}
                    >
                      <Option value="hillshade">Hillshade</Option>
                      <Option value="multidirectional">Multidirectional hillshade</Option>
                      <Option value="slope">Slope (darker = steeper)</Option>
                    </Dropdown>
                  </Field>
                  {(
                    [
                      ["azimuth", "Sun azimuth °", 0, 360, 5, 315],
                      ["altitude", "Sun altitude °", 5, 90, 5, 45],
                      ["zFactor", "Vertical exaggeration ×", 0.1, 10, 0.1, 1],
                      ["strength", "Shading strength", 0, 1, 0.05, 0.6],
                    ] as const
                  )
                    .filter(([k]) => r.shading!.mode === "hillshade" || (r.shading!.mode === "multidirectional" && k !== "azimuth") || k === "zFactor" || k === "strength")
                    .map(([k, label, mn, mx, st, def]) => (
                      <Field key={k} label={`${label} ${(r.shading![k] ?? def).toFixed(2)}`}>
                        <Slider min={mn} max={mx} step={st} value={r.shading![k] ?? def} onChange={(_, d) => set({ shading: { ...r.shading!, [k]: d.value } })} />
                      </Field>
                    ))}
                  <div className="props-hint">Heights are read in the raster's own vertical unit; set the exaggeration if it is not metres.</div>
                </>
              )}
            </>
          )}
          <Checkbox label="NoData is transparent" checked={r.nodataTransparent ?? true} onChange={(_, d) => set({ nodataTransparent: !!d.checked })} />
          <Field label="Transparent value range (min, max)">
            <div className="class-row">
              {([0, 1] as const).map((k) => (
                <Input
                  key={k}
                  size="small"
                  style={{ width: 90 }}
                  placeholder={k ? "max" : "min"}
                  defaultValue={r.transparentRanges?.[0]?.[k] != null ? String(r.transparentRanges[0][k]) : ""}
                  onBlur={(e) => {
                    const cur: [number, number] = r.transparentRanges?.[0] ?? [NaN, NaN];
                    cur[k] = e.target.value === "" ? NaN : Number(e.target.value);
                    set({ transparentRanges: Number.isFinite(cur[0]) && Number.isFinite(cur[1]) ? [cur] : [] });
                  }}
                />
              ))}
            </div>
          </Field>
          {(
            [
              ["brightness", "Brightness", -1, 1, 0.05, 0],
              ["contrast", "Contrast", -1, 1, 0.05, 0],
              ["gamma", "Gamma", 0.2, 3, 0.05, 1],
              ["saturation", "Saturation", -1, 1, 0.05, 0],
            ] as const
          ).map(([k, label, mn, mx, st, def]) => (
            <Field key={k} label={`${label} ${(r[k] ?? def).toFixed(2)}`}>
              <Slider min={mn} max={mx} step={st} value={r[k] ?? def} onChange={(_, d) => set({ [k]: d.value })} />
            </Field>
          ))}
          <Button onClick={() => set({ brightness: 0, contrast: 0, gamma: 1, saturation: 0 })}>Reset adjustments</Button>
        </>
      )}
    </div>
  );
}
