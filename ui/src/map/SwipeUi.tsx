// Swipe compare (UX4) interface: the divider drawn over a map and the Map tab menu that chooses the layers.
import { Menu, MenuDivider, MenuItem, MenuItemCheckbox, MenuList, MenuPopover, MenuTrigger } from "@fluentui/react-components";
import type { KeyboardEvent, PointerEvent as RPointerEvent } from "react";
import { RIcon } from "../ribbon/icons/RIcon";
import { mapOf } from "../store/maps";
import { displayInfo, drawableLayerIds, useApp } from "../store/store";
import { useSwipe } from "./swipe";

/** The draggable divider. Left of it the chosen layers show; right of it, what is beneath them. */
export function SwipeDivider({ mapId }: { mapId: string }) {
  const st = useSwipe((s) => s.byMap[mapId]);
  if (!st) return null;
  const setPos = (p: number) => useSwipe.getState().setPos(mapId, p);
  const onDown = (e: RPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const wrap = e.currentTarget.parentElement!.getBoundingClientRect();
    const target = e.currentTarget;
    target.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => setPos((ev.clientX - wrap.left) / wrap.width);
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
      target.removeEventListener("pointercancel", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
    target.addEventListener("pointercancel", up);
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    if (e.key === "ArrowLeft") setPos(st.pos - step);
    else if (e.key === "ArrowRight") setPos(st.pos + step);
    else if (e.key === "Escape") useSwipe.getState().off(mapId);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  return (
    <div
      className="swipe-divider"
      style={{ left: `${st.pos * 100}%` }}
      role="slider"
      tabIndex={0}
      aria-label="Swipe divider"
      aria-orientation="horizontal"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(st.pos * 100)}
      title="Drag to compare. Arrow keys move it, Esc turns swipe off."
      data-testid="swipe-divider"
      onPointerDown={onDown}
      onMouseDown={(e) => e.stopPropagation()}
      onKeyDown={onKey}
    >
      <span className="swipe-grip" aria-hidden="true">⇄</span>
    </div>
  );
}

/** Map › Navigate › Swipe: tick the layers to show left of the divider; untick them all (or Turn Off) to end it. */
export function SwipeMenu() {
  const mapId = useApp((s) => s.activeMapId);
  const doc = useApp((s) => mapOf(s, s.activeMapId));
  const layers = useApp((s) => s.layers);
  const results = useApp((s) => s.results);
  const showOriginal = useApp((s) => s.showOriginal);
  const chosen = useSwipe((s) => s.byMap[mapId]?.layers) ?? [];
  // Layers that are drawn (shown, with a coordinate system, not tables): the ones a swipe can clip.
  const ids = doc ? drawableLayerIds(mapId) : [];
  const label = (id: string) => displayInfo({ layers, results, showOriginal }, id)?.name ?? id;
  return (
    <Menu checkedValues={{ layers: chosen }} onCheckedValueChange={(_, d) => useSwipe.getState().setLayers(mapId, d.checkedItems)}>
      <MenuTrigger>
        <button className={`rbtn large${chosen.length ? " active" : ""}`} title="Compare layers: the ticked layers show only left of a draggable divider" aria-label="Swipe" aria-pressed={chosen.length > 0}>
          <span className="rbtn-icon"><RIcon name="map" /></span>
          <span className="rbtn-label">Swipe ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {ids.length === 0 && <MenuItem disabled>No visible layers to compare</MenuItem>}
          {ids.map((id) => (
            <MenuItemCheckbox key={id} name="layers" value={id}>{label(id)}</MenuItemCheckbox>
          ))}
          {chosen.length > 0 && <MenuDivider />}
          {chosen.length > 0 && <MenuItem onClick={() => useSwipe.getState().off(mapId)}>Turn Off Swipe</MenuItem>}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}
