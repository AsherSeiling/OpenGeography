// Geodesic Measure tool panel: distance (with per-segment azimuths) or area.
import { Button } from "@fluentui/react-components";
import { DismissRegular, SaveRegular } from "@fluentui/react-icons";
import { api, errorText } from "../ipc/api";
import { useApp } from "../store/store";
import {
  AREA_UNITS, LENGTH_UNITS, convertAzimuth, degToMils, formatArea, formatLength, gmAngle, inverse, pathLength, polygonArea, toMgrs, type North,
} from "./geodesy";
import { useNav } from "./navStore";

export function MeasurePanel() {
  const n = useNav();
  const { points, measureMode, lengthUnit, areaUnit, north, angleUnit, done } = n;
  const segments = points.slice(1).map((p, i) => {
    const r = inverse(points[i], p);
    const az = convertAzimuth(r.azimuth, "true", north, points[i]);
    return { d: r.distance, az };
  });
  const total = pathLength(points);
  const area = measureMode === "area" && points.length > 2 ? polygonArea(points) : null;
  const fmtAz = (a: number) => (angleUnit === "mils" ? `${Math.round(degToMils(a)).toString().padStart(4, "0")} mils` : `${a.toFixed(2)}°`);
  const gm = points.length ? gmAngle(points[0]) : null;

  const save = async () => {
    if (points.length < 2) return;
    const ring = measureMode === "area" ? [...points, points[0]] : points;
    const coords = ring.map(([x, y]) => `${x} ${y}`).join(", ");
    const wkt = measureMode === "area" ? `POLYGON((${coords}))` : `LINESTRING(${coords})`;
    const props = measureMode === "area" ? { AREA_M2: area?.area ?? 0, PERIM_M: area?.perimeter ?? 0 } : { LENGTH_M: total };
    try {
      const info = await api.createFeatures(measureMode === "area" ? "Measured area" : "Measured distance", [{ wkt, props }]);
      useApp.getState().addLayers([info]);
      useApp.getState().setStatus(`Saved ${info.name} as a layer`);
    } catch (e) {
      useApp.getState().setStatus(errorText(e));
    }
  };

  return (
    <div className="measure-panel" onMouseDown={(e) => e.stopPropagation()}>
      <div className="mp-head">
        <div className="mp-tabs">
          {(["distance", "area"] as const).map((m) => (
            <button key={m} className={measureMode === m ? "on" : ""} onClick={() => n.set({ measureMode: m })}>
              {m === "distance" ? "Distance" : "Area"}
            </button>
          ))}
        </div>
        <button className="mp-x" title="Close (back to Explore)" onClick={() => { n.clearMeasure(); useApp.getState().setTool("pan"); }}>
          <DismissRegular />
        </button>
      </div>

      {points.length === 0 && <div className="mp-hint">Click the map to add points. Double-click to finish, Esc to start over. Lines follow the true (geodesic) path on the WGS 84 ellipsoid.</div>}

      {measureMode === "distance" && points.length > 1 && (
        <>
          <div className="mp-total">{formatLength(total, lengthUnit)}</div>
          <table className="mp-segs">
            <thead>
              <tr><th>#</th><th>Length</th><th>Azimuth ({north})</th></tr>
            </thead>
            <tbody>
              {segments.map((s, i) => (
                <tr key={i}><td>{i + 1}</td><td>{formatLength(s.d, lengthUnit)}</td><td>{fmtAz(s.az)}</td></tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {measureMode === "area" && (
        area ? (
          <>
            <div className="mp-total">{formatArea(area.area, areaUnit)}</div>
            <div className="mp-sub">Perimeter {formatLength(area.perimeter, lengthUnit)}</div>
          </>
        ) : points.length > 0 ? <div className="mp-hint">Add at least three points.</div> : null
      )}

      {points.length > 0 && (
        <div className="mp-coords">
          <div>Start: {toMgrs(points[0])}</div>
          {points.length > 1 && <div>{measureMode === "area" ? "Last" : "End"}: {toMgrs(points[points.length - 1])}</div>}
          {gm !== null && <div className="muted">G-M angle here: {Math.abs(gm).toFixed(2)}° {gm >= 0 ? "E" : "W"} (WMM2025)</div>}
        </div>
      )}

      <div className="mp-opts">
        <select value={lengthUnit} onChange={(e) => n.set({ lengthUnit: e.target.value })} title="Length units">
          {Object.entries(LENGTH_UNITS).map(([k, [l]]) => <option key={k} value={k}>{l}</option>)}
        </select>
        {measureMode === "area" && (
          <select value={areaUnit} onChange={(e) => n.set({ areaUnit: e.target.value })} title="Area units">
            {Object.entries(AREA_UNITS).map(([k, [l]]) => <option key={k} value={k}>{l}</option>)}
          </select>
        )}
        {measureMode === "distance" && (
          <>
            <select value={north} onChange={(e) => n.set({ north: e.target.value as North })} title="Azimuth reference">
              <option value="true">True north</option>
              <option value="grid">Grid north (UTM/MGRS)</option>
              <option value="magnetic">Magnetic north</option>
            </select>
            <select value={angleUnit} onChange={(e) => n.set({ angleUnit: e.target.value as "deg" | "mils" })} title="Angle units">
              <option value="deg">Degrees</option>
              <option value="mils">Mils (6400)</option>
            </select>
          </>
        )}
      </div>
      <div className="btn-row">
        <Button size="small" onClick={n.clearMeasure} disabled={!points.length}>Clear</Button>
        <Button size="small" icon={<SaveRegular />} onClick={save} disabled={points.length < (measureMode === "area" ? 3 : 2)}>Save as layer</Button>
        {!done && points.length > 1 && <span className="muted small">double-click to finish</span>}
      </div>
    </div>
  );
}
