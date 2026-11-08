// Settings › Startup panes (UR-17): save the panes as they are now as the arrangement new projects open with, or go back to the built-in one.
import { Button, Field } from "@fluentui/react-components";
import { useState } from "react";
import { defaultLayout } from "../panes/panes";
import { resetStartupLayout, saveStartupLayout, useSettings } from "../store/settings";

export function StartupPanesSection() {
  const saved = useSettings((s) => !!s.settings.startupLayout);
  const [msg, setMsg] = useState<string | null>(null);
  return (
    <Field label="Startup panes" size="small" hint={saved ? "You saved a startup layout. New projects and empty starts open with it; a project keeps the layout it was saved with." : "New projects open with the built-in panes (Map Layers, Catalog, the map, Properties)."} validationMessage={msg ?? undefined} validationState={msg ? "error" : "none"}>
      <div className="btn-row">
        <Button size="small" onClick={async () => setMsg(await saveStartupLayout())} title="Arrange the panes first, then use this">Save the current panes as my startup layout</Button>
        <Button size="small" disabled={!saved} onClick={async () => { setMsg(await resetStartupLayout()); }}>Reset to the built-in panes</Button>
        <Button size="small" appearance="subtle" onClick={() => defaultLayout()} title="Rearrange the panes now to your startup layout (or the built-in one)">Apply now</Button>
      </div>
    </Field>
  );
}
