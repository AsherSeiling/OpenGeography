// Dev builds only, `?trace=layout`: logs every change of the Properties target,
// the layout selection and the open layout, each with a timestamp and call stack
// (bug U-5, case LP-0). Attach the console output to a report.
import { useLayouts } from "../store/layout";
import { useApp } from "../store/store";

const t0 = performance.now();
const stamp = () => `+${Math.round(performance.now() - t0)} ms`;
const where = () => (new Error().stack ?? "").split("\n").slice(3, 8).map((l) => l.trim()).join("\n    ");

export function startLayoutTrace() {
  if (!new URLSearchParams(window.location.search).get("trace")?.split(",").includes("layout")) return;
  const log = (what: string, value: unknown) => console.info(`[trace:layout] ${stamp()} ${what}`, value, `\n    ${where()}`);
  useApp.subscribe((s, p) => s.properties !== p.properties && log("properties", s.properties));
  useLayouts.subscribe((s, p) => {
    if (s.selected !== p.selected) log("selected", s.selected);
    if (s.id !== p.id) log("open layout", s.id);
  });
  log("trace on", null);
}
