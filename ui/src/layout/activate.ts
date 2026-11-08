// Activating a map frame in the layout editor (UR-18): drag pans the map inside the frame, the wheel zooms at the cursor, Esc ends it.
// The pure part: turning a move or a wheel turn on the page into the frame's new centre and scale. `upm` is the ground one millimetre
// of the frame covers, in the frame CRS's units [east, north] (the engine's `layout_frame_units_per_mm`); `rotation` is the frame's
// rotation on the page in degrees, clockwise.

export type Upm = [number, number];

/** A page-space vector (mm, x right, y down) seen from inside a frame turned `rotation` degrees clockwise. */
export function intoFrame(dx: number, dy: number, rotation: number): [number, number] {
  const a = (rotation * Math.PI) / 180;
  return [dx * Math.cos(a) + dy * Math.sin(a), -dx * Math.sin(a) + dy * Math.cos(a)];
}

/** The centre after dragging the map by (dxMm, dyMm) on the page: the ground under the pointer stays under it. */
export function panCenter(center: [number, number], dxMm: number, dyMm: number, upm: Upm, rotation = 0): [number, number] {
  const [x, y] = intoFrame(dxMm, dyMm, rotation);
  return [center[0] - x * upm[0], center[1] + y * upm[1]];
}

/**
 * Zoom by `factor` (above 1 = closer) around the point (offX, offY) mm from the frame's centre on the page: the ground under that
 * point stays under it. Returns the new centre and scale (scale / factor).
 */
export function zoomCenter(center: [number, number], scale: number, factor: number, offX: number, offY: number, upm: Upm, rotation = 0): { center: [number, number]; scale: number } {
  const [x, y] = intoFrame(offX, offY, rotation);
  const k = 1 - 1 / factor;
  return { center: [center[0] + x * upm[0] * k, center[1] - y * upm[1] * k], scale: scale / factor };
}

/** One wheel event's zoom factor: a notch is a quarter step closer or away. */
export const frameWheelFactor = (deltaY: number) => Math.exp(-Math.max(-60, Math.min(60, deltaY)) * 0.0025);

/** The scale limits of an activated frame, so a wheel can't run it off the end of the numbers. */
export const MIN_SCALE = 1;
export const MAX_SCALE = 1e9;
export const clampScale = (s: number) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, s));

/** Why a frame can't be activated, or null. */
export function activationRefusal(f: { locked: boolean; followsSeries?: boolean; crs: string; name?: string; id: string }): string | null {
  const who = f.name || f.id;
  if (f.locked) return `${who} is locked, so its map can't be moved. Unlock it (Arrange › Lock) and try again.`;
  if (f.followsSeries) return `${who} moves with the map series, so its extent comes from the series. Change the series settings instead.`;
  if (!f.crs) return `${who} has no coordinate system yet. Choose one (Map Frame › Coordinate System) before moving its map.`;
  return null;
}
