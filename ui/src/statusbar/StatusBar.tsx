// Status bar: editable map scale, the pointer in one chosen coordinate format (decimal degrees, DMS, map E/N or MGRS; remembered in
// the program settings), running background jobs, and the map CRS (click to change).
import { useEffect, useState } from "react";
import { api } from "../ipc/api";
import { pickMapCrs } from "../actions";
import { getActiveCrs, setScale } from "../map/mapController";
import { formatScale, parseScale, SCALE_PRESETS } from "../map/style";
import { useMapView } from "../panes/MapView";
import { StatusProgress } from "../progress/progress";
import { useApp } from "../store/store";
import { useSettings } from "../store/settings";
import { COORD_FORMATS, formatPointer, readCoordFormat, type CoordFormat } from "./coordFormat";

/** True when lon/lat lies outside the CRS's area of use [west, south, east, north] (areas crossing the antimeridian have west > east). */
export function outsideArea(lon: number, lat: number, area: [number, number, number, number]): boolean {
  const [w, s, e, n] = area;
  if (lat < s || lat > n) return true;
  return w <= e ? lon < w || lon > e : lon < w && lon > e;
}

/** Scale factor k and meridian convergence at the pointer, and a notice when the pointer is outside the CRS's area of use. */
function PointFactors({ x, y, lon, lat }: { x: number; y: number; lon: number; lat: number }) {
  const [f, setF] = useState<[number, number] | null>(null);
  const crs = getActiveCrs();
  const id = crs?.id;
  useEffect(() => {
    if (!id || crs!.info.geographic) { setF(null); return; }
    let live = true;
    const t = setTimeout(() => api.mapFactors(id, x, y).then((r) => live && setF(r)).catch(() => live && setF(null)), 120);
    return () => { live = false; clearTimeout(t); };
  }, [id, x, y]);
  const out = crs && crs.info.areaOfUseKnown && outsideArea(lon, lat, crs.info.areaOfUse);
  return (
    <>
      {f && (
        <span title="Point scale factor k (grid distance ÷ ground distance) and meridian convergence γ at the pointer">
          k {f[0].toFixed(5)} · γ {f[1].toFixed(3)}°
        </span>
      )}
      {out && <span className="sb-warn" title={`The pointer is outside the area this coordinate system is meant for (${crs!.info.areaOfUse.map((v) => v.toFixed(1)).join(", ")}). Distances and areas get less accurate.`}>⚠ outside area of use</span>}
    </>
  );
}

export function StatusBar() {
  const pointer = useMapView((s) => s.pointer);
  const scale = useMapView((s) => s.scale);
  const status = useApp((s) => s.status);
  const mapCrs = useApp((s) => s.mapCrs);
  const projectDir = useApp((s) => s.projectDir);
  const dirty = useApp((s) => s.dirty);
  const [scaleText, setScaleText] = useState<string | null>(null);
  const fmt = useSettings((st) => readCoordFormat(st.settings.coordinateFormat));
  const setFmt = (f: CoordFormat) => void useSettings.getState().update({ coordinateFormat: f }).then((err) => err && useApp.getState().setStatus(err));
  // Every number comes from the engine with the pointer (R2): lon/lat from `transformPoint`, E/N are the view coordinates it was asked about.
  const lon = pointer?.lon ?? null;
  const lat = pointer?.lat ?? null;
  const projected = pointer && getActiveCrs() && !getActiveCrs()!.info.geographic ? ([pointer.x, pointer.y] as const) : null;

  const applyScale = (text: string) => {
    const s = parseScale(text);
    if (s) void setScale(s);
    setScaleText(null);
  };

  return (
    <div className="statusbar">
      <div className="sb-scale">
        <input
          list="scale-presets"
          value={scaleText ?? (scale ? formatScale(scale) : "")}
          onFocus={(e) => {
            setScaleText(scale ? formatScale(scale) : "");
            e.target.select();
          }}
          onChange={(e) => setScaleText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && applyScale((e.target as HTMLInputElement).value)}
          onBlur={(e) => applyScale(e.target.value)}
          title="Map scale at 96 DPI — type a scale (e.g. 1:24000 or 50k) and press Enter"
        />
        <datalist id="scale-presets">
          {SCALE_PRESETS.map((s) => (
            <option key={s} value={formatScale(s)} />
          ))}
        </datalist>
      </div>
      <div className="sb-coords">
        {lon !== null && lat !== null ? (
          <>
            {(() => {
              const r = formatPointer(fmt, { lon, lat, x: pointer!.x, y: pointer!.y }, !!projected);
              return <span title={r.title} data-testid="sb-pointer" data-format={r.format}>{r.text}</span>;
            })()}
            {projected && <PointFactors x={projected[0]} y={projected[1]} lon={lon} lat={lat} />}
          </>
        ) : (
          <span className="muted">{status}</span>
        )}
      </div>
      <select className="sb-format" value={fmt} onChange={(e) => setFmt(e.target.value as CoordFormat)} aria-label="Coordinate format" title="How the pointer position is shown">
        {COORD_FORMATS.map((f) => (
          <option key={f.id} value={f.id} title={f.title}>{f.label}</option>
        ))}
      </select>
      <div className="sb-right">
        <StatusProgress />
        {lon !== null && <span className="muted sb-msg">{status}</span>}
        <button className="sb-crs" onClick={pickMapCrs} title="Map coordinate system — click to change">
          {mapCrs ? mapCrs.name : "No coordinate system"}
        </button>
        <span className="muted">{projectDir ? projectDir.split(/[\\/]/).pop() : "Untitled"}{dirty ? " •" : ""}</span>
      </div>
    </div>
  );
}
