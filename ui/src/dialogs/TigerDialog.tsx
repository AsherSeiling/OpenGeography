// Data → Sample Data (TIGER): download Census boundaries, roads and water
// for a state or county straight from TIGERweb.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText, type TigerDataset, type TigerVintage } from "../ipc/api";
import { zoomToExtent } from "../map/mapController";
import { isCancelled, TaskProgress, WaitBar } from "../progress/progress";
import { useApp } from "../store/store";

export const STATES: [string, string, string][] = [
  ["01", "Alabama", "AL"], ["02", "Alaska", "AK"], ["04", "Arizona", "AZ"], ["05", "Arkansas", "AR"], ["06", "California", "CA"],
  ["08", "Colorado", "CO"], ["09", "Connecticut", "CT"], ["10", "Delaware", "DE"], ["11", "District of Columbia", "DC"], ["12", "Florida", "FL"],
  ["13", "Georgia", "GA"], ["15", "Hawaii", "HI"], ["16", "Idaho", "ID"], ["17", "Illinois", "IL"], ["18", "Indiana", "IN"],
  ["19", "Iowa", "IA"], ["20", "Kansas", "KS"], ["21", "Kentucky", "KY"], ["22", "Louisiana", "LA"], ["23", "Maine", "ME"],
  ["24", "Maryland", "MD"], ["25", "Massachusetts", "MA"], ["26", "Michigan", "MI"], ["27", "Minnesota", "MN"], ["28", "Mississippi", "MS"],
  ["29", "Missouri", "MO"], ["30", "Montana", "MT"], ["31", "Nebraska", "NE"], ["32", "Nevada", "NV"], ["33", "New Hampshire", "NH"],
  ["34", "New Jersey", "NJ"], ["35", "New Mexico", "NM"], ["36", "New York", "NY"], ["37", "North Carolina", "NC"], ["38", "North Dakota", "ND"],
  ["39", "Ohio", "OH"], ["40", "Oklahoma", "OK"], ["41", "Oregon", "OR"], ["42", "Pennsylvania", "PA"], ["44", "Rhode Island", "RI"],
  ["45", "South Carolina", "SC"], ["46", "South Dakota", "SD"], ["47", "Tennessee", "TN"], ["48", "Texas", "TX"], ["49", "Utah", "UT"],
  ["50", "Vermont", "VT"], ["51", "Virginia", "VA"], ["53", "Washington", "WA"], ["54", "West Virginia", "WV"], ["55", "Wisconsin", "WI"],
  ["56", "Wyoming", "WY"], ["72", "Puerto Rico", "PR"],
];

const DEFAULT_PICK = ["counties", "tracts", "primary_roads"];

