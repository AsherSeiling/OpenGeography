// Export several layers at once: one GeoPackage per layer (GeoTIFF for rasters) in a folder. Existing files are never overwritten.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Input } from "@fluentui/react-components";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { runExportLayers } from "../actions";
import { IN_TAURI } from "../ipc/api";
import { useApp } from "../store/store";

export function ExportLayersDialog({ ids, onClose }: { ids: string[]; onClose: () => void }) {
  const layers = useApp((s) => s.layers);
  const names = ids.map((id) => layers[id]?.name).filter(Boolean) as string[];
  const [dir, setDir] = useState("");
  const [busy, setBusy] = useState(false);
  const go = async () => {
    setBusy(true);
    const ok = await runExportLayers(ids, dir.trim());
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Export {ids.length} layers</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small" data-testid="export-layers-names">{names.join(", ")}</div>
            <Field label="Folder" size="small">
              <div className="btn-row">
                <Input className="grow" size="small" value={dir} placeholder="Folder to write into" onChange={(_, d) => setDir(d.value)} />
                {IN_TAURI && (
                  <Button size="small" onClick={async () => { const p = await open({ directory: true, title: "Folder for the exported layers" }); if (typeof p === "string") setDir(p); }}>Browse…</Button>
                )}
              </div>
            </Field>
            <div className="muted small">One GeoPackage per layer (GeoTIFF for rasters), named after the layer. Node-tree results are exported as shown. The coordinate system is written with each file. Source files are never overwritten.</div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!dir.trim() || busy} onClick={() => void go()}>{busy ? "Exporting…" : "Export"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
