// Symbology › Topographic symbol: give a layer a symbol from the USGS sheet (or the military overlay). The engine owns the catalogue
// and draws the previews; picking merges the symbol into the layer's style (the map and layouts both draw it).
import { Button, Dropdown, Field, Option, OptionGroup, RadioGroup, Radio } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText, type TopoSymbol } from "../ipc/api";
import { displayInfo, useApp } from "../store/store";
import { clearSymbol, svgUri, symbolGroups, wearSymbol } from "./symbols";

export function TopoSymbolPicker({ id }: { id: string }) {
  const info = useApp((s) => displayInfo(s, id));
  const style = useApp((s) => s.styles[id]);
  const [era, setEra] = useState<"classic" | "modern">("classic");
  const [symbols, setSymbols] = useState<TopoSymbol[]>([]);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    api.topoSymbols(era).then((s) => live && setSymbols(s), (e) => live && setErr(errorText(e)));
    return () => {
      live = false;
    };
  }, [era]);
  if (!info || !style || info.geomType === "Raster" || info.geomType === "Table") return null;
  const groups = symbolGroups(symbols, info.geomType);
  if (!groups.length) return null;
  const preview = (s: TopoSymbol) => {
    const key = `${era}:${s.id}`;
    if (!previews[key]) {
      void api.topoSymbolSvg(s.id, era).then((svg) => setPreviews((p) => ({ ...p, [key]: svgUri(svg) })), () => {});
    }
    return previews[key];
  };
  const wearing = symbols.find((s) => s.id === style.symbol);
  return (
    <details>
      <summary>Topographic symbol (USGS sheet, military overlay)</summary>
      <div className="stack">
        <Field label="Print style">
          <RadioGroup layout="horizontal" value={era} onChange={(_, d) => setEra(d.value as "classic" | "modern")}>
            <Radio value="classic" label="Classic (1970s–80s print)" />
            <Radio value="modern" label="Modern" />
          </RadioGroup>
        </Field>
        <Field label="Symbol">
          <Dropdown
            placeholder="Choose a symbol…"
            value={wearing?.name ?? ""}
            selectedOptions={wearing ? [wearing.id] : []}
            onOptionSelect={(_, d) => {
              const s = symbols.find((x) => x.id === d.optionValue);
              if (s) useApp.getState().setStyle(id, wearSymbol(style, s));
            }}
          >
            {groups.map((g) => (
              <OptionGroup key={g.category} label={g.category}>
                {g.items.map((s) => (
                  <Option key={s.id} value={s.id} text={s.name}>
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                      {preview(s) ? <img src={preview(s)} alt="" width={42} height={30} style={{ border: "1px solid #bbb" }} /> : <span style={{ width: 42, height: 30 }} />}
                      {s.name}
                    </span>
                  </Option>
                ))}
              </OptionGroup>
            ))}
          </Dropdown>
        </Field>
        <div className="class-row">
          <Button size="small" onClick={() => useApp.getState().setStyle(id, clearSymbol(style))} disabled={!(style.symbol || style.strokes || style.pattern || style.marker)}>
            Remove symbol
          </Button>
        </div>
        <div className="props-hint">The symbol's colours, line stack, pattern and marker replace this layer's own; the map and print layouts both draw it. Tick marks, casings and dashes are strokes: use Properties › Print layout symbols to adjust single parts.</div>
        {err && <div className="field-error">{err}</div>}
      </div>
    </details>
  );
}
