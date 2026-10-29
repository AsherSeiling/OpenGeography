// Data → Overture Maps: buildings, places, roads, water, land use or boundaries for the map view or a layer's extent.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent, type OvertureTheme } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress } from "../progress/progress";
import { useApp } from "../store/store";

export function OvertureDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [themes, setThemes] = useState<OvertureTheme[]>([]);
  const [theme, setTheme] = useState("buildings");
  const [area, setArea] = useState("view");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.overtureThemes().then(setThemes).catch((e) => setErr(errorText(e)));
  }, []);
  const lyr = layers.find((l) => l.id === area);
  const extent: Extent | null = area === "view" ? getViewBounds() : lyr?.extent ?? null;
  const areaLabel = area === "view" ? "map view" : lyr?.name ?? "";
  const km2 = extent ? (extent[2] - extent[0]) * 111.32 * Math.cos((((extent[1] + extent[3]) / 2) * Math.PI) / 180) * ((extent[3] - extent[1]) * 110.95) : 0;

  const go = async () => {
    if (!extent) return;
    setBusy(true);
    setErr(null);
    try {
      const info = await api.overtureImport(theme, extent, areaLabel);
      useApp.getState().addLayers([info]);
      useApp.getState().setStatus(`Added ${info.name} · ${info.featureCount.toLocaleString()} features`);
      if (area !== "view") zoomToExtent(info.extent);
      onClose();
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("Overture download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 600 }}>
        <DialogBody>
          <DialogTitle>Overture Maps</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Open map data from the Overture Maps Foundation (monthly release), read for your area. It reads from large files on the internet and can take a few minutes; Cancel works. Much of it comes from OpenStreetMap (ODbL): credit it when you publish. Cited in Sources.</div>
            <Field label="Area" hint={extent ? `About ${Math.round(km2).toLocaleString()} km²` : undefined}>
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${lyr?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            <RadioGroup value={theme} onChange={(_, d) => setTheme(d.value)}>
              {themes.map((t) => (
                <Radio key={t.id} value={t.id} label={t.label} />
              ))}
            </RadioGroup>
            {km2 > 400 && theme === "buildings" && <MessageBar intent="warning"><MessageBarBody>Buildings over a large area are many features and slow to read. A city district is a good size.</MessageBarBody></MessageBar>}
            <TaskProgress active={busy} label="Downloading from Overture…" match={["Downloading Overture"]} />
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !extent} onClick={go}>{busy ? "Downloading…" : "Download"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
