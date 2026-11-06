// Export Features: all features (after the definition query) or just the selection.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useShallow } from "zustand/react/shallow";
import { formatById, formatsFor, refusal } from "./exportFormats";
import { getViewBounds } from "../map/mapController";
import { projectLayerInfos } from "../store/maps";
import { DEFAULT_GRID, GRID_PRECISIONS, type GridChoice } from "./gridRef";
import { useState } from "react";
import { layerFromSelection, runExport } from "../actions";
import { displayInfo, useApp } from "../store/store";

export function ExportDialog({ owner, onClose }: { owner: string; onClose: () => void }) {
  const info = useApp((s) => displayInfo(s, owner));
  const base = useApp((s) => s.layers[owner]);
  const nsel = useApp((s) => s.selection[owner]?.length ?? 0);
  const [which, setWhich] = useState<"all" | "selected">(nsel > 0 ? "selected" : "all");
  const [withGrid, setWithGrid] = useState(false);
  const [style, setStyle] = useState<GridChoice["style"]>(DEFAULT_GRID.style);
  const [precision, setPrecision] = useState(DEFAULT_GRID.precision);
  // UR-15: the format, the coordinate system, the fields and an optional clip.
  const geomType = info?.geomType ?? "Point";
  const formats = formatsFor(geomType);
  const [formatId, setFormatId] = useState(formats[0].id);
  const [crsPick, setCrsPick] = useState<string>("own");
  const [crsText, setCrsText] = useState("");
  const [allFields, setAllFields] = useState(true);
  const [fieldSet, setFieldSet] = useState<string[] | null>(null);
  const [clip, setClip] = useState<string>("none");
  const mapCrs = useApp((s) => s.mapCrs);
  const polygons = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && (l.geomType === "Polygon" || l.geomType === "Mixed") && l.id !== owner)));
  if (!info || !base) return null;
  const format = formatById(geomType, formatId);
  const refused = refusal(format.id, geomType);
  const shaped = geomType !== "Raster" && geomType !== "Table";
  const fieldNames = base.fields.map((f) => f.name);
  const picked = fieldSet ?? fieldNames;
  const crsOptions: [string, string][] = [["own", `The layer's own${base.crsName ? ` (${base.crsName})` : ""}`], ["EPSG:4326", "WGS 84 longitude/latitude (EPSG:4326)"], ["EPSG:4269", "NAD83 longitude/latitude (EPSG:4269)"], ["EPSG:3857", "Web Mercator (EPSG:3857)"], ...(mapCrs ? [[mapCrs.id, `The map's (${mapCrs.name})`] as [string, string]] : []), ["other", "Another… (type its code below)"]];
  const crsValue = crsPick === "own" ? "" : crsPick === "other" ? crsText.trim() : crsPick;
  const choices = () => ({
    crs: shaped ? crsValue : "",
    fields: allFields || picked.length === fieldNames.length ? null : picked,
    clipExtent: shaped && clip === "view" ? (getViewBounds()?.join(",") ?? "") : "",
    clipLayer: shaped && clip !== "none" && clip !== "view" ? clip : "",
  });
  const total = base.definitionQuery && !info.resultOf ? (base.queryCount ?? info.featureCount) : info.featureCount;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Export Features — {base.name}</DialogTitle>
          <DialogContent className="stack">
            <RadioGroup value={which} onChange={(_, d) => setWhich(d.value as never)}>
              <Radio value="all" label={`All features (${total.toLocaleString()}${base.definitionQuery ? ", after the definition query" : ""})`} />
              <Radio value="selected" disabled={nsel === 0} label={`Selected features only (${nsel.toLocaleString()})`} />
            </RadioGroup>
            {geomType !== "Raster" && (
              <Field label="File format" size="small">
                <Dropdown size="small" aria-label="File format" value={format.label} selectedOptions={[format.id]} onOptionSelect={(_, d) => setFormatId(d.optionValue!)}>
                  {formats.map((f) => <Option key={f.id} value={f.id} disabled={!!refusal(f.id, geomType)} text={f.label}>{f.label}</Option>)}
                </Dropdown>
              </Field>
            )}
            {format.note && !refused && <div className="props-hint">{format.note}</div>}
            {refused && <div className="field-error">{refused}</div>}
            {shaped && (
              <Field label="Coordinate system of the file" size="small">
                <Dropdown size="small" aria-label="Output coordinate system" value={crsOptions.find((o) => o[0] === crsPick)![1]} selectedOptions={[crsPick]} onOptionSelect={(_, d) => setCrsPick(d.optionValue!)}>
                  {crsOptions.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
                </Dropdown>
              </Field>
            )}
            {shaped && crsPick === "other" && <Input size="small" aria-label="Other coordinate system" placeholder="EPSG:26914, or a PROJ or WKT definition" value={crsText} onChange={(_, d) => setCrsText(d.value)} />}
            {geomType !== "Raster" && fieldNames.length > 0 && (
              <Field label="Fields" size="small">
                <Checkbox label={`All fields (${fieldNames.length})`} checked={allFields} onChange={(_, d) => { setAllFields(!!d.checked); if (d.checked) setFieldSet(null); }} />
              </Field>
            )}
            {geomType !== "Raster" && !allFields && (
              <div className="classes" data-testid="export-fields" style={{ maxHeight: 150 }}>
                {fieldNames.map((n) => (
                  <Checkbox key={n} label={n} checked={picked.includes(n)} onChange={(_, d) => setFieldSet(d.checked ? fieldNames.filter((x) => x === n || picked.includes(x)) : picked.filter((x) => x !== n))} />
                ))}
              </div>
            )}
            {shaped && (
              <Field label="Clip to" size="small" hint="Cut the shapes to an area; features entirely outside are left out.">
                <Dropdown size="small" aria-label="Clip to" value={clip === "none" ? "Don't clip" : clip === "view" ? "The current map view" : `Shapes of ${polygons.find((l) => l.id === clip)?.name ?? ""}`} selectedOptions={[clip]} onOptionSelect={(_, d) => setClip(d.optionValue!)}>
                  <Option value="none">Don't clip</Option>
                  <Option value="view">The current map view</Option>
                  {polygons.map((l) => <Option key={l.id} value={l.id} text={`Shapes of ${l.name}`}>{`Shapes of ${l.name}`}</Option>)}
                </Dropdown>
              </Field>
            )}
            {base.geomType !== "Raster" && base.geomType !== "Table" && (
              <Checkbox label="Add a USNG / MGRS reference column (a point on lines and polygons)" checked={withGrid} onChange={(_, d) => setWithGrid(!!d.checked)} />
            )}
            {withGrid && (
              <div className="props-grid">
                <Field label="Written as" size="small">
                  <Dropdown size="small" aria-label="Reference style" value={style === "usng" ? "USNG (15T VG 97470 47556)" : "MGRS (15TVG9747047556)"} selectedOptions={[style]} onOptionSelect={(_, d) => setStyle(d.optionValue as GridChoice["style"])}>
                    <Option value="mgrs">MGRS (15TVG9747047556)</Option>
                    <Option value="usng">USNG (15T VG 97470 47556)</Option>
                  </Dropdown>
                </Field>
                <Field label="Precision" size="small">
                  <Dropdown size="small" aria-label="Reference precision" value={GRID_PRECISIONS.find((p) => p[0] === precision)![1]} selectedOptions={[String(precision)]} onOptionSelect={(_, d) => setPrecision(Number(d.optionValue))}>
                    {GRID_PRECISIONS.map(([v, l]) => <Option key={v} value={String(v)}>{l}</Option>)}
                  </Dropdown>
                </Field>
                <div className="props-hint">The UTM zone is chosen for each feature (the polar UPS grid beyond 84°N and 80°S) on WGS 84.</div>
              </div>
            )}
            <div className="muted small">
              Next you'll choose the file name. The coordinate system is written with the data{shaped ? "; the file's format is the one above" : ""}.
              {info.resultOf ? " This layer shows its node-tree result, so that is what gets exported." : ""}
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            {which === "selected" && (
              <Button onClick={() => { onClose(); layerFromSelection(owner); }} title="Copy the selected features into a new layer instead of a file">
                New layer instead
              </Button>
            )}
            <Button appearance="primary" disabled={!!refused || (shaped && crsPick === "other" && !crsText.trim())} onClick={() => { onClose(); runExport(owner, which === "selected", withGrid ? { style, precision } : null, geomType === "Raster" ? undefined : format.id, choices()); }}>Choose file…</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
