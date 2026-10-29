// Projection builder (R8): pick a family, move the central meridian and standard parallels, see the PROJ string, use it as the map CRS.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, MessageBar, MessageBarBody, Option, Slider, Switch } from "@fluentui/react-components";
import { useEffect, useRef, useState } from "react";
import { setMapCrsChecked } from "../actions";
import { builderName, buildProjString, DEFAULTS, FAMILIES, validate, type BuilderParams } from "../crs/projectionBuilder";
import { useApp } from "../store/store";

export function ProjectionBuilderDialog({ onClose }: { onClose: () => void }) {
  const [p, setP] = useState<BuilderParams>(DEFAULTS);
  const [live, setLive] = useState(true);
  const problems = validate(p);
  const proj = buildProjString(p);
  const fam = FAMILIES.find((f) => f.id === p.family)!;
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const apply = async (params: BuilderParams) => {
    const s = buildProjString(params);
    if (!s) return;
    await setMapCrsChecked(s, builderName(params));
    useApp.getState().setWorldView(true);
  };
  // Dragging the central meridian re-centres the world after a short pause.
  useEffect(() => {
    if (!live || !proj) return;
    clearTimeout(timer.current);
    timer.current = setTimeout(() => void apply(p), 350);
    return () => clearTimeout(timer.current);
  }, [p, live]);

  const set = (patch: Partial<BuilderParams>) => setP((x) => ({ ...x, ...patch }));
  const slider = (label: string, key: "lon0" | "lat0" | "lat1" | "lat2" | "latTs", min: number, max: number) => (
    <Field label={`${label}: ${p[key]}°`} size="small">
      <Slider min={min} max={max} step={1} value={p[key]} onChange={(_, d) => set({ [key]: d.value } as Partial<BuilderParams>)} aria-label={label} />
    </Field>
  );

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Projection Builder</DialogTitle>
          <DialogContent>
            <div className="stack">
              <Field label="Projection">
                <Dropdown value={fam.label} onOptionSelect={(_, d) => set({ family: d.optionValue as BuilderParams["family"] })}>
                  {(["Conic", "Cylindrical", "Azimuthal", "World"] as const).map((g) =>
                    FAMILIES.filter((f) => f.group === g).map((f) => (
                      <Option key={f.id} value={f.id} text={f.label}>{g}: {f.label}</Option>
                    )),
                  )}
                </Dropdown>
              </Field>
              <div className="muted">{fam.note}</div>
              {slider("Central meridian", "lon0", -180, 180)}
              {fam.uses.lat0 && slider("Latitude of origin", "lat0", -90, 90)}
              {fam.uses.lat1 && slider("First standard parallel", "lat1", -89, 89)}
              {fam.uses.lat2 && slider("Second standard parallel", "lat2", -89, 89)}
              {fam.uses.latTs && slider("Standard parallel", "latTs", -89, 89)}
              {problems.map((m) => (
                <MessageBar key={m} intent="error"><MessageBarBody>{m}</MessageBarBody></MessageBar>
              ))}
              <Field label="PROJ definition (a custom coordinate system)">
                <div className="mono wrap" data-testid="proj-string">{proj ?? "—"}</div>
              </Field>
              <Switch checked={live} onChange={(_, d) => setLive(d.checked)} label="Update the map as I move the sliders" />
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Close</Button>
            <Button appearance="primary" disabled={!proj} onClick={() => void apply(p)}>Use as map CRS (world view)</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
