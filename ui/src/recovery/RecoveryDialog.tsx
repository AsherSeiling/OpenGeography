// UX1: at start, offer the unsaved work a run that ended without a clean save left behind. Restore it, discard it, or decide later
// (it is kept until you do). The copies are the app's own; the project folders are never touched here.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, MessageBar, MessageBarBody } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { restoreRecovery } from "../actions";
import { api, errorText, type RecoveryEntry } from "../ipc/api";
import { describeEntry } from "./format";

export function RecoveryDialog() {
  const [entries, setEntries] = useState<RecoveryEntry[]>([]);
  const [open, setOpen] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.listRecovery().then((l) => {
      if (l.length) {
        setEntries(l);
        setOpen(true);
      }
    }, () => {});
  }, []);
  if (!open) return null;
  const discard = async (ids: string[]) => {
    setErr(null);
    try {
      for (const id of ids) await api.discardRecovery(id);
      const left = entries.filter((e) => !ids.includes(e.id));
      setEntries(left);
      if (!left.length) setOpen(false);
    } catch (e) {
      setErr(errorText(e));
    }
  };
  const restore = async (id: string) => {
    setBusy(true);
    setErr(null);
    const ok = await restoreRecovery(id);
    setBusy(false);
    if (ok) setOpen(false);
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && setOpen(false)}>
      <DialogSurface className="dlg-mid" data-testid="recovery-dialog">
        <DialogBody>
          <DialogTitle>Recover unsaved work?</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small">GIS Studio didn't close normally last time. It keeps an automatic copy of unsaved work, apart from your project folders. Restoring opens the copy as unsaved work; the original project is not changed until you save.</div>
            {entries.map((e) => (
              <div key={e.id} className="row2" data-testid="recovery-entry" style={{ alignItems: "center" }}>
                <div style={{ flex: 1 }}>{e.problem ? <MessageBar intent="warning"><MessageBarBody>{e.problem}</MessageBarBody></MessageBar> : describeEntry(e)}</div>
                {!e.problem && <Button size="small" appearance="primary" disabled={busy} onClick={() => void restore(e.id)}>Restore</Button>}
                <Button size="small" disabled={busy} onClick={() => void discard([e.id])}>Discard</Button>
              </div>
            ))}
            {err && <MessageBar intent="error"><MessageBarBody>{err}</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" disabled={busy} onClick={() => setOpen(false)}>Decide later</Button>
            {entries.length > 1 && <Button disabled={busy} onClick={() => void discard(entries.map((e) => e.id))}>Discard all</Button>}
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
