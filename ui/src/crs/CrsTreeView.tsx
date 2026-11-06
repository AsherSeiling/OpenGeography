// The CRS picker's "Browse" view (UR-11): the tree from crsTree.ts, opened one branch at a time so the 7,000 systems are never all in the page.
import { ChevronDownRegular, ChevronRightRegular } from "@fluentui/react-icons";
import { useMemo, useState, type ReactElement } from "react";
import type { CrsEntry } from "./crs";
import { buildCrsTree, type CrsNode } from "./crsTree";

export function CrsTreeView({ list, value, onChange }: { list: CrsEntry[]; value: string | null; onChange: (id: string, name: string) => void }) {
  const tree = useMemo(() => buildCrsTree(list), [list]);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const toggle = (k: string) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const node = (n: CrsNode, level: number): ReactElement => {
    const isOpen = !!open[n.key];
    return (
      <div key={n.key} role="group">
        <div
          role="treeitem"
          aria-expanded={isOpen}
          aria-level={level}
          tabIndex={0}
          className="crs-row crs-branch"
          style={{ paddingLeft: 6 + (level - 1) * 16, cursor: "pointer" }}
          data-crs-node={n.key}
          onClick={() => toggle(n.key)}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggle(n.key))}
        >
          <span className="crs-code">{isOpen ? <ChevronDownRegular /> : <ChevronRightRegular />}</span>
          <span className="crs-name">{n.label}</span>
          <span className="crs-area">{n.count.toLocaleString()}</span>
        </div>
        {isOpen && n.children?.map((c) => node(c, level + 1))}
        {isOpen &&
          n.entries?.map((e) => (
            <div
              key={e.id}
              role="option"
              aria-selected={e.id === value}
              className={`crs-row ${e.id === value ? "sel" : ""}`}
              style={{ paddingLeft: 6 + level * 16 }}
              onClick={() => onChange(e.id, e.name)}
              onDoubleClick={() => onChange(e.id, e.name)}
            >
              <span className="crs-code">{e.id}</span>
              <span className="crs-name">{e.name}</span>
              <span className="crs-area">{e.area}</span>
            </div>
          ))}
      </div>
    );
  };
  return <div role="tree" aria-label="Coordinate systems by region">{tree.map((n) => node(n, 1))}</div>;
}
