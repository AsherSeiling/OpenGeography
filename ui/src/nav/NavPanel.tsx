// Land navigation: Resection (find where you are from azimuths to known
// landmarks) and Intersection (find an unseen point from azimuths taken at
// known positions). Solved on the UTM/MGRS grid, like on a military map.
import { Button } from "@fluentui/react-components";
import { AddRegular, DeleteRegular, DismissRegular, LocationRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../ipc/api";
import { zoomToExtent } from "../map/mapController";
import { useApp } from "../store/store";
import {
  declination, degToMils, gmAngle, gridConvergence, intersection, milsToDeg, parsePosition, resection, toMgrs, type Fix, type LonLat, type North,
} from "./geodesy";
import { useNav } from "./navStore";

interface Row {
  pos: string;
  az: string;
}

const blank = (): Row => ({ pos: "", az: "" });

export function NavPanel() {
  const n = useNav();
  const mode = n.navPanel!;
  const [rows, setRows] = useState<Row[]>([blank(), blank()]);
  const [north, setNorth] = useState<North>("magnetic");
  const [unit, setUnit] = useState<"deg" | "mils">("deg");
  const [gmOverride, setGmOverride] = useState("");
  const [fix, setFix] = useState<Fix | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setFix(null);
    setErr(null);
    n.clearNav();
  }, [mode]);

  // Parse rows; first valid position anchors the G-M angle.
  const parsed = rows.map((r) => {
    let p: LonLat | null = null;
    let e: string | null = null;
    if (r.pos.trim()) {
      try {
        p = parsePosition(r.pos);
      } catch (x) {
        e = (x as Error).message;
      }
    }
    return { p, e };
  });
  const anchor = parsed.find((x) => x.p)?.p ?? null;
  const auto = useMemo(() => (anchor ? { gm: gmAngle(anchor), decl: declination(anchor), conv: gridConvergence(anchor) } : null), [anchor?.[0], anchor?.[1]]);
  const gm = gmOverride.trim() !== "" && !isNaN(Number(gmOverride)) ? Number(gmOverride) : auto?.gm;

  const setRow = (i: number, p: Partial<Row>) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, ...p } : r)));

  const pickFor = (i: number) => {
    n.set({ pick: (p: LonLat) => setRow(i, { pos: toMgrs(p) }) });
  };

  const solve = () => {
    setErr(null);
    try {
      const sightings = rows.map((r, i) => {
        const p = parsed[i].p;
        if (!p) throw new Error(`Point ${i + 1}: ${parsed[i].e ?? "enter a position"}`);
        const v = Number(r.az);
        if (r.az.trim() === "" || isNaN(v)) throw new Error(`Point ${i + 1}: enter an azimuth`);
        const deg = unit === "mils" ? milsToDeg(v) : v;
        if (deg < 0 || deg >= 360) throw new Error(`Point ${i + 1}: azimuth must be 0–360° (0–6400 mils)`);
        return { point: p, azimuth: deg, north };
      });
      const f = mode === "resection" ? resection(sightings, gm) : intersection(sightings, gm);
      setFix(f);
      n.set({ rays: f.rays, fix: f.position, triangle: f.intersections.length > 2 ? f.intersections : [], known: sightings.map((s) => s.point) });
      const all = [...sightings.map((s) => s.point), f.position];
      zoomToExtent([Math.min(...all.map((p) => p[0])), Math.min(...all.map((p) => p[1])), Math.max(...all.map((p) => p[0])), Math.max(...all.map((p) => p[1]))], 15);
    } catch (e) {
      setFix(null);
      n.clearNav();
      setErr(errorText(e));
    }
  };

  const addToMap = async () => {
    if (!fix) return;
    try {
      const info = await api.createFeatures(mode === "resection" ? "Resection fix" : "Intersection point", [
        {
          wkt: `POINT(${fix.position[0]} ${fix.position[1]})`,
          props: { MGRS: fix.mgrs, METHOD: mode, LAT: fix.position[1], LON: fix.position[0], ERROR_M: Math.round(fix.errorSpread * 10) / 10 },
        },
      ]);
      useApp.getState().addLayers([info]);
      useApp.getState().setStatus(`Added ${info.name} at ${fix.mgrs}`);
    } catch (e) {
      setErr(errorText(e));
    }
  };

  const azLabel = mode === "resection" ? "Azimuth from you to the landmark" : "Azimuth from this point to the target";
  const fmtGm = (a: number) => `${Math.abs(a).toFixed(2)}° ${a >= 0 ? "E" : "W"}${unit === "mils" ? ` (${Math.round(degToMils(Math.abs(a)))} mils)` : ""}`;

  return (
    <div className="nav-panel" onMouseDown={(e) => e.stopPropagation()}>
      <div className="mp-head">
        <div className="mp-tabs">
          <button className={mode === "resection" ? "on" : ""} onClick={() => n.set({ navPanel: "resection" })}>Resection</button>
          <button className={mode === "intersection" ? "on" : ""} onClick={() => n.set({ navPanel: "intersection" })}>Intersection</button>
        </div>
        <button className="mp-x" title="Close" onClick={() => { n.clearNav(); n.set({ navPanel: null, pick: null }); }}>
          <DismissRegular />
        </button>
      </div>
      <div className="mp-hint">
        {mode === "resection"
          ? "Find your position: shoot azimuths to 2–3 landmarks you can identify on the map. Back azimuths are drawn from each landmark; they cross at your position."
          : "Locate an unseen point: from 2–3 known positions, shoot an azimuth to the target. The lines cross at the target."}
      </div>
      <div className="nav-opts">
        <label>
          Azimuths are
          <select value={north} onChange={(e) => setNorth(e.target.value as North)}>
            <option value="magnetic">Magnetic (compass)</option>
            <option value="grid">Grid (map/protractor)</option>
            <option value="true">True</option>
          </select>
        </label>
        <label>
          in
          <select value={unit} onChange={(e) => setUnit(e.target.value as "deg" | "mils")}>
            <option value="deg">degrees</option>
            <option value="mils">mils (6400)</option>
          </select>
        </label>
      </div>
      {rows.map((r, i) => (
        <div key={i} className="nav-row">
          <div className="nav-row-title">{mode === "resection" ? `Landmark ${i + 1}` : `Known point ${i + 1}`}</div>
          <div className="nav-row-fields">
            <input className={`qe-ctl nav-pos ${parsed[i].e ? "bad" : ""}`} value={r.pos} placeholder="MGRS (14S PF 1234 5678) or lat, lon" title={parsed[i].e ?? "Position"} onChange={(e) => setRow(i, { pos: e.target.value })} />
            <button className="nav-pick" title="Pick on the map" onClick={() => pickFor(i)}><LocationRegular /></button>
            <input className="qe-ctl nav-az" value={r.az} placeholder={unit === "mils" ? "mils" : "deg"} title={azLabel} onChange={(e) => setRow(i, { az: e.target.value })} />
            <button className="nav-pick" title="Remove" disabled={rows.length <= 2} onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}><DeleteRegular /></button>
          </div>
          {parsed[i].e && <div className="nav-err">{parsed[i].e}</div>}
        </div>
      ))}
      {rows.length < 4 && <Button size="small" appearance="subtle" icon={<AddRegular />} onClick={() => setRows((rs) => [...rs, blank()])}>Add {mode === "resection" ? "landmark" : "point"}</Button>}

      {north === "magnetic" && (
        <div className="nav-gm">
          <div>
            G-M angle: <b>{gm !== undefined ? fmtGm(gm) : "—"}</b>
            {auto && gmOverride.trim() === "" && <span className="muted"> (declination {fmtGm(auto.decl)}, grid convergence {fmtGm(auto.conv)}; WMM2025)</span>}
          </div>
          <label className="muted small">
            Use the G-M angle from your map's declination diagram instead (degrees, east +):
            <input className="qe-ctl" style={{ width: 70, marginLeft: 6 }} value={gmOverride} onChange={(e) => setGmOverride(e.target.value)} placeholder="auto" />
          </label>
        </div>
      )}

      <div className="btn-row">
        <Button appearance="primary" size="small" onClick={solve}>Solve</Button>
        <Button size="small" disabled={!fix} onClick={addToMap}>Add point to map</Button>
      </div>
      {err && <div className="nav-err">{err}</div>}
      {fix && (
        <div className="nav-result">
          <div className="nav-mgrs">{fix.mgrs}</div>
          <div>{fix.position[1].toFixed(6)}°, {fix.position[0].toFixed(6)}°</div>
          {fix.intersections.length > 2 ? (
            <div className={fix.errorSpread > 100 ? "nav-warn" : "muted"}>
              Triangle of error: {fix.errorSpread.toFixed(0)} m across{fix.errorSpread > 100 ? " — re-shoot your azimuths if it's this large" : ""}. Position shown is the best fit.
            </div>
          ) : (
            <div className="muted">Add a third {mode === "resection" ? "landmark" : "point"} to check accuracy (triangle of error).</div>
          )}
        </div>
      )}
    </div>
  );
}
