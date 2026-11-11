// The app's own colour control (docs/COLOUR_AND_OUTLINE_PLAN.md, CP1): a swatch that opens a Fluent colour picker with
// transparency, a hex field, RGB fields, recent / project / Okabe-Ito colours and (where a setting is optional) a None button.
// It replaces every native colour input, so the operating system's own panel is never used.
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../store/store";
import { AlphaSlider, Button, ColorArea, ColorPicker, ColorSlider, Input, Popover, PopoverSurface, PopoverTrigger } from "@fluentui/react-components";
import { DEFAULT_COLOUR, OKABE_ITO, alphaToPct, formatColor, hsvToRgb, parseColor, pctToAlpha, recentColours, rememberColour, rgbToHsv, type Hsv, type Rgba } from "./color";

export interface ColorFieldProps {
  /** The stored colour (`#rrggbb` or `#rrggbbaa`). Empty or null means "none" when `optional`. */
  value: string | null | undefined;
  /** Called on every change while the picker is open (a live preview), with the new stored colour. */
  onChange: (value: string) => void;
  /** Called by the None button; giving it makes the setting optional (the button is shown). */
  onClear?: () => void;
  /** Screen-reader name of the swatch. */
  label: string;
  /** Offer transparency. Turn off where the renderer can't honour it (paper, app theme). Default true. */
  alpha?: boolean;
  /** The swatch colour to show and start from when `value` is empty. */
  fallback?: string;
  /** Colours this project already uses; by default those of the layers' styles. */
  projectColors?: string[];
  disabled?: boolean;
  className?: string;
}

const clampByte = (n: number) => Math.max(0, Math.min(255, Math.round(n)));

