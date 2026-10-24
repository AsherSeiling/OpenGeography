// Data → Elevation (3DEP): download USGS elevation for the map view or a
// layer's extent, clipped to that area, in the local UTM zone (meters).
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress } from "../progress/progress";
import { useApp } from "../store/store";

const RES: [number, string][] = [
  [1, "1 m (lidar areas; small areas only)"],
  [3, "3 m"],
  [10, "10 m (1/3 arc-second, nationwide)"],
  [30, "30 m (1 arc-second)"],
];

export function ElevationDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [area, setArea] = useState<string>("view");
  const [res, setRes] = useState(10);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const extent = (): [Extent, string] | null => {
    if (area === "view") {
      const b = getViewBounds();
      return b ? [b, "map view"] : null;
    }
    const l = layers.find((x) => x.id === area);
    return l?.extent ? [l.extent, l.name] : null;
  };
  const e = extent();
  const km = e ? Math.round((e[0][2] - e[0][0]) * 111 * Math.cos((((e[0][1] + e[0][3]) / 2) * Math.PI) / 180)) : 0;
  const cells = e ? ((km * 1000) / res) * (((e[0][3] - e[0][1]) * 111000) / res) : 0;

  const go = async () => {
    if (!e) return;
    setBusy(true);
    setErr(null);
    try {
      const info = await api.downloadElevation(e[0], res, e[1]);
      useApp.getState().addLayers([info]);
      useApp.getState().setStatus(`Added ${info.name} · ${info.raster?.min.toFixed(0)}–${info.raster?.max.toFixed(0)} m`);
      zoomToExtent(info.extent);
      onClose();
    } catch (x) {
      if (isCancelled(x)) useApp.getState().setStatus("Elevation download cancelled");
      else setErr(errorText(x));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Elevation — USGS 3DEP</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Best available 3D Elevation Program data from The National Map, clipped to the area, in meters (NAVD88), projected to the local UTM zone. Public domain.</div>
            <Field label="Area">
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${layers.find((l) => l.id === area)?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            <Field label="Resolution" hint={e ? `About ${km} km wide · ${Math.round(cells / 1e6) || "<1"} million cells (large areas use a coarser resolution automatically)` : undefined}>
              <Dropdown value={RES.find(([r]) => r === res)![1]} onOptionSelect={(_, d) => setRes(Number(d.optionValue))}>
                {RES.map(([r, l]) => (
                  <Option key={r} value={String(r)}>{l}</Option>
                ))}
              </Dropdown>
            </Field>
            <TaskProgress active={busy} label="Downloading elevation…" match={["Downloading elevation"]} />
            {err && (
              <MessageBar intent="error">
                <MessageBarBody>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !e} onClick={go}>{busy ? "Downloading…" : "Download"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
