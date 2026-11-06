// Data → National Map: download USGS hydrography, transportation, structures,
// geographic names and federal lands for the map view or a layer's area,
// clipped to that area.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent, type TnmDataset } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { WaitBar } from "../progress/progress";
import { reportError } from "../errors/store";
import { startDownload } from "../downloads/background";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";

const DEFAULT_PICK = ["nhd_flowlines", "nhd_waterbodies", "roads_major", "gn_places"];
/** Datasets that get very large over big areas. */
const DENSE = new Set(["nhd_flowlines", "roads_local", "nhd_points"]);

export function NationalMapDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [datasets, setDatasets] = useState<TnmDataset[]>([]);
  const [area, setArea] = useState("view");
  const [clip, setClip] = useState(true);
  const [pick, setPick] = useState<string[]>(DEFAULT_PICK);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.tnmDatasets().then(setDatasets).catch((e) => setErr(errorText(e)));
  }, []);

  // Area: the map view, a layer's extent, or a polygon layer's shape.
  const [mode, layerId] = area.split(":");
  const layer = layers.find((l) => l.id === layerId);
  const extent: Extent | null = (() => {
    if (area === "view") {
      return getViewBounds();
    }
    return layer?.extent ?? null;
  })();
  const areaLabel = area === "view" ? "map view" : layer?.name ?? "";
  const areaText = area === "view" ? "Current map view" : mode === "shape" ? `Shape of ${layer?.name}` : `Extent of ${layer?.name}`;
  const km2 = extent ? ((extent[2] - extent[0]) * 111.32 * Math.cos((((extent[1] + extent[3]) / 2) * Math.PI) / 180)) * ((extent[3] - extent[1]) * 110.95) : 0;
  const tooBig = km2 > 2500 && pick.some((id) => DENSE.has(id));

  const groups = [...new Set(datasets.map((d) => d.group))];
  const toggle = (id: string, on: boolean) => setPick((p) => (on ? [...p, id] : p.filter((x) => x !== id)));

  const download = () => {
    if (!extent) return;
    const ordered = datasets.filter((d) => pick.includes(d.id)).map((d) => d.id);
    startDownload({
      label: `National Map download for ${areaLabel}`,
      match: ["Downloading from The National Map"],
      run: () => api.tnmImport(ordered, extent, mode === "shape" ? layerId : null, clip, areaLabel),
      done: ({ layers: out, empty, failed }) => {
        useApp.getState().addLayers(out);
        if (area !== "view") zoomToExtent(extent);
        const failedText = failed.length ? ` · these failed, try them again later: ${failed.join("; ")}` : "";
        if (failed.length) reportError(`Added ${out.length} layer(s), but these failed — try them again later:\n${failed.join("\n")}`, "download:National Map", { title: "Part of the National Map download failed", context: "downloading from The National Map" });
        return `Added ${out.length} National Map layer(s) for ${areaLabel} · ${out.reduce((n, l) => n + l.featureCount, 0).toLocaleString()} features` + (empty.length ? ` · none in this area: ${empty.join(", ")}` : "") + failedText;
      },
    });
    onClose();
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
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
            {err && (
              <MessageBar intent="error">
                <MessageBarBody style={{ whiteSpace: "pre-line" }}>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!extent || pick.length === 0} onClick={download}>Download</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
