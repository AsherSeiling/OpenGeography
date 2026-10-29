// Map → Datum Grids: the datum-shift grids PROJ can use, sorted by region and purpose. Grids the app ships are marked; others download
// on the user's click (checksum-verified) into the user's grid folder and work at once. CRS accuracy plan B1/B2.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Input, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, errorText, type DatumGrid } from "../ipc/api";
import { isCancelled, TaskProgress } from "../progress/progress";

const CONTINENTS = ["North America", "Europe", "Oceania", "South America", "Asia", "Africa", "Other"];
const KINDS: [DatumGrid["kind"] | "all", string][] = [
  ["all", "Every purpose"],
  ["horizontal", "Horizontal datum shift"],
  ["vertical", "Heights (geoid / vertical datum)"],
  ["3d", "3D / geocentric"],
  ["deformation", "Plate motion / deformation"],
  ["other", "Other"],
];
const KIND_ORDER: Record<string, number> = { horizontal: 0, vertical: 1, "3d": 2, deformation: 3, other: 4 };
const KIND_LABEL: Record<string, string> = { horizontal: "Horizontal shift", vertical: "Heights", "3d": "3D", deformation: "Plate motion", other: "Other" };
const SHOW: [string, string][] = [["all", "All grids"], ["installed", "Installed"], ["missing", "Not installed"], ["shipped", "Included with the app"]];
export const CONFIRM_BYTES = 50 * 1024 * 1024;