export function TigerDialog({ onClose }: { onClose: () => void }) {
  const [datasets, setDatasets] = useState<TigerDataset[]>([]);
  const [vintages, setVintages] = useState<TigerVintage[]>([]);
  // The 2020 Census: decennial geography with blocks and population counts.
  const [vintage, setVintage] = useState("Census2020");
  const [newestFirst, setNewestFirst] = useState(true);
  const [available, setAvailable] = useState<Record<string, boolean> | null>(null);
  const [state, setState] = useState("40");
  const [counties, setCounties] = useState<[string, string][]>([]);
  const [county, setCounty] = useState<string>("");
  const [pick, setPick] = useState<string[]>(DEFAULT_PICK);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    api.tigerDatasets().then(setDatasets).catch((e) => setErr(errorText(e)));
    api.tigerVintages().then(setVintages).catch((e) => setErr(errorText(e)));
  }, []);
  useEffect(() => {
    setAvailable(null);
    api.tigerAvailability(vintage).then((a) => setAvailable(Object.fromEntries(a))).catch((e) => setErr(errorText(e)));
  }, [vintage]);
  useEffect(() => {
    setCounties([]);
    api.tigerCounties(state, vintage)
      .then((cs) => {
        setCounties(cs);
        // Keep the county if this year still has it (counties do change).
        setCounty((c) => (cs.some(([code]) => code === c) ? c : ""));
      })
      .catch((e) => setErr(errorText(e)));
  }, [state, vintage]);
  const v = vintages.find((x) => x.id === vintage);
  const years = newestFirst ? vintages : [...vintages].reverse();
  const unavailable = (id: string) => available !== null && available[id] === false;

  const st = STATES.find(([c]) => c === state)!;
  const countyName = counties.find(([c]) => c === county)?.[1];
  const label = countyName ? `${countyName}, ${st[2]}` : st[1];
  // Ticked datasets this year doesn't have are skipped, not blocking.
  const chosen = pick.filter((id) => !unavailable(id));
  const blocked = chosen.filter((id) => datasets.find((d) => d.id === id)?.needsCounty && !county);

  const download = async () => {
    setBusy(true);
    setErr(null);
    try {
      const layers = await api.tigerImport(chosen, vintage, state, county || null, label);
      const app = useApp.getState();
      app.addLayers(layers);
      app.setStatus(`Added ${layers.length} TIGER layer(s) for ${label} · ${layers.reduce((n, l) => n + l.featureCount, 0).toLocaleString()} features`);
      const ext = layers.map((l) => l.extent).filter(Boolean) as [number, number, number, number][];
      if (ext.length) zoomToExtent([Math.min(...ext.map((e) => e[0])), Math.min(...ext.map((e) => e[1])), Math.max(...ext.map((e) => e[2])), Math.max(...ext.map((e) => e[3]))]);
      onClose();
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("TIGER download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Sample Data — Census TIGER</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Boundaries, roads and water from the U.S. Census Bureau (TIGERweb). Public domain, free to use. Downloads are cached.</div>
            <Field
              label="Year"
              hint={
                v?.decennial
                  ? "Decennial Census geography: includes blocks, and tracts/block groups/blocks carry population (POP100) and housing units (HU100)."
                  : v
                    ? "ACS-year geography (boundaries as of that year). Blocks exist only in decennial Census years."
                    : undefined
              }
            >
              <div className="tiger-year">
                <Dropdown value={v?.label ?? vintage} onOptionSelect={(_, d) => setVintage(d.optionValue!)} disabled={!vintages.length}>
                  {years.map((y) => (
                    <Option key={y.id} value={y.id} text={y.label}>
                      {y.decennial ? <b>{y.label}</b> : y.label}
                    </Option>
                  ))}
                </Dropdown>
                <Button size="small" appearance="subtle" onClick={() => setNewestFirst((n) => !n)} title="Change the sort order of the years">
                  {newestFirst ? "Newest first ↓" : "Oldest first ↑"}
                </Button>
              </div>
            </Field>
            <div className="row2">
              <Field label="State">
                <Dropdown value={st[1]} onOptionSelect={(_, d) => setState(d.optionValue!)}>
                  {STATES.map(([c, n]) => (
                    <Option key={c} value={c}>{n}</Option>
                  ))}
                </Dropdown>
              </Field>
              <Field label="County (optional)">
                <Dropdown value={countyName ?? "Whole state"} onOptionSelect={(_, d) => setCounty(d.optionValue ?? "")} disabled={!counties.length}>
                  <Option value="">Whole state</Option>
                  {counties.map(([c, n]) => (
                    <Option key={c} value={c}>{n}</Option>
                  ))}
                </Dropdown>
              </Field>
            </div>
            <Field label="Layers">
              <div className="tiger-list">
                {datasets.length === 0 && <WaitBar label="Contacting TIGERweb…" />}
                {datasets.map((d) => (
                  <Checkbox
                    key={d.id}
                    label={`${d.label}${d.needsCounty ? " (county only)" : ""}${!d.byYear ? " — current data" : ""}${unavailable(d.id) ? " — not in this year" : ""}`}
                    checked={pick.includes(d.id) && !unavailable(d.id)}
                    disabled={(d.needsCounty && !county) || unavailable(d.id)}
                    title={unavailable(d.id) ? "This year's TIGER geography doesn't include it (blocks exist only in decennial Census years and Current)." : !d.byYear ? "TIGERweb publishes roads and water only as current data." : undefined}
                    onChange={(_, v) => setPick((p) => (v.checked ? [...p, d.id] : p.filter((x) => x !== d.id)))}
                  />
                ))}
              </div>
            </Field>
            <TaskProgress active={busy} label={`Downloading ${chosen.length} layer(s) for ${label}…`} match={["Downloading Census"]} />
            {err && (
              <MessageBar intent="error">
                <MessageBarBody>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={busy || !chosen.length || blocked.length > 0} onClick={download}>
              {busy ? "Downloading…" : `Add ${chosen.length} layer(s)`}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
