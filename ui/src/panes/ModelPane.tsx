import { Button, Toolbar, ToolbarDivider, Tooltip } from "@fluentui/react-components";
import { ArrowRedoRegular, ArrowUndoRegular, DeleteRegular, EraserRegular, Play20Filled, ZoomFitRegular } from "@fluentui/react-icons";
import { Background, Controls, ReactFlow, ReactFlowProvider, useReactFlow, type Connection, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useCallback, useEffect, useRef } from "react";
import { useStore } from "zustand";
import { runModel } from "../actions";
import type { DropDetail } from "../dnd";
import { api } from "../ipc/api";
import { nodeTypes } from "../nodes/ModelNodes";
import { outputKind, specs, useModel, useRun } from "../store/model";
import { useApp } from "../store/store";

let addAtCenter: ((tool: string, params?: Record<string, unknown>) => void) | null = null;

/** Add a tool node in the middle of the visible model canvas. */
export function addToolToModel(tool: string, params?: Record<string, unknown>) {
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
  const edges = useModel((s) => s.edges);
  const running = useRun((s) => s.running);
  const results = useRun((s) => s.results);
  const flow = useReactFlow();
  const wrap = useRef<HTMLDivElement>(null);
  const temporal = useModel.temporal;
  const canUndo = useStore(temporal, (s) => s.pastStates.length > 0);
  const canRedo = useStore(temporal, (s) => s.futureStates.length > 0);

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
      if (payload.kind === "tool") selectNode(useModel.getState().addNode(payload.id, pos));
      else selectNode(useModel.getState().addNode("input_layer", pos, { layer: payload.id }));
    };
    el.addEventListener("gis-drop", onDrop);
    return () => el.removeEventListener("gis-drop", onDrop);
  });

  // Fit the graph into view once the pane has a real size, and whenever a
  // model is loaded into an empty canvas.
  const fittedFor = useRef(0);
  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const fit = () => {
      const n = useModel.getState().nodes.length;
      if (el.clientWidth > 80 && el.clientHeight > 80 && n > 0 && fittedFor.current === 0) {
        fittedFor.current = n;
        requestAnimationFrame(() => flow.fitView({ padding: 0.25, maxZoom: 1.2 }));
      }
      if (n === 0) fittedFor.current = 0;
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    fit();
    return () => ro.disconnect();
  }, [flow, nodes.length]);

  const deleteSelected = () => {
    const ids = useModel.getState().nodes.filter((n) => n.selected).map((n) => n.id);
    useModel.getState().removeNodes(ids);
    useModel.setState({ edges: useModel.getState().edges.filter((e) => !e.selected) });
  };

  const hasErrors = Object.values(results).some((r) => r.status === "error");

  return (
    <div className="model">
      <Toolbar size="small" className="model-bar">
        <Button appearance="primary" size="small" icon={<Play20Filled />} disabled={running || !nodes.length} onClick={() => runModel()}>
          {running ? "Running…" : "Run"}
        </Button>
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
      <div className="model-canvas" ref={wrap} data-drop="model">
        {nodes.length === 0 && (
          <div className="model-empty">
            <div>
              Drag a layer here from <b>Contents</b> (it becomes a blue input), then tools from <b>Geoprocessing</b>.
              <br />
              Connect each green <b>Output</b> to the next tool's input, end with <b>Add to Map</b>, and click Run.
            </div>
          </div>
        )}
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={useModel.getState().onNodesChange}
          onEdgesChange={useModel.getState().onEdgesChange}
          onConnect={useModel.getState().connect}
          isValidConnection={isValidConnection}
          onNodeClick={(_, n) => useApp.getState().setProperties({ kind: "node", id: n.id })}
          onNodeDragStart={() => temporal.getState().pause()}
          onNodeDragStop={() => {
            temporal.getState().resume();
            useModel.setState({ nodes: [...useModel.getState().nodes] });
          }}
          onPaneClick={() => useApp.getState().properties?.kind === "node" && useApp.getState().setProperties(null)}
          deleteKeyCode={["Delete", "Backspace"]}
          defaultEdgeOptions={{ type: "smoothstep" }}
          minZoom={0.2}
        >
          <Background gap={16} size={1} color="#d8d8d8" />
          <Controls showInteractive={false} position="bottom-left" />
        </ReactFlow>
      </div>
    </div>
  );
}
