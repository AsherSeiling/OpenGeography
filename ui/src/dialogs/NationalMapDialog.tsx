// Data → National Map: download USGS hydrography, transportation, structures,
// geographic names and federal lands for the map view or a layer's area,
// clipped to that area.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent, type TnmDataset } from "../ipc/api";
import { getMap, zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress, WaitBar } from "../progress/progress";
import { useApp } from "../store/store";

const DEFAULT_PICK = ["nhd_flowlines", "nhd_waterbodies", "roads_major", "gn_places"];
/** Datasets that get very large over big areas. */
const DENSE = new Set(["nhd_flowlines", "roads_local", "nhd_points"]);

export function NationalMapDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [datasets, setDatasets] = useState<TnmDataset[]>([]);
  const [area, setArea] = useState("view");
  const [clip, setClip] = useState(true);
  const [pick, setPick] = useState<string[]>(DEFAULT_PICK);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.tnmDatasets().then(setDatasets).catch((e) => setErr(errorText(e)));
  }, []);

  // Area: the map view, a layer's extent, or a polygon layer's shape.
  const [mode, layerId] = area.split(":");
  const layer = layers.find((l) => l.id === layerId);
  const extent: Extent | null = (() => {
    if (area === "view") {
      const b = getMap()?.getBounds();
      return b ? [b.getWest(), b.getSouth(), b.getEast(), b.getNorth()] : null;
    }
    return layer?.extent ?? null;
  })();
  const areaLabel = area === "view" ? "map view" : layer?.name ?? "";
  const areaText = area === "view" ? "Current map view" : mode === "shape" ? `Shape of ${layer?.name}` : `Extent of ${layer?.name}`;
  const km2 = extent ? ((extent[2] - extent[0]) * 111.32 * Math.cos((((extent[1] + extent[3]) / 2) * Math.PI) / 180)) * ((extent[3] - extent[1]) * 110.95) : 0;
  const tooBig = km2 > 2500 && pick.some((id) => DENSE.has(id));

  const groups = [...new Set(datasets.map((d) => d.group))];
  const toggle = (id: string, on: boolean) => setPick((p) => (on ? [...p, id] : p.filter((x) => x !== id)));

  const download = async () => {
    if (!extent) return;
    setBusy(true);
    setErr(null);
    try {
      const ordered = datasets.filter((d) => pick.includes(d.id)).map((d) => d.id);
      const { layers: out, empty, failed } = await api.tnmImport(ordered, extent, mode === "shape" ? layerId : null, clip, areaLabel);
      const app = useApp.getState();
      app.addLayers(out);
      app.setStatus(
        `Added ${out.length} National Map layer(s) for ${areaLabel} · ${out.reduce((n, l) => n + l.featureCount, 0).toLocaleString()} features` +
          (empty.length ? ` · none in this area: ${empty.join(", ")}` : ""),
      );
      if (area !== "view") zoomToExtent(extent);
      if (failed.length) {
        // Keep the dialog open so the failures are seen; the rest were added.
        setErr(`Added ${out.length} layer(s), but these failed — try them again later:\n${failed.join("\n")}`);
        setPick((p) => p.filter((id) => failed.some((f) => f.startsWith(datasets.find((d) => d.id === id)?.label ?? "\0"))));
      } else {
        onClose();
      }
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("National Map download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 620 }}>
        <DialogBody>
          <DialogTitle>The National Map — USGS vector data</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Hydrography (NHD and watersheds), transportation, structures, geographic names (GNIS) and federal lands from USGS The National Map, clipped to your area. Public domain.</div>
            <Field label="Area" hint={extent ? `About ${Math.round(km2).toLocaleString()} km²` : undefined}>
              <Dropdown value={areaText} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={`e${l.id}`} value={`extent:${l.id}`}>{`Extent of ${l.name}`}</Option>
                ))}
                {layers
                  .filter((l) => l.geomType === "Polygon")
                  .map((l) => (
                    <Option key={`s${l.id}`} value={`shape:${l.id}`}>{`Shape of ${l.name}`}</Option>
                  ))}
              </Dropdown>
            </Field>
            <Checkbox checked={clip} onChange={(_, d) => setClip(!!d.checked)} label={mode === "shape" ? "Clip features to the layer's shape" : "Clip features to the area (otherwise whole features that touch it)"} />
            {datasets.length === 0 && !err && <WaitBar label="Loading the National Map catalog…" />}
            <div className="tnm-groups">
              {groups.map((g) => (
                <div key={g} className="tnm-group">
                  <div className="tnm-group-title">{g}</div>
                  {datasets
                    .filter((d) => d.group === g)
                    .map((d) => (
                      <Checkbox key={d.id} checked={pick.includes(d.id)} onChange={(_, c) => toggle(d.id, !!c.checked)} label={d.label} />
                    ))}
                </div>
              ))}
            </div>
            {tooBig && (
              <MessageBar intent="warning">
                <MessageBarBody>This is a large area for streams, local roads or water points — the download may be slow or exceed the 300,000-feature limit. Zoom in or use a smaller layer.</MessageBarBody>
              </MessageBar>
            )}
            <TaskProgress active={busy} label={`Downloading ${pick.length} layer(s)…`} match={["Downloading from The National Map"]} />
            {err && (
              <MessageBar intent="error">
                <MessageBarBody style={{ whiteSpace: "pre-line" }}>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !extent || pick.length === 0} onClick={download}>{busy ? "Downloading…" : "Download"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