export function ColorField({ value, onChange, label, alpha = true, onClear, fallback = DEFAULT_COLOUR, projectColors, disabled, className }: ColorFieldProps) {
  const shown = parseColor(value) ?? parseColor(fallback) ?? { r: 0, g: 0, b: 0, a: 1 };
  const optional = !!onClear;
  const none = optional && !parseColor(value);
  const [open, setOpen] = useState(false);
  const start = useRef<string | null>(null);
  const [hsv, setHsv] = useState<Hsv>(() => rgbToHsv(shown));
  const [hexText, setHexText] = useState<string | null>(null);
  const [recent, setRecent] = useState<string[]>([]);
  const styles = useApp((s) => s.styles);
  const used = useMemo(() => {
    if (projectColors) return projectColors;
    const out: string[] = [];
    for (const st of Object.values(styles)) out.push(st.fill, st.stroke, ...(st.classes?.map((c) => c.color) ?? []), ...(st.colors ?? []));
    return out.map((c) => c?.toLowerCase());
  }, [styles, projectColors]);

  // Keep the picker in step with outside changes (undo, the Opacity field) without losing the hue of a grey.
  const stored = formatColor(shown);
  const lastStored = useRef(stored);
  useEffect(() => {
    if (stored !== lastStored.current) {
      lastStored.current = stored;
      setHsv((h) => {
        const mine = hsvToRgb(h);
        return formatColor(mine) === stored ? h : rgbToHsv(shown);
      });
    }
  }, [stored]); // eslint-disable-line react-hooks/exhaustive-deps

  const emit = (c: Rgba) => {
    const out = formatColor({ ...c, a: alpha ? c.a : 1 });
    lastStored.current = out;
    onChange(out);
  };
  const fromHsv = (h: Hsv) => {
    setHsv(h);
    setHexText(null);
    emit(hsvToRgb(h));
  };
  const fromRgb = (c: Rgba) => {
    setHsv((h) => {
      const next = rgbToHsv(c);
      return next.s === 0 || next.v === 0 ? { ...next, h: h.h } : next; // greys and black keep the hue the user was on
    });
    setHexText(null);
    emit(c);
  };

  const onOpen = (o: boolean) => {
    if (o) {
      start.current = parseColor(value) ? stored : null;
      setHsv(rgbToHsv(shown));
      setRecent(recentColours());
      setHexText(null);
    } else if (start.current !== (parseColor(value) ? stored : null)) {
      if (parseColor(value)) rememberColour(stored);
    }
    setOpen(o);
  };
  const cancel = () => {
    if (start.current === null) onClear?.();
    else onChange(start.current);
    setOpen(false);
  };

  const cur = hsvToRgb(hsv);
  const swatchCss = none ? undefined : `rgba(${shown.r},${shown.g},${shown.b},${shown.a})`;
  const palette = (title: string, colours: string[]) =>
    colours.length ? (
      <div className="cf-group">
        <div className="cf-group-title">{title}</div>
        <div className="cf-chips">
          {colours.map((c) => (
            <button key={c} type="button" className="cf-chip" aria-label={`Use ${c}`} title={c} onClick={() => { const p = parseColor(c); if (p) fromRgb(alpha ? p : { ...p, a: 1 }); }}>
              <span className="cf-chip-fill" style={{ background: c }} />
            </button>
          ))}
        </div>
      </div>
    ) : null;

  return (
    <Popover open={open} onOpenChange={(_, d) => (d.open ? onOpen(true) : onOpen(false))} positioning="below-start" trapFocus>
      <PopoverTrigger disableButtonEnhancement>
        <button type="button" className={`cf-swatch ${none ? "none" : ""} ${className ?? ""}`} aria-label={`${label}: ${none ? "none" : stored}`} title={`${label} (${none ? "none" : stored})`} disabled={disabled}>
          <span className="cf-swatch-fill" style={{ background: swatchCss }} />
        </button>
      </PopoverTrigger>
      <PopoverSurface className="cf-surface" aria-label={`${label} colour picker`} onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); cancel(); } }}>
        <ColorPicker color={hsv} onColorChange={(_, d) => fromHsv({ ...d.color, a: alpha ? d.color.a ?? 1 : 1 })}>
          <ColorArea className="cf-area" inputX={{ "aria-label": "Saturation" }} inputY={{ "aria-label": "Brightness" }} />
          <ColorSlider className="cf-slider" channel="hue" aria-label="Hue" />
          {alpha && <AlphaSlider className="cf-slider" aria-label="Opacity" />}
        </ColorPicker>
        <div className="cf-fields">
          <label className="cf-f cf-hex">
            <span>Hex</span>
            <Input
              size="small"
              aria-label={`${label} hex value`}
              value={hexText ?? formatColor({ ...cur, a: alpha ? cur.a : 1 })}
              onChange={(_, d) => {
                setHexText(d.value);
                const p = parseColor(d.value);
                if (p) {
                  setHsv(rgbToHsv(p));
                  emit(p);
                }
              }}
              onBlur={() => setHexText(null)}
            />
          </label>
          {(["r", "g", "b"] as const).map((k) => (
            <label key={k} className="cf-f cf-n">
              <span>{k.toUpperCase()}</span>
              <Input size="small" type="number" aria-label={`${label} ${k === "r" ? "red" : k === "g" ? "green" : "blue"}`} min={0} max={255} value={String(cur[k])} onChange={(_, d) => d.value !== "" && fromRgb({ ...cur, [k]: clampByte(Number(d.value)) })} />
            </label>
          ))}
          {alpha && (
            <label className="cf-f cf-n">
              <span>Opacity %</span>
              <Input size="small" type="number" aria-label={`${label} opacity percent`} min={0} max={100} value={String(alphaToPct(cur.a))} onChange={(_, d) => d.value !== "" && fromRgb({ ...cur, a: pctToAlpha(Number(d.value)) })} />
            </label>
          )}
        </div>
        {palette("Colour-blind-safe", OKABE_ITO)}
        {palette("This project", used.filter((c, i, a) => parseColor(c) && a.indexOf(c) === i).slice(0, 16))}
        {palette("Recent", recent)}
        <div className="cf-actions">
          {optional && (
            <Button size="small" appearance="subtle" onClick={() => { onClear?.(); setOpen(false); }}>
              None
            </Button>
          )}
          <Button size="small" appearance="subtle" onClick={cancel}>
            Cancel
          </Button>
          <Button size="small" appearance="primary" onClick={() => onOpen(false)}>
            Done
          </Button>
        </div>
      </PopoverSurface>
    </Popover>
  );
}
