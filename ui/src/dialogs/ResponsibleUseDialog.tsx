// View ▸ Help: limitations and responsible use (CRS accuracy plan D3).
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from "@fluentui/react-components";
import { RESPONSIBLE_USE, RESPONSIBLE_USE_TITLE } from "../help/responsibleUse";

export function ResponsibleUseDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 720 }}>
        <DialogBody>
          <DialogTitle>{RESPONSIBLE_USE_TITLE}</DialogTitle>
          <DialogContent className="stack" data-testid="responsible-use">
            {RESPONSIBLE_USE.map((s) => (
              <div key={s.heading}>
                <div style={{ fontWeight: 600 }}>{s.heading}</div>
                <div>{s.text}</div>
              </div>
            ))}
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
