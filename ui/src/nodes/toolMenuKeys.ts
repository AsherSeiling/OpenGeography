// Global Shift+A (or whatever the keymap binds to "add"): opens the tool menu from anywhere, with the Node Editor open, closed or in a
// hidden tab. Not while typing in a text box, except the existing Blender-style rule: a text box outside the editor that still has
// focus gives way when the pointer is over the canvas and nothing was typed in it for a moment.
import { useEffect } from "react";
import { actionFor, eventCombo, useKeymap } from "./keymap";
import { openToolMenu, useToolMenu, type MenuPos } from "./toolMenuStore";
import { editorHandle, editorVisible } from "./toolPlacement";

const TEXT = "textarea, [contenteditable], input:not([type=checkbox]):not([type=radio]):not([type=button]):not([type=range])";

/** Pointer position if it is over the visible canvas: where the menu opens and the node goes. */
export function pointerOnCanvas(p: MenuPos | null): MenuPos | null {
  const r = editorHandle()?.rect();
  return p && r && editorVisible() && p.x >= r.left && p.x <= r.right && p.y >= r.top && p.y <= r.bottom ? p : null;
}

export function useToolMenuShortcut() {
  useEffect(() => {
    let pointer: MenuPos | null = null;
    let sawMove = false;
    let lastTyped = 0;
    const track = (e: MouseEvent) => {
      pointer = { x: e.clientX, y: e.clientY };
      if (e.type === "pointermove" || e.type === "mousemove") sawMove = true;
    };
    const pointerOver = (el: Element | null | undefined) => {
      const r = el?.getBoundingClientRect();
      return !!pointer && !!r && pointer.x >= r.left && pointer.x <= r.right && pointer.y >= r.top && pointer.y <= r.bottom;
    };
    /** A text field the keys belong to; inside a node card only while the pointer is still on that card. */
    const typingField = (): HTMLElement | null => {
      const field = (document.activeElement as HTMLElement | null)?.closest?.(TEXT) as HTMLElement | null;
      if (!field) return null;
      if (!field.closest(".model")) return field;
      return !sawMove || pointerOver(field.closest(".react-flow__node") ?? field) ? field : null;
    };
    const onKey = (e: KeyboardEvent) => {
      const combo = eventCombo(e);
      const wantsAdd = !!combo && actionFor(useKeymap.getState().bindings, combo) === "add";
      if (!wantsAdd) {
        if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && typingField()) lastTyped = Date.now();
        return;
      }
      const say = useToolMenu.getState().say;
      say("Shift+A received");
      if (editorHandle()?.busy()) return;
      if (document.querySelector(".toolmenu, [role=dialog]")) {
        say("Shift+A ignored: a menu or dialog is open");
        return;
      }
      const field = typingField();
      if (field) {
        const idle = Date.now() - lastTyped > 1200;
        if (!field.closest(".model") && sawMove && pointerOnCanvas(pointer) && idle) field.blur();
        else {
          say("Shift+A ignored: a text box has the keys (click the canvas first)");
          return; // the keys are for that field
        }
      }
      e.preventDefault();
      e.stopPropagation();
      // A node field the pointer has left lets go, so later shortcuts act on the tree.
      const active = document.activeElement as HTMLElement | null;
      if (active?.closest?.(".model .react-flow__node")) active.blur();
      openToolMenu(pointerOnCanvas(pointer));
    };
    window.addEventListener("keydown", onKey, true);
    for (const t of ["pointermove", "mousemove", "pointerdown"]) window.addEventListener(t, track as EventListener, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      for (const t of ["pointermove", "mousemove", "pointerdown"]) window.removeEventListener(t, track as EventListener, true);
    };
  }, []);
}