const mb = (b: number) => (b >= 1e6 ? `${(b / 1e6).toFixed(b >= 1e7 ? 0 : 1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

/** Grids in the order the list shows them: continent, country, purpose, then name. */
export function sortGrids(g: DatumGrid[]): DatumGrid[] {
  const ci = (c: string) => { const i = CONTINENTS.indexOf(c); return i < 0 ? CONTINENTS.length : i; };
  return [...g].sort((a, b) => ci(a.continent) - ci(b.continent) || a.country.localeCompare(b.country) || (KIND_ORDER[a.kind] ?? 9) - (KIND_ORDER[b.kind] ?? 9) || a.name.localeCompare(b.name));
}

export function filterGrids(g: DatumGrid[], q: { text: string; continent: string; kind: string; show: string }): DatumGrid[] {
  const t = q.text.trim().toLowerCase();
  return g.filter((x) =>
    (q.continent === "all" || x.continent === q.continent) &&
    (q.kind === "all" || x.kind === q.kind) &&
    (q.show === "all" || (q.show === "installed" ? x.installed : q.show === "missing" ? !x.installed : x.bundled)) &&
    (!t || `${x.name} ${x.country} ${x.area} ${x.from} ${x.to} ${x.description} ${x.source}`.toLowerCase().includes(t)),
  );
}

export function DatumGridsDialog({ onClose }: { onClose: () => void }) {
  const [grids, setGrids] = useState<DatumGrid[]>([]);
  const [text, setText] = useState("");
  const [continent, setContinent] = useState("all");
  const [kind, setKind] = useState("all");
  const [show, setShow] = useState("all");
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const reload = () => api.listDatumGrids().then(setGrids).catch((e) => setErr(errorText(e)));
  useEffect(() => { reload(); }, []);

  const rows = useMemo(() => filterGrids(sortGrids(grids), { text, continent, kind, show }), [grids, text, continent, kind, show]);
  const installed = grids.filter((g) => g.installed).length;

  const get = async (g: DatumGrid) => {
    if (g.size > CONFIRM_BYTES && confirm !== g.name) { setConfirm(g.name); return; }
    setConfirm(null);
    setBusy(g.name);
    setErr(null);
    try {
      await api.downloadDatumGrid(g.name, true);
      await reload();
    } catch (e) {
      if (!isCancelled(e)) setErr(errorText(e));
    }
    setBusy(null);
  };
  const remove = async (g: DatumGrid) => {
    setErr(null);
    try { await api.removeDatumGrid(g.name); await reload(); } catch (e) { setErr(errorText(e)); }
  };

  let lastCountry = "";
  let lastContinent = "";
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ maxWidth: 860, width: "92vw" }}>
        <DialogBody>
          <DialogTitle>Datum Grids</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">
              Datum grids make conversions between datums (for example NAD27 to NAD83) accurate. Without the right grid the program uses a rougher shift and says so.
              {` ${installed} of ${grids.length} are installed.`} Downloads come from the PROJ-data server, are checked against a checksum and work immediately.
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <Input style={{ flex: 1, minWidth: 180 }} placeholder="Search by country, datum or name" value={text} onChange={(_, d) => setText(d.value)} aria-label="Search grids" />
              <Dropdown aria-label="Region" value={continent === "all" ? "Every region" : continent} onOptionSelect={(_, d) => setContinent(d.optionValue!)}>
                <Option value="all">Every region</Option>
                {CONTINENTS.map((c) => (<Option key={c} value={c}>{c}</Option>))}
              </Dropdown>
              <Dropdown aria-label="Purpose" value={KINDS.find((k) => k[0] === kind)?.[1]} onOptionSelect={(_, d) => setKind(d.optionValue!)}>
                {KINDS.map(([v, l]) => (<Option key={v} value={v}>{l}</Option>))}
              </Dropdown>
              <Dropdown aria-label="Status" value={SHOW.find((k) => k[0] === show)?.[1]} onOptionSelect={(_, d) => setShow(d.optionValue!)}>
                {SHOW.map(([v, l]) => (<Option key={v} value={v}>{l}</Option>))}
              </Dropdown>
            </div>
            <div data-testid="grid-list" style={{ maxHeight: "50vh", overflow: "auto", border: "1px solid var(--border, #8884)", borderRadius: 4 }}>
              {rows.length === 0 && <div className="muted" style={{ padding: 12 }}>No grids match.</div>}
              {rows.map((g) => {
                const heads: ReactNode[] = [];
                if (g.continent !== lastContinent) { lastContinent = g.continent; lastCountry = ""; heads.push(<div key={`c-${g.continent}`} style={{ padding: "6px 10px", fontWeight: 700, background: "var(--subtle, #8882)" }}>{g.continent}</div>); }
                if (g.country !== lastCountry) { lastCountry = g.country; heads.push(<div key={`k-${g.continent}-${g.country}`} style={{ padding: "4px 10px 2px", fontWeight: 600 }}>{g.country}</div>); }
                return (
                  <div key={g.name}>
                    {heads}
                    <div data-grid={g.name} style={{ display: "grid", gridTemplateColumns: "1fr 90px 70px 150px", gap: 8, padding: "4px 10px", alignItems: "center", borderTop: "1px solid #8882" }}>
                      <div>
                        <div>{g.description || `${g.from} → ${g.to}`}{g.area ? <span className="muted"> · {g.area}</span> : null}</div>
                        <div className="muted mono" style={{ fontSize: 12 }}>{g.name} · {g.source}</div>
                      </div>
                      <div className="muted">{KIND_LABEL[g.kind] ?? g.kind}</div>
                      <div className="muted">{mb(g.size)}</div>
                      <div style={{ textAlign: "right" }}>
                        {g.installed ? (
                          g.location === "downloaded" ? (
                            <Button size="small" appearance="subtle" onClick={() => remove(g)} aria-label={`Remove ${g.name}`}>Installed · Remove</Button>
                          ) : (
                            <span className="muted">Included with the app</span>
                          )
                        ) : (
                          <Button size="small" appearance="primary" disabled={!!busy} onClick={() => get(g)} aria-label={`Download ${g.name}`}>
                            {busy === g.name ? "Downloading…" : confirm === g.name ? `Confirm ${mb(g.size)}` : "Download"}
                          </Button>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
            <TaskProgress active={!!busy} label="Downloading…" match={["Datum grid"]} />
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" disabled={!!busy} onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
