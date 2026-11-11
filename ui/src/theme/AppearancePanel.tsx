import { DismissRegular } from "@fluentui/react-icons";
import { ColorField } from "./ColorField";
import { useEffect, useRef, type KeyboardEvent } from "react";
import { useApp } from "../store/store";
import { ACCENTS, FAMILIES, FAMILY_LABEL, HILITES, PALETTE_NAMES, closeAppearance, resetAppearance, setAppearance, useAppearance } from "./appearance";

function Swatches({ label, list, value, onPick }: { label: string; list: [string, string][]; value: string; onPick: (c: string) => void }) {
  return (
    <div className="apr-swatches" role="group" aria-label={label}>
      {list.map(([name, c]) => (
        <button key={c} type="button" className="apr-swatch" style={{ background: c }} aria-label={name} title={name} aria-pressed={value.toUpperCase() === c.toUpperCase()} onClick={() => onPick(c)} />
      ))}
      <label className="apr-custom">
        Custom
        <ColorField label={`${label}: custom colour`} alpha={false} value={value.toLowerCase()} onChange={onPick} />
      </label>
    </div>
  );
}

/** Floating, non-modal Colours panel (plan §2a). Every change applies to the real screen at once. */
export function AppearancePanel() {
  const open = useAppearance((s) => s.open);
  const { palette, accent, hilite, fam } = useAppearance();
  const dark = useApp((s) => s.dark);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>("button")?.focus();
    return () => prev?.focus?.();
  }, [open]);

  if (!open) return null;
  const onKey = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") closeAppearance();
  };

  return (
    <div className="apr" role="dialog" aria-label="Colours" ref={ref} onKeyDown={onKey}>
      <div className="apr-h">
        <span>Colours</span>
        <button type="button" className="apr-x" aria-label="Close colours" onClick={closeAppearance}>
          <DismissRegular fontSize={16} />
        </button>
      </div>
      <div className="apr-b">
        <section>
          <h3>Theme</h3>
          <div className="apr-seg" role="group" aria-label="Theme">
            {([false, true] as const).map((d) => (
              <button key={String(d)} type="button" aria-pressed={dark === d} onClick={() => useApp.setState({ dark: d })}>
                {d ? "Dark" : "Light"}
              </button>
            ))}
          </div>
        </section>
        <section>
          <h3>Panels</h3>
          <div className="apr-seg" role="group" aria-label="Panels">
            {PALETTE_NAMES.map((p) => (
              <button key={p} type="button" aria-pressed={palette === p} onClick={() => setAppearance({ palette: p })}>
                {p}
              </button>
            ))}
          </div>
        </section>
        <section>
          <h3>Accent colour</h3>
          <Swatches label="Accent colour" list={ACCENTS} value={accent} onPick={(c) => setAppearance({ accent: c })} />
        </section>
        <section>
          <h3>Highlight colour</h3>
          <Swatches label="Highlight colour" list={HILITES} value={hilite} onPick={(c) => setAppearance({ hilite: c })} />
          <p>Selected rows and selected node cards. Text on it stays readable by itself.</p>
        </section>
        <section>
          <h3>Tool colours</h3>
          <div className="apr-fams">
            {FAMILIES.map((k) => (
              <div key={k} className="apr-fam">
                <label htmlFor={`apr-fam-${k}`}>{FAMILY_LABEL[k]}</label>
                <ColorField label={FAMILY_LABEL[k]} alpha={false} value={fam[k].toLowerCase()} onChange={(v) => setAppearance({ fam: { [k]: v } })} />
              </div>
            ))}
          </div>
        </section>
      </div>
      <footer>
        <button type="button" className="apr-reset" onClick={resetAppearance}>
          Reset to default
        </button>
      </footer>
    </div>
  );
}
