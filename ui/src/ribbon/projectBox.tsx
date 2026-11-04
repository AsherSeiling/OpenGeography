// The project's name with a menu (New, Open, Save As): a small box at the far left of the tab row, styled like the search box.
import { Menu, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { ChevronDownRegular } from "@fluentui/react-icons";
import { useApp } from "../store/store";
import { commands } from "./commands";

export const PROJECT_MENU = ["new", "open", "saveAs"];

export function ProjectBox() {
  const name = useApp((s) => s.projectDir?.split(/[\\/]/).filter(Boolean).pop()?.replace(/\.gproj$/, "") || "Untitled");
  return (
    <div className="proj-box">
      <Menu>
        <MenuTrigger disableButtonEnhancement>
          <button className="proj-btn" aria-label={`Project ${name}`} title="New, open or save the project as a new folder">
            <span className="proj-name">{name}</span>
            <ChevronDownRegular />
          </button>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            {PROJECT_MENU.map((id) => (
              <MenuItem key={id} icon={commands[id].icon} secondaryContent={commands[id].keys} onClick={() => commands[id].run()}>
                {commands[id].label}
              </MenuItem>
            ))}
          </MenuList>
        </MenuPopover>
      </Menu>
    </div>
  );
}
