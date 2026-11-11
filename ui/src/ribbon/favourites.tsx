// UX13: favourite and recent tools on the ribbon. The same lists as the Shift+A menu (a star there pins a tool here), read from
// `toolMenuStore`; the tools come from `tool_specs` only. A button adds the tool to the open layer's node tree, as the menu does.
import { Menu, MenuItem, MenuList, MenuPopover, MenuTrigger, Tooltip } from "@fluentui/react-components";
import { ChevronDownRegular } from "@fluentui/react-icons";
import { showPane } from "../panes/panes";
import { ToolIcon } from "../nodes/toolIcon";
import { useToolMenu } from "../nodes/toolMenuStore";
import { editorVisible, placeTool, whenEditorVisible } from "../nodes/toolPlacement";
import { useModel, useRun } from "../store/model";
import { useApp } from "../store/store";

/** At most this many favourites get a button on the ribbon; the rest stay in the Shift+A menu and the Recent menu. */
export const RIBBON_FAVOURITES = 4;

export interface ToolRef {
  id: string;
  name: string;
}

/** The tools of `ids` that still exist (a tool removed from the app drops out), in order, at most `max`. */
export function knownTools(ids: readonly string[], specs: Record<string, { name: string; role?: string }>, max = Infinity): ToolRef[] {
  const out: ToolRef[] = [];
  for (const id of ids) {
    const s = specs[id];
    if (s && s.role !== "data" && id !== "layer_output" && id !== "add_to_map" && !out.some((o) => o.id === id)) out.push({ id, name: s.name });
    if (out.length >= max) break;
  }
  return out;
}

/** Add a tool to the open layer's node tree (opening the Node Editor if it is hidden). Says why when there is no tree to add it to. */
export async function addToolFromRibbon(tool: string) {
  const status = useApp.getState().setStatus;
  if (!useModel.getState().layer) {
    status("Select a layer first: a tool is added to the node tree of the active layer.");
    return;
  }
  useToolMenu.getState().addRecent(tool);
  if (!editorVisible()) {
    showPane("model");
    await whenEditorVisible();
  }
  placeTool(tool, { at: null, wire: null });
}

export function FavouriteTools() {
  const fav = useToolMenu((s) => s.fav);
  const recent = useToolMenu((s) => s.recent);
  const specs = useRun((s) => s.specs);
  const favs = knownTools(fav, specs, RIBBON_FAVOURITES);
  const recents = knownTools(recent, specs);
  if (!favs.length && !recents.length) return null;
  return (
    <>
      <span className="qa-sep" aria-hidden="true" />
      {favs.map((t) => (
        <Tooltip key={t.id} content={`Add ${t.name} to the active layer's node tree (pinned from the Add menu)`} relationship="description" withArrow>
          <button className="qa-btn" data-qa={`fav-${t.id}`} aria-label={t.name} onClick={() => void addToolFromRibbon(t.id)}>
            <ToolIcon tool={t.id} />
            <span className="qa-label">{t.name}</span>
          </button>
        </Tooltip>
      ))}
      {recents.length > 0 && (
        <Menu>
          <MenuTrigger disableButtonEnhancement>
            <button className="qa-btn" data-qa="recent-tools" aria-label="Recent tools" title="Tools you added recently">
              <span className="qa-label">Recent</span>
              <ChevronDownRegular />
            </button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              {recents.map((t) => (
                <MenuItem key={t.id} icon={<ToolIcon tool={t.id} />} onClick={() => void addToolFromRibbon(t.id)}>
                  {t.name}
                </MenuItem>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>
      )}
    </>
  );
}
