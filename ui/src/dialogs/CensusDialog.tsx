// Data → Census Data: join American Community Survey or 2020 Census values to a layer by GEOID.
// The optional API key is kept in the operating system's keychain (never shown again, never in the project).
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option, Textarea } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { guessGeoidField, parseVariables, PRESETS, vintages } from "../census/presets";
import { api, errorText, type CensusDataset } from "../ipc/api";
import { isCancelled, TaskProgress } from "../progress/progress";
import { displayInfo, useApp } from "../store/store";

export function CensusDialog({ layer, onClose }: { layer: string; onClose: () => void }) {
  const info = useApp((s) => displayInfo(s, layer));
  const fields = (info?.fields ?? []).filter((f) => !f.name.startsWith("__"));
  const [datasets, setDatasets] = useState<CensusDataset[]>([]);
  const [dataset, setDataset] = useState("acs5");
  const [vintage, setVintage] = useState(vintages("acs5")[0]);
  const [geoid, setGeoid] = useState<string>(guessGeoidField(fields) ?? "");
  const [picked, setPicked] = useState<string[]>(["B01003_001E"]);
  const [custom, setCustom] = useState("");
  const [hasKey, setHasKey] = useState(false);
  const [keyText, setKeyText] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    api.censusDatasets().then(setDatasets).catch((e) => setErr(errorText(e)));
    api.censusKeyStatus().then(setHasKey).catch(() => setHasKey(false));
  }, []);

  const parsed = parseVariables(custom);
  const variables = [...new Set([...picked, ...parsed.codes])];
  const family = dataset.startsWith("dec_") ? "dhc" : "acs";
  const ok = !!info && !!geoid && variables.length > 0 && parsed.invalid.length === 0;

  const saveKey = async () => {
    setErr(null);
    try {
      await api.censusKeySet(keyText);
      setKeyText("");
      setHasKey(true);
      setNote("The key is saved in your system keychain.");
    } catch (e) {
      setErr(errorText(e));
    }
  };
  const removeKey = async () => {
    try {
      await api.censusKeyDelete();
      setHasKey(false);
      setNote("The key was removed from your system keychain.");
    } catch (e) {
      setErr(errorText(e));
    }
  };

  const join = async () => {
    setBusy(true);
    setErr(null);
    setNote(null);
    try {
      const r = await api.censusJoin(layer, geoid, { dataset, vintage, variables });
      useApp.getState().addLayers([r.layer]);
      useApp.getState().setStatus(`Added ${r.layer.name} · ${r.matched.toLocaleString()} of ${r.layer.featureCount.toLocaleString()} features matched`);
      if (r.notes.length > 1 || r.unmatched > 0) {
        setNote(r.notes.join(" "));
        setBusy(false);
        return; // keep the dialog open so the notes (unmatched, suppressed) are read
      }
      onClose();
    } catch (e) {
      if (isCancelled(e)) useApp.getState().setStatus("Census download cancelled");
      else setErr(errorText(e));
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 640 }}>
        <DialogBody>
          <DialogTitle>Census data — join to {info?.name ?? "layer"}</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Adds Census Bureau values to each feature by its GEOID (state, county, tract or block group), as a new layer; your layer is not changed. Public domain.</div>
            <Field label="GEOID field" validationMessage={!geoid ? "Choose the field that holds the Census GEOID" : undefined} validationState={!geoid ? "error" : "none"}>
              <Dropdown value={geoid} onOptionSelect={(_, d) => setGeoid(d.optionValue ?? "")}>
                {fields.map((f) => (
                  <Option key={f.name} value={f.name}>{f.name}</Option>
                ))}
              </Dropdown>
            </Field>
            <Field label="Data set">
              <Dropdown
                value={datasets.find((d) => d.id === dataset)?.label ?? dataset}
                onOptionSelect={(_, d) => {
                  setDataset(d.optionValue!);
                  setVintage(vintages(d.optionValue!)[0]);
                  setPicked([]);
                }}
              >
                {datasets.map((d) => (
                  <Option key={d.id} value={d.id} text={d.label}>{d.label}</Option>
                ))}
              </Dropdown>
            </Field>
            <Field label="Year">
              <Dropdown value={String(vintage)} onOptionSelect={(_, d) => setVintage(Number(d.optionValue))}>
                {vintages(dataset).map((y) => (
                  <Option key={y} value={String(y)}>{String(y)}</Option>
                ))}
              </Dropdown>
            </Field>
            <Field label="Variables">
              <div className="stack">
                {PRESETS.filter((p) => p.dataset === family).map((p) => (
                  <Checkbox key={p.code} checked={picked.includes(p.code)} onChange={(_, c) => setPicked((x) => (c.checked ? [...x, p.code] : x.filter((v) => v !== p.code)))} label={`${p.label} (${p.code})`} />
                ))}
                <Textarea value={custom} onChange={(_, d) => setCustom(d.value)} placeholder="Other variable codes, separated by commas (find them at api.census.gov/data)" rows={2} />
                {parsed.invalid.length > 0 && <MessageBar intent="error"><MessageBarBody>{parsed.invalid.join(", ")} {parsed.invalid.length === 1 ? "is" : "are"} not Census variable codes (they look like B19013_001E).</MessageBarBody></MessageBar>}
              </div>
            </Field>
            <Field label="Census API key" hint={hasKey ? "A key is saved in your system keychain." : "Optional. Without one the Census Bureau limits how much you can fetch. Free: api.census.gov/data/key_signup.html"}>
              <div style={{ display: "flex", gap: 8 }}>
                <Input type="password" style={{ flex: 1 }} value={keyText} onChange={(_, d) => setKeyText(d.value)} placeholder={hasKey ? "•••••••• (saved)" : "Paste your key"} />
                <Button disabled={!keyText.trim()} onClick={saveKey}>Save in keychain</Button>
                {hasKey && <Button onClick={removeKey}>Remove</Button>}
              </div>
            </Field>
            <TaskProgress active={busy} label="Fetching Census data…" match={["Fetching Census data"]} />
            {note && <MessageBar intent="info"><MessageBarBody>{note}</MessageBarBody></MessageBar>}
            {err && (
              <MessageBar intent="error">
                <MessageBarBody style={{ whiteSpace: "pre-line" }}>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={onClose}>{note ? "Close" : "Cancel"}</Button>
            <Button appearance="primary" disabled={busy || !ok} onClick={join}>{busy ? "Fetching…" : "Join"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
