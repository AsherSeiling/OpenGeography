// Office / ArcGIS Pro style ribbon: tabs → labeled groups → large and small
// buttons. Contextual tabs appear when a layer is selected.
import { Combobox, Menu, MenuItem, MenuItemRadio, MenuList, MenuPopover, MenuTrigger, Option, Tooltip } from "@fluentui/react-components";
import { ImageMultipleRegular, SearchRegular } from "@fluentui/react-icons";
import { useMemo, useState, type ReactNode } from "react";
import { useRun } from "../store/model";
import { useNav } from "../nav/navStore";
import { useApp } from "../store/store";
import { BASEMAP_OPTIONS, commands, TOOL_BUTTONS, toolCommand, type Command } from "./commands";

type Size = "large" | "small";
type Item = { cmd: string; size?: Size } | { custom: ReactNode };
interface Group {
  label: string;
  items: Item[];
}

const TABS: Record<string, Group[]> = {
  Map: [
    { label: "Navigate", items: [{ cmd: "explore" }, { cmd: "fullExtent" }] },
    { label: "Layer", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "elevation" }, { custom: <BasemapGallery /> }] },
    {
      label: "Selection",
      items: [{ cmd: "select" }, { cmd: "selectBox", size: "small" }, { cmd: "selectByAttributes", size: "small" }, { cmd: "selectByLocation", size: "small" }, { cmd: "zoomSelection", size: "small" }, { cmd: "clearSelection" }],
    },
    { label: "Inquiry", items: [{ cmd: "identify" }, { cmd: "measure" }, { cmd: "measureArea", size: "small" }] },
    { label: "Land Nav", items: [{ cmd: "resection" }, { cmd: "intersection" }] },
    { label: "Coordinate System", items: [{ cmd: "mapCrs" }] },
  ],
  Insert: [
    { label: "Layouts", items: [{ cmd: "newLayout" }] },
    { label: "Maps", items: [{ cmd: "topoMap" }] },
  ],
  Analysis: [
    { label: "Geoprocessing", items: [{ cmd: "modelBuilder" }, { cmd: "runModel" }, { cmd: "tools" }] },
    { label: "Tools", items: [{ custom: <ToolGallery /> }] },
  ],
  Data: [
    { label: "Add", items: [{ cmd: "addData" }, { cmd: "catalog" }, { cmd: "tiger" }, { cmd: "nationalMap" }, { cmd: "elevation" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }] },
    { label: "Coordinate Systems", items: [{ cmd: "defineProjection" }, { cmd: "mapCrs" }] },
    { label: "Export", items: [{ cmd: "exportLayer" }, { cmd: "layerFromSelection" }] },
    { label: "Filter", items: [{ cmd: "definitionQuery" }] },
    { label: "Sources", items: [{ cmd: "sources" }] },
    { label: "Testing", items: [{ cmd: "testData" }] },
  ],
  View: [
    {
      label: "Windows",
      items: [
        { cmd: "paneContents", size: "small" },
        { cmd: "paneTable", size: "small" },
        { cmd: "paneModel", size: "small" },
        { cmd: "paneProperties", size: "small" },
        { cmd: "paneTools", size: "small" },
        { cmd: "paneSources", size: "small" },
        { cmd: "paneCatalog", size: "small" },
        { cmd: "paneMap", size: "small" },
        { cmd: "resetPanes" },
      ],
    },
    { label: "Performance", items: [{ cmd: "fps" }, { cmd: "benchmark" }] },
    { label: "Appearance", items: [{ cmd: "theme" }] },
  ],
  "Feature Layer": [
    { label: "Layer", items: [{ cmd: "layerProps" }, { cmd: "zoomLayer" }, { cmd: "symbology" }, { cmd: "openTable" }] },
    { label: "Fields", items: [{ cmd: "addField" }, { cmd: "calcField" }, { cmd: "calcGeometry" }] },
    { label: "Data", items: [{ cmd: "definitionQuery" }, { cmd: "defineProjection" }, { cmd: "exportLayer" }, { cmd: "layerFromSelection" }] },
    { label: "Remove", items: [{ cmd: "removeLayer" }] },
  ],
};

