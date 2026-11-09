// Edit a Definition Query node's query with the full query editor.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, MessageBar, MessageBarBody } from "@fluentui/react-components";
import { useState } from "react";
import { QueryEditor, type QueryLanguage } from "../query/QueryEditor";
import { approveAuthored } from "../scripts/script";
import { fieldsForPort, sourceLayerFor, specs, useModel } from "../store/model";

export function NodeQueryDialog({ nodeId, param, onClose }: { nodeId: string; param: string; onClose: () => void }) {
  const node = useModel((s) => s.nodes.find((n) => n.id === nodeId));
  const spec = node ? specs()[node.data.tool] : undefined;
  const p = spec?.params.find((x) => x.id === param);
  const port = p?.fieldOf ?? spec?.inputs[0]?.id ?? "in";
  const fields = fieldsForPort(nodeId, port);
  const layer = sourceLayerFor(nodeId, port);
  const [q, setQ] = useState<{ text: string; error: string | null; language: QueryLanguage }>({ text: String(node?.data.params[param] ?? ""), error: null, language: "sql" });
  // A node saved before Python was the default has no language: it is SQL and opens as SQL.
  const saved = String(node?.data.params[param] ?? "");
  const python = node?.data.params.language === "python";
  if (!node || !p) return null;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface className="dlg-mid">
        <DialogBody>
          <DialogTitle>{spec?.name}: {p.label}</DialogTitle>
          <DialogContent className="stack">
            {fields.length === 0 ? (
              <MessageBar intent="warning">
                <MessageBarBody>Connect an input to this node first, so its fields can be listed.</MessageBarBody>
              </MessageBar>
            ) : (
              <QueryEditor layerId={layer ?? ""} fields={fields} initialSql={python ? null : saved || null} initialPython={python ? saved || null : null} onChange={(text, error, language) => setQ({ text, error, language })} />
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button
              appearance="primary"
              disabled={!!q.error || !q.text}
              onClick={async () => {
                // A script written here is approved on this computer; one that arrived with a project waits (see the node's message).
                if (q.language === "python") await approveAuthored(q.text);
                useModel.getState().setParam(nodeId, "language", q.language);
                useModel.getState().setParam(nodeId, param, q.text);
                onClose();
              }}
            >
              OK
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
