// Pointer-based drag and drop inside the app (tools and layers into the Model,
// layer reordering in Contents). HTML5 drag-and-drop can't be used: the
// desktop shell intercepts it to accept files dropped from the OS.
//
// Drop targets mark themselves with `data-drop="<kind>"` and listen for the
// "gis-drop" event, whose detail is a DropDetail.

export type DragPayload =
  | { kind: "tool"; id: string; label: string }
  | { kind: "layer"; id: string; label: string }
  /** A file or dataset from the Catalog pane; `id` is its path. */
  | { kind: "file"; id: string; label: string };

export interface DropDetail {
  payload: DragPayload;
  clientX: number;
  clientY: number;
}

const THRESHOLD = 4;
let suppressClick = false;

/** True right after a drag ends, so the click that follows can be ignored. */
export function justDragged() {
  return suppressClick;
}

/** Call from onPointerDown on a draggable item. */
export function startDrag(e: React.PointerEvent, payload: DragPayload) {
  if (e.button !== 0) return;
  const x0 = e.clientX;
  const y0 = e.clientY;
  let ghost: HTMLDivElement | null = null;
  let over: Element | null = null;

  const targetAt = (x: number, y: number) => document.elementFromPoint(x, y)?.closest("[data-drop]") ?? null;

  const move = (ev: PointerEvent) => {
    if (!ghost) {
      if (Math.hypot(ev.clientX - x0, ev.clientY - y0) < THRESHOLD) return;
      ghost = document.createElement("div");
      ghost.className = "drag-ghost";
      ghost.textContent = payload.label;
      document.body.appendChild(ghost);
      document.body.classList.add("dragging");
    }
    ghost.style.transform = `translate(${ev.clientX + 12}px, ${ev.clientY + 8}px)`;
    const t = targetAt(ev.clientX, ev.clientY);
    if (t !== over) {
      over?.classList.remove("drop-over");
      t?.classList.add("drop-over");
      over = t;
    }
    ev.preventDefault();
  };

  const up = (ev: PointerEvent) => {
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("keydown", cancel, true);
    over?.classList.remove("drop-over");
    if (!ghost) return; // it was a click
    ghost.remove();
    document.body.classList.remove("dragging");
    suppressClick = true;
    setTimeout(() => (suppressClick = false), 0);
    const t = targetAt(ev.clientX, ev.clientY);
    t?.dispatchEvent(new CustomEvent<DropDetail>("gis-drop", { bubbles: true, detail: { payload, clientX: ev.clientX, clientY: ev.clientY } }));
  };

  const cancel = (ev: KeyboardEvent) => {
    if (ev.key !== "Escape") return;
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("keydown", cancel, true);
    over?.classList.remove("drop-over");
    ghost?.remove();
    document.body.classList.remove("dragging");
  };

  // Capture phase: canvases like React Flow stop pointer events from bubbling.
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", up, true);
  window.addEventListener("keydown", cancel, true);
}
