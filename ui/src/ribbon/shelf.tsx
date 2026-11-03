// Analysis › Favourites: the tools starred in the Shift+A menu, then the ones used last. Replaces the old Tool Gallery.
import { useShallow } from "zustand/react/shallow";
import { useToolMenu } from "../nodes/toolMenuStore";
import { useRun } from "../store/model";
import { toolCommand } from "./commands";
import { RButton } from "./rbutton";

const SLOTS = 9; // three columns of three

export function Shelf() {
  const specs = useRun((s) => s.specs);
  const { fav, recent } = useToolMenu(useShallow((s) => ({ fav: s.fav, recent: s.recent })));
  const ids = [...new Set([...fav, ...recent])].filter((id) => specs[id] && specs[id].role !== "data").slice(0, SLOTS);
  if (!ids.length) return <div className="shelf-empty">Star a tool in the Add menu (Shift+A) and it stays here. Tools you use show up here too.</div>;
  return (
    <div className="shelf" data-testid="tool-shelf">
      {ids.map((id) => (
        <RButton key={id} cmd={toolCommand(id, specs[id].name, specs[id].description)} size="small" />
      ))}
    </div>
  );
}
