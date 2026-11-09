// "Approve this script?": the full script and the plain warning. Offered whenever a script is waiting (a layer's Python definition query,
// a model node), never for a script the user just wrote here.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, MessageBar, MessageBarBody } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText } from "../ipc/api";
import { useScripts } from "../scripts/script";

export function ScriptApprovalDialog() {
  const request = useScripts((s) => s.request);
  const close = useScripts((s) => s.close);
  const [warning, setWarning] = useState("");
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!request) return;
    setErr(null);
    api.scriptTrustInfo(request.script).then((t) => setWarning(t.warning)).catch((e) => setErr(errorText(e)));
  }, [request]);
  if (!request) return null;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && close(false)}>
      <DialogSurface className="dlg-mid" data-testid="script-approval">
        <DialogBody>
          <DialogTitle>Approve this script?</DialogTitle>
          <DialogContent className="stack">
            <MessageBar intent="warning">
              <MessageBarBody data-testid="script-warning">{warning || "An approved script can do anything you can do on this computer."}</MessageBarBody>
            </MessageBar>
            <pre className="mono script-view" data-testid="script-text" tabIndex={0} aria-label="The script">{request.script}</pre>
            {err && (
              <MessageBar intent="error">
                <MessageBarBody>{err}</MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => close(false)}>Don't run it</Button>
            <Button
              appearance="primary"
              onClick={async () => {
                try {
                  await api.approveScript(request.script);
                  close(true);
                } catch (e) {
                  setErr(errorText(e));
                }
              }}
            >
              Approve and run
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
