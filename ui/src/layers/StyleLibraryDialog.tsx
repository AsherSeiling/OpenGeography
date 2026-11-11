// UX9: the personal style library. Save the style of the layer you right-clicked under a name; apply a saved style to the layers selected
// in Map Layers (or to this layer); delete one. Stored once per user in the app's data folder, never in a project.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Input, MessageBar, MessageBarBody } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText, type StyleLibraryEntry } from "../ipc/api";
import { useApp } from "../store/store";
import { applyStyleTo, isVector } from "./styleClip";

export function StyleLibraryDialog({ layer, onClose }: { layer: string; onClose: () => void }) {
  const info = useApp((s) => s.layers[layer]);
  const picked = useApp((s) => s.selectedLayers);
  const [entries, setEntries] = useState<StyleLibraryEntry[]>([]);
  const [name, setName] = useState(info?.name ?? "");
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.listStyleLibrary().then(setEntries, (e) => setErr(errorText(e)));
  }, []);
  // The layers a saved style goes onto: the vector layers selected in Map Layers, else this one.
  const targets = (picked.length > 1 && picked.includes(layer) ? picked : [layer]).filter((id) => isVector(useApp.getState().layers[id]));
  const save = async () => {
    setErr(null);
    const style = useApp.getState().styles[layer];
    if (!style || !info) return;
    try {
      setEntries(await api.saveStyleLibrary(name, info.geomType, style));
      useApp.getState().setStatus(`Saved the style of ${info.name} in the library as “${name.trim()}”.`);
    } catch (e) {
      setErr(errorText(e));
    }
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 560 }} data-testid="style-library">
        <DialogBody>
          <DialogTitle>Style library</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small">Your saved styles are kept on this computer for every project. Applying one changes only the layer's symbology and labels, never its data.</div>
            <Field label={`Save the style of “${info?.name ?? ""}” as`} size="small">
              <div className="btn-row">
                <Input size="small" aria-label="Style name" value={name} onChange={(_, d) => setName(d.value)} onKeyDown={(e) => e.key === "Enter" && void save()} style={{ flex: 1 }} />
                <Button size="small" appearance="primary" disabled={!name.trim()} onClick={() => void save()}>Save</Button>
              </div>
            </Field>
            {entries.length === 0 && <div className="muted small">No saved styles yet.</div>}
            {entries.map((e) => (
              <div key={e.id} className="btn-row" data-testid="style-entry">
                <div style={{ flex: 1 }}>
                  <span title={e.geomType} style={{ display: "inline-block", width: 12, height: 12, borderRadius: 2, marginRight: 6, background: e.style.fill, border: `1px solid ${e.style.stroke}` }} />
                  {e.name} <span className="muted small">{e.geomType}</span>
                </div>
                <Button size="small" disabled={!targets.length} onClick={() => applyStyleTo(e.name, e.style, targets)}>
                  Apply{targets.length > 1 ? ` to ${targets.length} layers` : ""}
                </Button>
                <Button size="small" onClick={() => void api.deleteStyleLibrary(e.id).then(setEntries, (x) => setErr(errorText(x)))}>Delete</Button>
              </div>
            ))}
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
