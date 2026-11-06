// Data → ArcGIS Services: one dataset from the bundled catalog (PAD-US, FEMA flood zones, wetlands, wildfire perimeters …) or any
// ArcGIS MapServer / FeatureServer layer the user pastes, for the map view or a layer's extent. Each is cited in Sources.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type ArcgisEntry, type ArcgisServiceLayer, type Extent } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { WaitBar } from "../progress/progress";
import { startDownload } from "../downloads/background";
import { useApp } from "../store/store";
import { projectLayerInfos } from "../store/maps";

export function ArcGisDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => projectLayerInfos(s).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [catalog, setCatalog] = useState<ArcgisEntry[]>([]);
  const [source, setSource] = useState<string>("padus"); // a catalog id, or "custom"
  const [url, setUrl] = useState("");
  const [found, setFound] = useState<ArcgisServiceLayer[] | null>(null);
  const [layer, setLayer] = useState<number | null>(null);
  const [finding, setFinding] = useState(false);
  const [area, setArea] = useState("view");
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.arcgisCatalog().then(setCatalog).catch((e) => setErr(errorText(e)));
  }, []);

  const lyr = layers.find((l) => l.id === area);
  const extent: Extent | null = area === "view" ? getViewBounds() : lyr?.extent ?? null;
  const areaLabel = area === "view" ? "map view" : lyr?.name ?? "";
  const groups = [...new Set(catalog.map((c) => c.group))];
  const entry = catalog.find((c) => c.id === source);

  const find = async () => {
    setFinding(true);
    setErr(null);
    setFound(null);
    setLayer(null);
    try {
      const ls = await api.arcgisLayers(url);
      setFound(ls);
      setLayer(ls[0]?.id ?? null);
    } catch (e) {
      setErr(errorText(e));
    }
    setFinding(false);
  };

  const ready = !!extent && (source === "custom" ? layer !== null : !!entry);
  const go = () => {
    if (!extent) return;
    const picked = found?.find((l) => l.id === layer);
    const spec = source === "custom" ? { url, layer: String(layer), label: picked ? picked.name : undefined } : { entryId: source };
    startDownload({
      label: `ArcGIS download for ${areaLabel}`,
      match: ["Downloading"],
      run: () => api.arcgisImport(spec, extent, areaLabel),
      done: (out) => {
        if (out.length === 0) return "There are no features of this ArcGIS dataset in this area. Try a larger area.";
        useApp.getState().addLayers(out);
        if (area !== "view") zoomToExtent(extent);
        return `Added ${out[0].name} · ${out.reduce((n, l) => n + l.featureCount, 0).toLocaleString()} features for ${areaLabel}`;
      },
    });
    onClose();
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 640 }}>
        <DialogBody>
          <DialogTitle>ArcGIS services</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Public datasets served by ArcGIS REST, for your area, cited in Sources. Or paste the address of any MapServer or FeatureServer.</div>
            <Field label="Area">
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${lyr?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            {catalog.length === 0 && !err && <WaitBar label="Loading the catalog…" />}
            <RadioGroup value={source} onChange={(_, d) => setSource(d.value)}>
              {groups.map((g) => (
                <div key={g}>
                  <div className="tnm-group-title">{g}</div>
                  {catalog.filter((c) => c.group === g).map((c) => (
                    <Radio key={c.id} value={c.id} label={c.label} />
                  ))}
                </div>
              ))}
              <div className="tnm-group-title">Other</div>
              <Radio value="custom" label="A service address I paste" />
            </RadioGroup>
            {entry && <div className="muted">{entry.annotation} Source: {entry.author}. {entry.license}</div>}
            {source === "custom" && (
              <>
                <Field label="Service address (…/MapServer or …/FeatureServer)">
                  <div style={{ display: "flex", gap: 8 }}>
                    <Input style={{ flex: 1 }} value={url} onChange={(_, d) => setUrl(d.value)} placeholder="https://services.arcgis.com/…/FeatureServer" />
                    <Button disabled={!url.trim() || finding} onClick={find}>{finding ? "Looking…" : "Find layers"}</Button>
                  </div>
                </Field>
                {found && (
                  <Field label="Layer">
                    <Dropdown value={found.find((l) => l.id === layer)?.name ?? ""} onOptionSelect={(_, d) => setLayer(Number(d.optionValue))}>
                      {found.map((l) => (
                        <Option key={l.id} value={String(l.id)} text={l.name}>{`${l.name}${l.geometryType ? ` (${l.geometryType})` : ""}`}</Option>
                      ))}
                    </Dropdown>
                  </Field>
                )}
              </>
            )}
            {err && (
              <MessageBar intent="error">
                <MessageBarBody style={{ whiteSpace: "pre-line" }}>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!ready} onClick={go}>Download</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
