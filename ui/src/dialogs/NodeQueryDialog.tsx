// Edit a Definition Query node's query with the full query editor.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, MessageBar, MessageBarBody } from "@fluentui/react-components";
import { useState } from "react";
import { QueryEditor } from "../query/QueryEditor";
import { fieldsForPort, sourceLayerFor, specs, useModel } from "../store/model";

export function NodeQueryDialog({ nodeId, param, onClose }: { nodeId: string; param: string; onClose: () => void }) {
  const node = useModel((s) => s.nodes.find((n) => n.id === nodeId));
  const spec = node ? specs()[node.data.tool] : undefined;
  const p = spec?.params.find((x) => x.id === param);
  const port = p?.fieldOf ?? spec?.inputs[0]?.id ?? "in";
  const fields = fieldsForPort(nodeId, port);
  const layer = sourceLayerFor(nodeId, port);
  const [q, setQ] = useState<{ sql: string; error: string | null }>({ sql: String(node?.data.params[param] ?? ""), error: null });
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
              <QueryEditor layerId={layer ?? ""} fields={fields} initialSql={String(node.data.params[param] ?? "") || null} onChange={(sql, error) => setQ({ sql, error })} />
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button
              appearance="primary"
              disabled={!!q.error || !q.sql}
              onClick={() => {
                useModel.getState().setParam(nodeId, param, q.sql);
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
