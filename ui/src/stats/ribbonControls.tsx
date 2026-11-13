// Custom controls of the statistics ribbon tabs: the Add Card menu (every `Statistics ›` ToolSpec) and the note a not-yet-built tab shows.
import { Menu, MenuGroup, MenuGroupHeader, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { useMemo } from "react";
import { RIcon } from "../ribbon/icons/RIcon";
import { useRun } from "../store/model";
import { addStatCard, statTools } from "./actions";
import { useStats } from "./store";

/** Explore › Cards: pick a statistics tool; the card is added to the open sheet on the active layer. */
export function AddCardMenu({ only, label = "Add Card" }: { only?: string; label?: string }) {
  const specs = useRun((s) => s.specs);
  const open = useStats((s) => !!s.id);
  const groups = useMemo(() => {
    const by = new Map<string, ReturnType<typeof statTools>>();
    for (const t of statTools(specs).filter((t) => !only || t.category.includes(`› ${only}`))) {
      const sub = t.category.split("›").slice(1).join("›").trim() || "Statistics";
      by.set(sub, [...(by.get(sub) ?? []), t]);
    }
    return [...by];
  }, [specs]);
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" disabled={!open || groups.length === 0} title="Add a card to the sheet: any tool in the Statistics group, the same tools the node editor offers">
          <span className="rbtn-icon"><RIcon name="add-tool" /></span>
          <span className="rbtn-label">{label} ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {groups.map(([sub, tools]) => (
            <MenuGroup key={sub}>
              <MenuGroupHeader>{sub}</MenuGroupHeader>
              {tools.map((t) => (
                <MenuItem key={t.id} title={t.description} onClick={() => addStatCard(t.id)}>
                  {t.name}
                </MenuItem>
              ))}
            </MenuGroup>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** A tab whose tools arrive in a later package says so in plain text. It is not a control: nothing to click, nothing dead. */
export function ComingNote({ children }: { children: string }) {
  return <div className="stat-ribbon-note">{children}</div>;
}
