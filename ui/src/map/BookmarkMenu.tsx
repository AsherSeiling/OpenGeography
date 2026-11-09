// Bookmarks (UX3): the Map tab menu (go to, add the current view, rename, delete) and the Map Frame menu (point the frame at one).
// Names are asked in our own dialog: window.prompt isn't available in the desktop webview.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field, Input, Menu, MenuDivider, MenuItem, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import { useState } from "react";
import { create } from "zustand";
import { RIcon } from "../ribbon/icons/RIcon";
import { errorText } from "../ipc/api";
import { setFrameFromBookmark } from "../layout/actions";
import { useLayouts } from "../store/layout";
import { useApp } from "../store/store";
import { extentIn, useBookmarks, type Bookmark } from "./bookmarks";
import { getViewExtentInMapCrs, zoomToBookmark } from "./mapController";

const status = (s: string) => useApp.getState().setStatus(s);

interface Ask {
  title: string;
  initial: string;
  onOk: (name: string) => void;
}
export const useNameAsk = create<{ ask: Ask | null; show: (a: Ask) => void; hide: () => void }>((set) => ({ ask: null, show: (ask) => set({ ask }), hide: () => set({ ask: null }) }));

function touch() {
  useApp.setState({ dirty: true });
}

export function addBookmarkFromView() {
  const view = getViewExtentInMapCrs();
  if (!view) return status("The map view isn't ready yet.");
  useNameAsk.getState().show({
    title: "Add Bookmark",
    initial: "",
    onOk: (name) => {
      const b = useBookmarks.getState().add(name, view.crs, view.extent);
      touch();
      status(`Bookmark "${b.name}" saved with the project.`);
    },
  });
}

export function goToBookmark(b: Bookmark) {
  zoomToBookmark((mapCrs) => extentIn(b, mapCrs)).catch((e) => status(`Could not go to "${b.name}": ${errorText(e)}`));
}

export function renameBookmark(b: Bookmark) {
  useNameAsk.getState().show({
    title: "Rename Bookmark",
    initial: b.name,
    onOk: (name) => {
      useBookmarks.getState().rename(b.id, name);
      touch();
    },
  });
}

export function deleteBookmark(b: Bookmark) {
  useBookmarks.getState().remove(b.id);
  touch();
}

/** Map › Navigate › Bookmarks. */
export function BookmarksMenu() {
  const items = useBookmarks((s) => s.items);
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" title="Save the current view as a named place and return to it later" aria-label="Bookmarks">
          <span className="rbtn-icon"><RIcon name="map" /></span>
          <span className="rbtn-label">Bookmarks ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          <MenuItem onClick={addBookmarkFromView}>Add Bookmark (current view)…</MenuItem>
          {items.length > 0 && <MenuDivider />}
          {items.map((b) => (
            <Menu key={b.id} openOnHover hoverDelay={150}>
              <MenuTrigger disableButtonEnhancement>
                <MenuItem onClick={() => goToBookmark(b)}>{b.name}</MenuItem>
              </MenuTrigger>
              <MenuPopover>
                <MenuList>
                  <MenuItem onClick={() => goToBookmark(b)}>Go to</MenuItem>
                  <MenuItem onClick={() => renameBookmark(b)}>Rename…</MenuItem>
                  <MenuItem onClick={() => deleteBookmark(b)}>Delete</MenuItem>
                </MenuList>
              </MenuPopover>
            </Menu>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Map Frame › Map › Bookmark: centre the selected frame on a bookmark and pick the scale that shows all of it. */
export function FrameBookmarkMenu() {
  const items = useBookmarks((s) => s.items);
  const hasFrame = useLayouts((s) => !!s.doc?.elements.some((e) => e.kind === "map_frame"));
  return (
    <Menu>
      <MenuTrigger>
        <button className="rbtn large" disabled={!hasFrame} title="Point the selected map frame at a saved bookmark" aria-label="Frame from bookmark">
          <span className="rbtn-icon"><RIcon name="map" /></span>
          <span className="rbtn-label">Bookmark ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {items.length === 0 && <MenuItem disabled>No bookmarks yet (Map tab › Bookmarks)</MenuItem>}
          {items.map((b) => (
            <MenuItem key={b.id} onClick={() => void setFrameFromBookmark(b.id)}>{b.name}</MenuItem>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

/** Asks for a bookmark's name. Mounted once in the app. */
export function BookmarkNameDialog() {
  const ask = useNameAsk((s) => s.ask);
  return ask ? <AskBody key={ask.title + ask.initial} ask={ask} /> : null;
}

function AskBody({ ask }: { ask: Ask }) {
  const hide = useNameAsk((s) => s.hide);
  const [name, setName] = useState(ask.initial);
  const [typed, setTyped] = useState(false); // no error on a dialog that has only just opened
  const empty = typed && !name.trim();
  const ok = () => {
    if (!name.trim()) return;
    ask.onOk(name.trim());
    hide();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && hide()}>
      <DialogSurface aria-label={ask.title}>
        <DialogBody>
          <DialogTitle>{ask.title}</DialogTitle>
          <DialogContent>
            <Field label="Name" validationMessage={empty ? "Give the bookmark a name." : undefined} validationState={empty ? "error" : "none"}>
              <Input autoFocus value={name} onChange={(_, d) => { setName(d.value); setTyped(true); }} onKeyDown={(e) => e.key === "Enter" && ok()} />
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="primary" disabled={!name.trim()} onClick={ok}>OK</Button>
            <Button onClick={hide}>Cancel</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
