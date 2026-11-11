// UX8: the "Groups" button of the node editor: save the selected nodes as your own building block, insert a saved one, share files.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Input, Menu, MenuDivider, MenuItem, MenuList, MenuPopover, MenuTrigger, Textarea, Tooltip } from "@fluentui/react-components";
import { Box20Regular } from "@fluentui/react-icons";
import { useReactFlow } from "@xyflow/react";
import { useState } from "react";
import { api, errorText, type NodeGroupSummary } from "../ipc/api";
import { open, save } from "../ipc/dialogs";
import { useModel } from "../store/model";
import { useApp } from "../store/store";
import { groupProblem, insertGroup, openInputs, saveSelectionAsGroup } from "./groups";

const say = (t: string) => useApp.getState().setStatus(t);

export function GroupsMenu() {
  const flow = useReactFlow();
  const [items, setItems] = useState<NodeGroupSummary[]>([]);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  const hasTree = useModel((s) => !!s.layer);

  const refresh = async () => setItems(await api.nodeGroupsList().catch(() => []));

  const insert = async (g: NodeGroupSummary) => {
    try {
      const group = await api.nodeGroupGet(g.name);
      const centre = flow.screenToFlowPosition({ x: window.innerWidth / 2 - 200, y: window.innerHeight / 2 - 100 });
      const ids = insertGroup(group, centre);
      const todo = openInputs(group).length;
      say(`Inserted "${group.name}": ${ids.length} node(s)${todo ? `, ${todo} input(s) to connect` : ""}. Undo removes it in one step.`);
    } catch (e) {
      say(errorText(e));
    }
  };

  const doSave = async () => {
    try {
      const g = await saveSelectionAsGroup(name, desc);
      setNaming(false);
      say(`Saved "${g.name}" (${g.nodes.length} node(s)) to your node groups. Layers are not saved with it.`);
    } catch (e) {
      say(errorText(e));
    }
  };

  return (
    <>
      <Menu onOpenChange={(_, d) => d.open && void refresh()}>
        <MenuTrigger disableButtonEnhancement>
          <Tooltip content="Save the selected nodes as your own building block, or insert one you saved" relationship="label">
            <Button size="small" appearance="subtle" icon={<Box20Regular />} disabled={!hasTree} data-testid="groups-button">
              Groups
            </Button>
          </Tooltip>
        </MenuTrigger>
        <MenuPopover>
          <MenuList>
            <MenuItem
              onClick={() => {
                const problem = groupProblem(useModel.getState().nodes);
                if (problem) return say(problem);
                setName("");
                setDesc("");
                setNaming(true);
              }}
            >
              Save selection as a group…
            </MenuItem>
            <MenuDivider />
            {items.length === 0 && <MenuItem disabled>No saved groups yet</MenuItem>}
            {items.map((g) => (
              <MenuItem key={g.name} title={g.description || undefined} onClick={() => void insert(g)}>
                {g.name} · {g.nodes} node{g.nodes === 1 ? "" : "s"}
              </MenuItem>
            ))}
            <MenuDivider />
            <MenuItem
              onClick={async () => {
                try {
                  const p = (await open({ title: "Import a node group", multiple: false, filters: [{ name: "Node group", extensions: ["gisgroup"] }] })) as string | null;
                  if (!p) return;
                  const g = await api.nodeGroupImport(p);
                  say(`Imported "${g.name}" (${g.nodes.length} node(s)). It is now in your Groups list. Layers and unknown tools are checked, never trusted.`);
                } catch (e) {
                  say(errorText(e));
                }
              }}
            >
              Import a group file…
            </MenuItem>
            {items.length > 0 && (
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <MenuItem>Share a group as a file…</MenuItem>
                </MenuTrigger>
                <MenuPopover>
                  <MenuList>
                    {items.map((g) => (
                      <MenuItem
                        key={g.name}
                        onClick={async () => {
                          try {
                            const p = await save({ title: `Save "${g.name}" as a file`, defaultPath: `${g.name}.gisgroup`, filters: [{ name: "Node group", extensions: ["gisgroup"] }] });
                            if (!p) return;
                            await api.nodeGroupExport(g.name, p);
                            say(`Saved "${g.name}" to ${p}`);
                          } catch (e) {
                            say(errorText(e));
                          }
                        }}
                      >
                        {g.name}
                      </MenuItem>
                    ))}
                  </MenuList>
                </MenuPopover>
              </Menu>
            )}
            {items.length > 0 && (
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <MenuItem>Delete a saved group…</MenuItem>
                </MenuTrigger>
                <MenuPopover>
                  <MenuList>
                    {items.map((g) => (
                      <MenuItem
                        key={g.name}
                        onClick={async () => {
                          if (!window.confirm(`Delete the saved group "${g.name}"? Copies already in a tree stay. This can't be undone.`)) return;
                          try {
                            await api.nodeGroupDelete(g.name);
                            say(`Deleted "${g.name}"`);
                          } catch (e) {
                            say(errorText(e));
                          }
                        }}
                      >
                        {g.name}
                      </MenuItem>
                    ))}
                  </MenuList>
                </MenuPopover>
              </Menu>
            )}
          </MenuList>
        </MenuPopover>
      </Menu>
      <Dialog open={naming} onOpenChange={(_, d) => setNaming(d.open)}>
        <DialogSurface aria-label="Save node group">
          <DialogBody>
            <DialogTitle>Save selection as a group</DialogTitle>
            <DialogContent>
              <div className="stack">
                <label>
                  Name
                  <Input value={name} onChange={(_, d) => setName(d.value)} placeholder="e.g. Clean and dissolve" autoFocus />
                </label>
                <label>
                  What it does (optional)
                  <Textarea value={desc} onChange={(_, d) => setDesc(d.value)} />
                </label>
                <div className="muted">Saved with the nodes, their settings and the connections between them. Layers are left out; you connect the inputs when you insert it. A saved name is replaced when you save it again.</div>
              </div>
            </DialogContent>
            <DialogActions>
              <Button onClick={() => setNaming(false)}>Cancel</Button>
              <Button appearance="primary" disabled={!name.trim()} onClick={() => void doSave()}>
                Save
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
}
