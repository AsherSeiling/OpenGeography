// Add-on 7: the export panel with a pre-flight check. It reads the page and says, before anything is written, what the printed map
// would not tell its reader. The checks are advisory: every export button still works (the engine refuses what it cannot draw).
import { Button } from "@fluentui/react-components";
import { CheckmarkCircleRegular, DismissRegular, WarningRegular } from "@fluentui/react-icons";
import { crsName } from "../crs/crs";
import { mapOf } from "../store/maps";
import { useApp } from "../store/store";
import { useExportDpi } from "./exportDpi";
import { exportLayout } from "./actions";
import { allClear, preflight } from "./preflight";
import type { Layout } from "./types";
import { useLayoutView } from "./view";

export function PreflightPanel({ layout }: { layout: Layout }) {
  const maps = useApp((s) => s.maps);
  const dpi = useExportDpi((s) => s.dpi);
  const checks = preflight(layout, { mapExists: (id) => !!mapOf({ maps }, id), crsName: (id) => crsName(id) });
  const clear = allClear(checks);
  return (
    <section className="preflight" aria-label="Check before export" data-testid="preflight">
      <header>
        <b>Check before export</b>
        <span className={clear ? "pf-ok" : "pf-warn"}>{clear ? "Nothing to fix" : `${checks.filter((c) => !c.ok).length} to look at`}</span>
        <span className="grow" />
        <Button size="small" appearance="subtle" aria-label="Close the check" icon={<DismissRegular />} onClick={() => useLayoutView.getState().setPreflight(false)} />
      </header>
      <ul>
        {checks.map((c) => (
          <li key={c.id} className={c.ok ? "ok" : "warn"} data-check={c.id} data-ok={c.ok}>
            {c.ok ? <CheckmarkCircleRegular aria-label="Passed" /> : <WarningRegular aria-label="Needs attention" />}
            <span>{c.text}</span>
          </li>
        ))}
      </ul>
      <footer>
        <span className="muted">PNG and the images in a PDF: {dpi} dpi (Export › Quality)</span>
        <span className="grow" />
        {(["pdf", "png", "svg"] as const).map((f) => (
          <Button key={f} size="small" appearance={f === "pdf" ? "primary" : "secondary"} onClick={() => void exportLayout(f)}>Export {f.toUpperCase()}</Button>
        ))}
      </footer>
    </section>
  );
}
