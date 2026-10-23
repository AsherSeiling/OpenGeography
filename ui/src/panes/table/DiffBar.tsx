// Summary bar for a table comparison: count chips that filter the grid, field
// changes, the "Match rows by" key picker and the engine's note.
import { Button, Dropdown, Option, Spinner, Tooltip } from "@fluentui/react-components";
import type { DiffSummary } from "../../ipc/api";
import type { KindFilter } from "./useNodeTable";

const CHIPS: { kind: "added" | "removed" | "changed"; sign: string; title: string }[] = [
  { kind: "added", sign: "+", title: "Rows only in" },
  { kind: "removed", sign: "−", title: "Rows only in" },
  { kind: "changed", sign: "~", title: "Rows whose values changed" },
];

export function DiffBar(p: {
  diff: DiffSummary | null;
  comparing: boolean;
  error: string | null;
  kind: KindFilter;
  onKind: (k: KindFilter) => void;
  keyChoice: string | null;
  onKey: (k: string | null) => void;
  keyFields: string[];
  aLabel: string;
  bLabel: string;
  onRetry: () => void;
}) {
  const d = p.diff;
  if (p.comparing) {
    return (
      <div className="diffbar">
        <Spinner size="tiny" label="Comparing tables…" />
      </div>
    );
  }
  if (p.error || !d) {
    return (
      <div className="diffbar err">
        <span>{p.error ?? "No comparison."}</span>
        <Button size="small" onClick={p.onRetry}>Compare again</Button>
      </div>
    );
  }
  const fieldChanges = d.fieldsAdded.length + d.fieldsRemoved.length + d.fieldsRetyped.length;
  const title = (c: (typeof CHIPS)[number]) => (c.kind === "added" ? `${c.title} ${p.bLabel}` : c.kind === "removed" ? `${c.title} ${p.aLabel}` : c.title);
  return (
    <div className="diffbar">
      {CHIPS.map((c) => (
        <Tooltip key={c.kind} content={`${title(c)} — click to show only these rows`} relationship="label">
          <button className={`diff-chip ${c.kind} ${p.kind === c.kind ? "on" : ""}`} onClick={() => p.onKind(p.kind === c.kind ? "all" : c.kind)}>
            {c.sign}{d[c.kind].toLocaleString()} {c.kind}
          </button>
        </Tooltip>
      ))}
      <span className="muted">{d.unchanged.toLocaleString()} unchanged</span>
      {fieldChanges > 0 && (
        <Tooltip
          content={[
            d.fieldsAdded.length ? `Added: ${d.fieldsAdded.join(", ")}` : "",
            d.fieldsRemoved.length ? `Removed: ${d.fieldsRemoved.join(", ")}` : "",
            ...d.fieldsRetyped.map((f) => `Retyped: ${f.name} (${f.from} → ${f.to})`),
          ].filter(Boolean).join(" · ")}
          relationship="description"
        >
          <span className="diff-chip fields">
            fields {d.fieldsAdded.length ? `+${d.fieldsAdded.length} ` : ""}{d.fieldsRemoved.length ? `−${d.fieldsRemoved.length} ` : ""}{d.fieldsRetyped.length ? `~${d.fieldsRetyped.length}` : ""}
          </span>
        </Tooltip>
      )}
      <span className="attr-label">Match rows by:</span>
      <Dropdown
        size="small"
        value={p.keyChoice ?? `Auto${d.keySource === "auto" && d.keyUsed ? ` (${d.keyUsed})` : d.keySource === "none" ? " (whole rows)" : ""}`}
        selectedOptions={[p.keyChoice ?? ""]}
        onOptionSelect={(_, o) => p.onKey(o.optionValue || null)}
        style={{ minWidth: 120 }}
      >
        <Option value="" text="Auto">Auto</Option>
        {p.keyFields.map((f) => (
          <Option key={f} value={f} text={f}>{f}</Option>
        ))}
      </Dropdown>
      {d.note && <span className="diff-note" title={d.note}>{d.note}</span>}
    </div>
  );
}
