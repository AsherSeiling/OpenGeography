// One ribbon command as a button. Pressed ("on") commands show blue semibold text, never a fill or an outline.
import { Tooltip } from "@fluentui/react-components";
import type { Command } from "./commands";
import type { Size } from "./tabs";

export function RButton({ cmd, size, iconOnly }: { cmd: Command; size: Size; iconOnly?: boolean }) {
  const disabled = cmd.enabled ? !cmd.enabled() : false;
  const on = cmd.checked?.() ?? false;
  const btn = (
    <button className={`rbtn ${size}${iconOnly ? " iconly" : ""}${on ? " on" : ""}`} disabled={disabled} onClick={() => cmd.run()} aria-pressed={cmd.checked ? on : undefined} aria-label={iconOnly ? cmd.label : undefined}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">{cmd.label}</span>
    </button>
  );
  const tip = [iconOnly ? cmd.label : null, cmd.tip, cmd.keys && `(${cmd.keys})`].filter(Boolean).join(" · ");
  return tip ? (
    <Tooltip content={tip} relationship="description" withArrow>
      {btn}
    </Tooltip>
  ) : (
    btn
  );
}
