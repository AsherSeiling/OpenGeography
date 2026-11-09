// Go To Coordinates (RB4): type a position, see how it was read and the same place in the other formats, press Enter to go.
// The panel floats over the map (it never blocks it), closes with Escape, and creates no layer and changes no data: the result is
// the map centred on the place and a temporary marker.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from "@fluentui/react-components";
import { CopyRegular, DismissRegular } from "@fluentui/react-icons";
import { useEffect, useMemo, useRef, useState, type ClipboardEvent, type KeyboardEvent } from "react";
import { CrsPicker } from "../dialogs/Dialogs";
import { api, errorText, type LocationRequest, type LocationResult } from "../ipc/api";
import { displayCrsId } from "../map/crs";
import { centerOn, clearMarker, getActiveCrs, showMarker } from "../map/mapController";
import { useApp } from "../store/store";
import { useSettings } from "../store/settings";
import { readCoordFormat } from "../statusbar/coordFormat";
import { GOTO_KINDS, kindForStatusFormat, readSavedKind, saveKind, useGoTo, type GoToKind } from "./gotoStore";
import { splitPasted } from "./split";

interface Fields {
  lat: string;
  lon: string;
  x: string;
  y: string;
  zone: string;
  hemisphere: "N" | "S";
  easting: string;
  northing: string;
  text: string;
  datum: string;
  scale: string;
}
const BLANK: Fields = { lat: "", lon: "", x: "", y: "", zone: "", hemisphere: "N", easting: "", northing: "", text: "", datum: "wgs84", scale: "" };

/** The scale box: empty keeps the current scale; "5000" or "1:5000" sets 1:5000. Null = not a usable scale. */
export function parseScale(s: string): number | null | undefined {
  const t = s.trim();
  if (!t) return undefined;
  const m = t.match(/^(?:1\s*:\s*)?([0-9][0-9 ,_]*)$/);
  const v = m ? Number(m[1].replace(/[ ,_]/g, "")) : NaN;
  return Number.isFinite(v) && v >= 1 ? v : null;
}

/** The map's CRS as the engine should see it (the one the map is drawn in, so a world view is honoured); null while the map has none. */
function engineMapCrs(): string | null {
  const s = useApp.getState();
  return s.mapCrs ? displayCrsId(s.mapCrs.id, s.worldView) : null;
}

