import { invoke } from "@tauri-apps/api/core";
import { Component, StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./theme/app.css";
import "./devhooks";

const inTauri = "__TAURI_INTERNALS__" in window;

/** Send UI errors to the app's log (visible when run from a terminal). */
export function report(level: "info" | "error", message: string) {
  if (inTauri) invoke("frontend_log", { level, message }).catch(() => {});
  else console[level === "error" ? "error" : "info"](message);
}

window.addEventListener("error", (e) => report("error", `${e.message} at ${e.filename}:${e.lineno}:${e.colno}`));
window.addEventListener("unhandledrejection", (e) => report("error", `Unhandled: ${e.reason instanceof Error ? e.reason.stack : String(e.reason)}`));

/** Show what went wrong instead of a blank window. */
class Crash extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error) {
    report("error", `UI crashed: ${error.stack ?? error.message}`);
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ padding: 24, font: "13px system-ui", color: "#262626" }}>
        <h2 style={{ marginTop: 0 }}>GIS Studio hit an error while starting</h2>
        <pre style={{ whiteSpace: "pre-wrap", background: "#f3f3f3", padding: 12 }}>{this.state.error.stack ?? this.state.error.message}</pre>
        <button onClick={() => location.reload()}>Reload</button>
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
