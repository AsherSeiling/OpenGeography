import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Text } from "@fluentui/react-components";
import { useState } from "react";
import { useErrors, type AppError } from "./store";

/** Content shared by the dialog and the last-resort screen. */
export function ErrorBody({ error, remaining }: { error: AppError; remaining?: number }) {
  return (
    <>
      <Text block>{error.message}</Text>
      {error.count > 1 && (
        <Text block size={200} style={{ marginTop: 6, opacity: 0.7 }}>
          This happened {error.count} times.
        </Text>
      )}
      {!!remaining && (
        <Text block size={200} style={{ marginTop: 6, opacity: 0.7 }}>
          {remaining} more {remaining === 1 ? "error" : "errors"} waiting.
        </Text>
      )}
      <details style={{ marginTop: 12 }}>
        <summary style={{ cursor: "pointer", fontSize: 12 }}>Details</summary>
        <pre style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: 11, maxHeight: 220, overflow: "auto", background: "var(--colorNeutralBackground3, #f3f3f3)", padding: 8, margin: "6px 0 0" }}>{error.details}</pre>
      </details>
    </>
  );
}

export async function copyDetails(error: AppError) {
  const text = `${error.title}\n${error.message}\n\n${error.details}`;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard blocked: nothing else to do.
  }
}

export function ErrorDialog() {
  const error = useErrors((s) => s.queue[0]);
  const more = useErrors((s) => s.queue.length - 1);
  const dismiss = useErrors((s) => s.dismiss);
  const [copied, setCopied] = useState(false);
  if (!error) return null;
  return (
    <Dialog open modalType="alert" onOpenChange={(_, d) => !d.open && dismiss()}>
      <DialogSurface style={{ maxWidth: 560 }}>
        <DialogBody>
          <DialogTitle>{error.title}</DialogTitle>
          <DialogContent>
            <ErrorBody error={error} remaining={more} />
          </DialogContent>
          <DialogActions>
            <Button
              onClick={async () => {
                await copyDetails(error);
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              }}
            >
              {copied ? "Copied" : "Copy details"}
            </Button>
            <Button onClick={() => location.reload()}>Reload</Button>
            <Button appearance="primary" onClick={dismiss}>
              Dismiss
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
