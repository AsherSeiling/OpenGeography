import { Button, Toolbar, ToolbarDivider, Tooltip } from "@fluentui/react-components";
import { AddRegular, ArrowRedoRegular, ArrowUndoRegular, DeleteRegular, EraserRegular, Play20Filled, ZoomFitRegular } from "@fluentui/react-icons";
import { Background, Controls, ReactFlow, ReactFlowProvider, useReactFlow, type Connection, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "zustand";
import { runModel } from "../actions";
import type { DropDetail } from "../dnd";
import { api } from "../ipc/api";
import { AddMenu, type AddItem } from "../nodes/AddMenu";
import { nodeTypes } from "../nodes/ModelNodes";
import { edgeUnder, splice } from "../nodes/splice";
import { outputKind, specs, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";

let addAtCenter: ((tool: string, params?: Record<string, unknown>) => void) | null = null;

/** Add a tool node in the middle of the visible model canvas. */
export function addToolToModel(tool: string, params?: Record<string, unknown>) {
  if (!useModel.getState().layer) {
    useApp.getState().setStatus("Select a layer in Contents first: tools go into that layer's node tree.");
    return;
  }
  if (addAtCenter) addAtCenter(tool, params);
  else useModel.getState().addNode(tool, { x: 80, y: 80 }, params);
}

export function ModelPane() {
  return (
    <ReactFlowProvider>
      <ModelCanvas />
    </ReactFlowProvider>
  );
}

function ModelCanvas() {
  const nodes = useModel((s) => s.nodes);
  const treeLayer = useModel((s) => s.layer);
  const treeName = useApp((s) => (treeLayer ? s.layers[treeLayer]?.name : undefined));
  const edges = useModel((s) => s.edges);
  const running = useRun((s) => s.running);
  const results = useRun((s) => s.results);
  const flow = useReactFlow();
  const wrap = useRef<HTMLDivElement>(null);
  const temporal = useModel.temporal;
  const canUndo = useStore(temporal, (s) => s.pastStates.length > 0);
  const canRedo = useStore(temporal, (s) => s.futureStates.length > 0);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [spliceEdge, setSpliceEdge] = useState<string | null>(null);
  const mouse = useRef<{ x: number; y: number } | null>(null);

  addAtCenter = (tool, params) => {
    const r = wrap.current?.getBoundingClientRect();
    const pos = r ? flow.screenToFlowPosition({ x: r.left + r.width / 2 - 70, y: r.top + r.height / 2 - 30 }) : { x: 80, y: 80 };
    const n = useModel.getState().nodes.length;
    const id = useModel.getState().addNode(tool, { x: pos.x + (n % 5) * 24, y: pos.y + (n % 5) * 24 }, params);
    selectNode(id);
  };

  const selectNode = (id: string) => {
    useModel.setState({ nodes: useModel.getState().nodes.map((n) => ({ ...n, selected: n.id === id })) });
    useApp.getState().setProperties({ kind: "node", id });
  };

  const isValidConnection = useCallback((c: Connection | Edge) => {
    if (c.source === c.target) return false;
    const target = useModel.getState().nodes.find((n) => n.id === c.target);
    const port = target && specs()[target.data.tool]?.inputs.find((p) => p.id === c.targetHandle);
    if (!port) return false;
    // Vector inputs need features with geometry; tables go into table ports only.
    return port.kind === "table" || outputKind(c.source) !== "table";
  }, []);

  // Tools and layers dragged in from the Geoprocessing and Contents panes.
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const onDrop = (e: Event) => {
      const { payload, clientX, clientY } = (e as CustomEvent<DropDetail>).detail;
      const pos = flow.screenToFlowPosition({ x: clientX - 60, y: clientY - 20 });
      const owner = useModel.getState().layer;
      if (!owner) return;
      if (payload.kind === "tool") {
        addAndMaybeSplice(payload.id, pos);
        return;
      }
      if (payload.id === owner) return;
      // Dropped onto a node? A Layer node takes the layer; a tool gets a new
      // Layer node wired into its first empty input.
      const hitId = (document.elementFromPoint(clientX, clientY)?.closest(".react-flow__node") as HTMLElement | null)?.dataset.id;
      const hit = hitId ? useModel.getState().nodes.find((n) => n.id === hitId) : undefined;
      if (hit && hit.data.tool === "input_layer" && !hit.data.fixed) {
        useModel.getState().setParam(hit.id, "layer", payload.id);
        selectNode(hit.id);
        return;
      }
      if (hit) {
        const spec = specs()[hit.data.tool];
        const taken = new Set(useModel.getState().edges.filter((x) => x.target === hit.id).map((x) => x.targetHandle));
        // Prefer the input row nearest the drop point, if it's free.
        const rows = [...(document.querySelector(`.react-flow__node[data-id="${hit.id}"]`)?.querySelectorAll(".bnode-row.in") ?? [])] as HTMLElement[];
        const nearest = rows
          .map((el, i) => ({ i, d: Math.abs(el.getBoundingClientRect().top + el.offsetHeight / 2 - clientY) }))
          .sort((a, b) => a.d - b.d)[0];
        const preferred = nearest && spec?.inputs[nearest.i];
        const port = preferred && !taken.has(preferred.id) ? preferred : spec?.inputs.find((p) => !taken.has(p.id));
        if (port) {
          const id = useModel.getState().addNode("input_layer", { x: hit.position.x - 280, y: hit.position.y + 60 }, { layer: payload.id });
          useModel.getState().connect({ source: id, sourceHandle: "out", target: hit.id, targetHandle: port.id });
          selectNode(id);
          return;
        }
      }
      selectNode(useModel.getState().addNode("input_layer", pos, { layer: payload.id }));
    };
    el.addEventListener("gis-drop", onDrop);
    return () => el.removeEventListener("gis-drop", onDrop);
  });

  // Fit the graph into view once the pane has a real size, and whenever a
  // model is loaded into an empty canvas.
  const fittedFor = useRef(0);
  const fittedTree = useRef<string | null>(null);
  useEffect(() => {
    if (fittedTree.current !== treeLayer) {
      fittedTree.current = treeLayer;
      fittedFor.current = 0; // new tree: fit it into view
    }
    const el = wrap.current;
    if (!el) return;
    const fit = () => {
      const n = useModel.getState().nodes.length;
      if (el.clientWidth > 80 && el.clientHeight > 80 && n > 0 && fittedFor.current === 0) {
        fittedFor.current = n;
        setTimeout(() => flow.fitView({ padding: 0.2, maxZoom: 1 }), 80); // after nodes are measured
      }
      if (n === 0) fittedFor.current = 0;
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    fit();
    return () => ro.disconnect();
  }, [flow, nodes.length, treeLayer]);

  /** Add a node; if it lands on a connection, insert it there. */
  const addAndMaybeSplice = (tool: string, pos: { x: number; y: number }, params?: Record<string, unknown>) => {
    const id = useModel.getState().addNode(tool, pos, params);
    selectNode(id);
    // Wait for React Flow to measure the new node before testing for an edge under it.
    setTimeout(() => {
      const e = edgeUnder(id, flow.getInternalNode);
      if (e) splice(id, e);
    }, 60);
    return id;
  };

  const openMenu = (x: number, y: number) => {
    if (!useModel.getState().layer) {
      useApp.getState().setStatus("Select a layer in Contents first: its node tree opens here.");
      return;
    }
    setMenu({ x, y });
  };

  const pick = (item: AddItem) => {
    const at = menu ? flow.screenToFlowPosition({ x: menu.x, y: menu.y }) : { x: 80, y: 80 };
    setMenu(null);
    addAndMaybeSplice(item.tool, at, item.params);
  };

  // Shift+A opens the Add menu at the mouse (when the pointer is over the canvas).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = (e.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable]");
      if (typing || !e.shiftKey || e.key.toLowerCase() !== "a" || e.ctrlKey || e.metaKey || e.altKey) return;
      const m = mouse.current;
      const r = wrap.current?.getBoundingClientRect();
      if (!m || !r || m.x < r.left || m.x > r.right || m.y < r.top || m.y > r.bottom) return;
      e.preventDefault();
      openMenu(m.x, m.y);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const deleteSelected = () => {
    const ids = useModel.getState().nodes.filter((n) => n.selected).map((n) => n.id);
    useModel.getState().removeNodes(ids);
    useModel.setState({ edges: useModel.getState().edges.filter((e) => !e.selected) });
  };

  const hasErrors = Object.values(results).some((r) => r.status === "error");

  return (
    <div className="model">
      <Toolbar size="small" className="model-bar">
        <span className="model-title" title="Each layer has its own node tree. Select a layer in Contents to switch.">
          {treeName ? <>Node tree: <b>{treeName}</b></> : "No layer selected"}
        </span>
        <ToolbarDivider />
        <Button appearance="primary" size="small" icon={<Play20Filled />} disabled={running || !treeLayer} onClick={() => runModel()}>
          {running ? "Running…" : "Run"}
        </Button>
        <Tooltip content="Add a node (Shift+A over the canvas, or right-click)" relationship="label">
          <Button
            size="small"
            appearance="subtle"
            icon={<AddRegular />}
            disabled={!treeLayer}
            onClick={(e) => {
              const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
              openMenu(r.left, r.bottom + 4);
            }}
          >
            Add
          </Button>
        </Tooltip>
        <ToolbarDivider />
        <Tooltip content="Undo (Ctrl+Z)" relationship="label">
          <Button size="small" appearance="subtle" icon={<ArrowUndoRegular />} disabled={!canUndo} onClick={() => temporal.getState().undo()} />
        </Tooltip>
        <Tooltip content="Redo (Ctrl+Y)" relationship="label">
          <Button size="small" appearance="subtle" icon={<ArrowRedoRegular />} disabled={!canRedo} onClick={() => temporal.getState().redo()} />
        </Tooltip>
        <Tooltip content="Delete selected" relationship="label">
          <Button size="small" appearance="subtle" icon={<DeleteRegular />} onClick={deleteSelected} />
        </Tooltip>
        <Tooltip content="Fit to window" relationship="label">
          <Button size="small" appearance="subtle" icon={<ZoomFitRegular />} onClick={() => flow.fitView({ padding: 0.2 })} />
        </Tooltip>
        <Tooltip content="Clear cached results (forces a full re-run)" relationship="label">
          <Button
            size="small"
            appearance="subtle"
            icon={<EraserRegular />}
            onClick={async () => {
              await api.clearModelCache();
              useRun.getState().set({ results: {} });
              useApp.getState().setStatus("Model cache cleared");
            }}
          />
        </Tooltip>
        <span className="model-hint">
          {hasErrors
              ? "Some steps failed — select a red step to see the error in Properties."
              : ""}
        </span>
      </Toolbar>
      <div className="model-canvas" ref={wrap} data-drop="model" onMouseMove={(e) => (mouse.current = { x: e.clientX, y: e.clientY })}>
        {!treeLayer && (
          <div className="model-empty">
            <div>
              Select a layer in <b>Contents</b> to open its node tree.
              <br />
              Each layer has its own tree, from its <b>Layer Input</b> to its <b>Output</b>.
            </div>
          </div>
        )}
        {treeLayer && nodes.length <= 2 && (
          <div className="model-empty top">
            <div>
              Press <b>Shift+A</b> (or right-click) to add a tool. Drop it onto a connection to insert it there.
              <br />
              Drag another layer from <b>Contents</b> onto the canvas or onto a tool to use it (e.g. as Clip features).
            </div>
          </div>
        )}
        <ReactFlow
          nodes={nodes}
          edges={spliceEdge ? edges.map((e) => (e.id === spliceEdge ? { ...e, className: "splice-target", animated: true } : e)) : edges}
          nodeTypes={nodeTypes}
          onNodesChange={useModel.getState().onNodesChange}
          onEdgesChange={useModel.getState().onEdgesChange}
          onConnect={useModel.getState().connect}
          isValidConnection={isValidConnection}
          onNodeClick={(_, n) => useApp.getState().setProperties({ kind: "node", id: n.id })}
          onNodeDragStart={() => temporal.getState().pause()}
          onNodeDrag={(_, n) => setSpliceEdge(edgeUnder(n.id, flow.getInternalNode)?.id ?? null)}
          onNodeDragStop={(_, n) => {
            temporal.getState().resume();
            const e = edgeUnder(n.id, flow.getInternalNode);
            setSpliceEdge(null);
            if (e) splice(n.id, e);
            else useModel.setState({ nodes: [...useModel.getState().nodes] });
          }}
          onPaneContextMenu={(e) => {
            e.preventDefault();
            openMenu(e.clientX, e.clientY);
          }}
          onPaneClick={() => useApp.getState().properties?.kind === "node" && useApp.getState().setProperties(null)}
          deleteKeyCode={["Delete", "Backspace"]}
          defaultEdgeOptions={{ type: "smoothstep" }}
          minZoom={0.2}
        >
          <Background gap={16} size={1} color="#d8d8d8" />
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
        {menu && <AddMenu x={menu.x} y={menu.y} onPick={pick} onClose={() => setMenu(null)} />}
      </div>
    </div>
  );
}
