// "Table of:" and "Compare with" pickers in the table toolbar.
import { Badge, Button, Dropdown, Option, Tooltip } from "@fluentui/react-components";
import { ArrowSwapRegular, DismissRegular } from "@fluentui/react-icons";
import { defaultCompareInput, useTableTarget, type PickerEntry } from "./history";
import { useRun, type Tree } from "../../store/model";

const badge = (e: PickerEntry) => (e.state === "outofdate" ? " · out of date" : e.state === "norun" && !e.disabled ? " · not run" : "");

export function NodePicker(p: { owner: string; tree: Tree | undefined; entries: PickerEntry[]; nodeId: string | null; compareId: string | null }) {
  const set = useTableTarget.getState().set;
  const specs = useRun((s) => s.specs);
  const current = p.entries.find((e) => e.id === p.nodeId);
  const compareChoices = p.entries.filter((e) => !e.disabled && e.id !== p.nodeId);
  const input = p.tree && p.nodeId ? defaultCompareInput(p.tree, p.nodeId, specs) : null;
  return (
    <>
      <span className="attr-label">Table of:</span>
      <Dropdown
        size="small"
        value={current ? current.label + badge(current) : "Layer (as stored)"}
        selectedOptions={[p.nodeId ?? ""]}
        onOptionSelect={(_, d) => set(d.optionValue ? { tree: p.owner, nodeId: d.optionValue, compareId: null } : null)}
        style={{ minWidth: 150 }}
      >
        <Option value="" text="Layer (as stored)">Layer (as stored)</Option>
        {p.entries.map((e, i) => (
          <Option key={e.id} value={e.id} text={e.label} disabled={e.disabled} title={e.reason}>
            {i + 1}. {e.label}
            {e.state === "outofdate" && <Badge size="small" appearance="tint" color="warning" style={{ marginLeft: 6 }}>out of date</Badge>}
            {e.state === "norun" && !e.disabled && <Badge size="small" appearance="tint" color="informative" style={{ marginLeft: 6 }}>not run</Badge>}
          </Option>
        ))}
      </Dropdown>
      {p.nodeId && (
        <>
          <span className="attr-label">Compare with:</span>
          {p.compareId ? (
            <Dropdown
              size="small"
              value={p.entries.find((e) => e.id === p.compareId)?.label ?? ""}
              selectedOptions={[p.compareId]}
              onOptionSelect={(_, d) => set({ tree: p.owner, nodeId: p.nodeId!, compareId: d.optionValue || null })}
              style={{ minWidth: 130 }}
            >
              <Option value="" text="None">None</Option>
              {compareChoices.map((e) => (
                <Option key={e.id} value={e.id} text={e.label}>
                  {e.label}
                  {e.id === input ? " (input)" : ""}
                </Option>
              ))}
            </Dropdown>
          ) : (
            <Tooltip content={input ? "Show what this step changed compared with its input" : "This node has no input to compare with"} relationship="label">
              <Button size="small" icon={<ArrowSwapRegular />} disabled={!input} onClick={() => set({ tree: p.owner, nodeId: p.nodeId!, compareId: input })}>
                Compare with input
              </Button>
            </Tooltip>
          )}
          {p.compareId && (
            <Tooltip content="Close the comparison" relationship="label">
              <Button size="small" icon={<DismissRegular />} onClick={() => set({ tree: p.owner, nodeId: p.nodeId!, compareId: null })} />
            </Tooltip>
          )}
        </>
      )}
    </>
  );
}
