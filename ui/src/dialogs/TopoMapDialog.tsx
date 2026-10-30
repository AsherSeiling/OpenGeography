// Insert → Topo Map: the Topo Map Builder (LW9). Pick the area, scale and paper; the engine builds a land-navigation sheet as a new layout
// and lists what it checked. Contours, hillshade and the rest of the data come from the Data tab and Analysis tools.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useMemo, useState } from "react";
import { api, errorText, type ElevationPlan, type TopoBuild, type TopoSpec } from "../ipc/api";
import { isCancelled } from "../progress/progress";
import { contourStyle, hillshadeStyle, pickDem, sheetLabel } from "../layout/topoData";
import { buildTopoSheet } from "../layout/actions";
import { defaultDatum, PAPERS, parseCenter, quadCenter, SCALES, todayIso } from "../layout/topoArea";
import { getCenterLonLat } from "../map/mapController";
import { useApp } from "../store/store";

/** What the project already holds for a land-navigation map, as checks next to the engine's own. */
export function dataChecks(layers: { name: string; geomType: string }[]): { level: "ok" | "note" | "warning"; text: string }[] {
  const raster = layers.some((l) => l.geomType === "Raster");
  const contours = layers.some((l) => /contour/i.test(l.name));
  const water = layers.some((l) => /water|hydro|stream|river|lake/i.test(l.name));
  const roads = layers.some((l) => /road|trail|transport/i.test(l.name));
  return [
    raster ? { level: "ok", text: "An elevation or imagery raster is in the project." } : { level: "warning", text: "No elevation raster yet: use Data → Elevation to download 3DEP for this area." },
    contours ? { level: "ok", text: "A contour layer is in the project." } : { level: "warning", text: "No contour layer yet: run the Contours tool on the elevation raster (interval below) and label the index lines." },
    water ? { level: "ok", text: "Water features are in the project." } : { level: "note", text: "No water layer found: add The National Map hydrography or Overture water." },
    roads ? { level: "ok", text: "Roads or trails are in the project." } : { level: "note", text: "No roads or trails found: add The National Map transportation or Overture roads." },
  ];
}

