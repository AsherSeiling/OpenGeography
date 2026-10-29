// Data → Imagery & DEM: search a STAC catalog (Sentinel-2, Copernicus DEM) for the map view or a layer's extent and clip one scene
// to the area straight from the cloud.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Radio, RadioGroup } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, errorText, type Extent, type StacCollection, type StacItem } from "../ipc/api";
import { getViewBounds, zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress } from "../progress/progress";
import { useApp } from "../store/store";

export function StacDialog({ onClose }: { onClose: () => void }) {
  const layers = useApp(useShallow((s) => s.order.map((id) => s.layers[id]).filter((l) => l && l.extent && l.geomType !== "Raster")));
  const [colls, setColls] = useState<StacCollection[]>([]);
  const [coll, setColl] = useState("sentinel-2-l2a");
  const [area, setArea] = useState("view");
  const [dates, setDates] = useState("");
  const [cloud, setCloud] = useState("30");
  const [items, setItems] = useState<StacItem[] | null>(null);
  const [item, setItem] = useState<string>("");
  const [asset, setAsset] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [searching, setSearching] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.stacCatalog().then((a) => setColls(a.flatMap((x) => x.collections))).catch((e) => setErr(errorText(e)));
  }, []);
  const c = colls.find((x) => x.id === coll);
  const lyr = layers.find((l) => l.id === area);
  const extent: Extent | null = area === "view" ? getViewBounds() : lyr?.extent ?? null;
  const areaLabel = area === "view" ? "map view" : lyr?.name ?? "";
  const picked = items?.find((i) => i.id === item);
  const assets = picked && c ? c.assets.filter(([k]) => picked.assets.some((a) => a.key === k)) : [];

  const search = async () => {
    if (!extent) return;
    setSearching(true);
    setErr(null);
    setItems(null);
    try {
      const r = await api.stacSearch({ collection: coll, bbox: extent, datetime: dates.trim() || undefined, maxCloud: c?.cloud && cloud.trim() ? Number(cloud) : undefined, limit: 20 });
      setItems(r);
      setItem(r[0]?.id ?? "");
      setAsset(c?.assets.find(([k]) => r[0]?.assets.some((a) => a.key === k))?.[0] ?? "");
      if (r.length === 0) setErr("No scenes match. Try a longer date range, a higher cloud limit or a different area.");
    } catch (e) {
      setErr(errorText(e));
    }
    setSearching(false);
  };

  const go = async () => {
    if (!extent || !picked) return;
    const a = picked.assets.find((x) => x.key === asset);
    if (!a) return;
    setBusy(true);
    setErr(null);
    try {
      const label = `${c?.label.split(" (")[0] ?? coll} — ${picked.id} (${assets.find(([k]) => k === asset)?.[1] ?? asset})`;
      const info = await api.stacImport({ href: a.href, bbox: extent, label, collection: coll, itemId: picked.id, asset, areaLabel });
      useApp.getState().addLayers([info]);
      useApp.getState().setStatus(`Added ${info.name}`);
      if (area !== "view") zoomToExtent(info.extent);
      onClose();
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("Download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 660 }}>
        <DialogBody>
          <DialogTitle>Imagery and elevation (STAC)</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Search public Sentinel-2 imagery and the Copernicus DEM, then clip one scene to your area straight from the cloud. Each is cited in Sources.</div>
            <Field label="Area">
              <Dropdown value={area === "view" ? "Current map view" : `Extent of ${lyr?.name ?? ""}`} onOptionSelect={(_, d) => setArea(d.optionValue!)}>
                <Option value="view">Current map view</Option>
                {layers.map((l) => (
                  <Option key={l.id} value={l.id}>{`Extent of ${l.name}`}</Option>
                ))}
              </Dropdown>
            </Field>
            <Field label="Data">
              <Dropdown value={c?.label ?? coll} onOptionSelect={(_, d) => { setColl(d.optionValue!); setItems(null); }}>
                {colls.map((x) => (
                  <Option key={x.id} value={x.id} text={x.label}>{x.label}</Option>
                ))}
              </Dropdown>
            </Field>
            <div style={{ display: "flex", gap: 8 }}>
              <Field label="Dates (optional)" hint="2024-06-01/2024-09-30" style={{ flex: 1 }}>
                <Input value={dates} onChange={(_, d) => setDates(d.value)} placeholder="from/to" />
              </Field>
              {c?.cloud && (
                <Field label="Cloud up to (%)" style={{ width: 130 }}>
                  <Input value={cloud} onChange={(_, d) => setCloud(d.value)} />
                </Field>
              )}
            </div>
            <Button onClick={search} disabled={!extent || searching || busy}>{searching ? "Searching…" : "Search"}</Button>
            {items && items.length > 0 && (
              <>
                <Field label="Scene (newest first)">
                  <RadioGroup value={item} onChange={(_, d) => { setItem(d.value); const it = items.find((i) => i.id === d.value); setAsset(c?.assets.find(([k]) => it?.assets.some((a) => a.key === k))?.[0] ?? ""); }}>
                    {items.map((i) => (
                      <Radio key={i.id} value={i.id} label={`${i.datetime?.slice(0, 10) ?? "no date"} · ${i.id}${i.cloudCover != null ? ` · ${i.cloudCover.toFixed(0)} % cloud` : ""}`} />
                    ))}
                  </RadioGroup>
                </Field>
                <Field label="Image">
                  <Dropdown value={assets.find(([k]) => k === asset)?.[1] ?? ""} onOptionSelect={(_, d) => setAsset(d.optionValue!)}>
                    {assets.map(([k, l]) => (
                      <Option key={k} value={k}>{l}</Option>
                    ))}
                  </Dropdown>
                </Field>
              </>
            )}
            <TaskProgress active={busy} label="Downloading…" match={["Downloading"]} />
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !picked || !asset} onClick={go}>{busy ? "Downloading…" : "Download"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
