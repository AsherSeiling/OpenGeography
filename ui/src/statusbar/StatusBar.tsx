// Status bar: editable map scale, cursor coordinates in the map CRS, decimal
// degrees / DMS, MGRS, and the map CRS (click to change).
import { forward as toMgrs } from "mgrs";
import { useEffect, useRef, useState } from "react";
import { pickMapCrs } from "../actions";
import { api } from "../ipc/api";
import { getMap } from "../map/mapController";
import { formatScale, parseScale, SCALE_PRESETS, scaleForZoom, zoomForScale } from "../map/style";
import { useMapView } from "../panes/MapView";
import { useApp } from "../store/store";

function dms(v: number, pos: string, neg: string) {
  const a = Math.abs(v);
  const d = Math.floor(a);
  const m = Math.floor((a - d) * 60);
  const s = ((a - d) * 60 - m) * 60;
  return `${d}°${String(m).padStart(2, "0")}′${s.toFixed(1).padStart(4, "0")}″${v >= 0 ? pos : neg}`;
}

export function mgrsOf(lon: number, lat: number): string {
  if (lat < -80 || lat > 84) return "—";
  try {
    // "14SPF1234567890" → "14S PF 12345 67890" (readable grid reference)
    const m = toMgrs([lon, lat], 5);
    const r = m.match(/^(\d{1,2}[A-Z])([A-Z]{2})(\d{5})(\d{5})$/);
    return r ? `${r[1]} ${r[2]} ${r[3]} ${r[4]}` : m;
  } catch {
    return "—";
  }
}

export function StatusBar() {
  const { lon, lat, zoom, centerLat } = useMapView();
  const status = useApp((s) => s.status);
  const mapCrs = useApp((s) => s.mapCrs);
  const projectDir = useApp((s) => s.projectDir);
  const dirty = useApp((s) => s.dirty);
  const [projected, setProjected] = useState<[number, number] | null>(null);
  const scale = scaleForZoom(zoom, centerLat);
  const [scaleText, setScaleText] = useState<string | null>(null);

  // Project the cursor into the map CRS (throttled engine call).
  const pending = useRef(false);
  const latest = useRef<[number, number] | null>(null);
  useEffect(() => {
    if (lon === null || lat === null || !mapCrs || /^EPSG:(4326|4269|4267)$/.test(mapCrs.id)) {
      setProjected(null);
      return;
    }
    latest.current = [lon, lat];
    if (pending.current) return;
    pending.current = true;
    const t = setTimeout(async () => {
      const p = latest.current!;
      try {
        setProjected(await api.transformPoint(p[0], p[1], "EPSG:4326", mapCrs.id));
      } catch {
        setProjected(null);
      }
      pending.current = false;
    }, 50);
    return () => clearTimeout(t);
  }, [lon, lat, mapCrs]);

  const applyScale = (text: string) => {
    const s = parseScale(text);
    const map = getMap();
    if (s && map) map.easeTo({ zoom: zoomForScale(s, map.getCenter().lat), duration: 300 });
    setScaleText(null);
  };

  return (
    <div className="statusbar">
      <div className="sb-scale">
        <input
          list="scale-presets"
          value={scaleText ?? formatScale(scale)}
          onFocus={(e) => {
            setScaleText(formatScale(scale));
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
            {projected && (
              <span title={`Map coordinate system: ${mapCrs?.name}`}>
                {projected[0].toLocaleString("en-US", { maximumFractionDigits: 1 })} E {projected[1].toLocaleString("en-US", { maximumFractionDigits: 1 })} N
              </span>
            )}
            <span title="Decimal degrees (WGS 84)">{lat.toFixed(5)}°, {lon.toFixed(5)}°</span>
            <span title="Degrees minutes seconds">{dms(lat, "N", "S")} {dms(lon, "E", "W")}</span>
            <span title="MGRS / USNG grid reference (1 m)">{mgrsOf(lon, lat)}</span>
          </>
        ) : (
          <span className="muted">{status}</span>
        )}
      </div>
      <div className="sb-right">
        {lon !== null && <span className="muted sb-msg">{status}</span>}
        <button className="sb-crs" onClick={pickMapCrs} title="Map coordinate system — click to change">
          {mapCrs ? mapCrs.name : "No coordinate system"}
        </button>
        <span className="muted">{projectDir ? projectDir.split(/[\\/]/).pop() : "Untitled"}{dirty ? " •" : ""}</span>
      </div>
    </div>
  );
}
