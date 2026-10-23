// Right-click menu on a node card: open its table, or compare it with its input.
import { Menu, MenuItem, MenuList, MenuPopover } from "@fluentui/react-components";
import { ArrowSwapRegular, TableRegular } from "@fluentui/react-icons";
import { viewNodeTable } from "../actions";
import { defaultCompareInput } from "../panes/table/history";
import { useModel, useRun } from "../store/model";

export function NodeMenu(p: { x: number; y: number; nodeId: string; onClose: () => void }) {
  const target = { getBoundingClientRect: () => ({ x: p.x, y: p.y, left: p.x, top: p.y, right: p.x, bottom: p.y, width: 0, height: 0, toJSON: () => null }) };
  const m = useModel.getState();
  const node = m.nodes.find((n) => n.id === p.nodeId);
  const spec = node && useRun.getState().specs[node.data.tool];
  const hasTable = !!spec && spec.output !== "raster" && !node!.data.muted;
  const hasInput = hasTable && !!defaultCompareInput({ nodes: m.nodes, edges: m.edges }, p.nodeId, useRun.getState().specs);
  return (
    <Menu open positioning={{ target: target as never, position: "below", align: "start" }} onOpenChange={(_, d) => !d.open && p.onClose()}>
      <MenuPopover>
        <MenuList>
          <MenuItem icon={<TableRegular />} disabled={!hasTable} onClick={() => viewNodeTable(p.nodeId)}>View table</MenuItem>
          <MenuItem icon={<ArrowSwapRegular />} disabled={!hasInput} onClick={() => viewNodeTable(p.nodeId, true)}>Compare with input</MenuItem>
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}