export function TopoMapDialog({ onClose }: { onClose: () => void }) {
  const [where, setWhere] = useState<"view" | "position">("view");
  const [position, setPosition] = useState("");
  const [quad, setQuad] = useState(false);
  const [scale, setScale] = useState(24000);
  const [paper, setPaper] = useState<TopoSpec["paper"]>("tabloid");
  const [landscape, setLandscape] = useState(true);
  const [units, setUnits] = useState<TopoSpec["units"]>("imperial");
  const [datum, setDatum] = useState<TopoSpec["datum"] | "auto">("auto");
  const [title, setTitle] = useState("");
  const [date, setDate] = useState(todayIso());
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [built, setBuilt] = useState<TopoBuild | null>(null);
  const layers = useApp((s) => s.layers);
  const order = useApp((s) => s.order);
  const extra = useMemo(() => dataChecks(Object.values(layers)), [layers]);
  const dem = useMemo(() => pickDem(order, layers), [order, layers]);
  const [dataBusy, setDataBusy] = useState(false);
  const [dataMsg, setDataMsg] = useState<string | null>(null);
  const [plan, setPlan] = useState<ElevationPlan | null>(null);

  /** Step 1: 3DEP elevation for the sheet's area (the engine's plan first; a big download asks for a second click). */
  const getElevation = async () => {
    if (!built) return;
    setErr(null);
    setDataBusy(true);
    try {
      const label = sheetLabel(built.layout.name);
      const p = plan ?? (await api.planElevation(built.bboxLonlat, 10));
      if (p.needsConfirm && !plan) {
        setPlan(p);
        setDataMsg(`This download is about ${(p.bytes / 1e6).toFixed(0)} MB. Click again to download it.`);
        return;
      }
      setPlan(null);
      const info = await api.downloadElevation(built.bboxLonlat, 10, label, p.needsConfirm);
      useApp.getState().addLayers([info]);
      setDataMsg(`Added ${info.name}.`);
    } catch (e) {
      if (!isCancelled(e)) setErr(errorText(e));
    } finally {
      setDataBusy(false);
    }
  };

  /** Step 2: contours (index lines every fifth) and a hillshade from the elevation layer, drawn and ready for the sheet. */
  const makeContours = async () => {
    if (!built || !dem) return;
    setErr(null);
    setDataBusy(true);
    try {
      const made = await api.topoPrepareData(dem.id, built.contourIntervalM, 5, sheetLabel(built.layout.name));
      const app = useApp.getState();
      app.addLayers([made.hillshade, made.contours]); // the contours end up on top of the hillshade
      app.setStyle(made.contours.id, contourStyle(useApp.getState().styles[made.contours.id]));
      app.setStyle(made.hillshade.id, hillshadeStyle(useApp.getState().styles[made.hillshade.id]));
      app.setVisible(dem.id, false); // the hillshade shows the terrain; the coloured elevation would hide it
      setDataMsg(`Made ${made.contours.featureCount.toLocaleString()} contour lines every ${built.contourInterval} (index lines every fifth, labelled) and a hillshade.`);
    } catch (e) {
      if (!isCancelled(e)) setErr(errorText(e));
    } finally {
      setDataBusy(false);
    }
  };

  const build = async () => {
    setErr(null);
    let center: [number, number] | null = null;
    if (where === "view") {
      center = getCenterLonLat();
      if (!center) return setErr("The map view has no position yet: zoom the map to your area first, or type a position.");
    } else {
      const p = parseCenter(position);
      if ("error" in p) return setErr(p.error);
      center = p.center;
    }
    let name = title.trim();
    if (quad) {
      const q = quadCenter(center[0], center[1]);
      center = q.center;
      if (!name) name = `${q.name} quadrangle`;
    }
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date.trim())) return setErr("Write the declination date as YYYY-MM-DD.");
    setBusy(true);
    try {
      const spec: TopoSpec = { center, scale, paper, landscape, units, datum: datum === "auto" ? defaultDatum(center[0], center[1]) : datum, title: name, date: date.trim() };
      setBuilt(await buildTopoSheet(spec));
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 640 }}>
        <DialogBody>
          <DialogTitle>Topo Map Builder</DialogTitle>
          <DialogContent className="stack">
            <div className="props-hint">Builds a land-navigation sheet as a new layout: UTM grid with zone and 100 km square, lat/lon ticks, declination diagram, scale bars, grid-reference box, legend and a marginal note.</div>
            <Field label="Area" size="small">
              <Dropdown size="small" value={where === "view" ? "The centre of the map view" : "A position I type"} selectedOptions={[where]} onOptionSelect={(_, d) => setWhere(d.optionValue as "view" | "position")}>
                <Option value="view">The centre of the map view</Option>
                <Option value="position">A position I type</Option>
              </Dropdown>
            </Field>
            {where === "position" && (
              <Field label="Latitude, longitude  or  MGRS reference" size="small">
                <Input size="small" aria-label="Position" placeholder="42.0, -93.0   or   15T VG 975 475" value={position} onChange={(_, d) => setPosition(d.value)} />
              </Field>
            )}
            <Checkbox label="Centre the sheet on the 7.5′ quadrangle that contains it" checked={quad} onChange={(_, d) => setQuad(!!d.checked)} />
            <div className="props-grid">
              <Field label="Scale" size="small">
                <Dropdown size="small" value={SCALES.find((s) => s[0] === scale)?.[1] ?? `1:${scale}`} selectedOptions={[String(scale)]} onOptionSelect={(_, d) => setScale(Number(d.optionValue))}>
                  {SCALES.map(([v, l]) => <Option key={v} value={String(v)}>{l}</Option>)}
                </Dropdown>
              </Field>
              <Field label="Paper" size="small">
                <Dropdown size="small" value={PAPERS.find((p) => p[0] === paper)?.[1]} selectedOptions={[paper]} onOptionSelect={(_, d) => setPaper(d.optionValue as TopoSpec["paper"])}>
                  {PAPERS.map(([v, l]) => <Option key={v} value={v}>{l}</Option>)}
                </Dropdown>
              </Field>
              <Field label="Orientation" size="small">
                <Dropdown size="small" value={landscape ? "Landscape" : "Portrait"} selectedOptions={[landscape ? "l" : "p"]} onOptionSelect={(_, d) => setLandscape(d.optionValue === "l")}>
                  <Option value="l">Landscape</Option>
                  <Option value="p">Portrait</Option>
                </Dropdown>
              </Field>
              <Field label="Units" size="small">
                <Dropdown size="small" value={units === "imperial" ? "Imperial (ft, mi)" : "Metric (m, km)"} selectedOptions={[units]} onOptionSelect={(_, d) => setUnits(d.optionValue as TopoSpec["units"])}>
                  <Option value="imperial">Imperial (ft, mi)</Option>
                  <Option value="metric">Metric (m, km)</Option>
                </Dropdown>
              </Field>
              <Field label="Datum" size="small">
                <Dropdown size="small" value={datum === "auto" ? "By place (NAD 83 in North America)" : datum === "nad83" ? "NAD 83" : "WGS 84"} selectedOptions={[datum]} onOptionSelect={(_, d) => setDatum(d.optionValue as typeof datum)}>
                  <Option value="auto">By place (NAD 83 in North America)</Option>
                  <Option value="nad83">NAD 83</Option>
                  <Option value="wgs84">WGS 84</Option>
                </Dropdown>
              </Field>
              <Field label="Declination date (YYYY-MM-DD)" size="small">
                <Input size="small" aria-label="Declination date" value={date} onChange={(_, d) => setDate(d.value)} />
              </Field>
            </div>
            <Field label="Title (blank = the quadrangle's name or “Topographic map”)" size="small">
              <Input size="small" aria-label="Sheet title" value={title} onChange={(_, d) => setTitle(d.value)} />
            </Field>
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
            {built && (
              <div className="stack" data-testid="topo-checks" aria-label="Self-check">
                <strong>Self-check</strong>
                {[...built.checks, ...extra].map((c, i) => (
                  <MessageBar key={i} intent={c.level === "ok" ? "success" : c.level === "warning" ? "warning" : "info"}><MessageBarBody>{c.text}</MessageBarBody></MessageBar>
                ))}
                <div className="props-hint">Contour interval for this scale: {built.contourInterval}.</div>
                <strong>Prepare the data</strong>
                <div className="props-hint">The sheet's frame follows the map layers, so the contours and hillshade below appear on it at once.</div>
                <div className="row2">
                  <Button size="small" disabled={dataBusy} onClick={() => void getElevation()}>{plan ? "Download anyway" : "1. Download elevation (3DEP, 10 m)"}</Button>
                  <Button size="small" disabled={dataBusy || !dem} onClick={() => void makeContours()} title={dem ? `From ${dem.name}` : "Download or add an elevation layer first"}>2. Make contours and hillshade</Button>
                </div>
                {dataMsg && <MessageBar intent="success"><MessageBarBody>{dataMsg}</MessageBarBody></MessageBar>}
              </div>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" disabled={busy} onClick={() => void build()}>{built ? "Build another" : "Build the sheet"}</Button>
            <Button onClick={onClose}>{built ? "Close" : "Cancel"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
