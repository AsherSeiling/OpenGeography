import { Button } from "@fluentui/react-components";
import { Component, type ReactNode } from "react";
import { reportError } from "./store";

/** Keeps a crash inside one dockview pane: the pane shows a card, the rest of the app keeps working. */
export class PaneBoundary extends Component<{ pane: string; title: string; children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: { componentStack?: string | null }) {
    const e = Object.assign(new Error(error.message), { stack: `${error.stack ?? ""}\n${info.componentStack ?? ""}` });
    reportError(e, `pane:${this.props.pane}`, { title: `The ${this.props.title} pane hit an error`, message: "This pane stopped working. Your data is not changed and the rest of the app keeps running. Use Reopen on the pane to try again; if it keeps failing, copy the details below when you report it.", pane: this.props.pane, context: `showing the ${this.props.title} pane` });
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 8, alignItems: "flex-start", font: "13px system-ui" }}>
        <b>This pane hit an error</b>
        <span style={{ opacity: 0.8 }}>The {this.props.title} pane stopped. The rest of the app still works.</span>
        <Button appearance="primary" onClick={() => this.setState({ error: null })}>
          Reopen
        </Button>
      </div>
    );
  }
}
