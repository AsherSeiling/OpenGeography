// Analysis › Tools: one dropdown per tool family. Picking a tool does what picking it in Shift+A does (addToolFromUi).
import { Menu, MenuGroup, MenuGroupHeader, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { useMemo } from "react";
import { useRun } from "../store/model";
import { RIcon } from "./icons/RIcon";
import { addToolFromUi } from "./addTool";
import { toolMenus } from "./toolMenus";

const ICON: Record<string, string> = { Vector: "intersect", Raster: "image", Hydrology: "globe", Tables: "attribute-table", "Coordinate Systems": "coordinate-system", Styling: "palette" };

export function ToolMenus() {
  const specs = useRun((s) => s.specs);
  const menus = useMemo(() => toolMenus(Object.values(specs)), [specs]);
  if (!menus.length) return <div className="shelf-empty">Tools load when the engine starts.</div>;
  // Small buttons stack in columns of three, as elsewhere on the ribbon.
  const cols: (typeof menus)[] = [];
  for (let i = 0; i < menus.length; i += 3) cols.push(menus.slice(i, i + 3));
  return (
    <div className="rg-cols" data-testid="tool-menus">
      {cols.map((c, ci) => (
        <div key={ci} className="rcol">
          {c.map((m) => (
            <Menu key={m.label}>
              <MenuTrigger disableButtonEnhancement>
                <button className="rbtn small" data-tool-menu={m.label} title={`${m.label} tools: pick one to add it to the active layer's node tree`}>
                  <span className="rbtn-icon"><RIcon name={ICON[m.label] ?? "add-tool"} /></span>
                  <span className="rbtn-label">{m.label} ▾</span>
                </button>
              </MenuTrigger>
              <MenuPopover style={{ maxHeight: 460, overflowY: "auto" }}>
                <MenuList>
                  {m.tools.map((t) => (
                    <MenuItem key={t.id} title={t.description} onClick={() => void addToolFromUi(t.id)}>{t.name}</MenuItem>
                  ))}
                  {m.subs.map((s) => (
                    <MenuGroup key={s.name}>
                      <MenuGroupHeader>{s.name}</MenuGroupHeader>
                      {s.tools.map((t) => (
                        <MenuItem key={t.id} title={t.description} onClick={() => void addToolFromUi(t.id)}>{t.name}</MenuItem>
                      ))}
                    </MenuGroup>
                  ))}
                </MenuList>
              </MenuPopover>
            </Menu>
          ))}
        </div>
      ))}
    </div>
  );
}
