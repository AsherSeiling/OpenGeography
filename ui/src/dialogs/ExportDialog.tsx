// Export Features: all features (after the definition query) or just the selection.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Option, Radio, RadioGroup } from "@fluentui/react-components";
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
  if (!info || !base) return null;
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
            {base.geomType !== "Raster" && (
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
              Next you'll choose the file. The format follows the extension: GeoPackage, Shapefile, GeoJSON, FlatGeobuf, GeoParquet or CSV. The coordinate system is written with the data.
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
            <Button appearance="primary" onClick={() => { onClose(); runExport(owner, which === "selected", withGrid ? { style, precision } : null); }}>Choose file…</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