export function GoToPanel() {
  const statusFormat = useSettings((st) => readCoordFormat(st.settings.coordinateFormat));
  const mapCrs = useApp((s) => s.mapCrs);
  useApp((s) => s.worldView);
  const marker = useGoTo((s) => s.marker);
  const setOpen = useGoTo((s) => s.setOpen);
  const [kind, setKind] = useState<GoToKind>(() => readSavedKind() ?? kindForStatusFormat(statusFormat));
  const [f, setF] = useState<Fields>(BLANK);
  const [otherCrs, setOtherCrs] = useState<{ id: string; name: string } | null>(null);
  const [pickOpen, setPickOpen] = useState(false);
  const [res, setRes] = useState<LocationResult | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const first = useRef<HTMLInputElement | null>(null);
  const seq = useRef(0);
  const set = (p: Partial<Fields>) => {
    setNote(null);
    setF((o) => ({ ...o, ...p }));
  };

  useEffect(() => first.current?.focus(), [kind]);

  const scale = parseScale(f.scale);
  const request = useMemo<LocationRequest | null>(() => {
    const t = (s: string) => s.trim() !== "";
    switch (kind) {
      case "latlon": return t(f.lat) && t(f.lon) ? { kind, lat: f.lat, lon: f.lon, datum: f.datum } : null;
      case "xy": return t(f.x) && t(f.y) ? { kind, x: f.x, y: f.y } : null;
      case "crs": return t(f.x) && t(f.y) && otherCrs ? { kind, x: f.x, y: f.y, crs: otherCrs.id } : null;
      case "utm": return t(f.zone) && t(f.easting) && t(f.northing) ? { kind, zone: f.zone, hemisphere: f.hemisphere, easting: f.easting, northing: f.northing, datum: f.datum } : null;
      case "usng": return t(f.text) ? { kind, text: f.text, datum: f.datum } : null;
    }
  }, [kind, f, otherCrs]);

  // Read the input as it is typed (after a short pause); the engine does the reading, never this panel.
  useEffect(() => {
    const my = ++seq.current;
    if (!request) {
      setRes(null);
      setErr(null);
      return;
    }
    const h = setTimeout(() => {
      api
        .parseLocation(engineMapCrs(), request)
        .then((r) => {
          if (my !== seq.current) return;
          setRes(r);
          setErr(null);
        })
        .catch((e) => {
          if (my !== seq.current) return;
          setRes(null);
          setErr(errorText(e));
        });
    }, 200);
    return () => clearTimeout(h);
  }, [request, mapCrs?.id]);

  const scaleBad = scale === null;
  const canGo = !!res && res.mapX !== null && res.mapY !== null && !scaleBad;

  const go = async () => {
    const st = useApp.getState();
    if (!res || res.mapX === null || res.mapY === null) return;
    if (scaleBad) return;
    const want = engineMapCrs();
    if (!want || getActiveCrs()?.id !== want) {
      st.setStatus("The map is still switching to its coordinate system. Try again in a moment.");
      return;
    }
    try {
      await centerOn([res.mapX, res.mapY], scale ?? undefined);
      showMarker([res.mapX, res.mapY], `${res.lat.toFixed(6)}, ${res.lon.toFixed(6)} (WGS 84)`);
      useGoTo.getState().setMarker({ mapId: st.activeMapId, lon: res.lon, lat: res.lat });
      st.setStatus(`Went to ${res.lat.toFixed(6)}, ${res.lon.toFixed(6)} (WGS 84). The marker is temporary: Clear removes it.`);
    } catch (e) {
      st.setStatus(`Could not go there: ${errorText(e)}`);
    }
  };

  const clear = () => {
    clearMarker(useGoTo.getState().marker?.mapId);
    useApp.getState().setStatus("Marker cleared.");
  };

  const copy = async (label: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(label);
      useApp.getState().setStatus(`Copied: ${text}`);
    } catch {
      useApp.getState().setStatus("Copying isn't available here: select the text and copy it by hand.");
    }
  };

  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    // Only keys pressed inside the panel (the coordinate-system dialog is a portal whose Escape belongs to it).
    if (!e.currentTarget.contains(e.target as Node)) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
    } else if (e.key === "Enter" && !(e.target as HTMLElement).closest("button")) {
      e.preventDefault();
      void go();
    }
  };

  const pasteInto = (e: ClipboardEvent<HTMLInputElement>) => {
    const s = splitPasted(e.clipboardData.getData("text"));
    if (!s) return;
    e.preventDefault();
    if ("error" in s) setNote(s.error);
    else {
      setF((o) => ({ ...o, lat: s.lat, lon: s.lon }));
      setNote(s.note);
    }
  };
  // Typing a comma or semicolon into a box also splits a "lat, lon" pair.
  const typedInto = (which: "lat" | "lon", v: string) => {
    if (/[,;]/.test(v)) {
      const s = splitPasted(v);
      if (s && !("error" in s)) {
        setF((o) => ({ ...o, lat: s.lat, lon: s.lon }));
        setNote(s.note);
        return;
      }
    }
    set({ [which]: v } as Partial<Fields>);
  };

  const mapName = mapCrs?.name ?? null;
  const datumField = (opts: [string, string][]) => (
    <label className="goto-row">
      <span>Datum</span>
      <select value={f.datum} onChange={(e) => set({ datum: e.target.value })}>
        {opts.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
    </label>
  );
  const box = (label: string, key: keyof Fields, placeholder: string, extra?: { ref?: boolean; onPaste?: boolean; narrow?: boolean }) => (
    <label className={"goto-row" + (extra?.narrow ? " narrow" : "")}>
      <span>{label}</span>
      <input
        ref={extra?.ref ? first : undefined}
        value={f[key]}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => (extra?.onPaste ? typedInto(key as "lat" | "lon", e.target.value) : set({ [key]: e.target.value } as Partial<Fields>))}
        onPaste={extra?.onPaste ? pasteInto : undefined}
      />
    </label>
  );

  return (
    <div className="goto-panel" role="region" aria-label="Go To Coordinates" data-testid="goto-panel" onMouseDown={(e) => e.stopPropagation()} onKeyDown={onKey}>
      <div className="mp-head">
        <strong>Go To Coordinates</strong>
        <button className="mp-x" title="Close (Esc)" aria-label="Close Go To Coordinates" onClick={() => setOpen(false)}>
          <DismissRegular />
        </button>
      </div>
      <label className="goto-row">
        <span>Format</span>
        <select value={kind} aria-label="Format of the position" onChange={(e) => { const k = e.target.value as GoToKind; setKind(k); saveKind(k); setRes(null); setErr(null); setNote(null); }}>
          {GOTO_KINDS.map((k) => <option key={k.id} value={k.id}>{k.label}</option>)}
        </select>
      </label>

      {kind === "latlon" && (
        <>
          {box("Latitude", "lat", "34.0522 or 34°03′08″ N", { ref: true, onPaste: true })}
          {box("Longitude", "lon", "-118.2437 or 118°14′37″ W", { onPaste: true })}
          {datumField([["wgs84", "WGS 84 (EPSG:4326)"], ["nad83", "NAD 83 (EPSG:4269)"]])}
          <div className="mp-hint">Latitude first, then longitude. Use a minus sign or a letter (N, S, E, W). Paste "34.05, -118.25" into either box to fill both.</div>
        </>
      )}
      {kind === "xy" && (
        <>
          {box("X (easting)", "x", "500000", { ref: true })}
          {box("Y (northing)", "y", "4000000")}
          <div className="mp-hint">{mapName ? `In the map's coordinate system: ${mapName}.` : "This map has no coordinate system yet. Set one first (Map, Coordinate System)."}</div>
        </>
      )}
      {kind === "crs" && (
        <>
          <div className="goto-row">
            <span>Coordinate system</span>
            <Button size="small" onClick={() => setPickOpen(true)} aria-label="Choose the coordinate system of x and y">{otherCrs ? otherCrs.name : "Choose…"}</Button>
          </div>
          {box("X (easting)", "x", "500000", { ref: true })}
          {box("Y (northing)", "y", "4000000")}
        </>
      )}
      {kind === "utm" && (
        <>
          {box("Zone", "zone", "1 to 60", { ref: true, narrow: true })}
          <label className="goto-row narrow">
            <span>Hemisphere</span>
            <select value={f.hemisphere} onChange={(e) => set({ hemisphere: e.target.value as "N" | "S" })}>
              <option value="N">North</option>
              <option value="S">South</option>
            </select>
          </label>
          {box("Easting (m)", "easting", "500000")}
          {box("Northing (m)", "northing", "4000000")}
          {datumField([["wgs84", "WGS 84 (EPSG:326xx / 327xx)"], ["nad83", "NAD 83 (EPSG:269xx, zones 1-23 N)"]])}
        </>
      )}
      {kind === "usng" && (
        <>
          {box("Reference", "text", "15T VK 99976 82994", { ref: true })}
          {datumField([["wgs84", "WGS 84"], ["nad83", "NAD 83"], ["auto", "Automatic (NAD 83 in zones 1-23 N, else WGS 84)"]])}
          <div className="mp-hint">The position is the centre of the named square. Polar references (A, B, Y, Z) are on WGS 84.</div>
        </>
      )}

      <label className="goto-row narrow">
        <span>Scale 1:</span>
        <input value={f.scale} placeholder="keep current" spellCheck={false} aria-invalid={scaleBad} onChange={(e) => set({ scale: e.target.value })} />
      </label>
      {scaleBad && <div className="goto-err" role="alert">The scale should be a number such as 5000 (meaning 1:5000), or left empty.</div>}
      {note && <div className="mp-hint" role="status">{note}</div>}

      <div aria-live="polite" className="goto-result">
        {!request && <div className="mp-hint">Type a position to see how it is read.</div>}
        {err && <div className="goto-err" role="alert" data-testid="goto-error">{err}</div>}
        {res && (
          <>
            <div className="goto-reading" data-testid="goto-reading">{res.reading}</div>
            {res.mapMessage && <div className="goto-err" role="alert" data-testid="goto-map-message">{res.mapMessage}</div>}
            <table className="goto-formats">
              <caption className="sr-only">The same place in other formats</caption>
              <tbody>
                {res.formats.map((fm) => (
                  <tr key={fm.id}>
                    <th scope="row">{fm.label}</th>
                    <td className="mp-coords">{fm.text}</td>
                    <td>
                      <button className="mp-x" title={`Copy: ${fm.text}`} aria-label={`Copy ${fm.label}`} onClick={() => void copy(fm.label, fm.text)}>
                        <CopyRegular />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {copied && <span className="sr-only" role="status">Copied {copied}</span>}
          </>
        )}
      </div>

      <div className="goto-actions">
        <Button appearance="primary" disabled={!canGo} onClick={() => void go()} title="Centre the map on this place and drop a temporary marker (Enter)">Go</Button>
        <Button disabled={!marker} onClick={clear} title="Remove the temporary marker">Clear</Button>
        <Button onClick={() => setOpen(false)} title="Close (Esc)">Close</Button>
      </div>

      <Dialog open={pickOpen} onOpenChange={(_, d) => setPickOpen(d.open)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>Coordinate system of x and y</DialogTitle>
            <DialogContent>
              <CrsPicker value={otherCrs?.id ?? null} onChange={(id, name) => { setOtherCrs({ id, name }); setPickOpen(false); }} />
            </DialogContent>
            <DialogActions><Button onClick={() => setPickOpen(false)}>Cancel</Button></DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </div>
  );
}