export function Ribbon() {
  const [tab, setTab] = useState("Map");
  // Re-render when state that affects enabled/checked changes.
  useApp((s) => s.tool);
  useApp((s) => s.selection);
  useApp((s) => s.showFps);
  useApp((s) => s.dark);
  useNav((s) => s.navPanel);
  useNav((s) => s.measureMode);
  const active = useApp((s) => (s.activeLayer ? s.layers[s.activeLayer] : null));
  const contextual = active && active.geomType !== "Table";
  const current = tab === "Feature Layer" && !contextual ? "Map" : tab;

  return (
    <div className="ribbon">
      <div className="ribbon-tabs">
        <Menu>
          <MenuTrigger>
            <button className="rtab file">Project</button>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              {["new", "open", "save", "saveAs", "sources"].map((id) => (
                <MenuItem key={id} icon={commands[id].icon} secondaryContent={commands[id].keys} onClick={commands[id].run}>
                  {commands[id].label}
                </MenuItem>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>
        {["Map", "Insert", "Analysis", "Data", "View"].map((t) => (
          <button key={t} className={`rtab ${current === t ? "on" : ""}`} onClick={() => setTab(t)}>
            {t}
          </button>
        ))}
        {contextual && (
          <div className="rtab-context">
            <span className="rtab-context-label">Layer</span>
            <button className={`rtab ctx ${current === "Feature Layer" ? "on" : ""}`} onClick={() => setTab("Feature Layer")}>
              Feature Layer
            </button>
          </div>
        )}
        <div className="ribbon-spacer" />
        <TellMe />
      </div>
      <div className="ribbon-body">
        {TABS[current].map((g) => (
          <RibbonGroup key={g.label} group={g} />
        ))}
      </div>
    </div>
  );
}

function RibbonGroup({ group }: { group: Group }) {
  // Consecutive small buttons stack in columns of three, as in Office.
  const cols: ReactNode[] = [];
  let smalls: string[] = [];
  const flush = () => {
    if (smalls.length) cols.push(<div key={`s${cols.length}`} className="rcol">{smalls.map((id) => <RButton key={id} cmd={commands[id]} size="small" />)}</div>);
    smalls = [];
  };
  group.items.forEach((it, i) => {
    if ("custom" in it) {
      flush();
      cols.push(<div key={`c${i}`}>{it.custom}</div>);
    } else if (it.size === "small") {
      smalls.push(it.cmd);
      if (smalls.length === 3) flush();
    } else {
      flush();
      cols.push(<RButton key={it.cmd} cmd={commands[it.cmd]} size="large" />);
    }
  });
  flush();
  return (
    <div className="rgroup">
      <div className="rgroup-items">{cols}</div>
      <div className="rgroup-label">{group.label}</div>
    </div>
  );
}

export function RButton({ cmd, size }: { cmd: Command; size: Size }) {
  const disabled = cmd.enabled ? !cmd.enabled() : false;
  const checked = cmd.checked?.() ?? false;
  const btn = (
    <button className={`rbtn ${size} ${checked ? "checked" : ""}`} disabled={disabled} onClick={cmd.run} aria-pressed={cmd.checked ? checked : undefined}>
      <span className="rbtn-icon">{cmd.icon}</span>
      <span className="rbtn-label">{cmd.label}</span>
    </button>
  );
  const tip = [cmd.tip, cmd.keys && `(${cmd.keys})`].filter(Boolean).join(" ");
  return tip ? (
    <Tooltip content={tip} relationship="description" withArrow>
      {btn}
    </Tooltip>
  ) : (
    btn
  );
}

function BasemapGallery() {
  const basemap = useApp((s) => s.basemap);
  return (
    <Menu checkedValues={{ basemap: [basemap] }} onCheckedValueChange={(_, d) => useApp.getState().setBasemap(d.checkedItems[0] as never)}>
      <MenuTrigger>
        <button className="rbtn large">
          <span className="rbtn-icon"><ImageMultipleRegular /></span>
          <span className="rbtn-label">Basemap ▾</span>
        </button>
      </MenuTrigger>
      <MenuPopover>
        <MenuList>
          {BASEMAP_OPTIONS.map(([id, label]) => (
            <MenuItemRadio key={id} name="basemap" value={id}>
              {label}
            </MenuItemRadio>
          ))}
        </MenuList>
      </MenuPopover>
    </Menu>
  );
}

function ToolGallery() {
  const specs = useRun((s) => s.specs);
  const tools = TOOL_BUTTONS.map((id) => specs[id]).filter(Boolean);
  return (
    <div className="gallery">
      {tools.map((t) => {
        const c = toolCommand(t.id, t.name, t.description);
        return <RButton key={t.id} cmd={c} size="small" />;
      })}
    </div>
  );
}

/** "Tell me what you want to do" command search. */
function TellMe() {
  const specs = useRun((s) => s.specs);
  const [value, setValue] = useState("");
  const all = useMemo(() => {
    const tools = Object.values(specs).filter((s) => s.role !== "data").map((s) => toolCommand(s.id, s.name, s.description));
    return [...Object.values(commands), ...tools];
  }, [specs]);
  const q = value.toLowerCase();
  const matches = q ? all.filter((c) => `${c.label} ${c.tip ?? ""}`.toLowerCase().includes(q)).slice(0, 12) : [];
  return (
    <Combobox
      className="tellme"
      size="small"
      placeholder="Command search (Alt+Q)"
      value={value}
      freeform
      expandIcon={<SearchRegular />}
      onChange={(e) => setValue(e.target.value)}
      onOptionSelect={(_, d) => {
        const c = all.find((x) => x.id === d.optionValue);
        setValue("");
        c?.run();
      }}
    >
      {matches.map((c) => (
        <Option key={c.id} value={c.id} text={c.label}>
          {c.label}
        </Option>
      ))}
    </Combobox>
  );
}
