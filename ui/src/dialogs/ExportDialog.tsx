// Export Features: all features (after the definition query) or just the selection.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Radio, RadioGroup } from "@fluentui/react-components";
import { useState } from "react";
import { layerFromSelection, runExport } from "../actions";
import { displayInfo, useApp } from "../store/store";

export function ExportDialog({ owner, onClose }: { owner: string; onClose: () => void }) {
  const info = useApp((s) => displayInfo(s, owner));
  const base = useApp((s) => s.layers[owner]);
  const nsel = useApp((s) => s.selection[owner]?.length ?? 0);
  const [which, setWhich] = useState<"all" | "selected">(nsel > 0 ? "selected" : "all");
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
            <Button appearance="primary" onClick={() => { onClose(); runExport(owner, which === "selected"); }}>Choose file…</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
