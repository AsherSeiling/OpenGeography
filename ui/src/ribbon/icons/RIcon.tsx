// A full-colour ribbon icon (UR-23): drawn by scripts/ribbon_icons_lib.py, colours from the --ic-* variables (theme/tokens.css), so the
// light and dark themes need no second set. One component: a command names its icon once, as <RIcon name="add-data" />. The size
// is the font size of the place it sits in (1em); small places drop fine detail and thicken the line in ribbon.css.
import { RIBBON_ICONS } from "./generated";

export function RIcon({ name }: { name: string }) {
  const body = RIBBON_ICONS[name];
  if (!body && import.meta.env.DEV) console.warn(`No ribbon icon named "${name}"`);
  return <svg className="ric" viewBox="0 0 32 32" width="1em" height="1em" aria-hidden="true" focusable="false" dangerouslySetInnerHTML={{ __html: body ?? "" }} />;
}
