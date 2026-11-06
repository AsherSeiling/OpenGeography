// Data → Earthquakes & Soils: USGS earthquake points or NRCS SSURGO soil map units for the map view or a layer's extent.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, type Extent } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { startDownload } from "../downloads/background";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";

/** Longest side of the area soil map units may cover, in degrees (the engine's limit, `jsonapis::SSURGO_MAX_DEG`). */
export const SOIL_MAX_DEG = 0.25;

export function HazardsSoilsDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [what, setWhat] = useState<"quakes" | "soils">("quakes");
  const [area, setArea] = useState("view");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [mag, setMag] = useState("3");
  const lyr = layers.find((l) => l.id === area);
  const extent: Extent | null = area === "view" ? getViewBounds() : lyr?.extent ?? null;
  const areaLabel = area === "view" ? "map view" : lyr?.name ?? "";
  const tooBigForSoils = !!extent && what === "soils" && (extent[2] - extent[0] > SOIL_MAX_DEG || extent[3] - extent[1] > SOIL_MAX_DEG);

  const go = () => {
    if (!extent) return;
    startDownload({
      label: what === "quakes" ? `Earthquake download for ${areaLabel}` : `Soil download for ${areaLabel}`,
      match: ["Downloading"],
      run: () => (what === "quakes" ? api.earthquakesImport(extent, start.trim() || null, end.trim() || null, Number(mag), areaLabel) : api.ssurgoImport(extent, areaLabel)),
      done: (info) => {
        if (!info) return what === "quakes" ? "No earthquakes match. Try a longer period, a smaller smallest magnitude or a larger area." : "There are no soil map units here (SSURGO covers most of the United States; try a smaller area on land).";
        useApp.getState().addLayers([info]);
        if (area !== "view") zoomToExtent(info.extent);
        return `Added ${info.name} · ${info.featureCount.toLocaleString()} features`;
      },
    });
    onClose();
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 560 }}>
        <DialogBody>
          <DialogTitle>Earthquakes and soils</DialogTitle>
          <DialogContent className="stack">
            <Field label="Area">
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${lyr?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            <RadioGroup value={what} onChange={(_, d) => setWhat(d.value as "quakes" | "soils")}>
              <Radio value="quakes" label="Earthquakes (USGS catalog)" />
              <Radio value="soils" label="Soil map units (NRCS SSURGO)" />
            </RadioGroup>
            {what === "quakes" ? (
              <div style={{ display: "flex", gap: 8 }}>
                <Field label="From" hint="2024-01-01 (optional)" style={{ flex: 1 }}><Input value={start} onChange={(_, d) => setStart(d.value)} /></Field>
                <Field label="To" hint="optional" style={{ flex: 1 }}><Input value={end} onChange={(_, d) => setEnd(d.value)} /></Field>
                <Field label="Smallest magnitude" style={{ width: 150 }}><Input value={mag} onChange={(_, d) => setMag(d.value)} /></Field>
              </div>
            ) : (
              <div className="muted">Soil map units are very detailed, so the area can be at most {SOIL_MAX_DEG}° (about 25 km) on a side. Public domain; use for planning, not for a single lot.</div>
            )}
            {tooBigForSoils && <MessageBar intent="warning"><MessageBarBody>This area is too large for soil map units. Zoom in, or choose a smaller layer.</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!extent || tooBigForSoils} onClick={go}>Download</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
