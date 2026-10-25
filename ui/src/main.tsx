import { invoke } from "@tauri-apps/api/core";
import { Component, StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ErrorBody } from "./errors/ErrorDialog";
import { reportError, setErrorLog } from "./errors/store";
import "./theme/app.css";
import "./devhooks";

const inTauri = "__TAURI_INTERNALS__" in window;

/** Send UI errors to the app's log (visible when run from a terminal). */
export function report(level: "info" | "error", message: string) {
  if (inTauri) invoke("frontend_log", { level, message }).catch(() => {});
  else console[level === "error" ? "error" : "info"](message);
}

setErrorLog((m) => report("error", m));

window.addEventListener("error", (e) => {
  report("error", `${e.message} at ${e.filename}:${e.lineno}:${e.colno}`);
  reportError(e.error ?? e.message, "window", { context: `${e.filename}:${e.lineno}` });
});
window.addEventListener("unhandledrejection", (e) => {
  report("error", `Unhandled: ${e.reason instanceof Error ? e.reason.stack : String(e.reason)}`);
  reportError(e.reason, "promise");
});

/** Last resort: the whole UI failed to render. Per-pane boundaries catch pane crashes first. */
class Crash extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error) {
    report("error", `UI crashed: ${error.stack ?? error.message}`);
  }
  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    const e = { id: 0, key: "", count: 1, source: "root", title: "GIS Studio hit an error", message: "The window could not be drawn. Your project files are not changed.", details: error.stack ?? error.message };
    return (
      <div style={{ padding: 24, font: "13px system-ui", color: "#262626", maxWidth: 640 }}>
        <h2 style={{ marginTop: 0 }}>{e.title}</h2>
        <ErrorBody error={e} />
        <p>
          <button onClick={() => navigator.clipboard?.writeText(`${e.message}\n\n${e.details}`)}>Copy details</button> <button onClick={() => location.reload()}>Reload</button>
        </p>
      </div>
    );
  }
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Crash>
      <App />
    </Crash>
  </StrictMode>,
);

// Startup report: confirms in the log that the UI actually rendered.
setTimeout(() => {
  const r = document.querySelector(".ribbon")?.getBoundingClientRect();
  report("info", `UI ready: ${innerWidth}x${innerHeight}, ribbon ${r ? `${Math.round(r.width)}x${Math.round(r.height)}` : "missing"}, ${document.querySelectorAll("*").length} elements, ${location.href}`);
}, 3000);
