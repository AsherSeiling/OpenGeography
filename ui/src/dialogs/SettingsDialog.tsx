// Settings (UR-16): the choices that belong to the person, not to a project: theme, units, the coordinate system of new maps, and
// (UR-17) the startup panes. Saved at once to the per-user settings file; nothing here is a secret.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, Option } from "@fluentui/react-components";
import { useState } from "react";
import { api } from "../ipc/api";
import { AREA_UNITS, LENGTH_UNITS } from "../nav/geodesy";
import { useSettings } from "../store/settings";
import { useApp } from "../store/store";
import { StartupPanesSection } from "./StartupPanes";

const COMMON_CRS: [string, string][] = [["EPSG:4326", "WGS 84 (EPSG:4326)"], ["EPSG:4269", "NAD83 (EPSG:4269)"], ["EPSG:3857", "Web Mercator (EPSG:3857)"], ["EPSG:5070", "NAD83 / Conus Albers (EPSG:5070)"]];

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const { settings, update } = useSettings();
  const dark = useApp((s) => s.dark);
  const [crsText, setCrsText] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const save = async (patch: Parameters<typeof update>[0]) => setMsg(await update(patch));
  const crs = settings.defaultCrs;
  const crsChoice = !crs ? "none" : COMMON_CRS.some(([id]) => id === crs.id) ? crs.id : "other";
  const applyCrs = async (text: string) => {
    const t = text.trim();
    if (!t) return;
    try {
      const info = await api.crsInfo(t);
      await api.validateCrs(t);
      await save({ defaultCrs: { id: info.code ?? t, name: info.name || t } });
      setCrsText("");
    } catch (e) {
      setMsg(`${t} isn't a coordinate system the app can use: ${e instanceof Error ? e.message : String(e)}`);
    }
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 560 }}>
        <DialogBody>
          <DialogTitle>Settings</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small">These belong to you, not to a project. They are saved on this computer, in the app's own folder, as soon as you change them.</div>
            <Field label="Theme" size="small">
              <Dropdown size="small" aria-label="Theme" value={dark ? "Dark" : "Light"} selectedOptions={[dark ? "dark" : "light"]} onOptionSelect={(_, d) => void save({ theme: d.optionValue as "light" | "dark" })}>
                <Option value="light">Light</Option>
                <Option value="dark">Dark</Option>
              </Dropdown>
            </Field>
            <div className="row2">
              <Field label="Distances" size="small">
                <Dropdown size="small" aria-label="Distance unit" value={LENGTH_UNITS[settings.lengthUnit ?? "mi"]?.[0] ?? "Miles"} selectedOptions={[settings.lengthUnit ?? "mi"]} onOptionSelect={(_, d) => void save({ lengthUnit: d.optionValue })}>
                  {Object.entries(LENGTH_UNITS).map(([k, [l]]) => <Option key={k} value={k}>{l}</Option>)}
                </Dropdown>
              </Field>
              <Field label="Areas" size="small">
                <Dropdown size="small" aria-label="Area unit" value={AREA_UNITS[settings.areaUnit ?? "acres"]?.[0] ?? "Acres"} selectedOptions={[settings.areaUnit ?? "acres"]} onOptionSelect={(_, d) => void save({ areaUnit: d.optionValue })}>
                  {Object.entries(AREA_UNITS).map(([k, [l]]) => <Option key={k} value={k}>{l}</Option>)}
                </Dropdown>
              </Field>
            </div>
            <Field label="Coordinate system of a new map" size="small" hint="Used when there is no map to copy it from. Otherwise a new map takes the coordinate system of the last map, or of the first layer you add.">
              <Dropdown size="small" aria-label="Coordinate system of a new map" value={crsChoice === "none" ? "From the first layer" : crsChoice === "other" ? `${crs?.name}` : COMMON_CRS.find(([id]) => id === crsChoice)![1]} selectedOptions={[crsChoice]} onOptionSelect={(_, d) => {
                const v = d.optionValue!;
                if (v === "none") void save({ defaultCrs: null });
                else if (v !== "other") void save({ defaultCrs: { id: v, name: COMMON_CRS.find(([id]) => id === v)![1].replace(/ \(.*\)$/, "") } });
              }}>
                <Option value="none">From the first layer</Option>
                {COMMON_CRS.map(([id, l]) => <Option key={id} value={id}>{l}</Option>)}
                {crsChoice === "other" && <Option value="other" text={crs?.name ?? ""}>{crs?.name}</Option>}
              </Dropdown>
            </Field>
            <div className="btn-row">
              <Input size="small" aria-label="Another coordinate system" placeholder="Another: EPSG:26914, or a definition" value={crsText} onChange={(_, d) => setCrsText(d.value)} onKeyDown={(e) => e.key === "Enter" && void applyCrs(crsText)} style={{ flex: 1 }} />
              <Button size="small" disabled={!crsText.trim()} onClick={() => void applyCrs(crsText)}>Use</Button>
            </div>
            <Checkbox label="Show developer tools" checked={!!settings.showDeveloperTools} onChange={(_, d) => void save({ showDeveloperTools: !!d.checked })} />
            <div className="muted small">Adds a Developer group to the View tab: Frame Rate, Benchmark and Generate Test Data. Off by default.</div>
            <StartupPanesSection />
            {msg && <div className="field-error">{msg}</div>}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
