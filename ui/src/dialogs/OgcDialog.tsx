// Data → OGC API Features: USGS Water Data monitoring sites, or any OGC API – Features service the user pastes, for the map view
// or a layer's extent. Cited in Sources.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent, type OgcCollection, type OgcEntry } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress } from "../progress/progress";
import { useApp } from "../store/store";

export function OgcDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [catalog, setCatalog] = useState<OgcEntry[]>([]);
  const [source, setSource] = useState("");
  const [url, setUrl] = useState("");
  const [found, setFound] = useState<OgcCollection[] | null>(null);
  const [coll, setColl] = useState("");
  const [area, setArea] = useState("view");
  const [busy, setBusy] = useState(false);
  const [finding, setFinding] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.ogcCatalog().then((c) => { setCatalog(c); setSource((s) => s || c[0]?.id || "custom"); }).catch((e) => setErr(errorText(e)));
  }, []);
  const lyr = layers.find((l) => l.id === area);
  const extent: Extent | null = area === "view" ? getViewBounds() : lyr?.extent ?? null;
  const areaLabel = area === "view" ? "map view" : lyr?.name ?? "";
  const entry = catalog.find((c) => c.id === source);
  const ready = !!extent && (source === "custom" ? !!coll : !!entry);

  const find = async () => {
    setFinding(true);
    setErr(null);
    setFound(null);
    try {
      const c = await api.ogcCollections(url);
      setFound(c);
      setColl(c[0]?.id ?? "");
    } catch (e) {
      setErr(errorText(e));
    }
    setFinding(false);
  };
  const go = async () => {
    if (!extent) return;
    setBusy(true);
    setErr(null);
    try {
      const picked = found?.find((c) => c.id === coll);
      const spec = source === "custom" ? { api: url, collection: coll, label: picked?.title } : { entryId: source };
      const info = await api.ogcImport(spec, extent, areaLabel);
      if (!info) {
        setErr("There are no features of this dataset in this area. Try a larger area.");
      } else {
        useApp.getState().addLayers([info]);
        useApp.getState().setStatus(`Added ${info.name} · ${info.featureCount.toLocaleString()} features`);
        if (area !== "view") zoomToExtent(info.extent);
        onClose();
      }
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("Download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 620 }}>
        <DialogBody>
          <DialogTitle>OGC API – Features</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Vector data from services that follow the OGC API – Features standard, for your area, cited in Sources. Or paste the address of any such service.</div>
            <Field label="Area">
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${lyr?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            <RadioGroup value={source} onChange={(_, d) => setSource(d.value)}>
              {catalog.map((c) => (
                <Radio key={c.id} value={c.id} label={c.label} />
              ))}
              <Radio value="custom" label="A service address I paste" />
            </RadioGroup>
            {entry && <div className="muted">{entry.annotation} Source: {entry.author}. {entry.license}</div>}
            {source === "custom" && (
              <>
                <Field label="Service address">
                  <div style={{ display: "flex", gap: 8 }}>
                    <Input style={{ flex: 1 }} value={url} onChange={(_, d) => setUrl(d.value)} placeholder="https://example.gov/ogcapi" />
                    <Button disabled={!url.trim() || finding} onClick={find}>{finding ? "Looking…" : "Find collections"}</Button>
                  </div>
                </Field>
                {found && (
                  <Field label="Collection">
                    <Dropdown value={found.find((c) => c.id === coll)?.title ?? ""} onOptionSelect={(_, d) => setColl(d.optionValue!)}>
                      {found.map((c) => (
                        <Option key={c.id} value={c.id} text={c.title}>{c.title}</Option>
                      ))}
                    </Dropdown>
                  </Field>
                )}
              </>
            )}
            <TaskProgress active={busy} label="Downloading…" match={["Downloading"]} />
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !ready} onClick={go}>{busy ? "Downloading…" : "Download"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
