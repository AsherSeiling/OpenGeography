// What the ribbon does with a tool or with Public Data. Picking a tool from the search box or the favourites shelf places it exactly as
// the Shift+A menu does (nodes/toolPlacement.ts), after opening the Node Editor if it is closed.
import { showPane } from "../panes/panes";
import { useModel } from "../store/model";
import { useApp } from "../store/store";
import { useToolMenu } from "../nodes/toolMenuStore";
import { editorVisible, placeTool, whenEditorVisible } from "../nodes/toolPlacement";

/** Add `tool` to the open layer's node tree. Says so in the status bar when no layer is open: tools go into a layer's tree. */
export async function addToolFromUi(tool: string): Promise<void> {
  if (!useModel.getState().layer) {
    useApp.getState().setStatus("Select a layer in Map Layers first: tools go into that layer's node tree.");
    return;
  }
  useToolMenu.getState().addRecent(tool);
  if (!editorVisible()) {
    showPane("model");
    await whenEditorVisible();
  }
  placeTool(tool, { at: null, wire: null });
}

/** Map › Layer › Public Data: open the Catalog if it is closed, expand its Public Data branch and put the focus on that row. */
export function focusPublicData(): void {
  showPane("catalog");
  const t0 = Date.now();
  const find = () => {
    const row = document.querySelector<HTMLElement>('[role="treeitem"][data-node="public"]');
    if (!row) {
      if (Date.now() - t0 < 1500) window.setTimeout(find, 40);
      return;
    }
    if (row.getAttribute("aria-expanded") === "false") row.click();
    row.focus();
    row.scrollIntoView?.({ block: "nearest" });
    row.classList.add("flash");
    window.setTimeout(() => row.classList.remove("flash"), 900);
  };
  find();
}
