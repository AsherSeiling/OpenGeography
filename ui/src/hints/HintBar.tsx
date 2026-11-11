// UX15: the tip card. Sits over the bottom of an empty node editor or layout without moving anything.
import { Button } from "@fluentui/react-components";
import { useSettings } from "../store/settings";
import { HINTS, hintVisible, useHints, type HintId } from "./hints";

export function HintBar({ id, empty }: { id: HintId; empty: boolean }) {
  const showHints = useSettings((s) => s.settings.showHints);
  const dismissed = useHints((s) => s.dismissed);
  const automated = typeof navigator !== "undefined" && navigator.webdriver === true;
  if (!hintVisible({ showHints, dismissed, id, empty, automated })) return null;
  const h = HINTS[id];
  return (
    <div className="hint-bar" role="note" aria-label={`Tip: ${h.title}`} data-testid={`hint-${id}`}>
      <div>
        <strong>Tip · {h.title}.</strong> {h.text}
      </div>
      <div className="btn-row">
        <Button size="small" appearance="primary" onClick={() => useHints.getState().dismiss(id)}>Got it</Button>
        <Button size="small" onClick={() => void useSettings.getState().update({ showHints: false })}>Turn off tips</Button>
      </div>
    </div>
  );
}
