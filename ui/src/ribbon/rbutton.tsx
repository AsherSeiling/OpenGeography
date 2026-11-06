// One ribbon command as a button. Pressed ("on") commands show blue semibold text, never a fill or an outline.
import { Tooltip } from "@fluentui/react-components";
import { useState } from "react";
import type { Command } from "./commands";
import type { Size } from "./tabs";

export function RButton({ cmd, size, iconOnly }: { cmd: Command; size: Size; iconOnly?: boolean }) {
  const disabled = cmd.enabled ? !cmd.enabled() : false;
  const on = cmd.checked?.() ?? false;
  const [tipOpen, setTipOpen] = useState(false);
  const btn = (
    <button className={`rbtn ${size}${iconOnly ? " iconly" : ""}${on ? " on" : ""}`} disabled={disabled} onClick={() => cmd.run()} aria-pressed={cmd.checked ? on : undefined} aria-label={iconOnly ? cmd.label : undefined}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">{cmd.label}</span>
    </button>
  );
  const tip = [iconOnly ? cmd.label : null, cmd.tip, cmd.keys && `(${cmd.keys})`].filter(Boolean).join(" · ");
  // A button that disables itself when clicked (Clear Guides) gets no mouseleave, so its open tooltip would stay over the
  // ribbon tabs: the tooltip is held closed while the button is disabled.
  return tip ? (
    <Tooltip content={tip} relationship="description" withArrow visible={tipOpen && !disabled} onVisibleChange={(_, d) => setTipOpen(d.visible)}>
      {btn}
    </Tooltip>
  ) : (
    btn
  );
}
