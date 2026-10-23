import { Button, Toolbar, ToolbarDivider, Tooltip } from "@fluentui/react-components";
import { AddRegular, ArrowRedoRegular, ArrowUndoRegular, DeleteRegular, DismissRegular, EraserRegular, KeyboardRegular, Play20Filled, ZoomFitRegular } from "@fluentui/react-icons";
import { Background, Controls, ReactFlow, ReactFlowProvider, useReactFlow, type Connection, type Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "zustand";
import { addData, runModel, useDialog } from "../actions";
import type { DropDetail } from "../dnd";
import { api } from "../ipc/api";
import { AddMenu, type AddItem } from "../nodes/AddMenu";
import { nodeTypes } from "../nodes/ModelNodes";
import { copySelection, cutLinks, deleteSelection, duplicateSelection, invertSelection, makeLinks, paste, selectAll, toggleCollapse, toggleMute } from "../nodes/graphOps";
import { actionFor, displayCombo, eventCombo, useKeymap, type NodeAction } from "../nodes/keymap";
import { edgeUnder, splice } from "../nodes/splice";
import { useProgress } from "../progress/progress";
import { beginStep, connectProblem, specs, useModel, useRun } from "../store/model";
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
  const runTask = useProgress((s) => s.tasks.find((t) => t.label.startsWith("Running model")));
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

  const isValidConnection = useCallback((c: Connection | Edge) => !connectProblem(c.source, c.target, c.targetHandle), []);

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
      if (payload.kind === "file") {
        // A dataset from the Catalog: add it as a layer, then as a Layer input here.
        void addData([payload.id]).then((infos) => {
          if (infos[0] && useModel.getState().layer === owner) selectNode(useModel.getState().addNode("input_layer", pos, { layer: infos[0].id }));
        });
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
    const end = beginStep(); // adding + inserting is one undo step
    const id = useModel.getState().addNode(tool, pos, params);
    selectNode(id);
    // Wait for React Flow to measure the new node before testing for an edge under it.
    setTimeout(() => {
      trySplice(id);
      end();
    }, 60);
    return id;
  };

  /** Insert a free node into the connection under it, or say why not. */
  const trySplice = (id: string) => {
    const e = edgeUnder(id, flow.getInternalNode);
    const problem = e && splice(id, e);
    if (problem) useApp.getState().setStatus(`Not inserted into the connection: ${problem}.`);
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

  // Node-editor shortcuts (keymap.ts): only while the pointer is over the
  // canvas or focus is inside the Model pane, never while typing.
  const dragStep = useRef<(() => void) | null>(null);
  const grab = useRef<{ start: { x: number; y: number }; orig: Map<string, { x: number; y: number }>; end: () => void } | null>(null);
  const suppressMenu = useRef(false);
  const [cut, setCut] = useState<{ x: number; y: number }[] | null>(null);

  const overCanvas = () => {
    const m = mouse.current;
    const r = wrap.current?.getBoundingClientRect();
    return !!m && !!r && m.x >= r.left && m.x <= r.right && m.y >= r.top && m.y <= r.bottom;
  };
  // Is the node editor the active area? Pointer over it (Blender-style). If
  // the window never reports pointer movement (seen in some desktop
  // webviews), the last click decides instead: clicking in the editor
  // activates it until you click elsewhere.
  const sawMove = useRef(false);
  const clickedIn = useRef(false);
  const [active, setActive] = useState<"off" | "on" | "editing">("off");
  const editorActive = (target?: Element | null) =>
    !!useModel.getState().layer && (sawMove.current ? overCanvas() || !!target?.closest?.(".model .react-flow__node") : clickedIn.current);
  const pointerOver = (el: Element | null | undefined) => {
    const m = mouse.current;
    const r = el?.getBoundingClientRect();
    return !!m && !!r && m.x >= r.left && m.x <= r.right && m.y >= r.top && m.y <= r.bottom;
  };
  /**
   * A text field the keys should go to: a focused text/number input outside
   * the editor, or one inside a node while the pointer is still over that
   * node. Dropdowns and checkboxes never take shortcut keys.
   */
  const typingField = (): HTMLElement | null => {
    const el = document.activeElement as HTMLElement | null;
    const field = el?.closest?.("textarea, [contenteditable], input:not([type=checkbox]):not([type=radio]):not([type=button]):not([type=range])") as HTMLElement | null;
    if (!field) return null;
    if (!field.closest(".model")) return field;
    return !sawMove.current || pointerOver(field.closest(".react-flow__node") ?? field) ? field : null;
  };
  const editorState = () => (!editorActive(document.activeElement) ? "off" : typingField()?.closest(".model") ? "editing" : "on");
  useEffect(() => {
    // Window-level, capture phase: nothing inside the editor can swallow these.
    const track = (e: PointerEvent | MouseEvent) => {
      mouse.current = { x: e.clientX, y: e.clientY };
      if (e.type === "pointermove" || e.type === "mousemove") sawMove.current = true;
      if (e.type === "pointerdown") clickedIn.current = !!(e.target as Element)?.closest?.(".model");
      const a = editorState();
      setActive((was) => (was === a ? was : a));
    };
    const opts = { capture: true, passive: true };
    window.addEventListener("pointermove", track, opts);
    window.addEventListener("mousemove", track, opts);
    window.addEventListener("pointerdown", track, opts);
    const refocus = () => setActive(editorState());
    const unfocus = () => setTimeout(refocus, 0);
    window.addEventListener("focusin", refocus);
    window.addEventListener("focusout", unfocus);
    return () => {
      window.removeEventListener("focusin", refocus);
      window.removeEventListener("focusout", unfocus);
      window.removeEventListener("pointermove", track, opts);
      window.removeEventListener("mousemove", track, opts);
      window.removeEventListener("pointerdown", track, opts);
    };
  }, []);
  const pointerFlow = () => {
    const m = mouse.current;
    const r = wrap.current!.getBoundingClientRect();
    return flow.screenToFlowPosition(m && overCanvas() ? m : { x: r.left + r.width / 2, y: r.top + r.height / 2 });
  };

  const startGrab = () => {
    const sel = useModel.getState().nodes.filter((n) => n.selected);
    if (!sel.length) return;
    grab.current = { start: pointerFlow(), orig: new Map(sel.map((n) => [n.id, { ...n.position }])), end: beginStep() };
    useApp.getState().setStatus("Move: click or Enter to place · Esc or right-click to cancel");
  };
  const endGrab = (confirm: boolean) => {
    const g = grab.current;
    if (!g) return;
    grab.current = null;
    setSpliceEdge(null);
    if (!confirm) {
      useModel.setState({ nodes: useModel.getState().nodes.map((n) => (g.orig.has(n.id) ? { ...n, position: g.orig.get(n.id)! } : n)) });
    } else if (g.orig.size === 1) {
      trySplice([...g.orig.keys()][0]);
    }
    g.end();
    useApp.getState().setStatus(confirm ? "Moved" : "Move cancelled");
  };

  const runAction = (a: NodeAction) => {
    const temporal = useModel.temporal.getState();
    switch (a) {
      case "add": {
        const m = mouse.current;
        if (m) openMenu(m.x, m.y);
        break;
      }
      case "delete":
        return deleteSelection(false);
      case "deleteReconnect":
        return deleteSelection(true);
      case "duplicate":
        if (duplicateSelection().length) startGrab();
        return;
      case "copy": {
        const n = copySelection();
        useApp.getState().setStatus(n ? `Copied ${n} node(s)` : "Nothing to copy (the Layer Input and Output can't be copied)");
        return;
      }
      case "paste":
        paste(pointerFlow());
        return;
      case "grab":
        return startGrab();
      case "selectAll":
        return selectAll(true);
      case "deselectAll":
        return selectAll(false);
      case "invertSelection":
        return invertSelection();
      case "mute":
        return toggleMute();
      case "collapse":
        return toggleCollapse();
      case "makeLinks":
        makeLinks();
        return;
      case "frameAll":
        return void flow.fitView({ padding: 0.2, duration: 200 });
      case "frameSelected": {
        const sel = useModel.getState().nodes.filter((n) => n.selected);
        return void flow.fitView({ nodes: sel.length ? sel : undefined, padding: 0.3, duration: 200, maxZoom: 1.5 });
      }
      case "undo":
        return temporal.undo();
      case "redo":
        return temporal.redo();
      case "run":
        return void runModel();
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (grab.current) {
        if (e.key === "Escape" || e.key === "Enter") {
          e.preventDefault();
          endGrab(e.key === "Enter");
        }
        return;
      }
      const target = e.target as HTMLElement;
      if (document.querySelector(".addmenu, [role=dialog]")) return;
      const nodeField = target?.closest?.(".model .react-flow__node input, .model .react-flow__node select, .model .react-flow__node textarea") as HTMLElement | null;
      // Enter / Esc in a node's field finishes editing.
      if (nodeField && (e.key === "Enter" || e.key === "Escape") && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
        nodeField.blur();
        setActive(editorState());
        if (e.key === "Escape") e.preventDefault();
        return;
      }
      if (typingField()) return; // the keys are for that field
      if (!editorActive(target)) return;
      const combo = eventCombo(e);
      const action = combo && actionFor(useKeymap.getState().bindings, combo);
      if (!action) return;
      e.preventDefault();
      e.stopPropagation();
      // A node field the pointer has left lets go, so the shortcut acts on the tree.
      if (nodeField || (document.activeElement as HTMLElement)?.closest?.(".model .react-flow__node")) (document.activeElement as HTMLElement).blur?.();
      runAction(action);
    };
    const onMove = (e: MouseEvent) => {
      const g = grab.current;
      if (!g) return;
      const p = flow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
      const [dx, dy] = [p.x - g.start.x, p.y - g.start.y];
      useModel.setState({ nodes: useModel.getState().nodes.map((n) => (g.orig.has(n.id) ? { ...n, position: { x: g.orig.get(n.id)!.x + dx, y: g.orig.get(n.id)!.y + dy } } : n)) });
      if (g.orig.size === 1) setSpliceEdge(edgeUnder([...g.orig.keys()][0], flow.getInternalNode)?.id ?? null);
    };
    const onDown = (e: MouseEvent) => {
      if (!grab.current) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.button === 2) suppressMenu.current = true;
      endGrab(e.button === 0);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mousedown", onDown, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mousedown", onDown, true);
    };
  });

  // Cut links: Mod + right-drag across connections (Blender's Ctrl+RMB).
  const isMac = /Mac/.test(navigator.platform);
  const onCutDown = (e: React.PointerEvent) => {
    if (e.button !== 2 || !(isMac ? e.metaKey : e.ctrlKey)) return;
    e.preventDefault();
    e.stopPropagation();
    suppressMenu.current = true;
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setCut([{ x: e.clientX, y: e.clientY }]);
  };
  const onCutMove = (e: React.PointerEvent) => {
    if (cut) setCut([...cut, { x: e.clientX, y: e.clientY }]);
  };
  const onCutUp = () => {
    if (!cut) return;
    const n = cutLinks(
      cut,
      (id) => wrap.current?.querySelector(`.react-flow__edge[data-id="${CSS.escape(id)}"] path.react-flow__edge-path`) ?? null,
      (p, el) => {
        const q = p.matrixTransform(el.getScreenCTM() ?? undefined);
        return { x: q.x, y: q.y };
      },
    );
    setCut(null);
    if (n) useApp.getState().setStatus(`Cut ${n} connection(s)`);
  };

  const bindings = useKeymap((s) => s.bindings);
  const keys = (a: NodeAction) => (bindings[a][0] ? ` (${displayCombo(bindings[a][0])})` : "");

  const hasErrors = Object.values(results).some((r) => r.status === "error");

  return (
    <div className={`model ${active === "on" ? "shortcuts-on" : ""}`}>
      <Toolbar size="small" className="model-bar">
        <span className="model-title" title="Each layer has its own node tree. Select a layer in Contents to switch.">
          {treeName ? <>Node tree: <b>{treeName}</b></> : "No layer selected"}
        </span>
        <ToolbarDivider />
        <Button appearance="primary" size="small" icon={<Play20Filled />} disabled={running || !treeLayer} onClick={() => runModel()}>
          {running ? "Running…" : "Run"}
        </Button>
        {running && (
          <Tooltip content="Stop the model run. The layer keeps showing its last result." relationship="label">
            <Button size="small" icon={<DismissRegular />} disabled={!runTask || runTask.detail === "Cancelling…"} onClick={() => runTask && api.cancelTask(runTask.id)}>
              {runTask?.detail === "Cancelling…" ? "Cancelling…" : "Cancel"}
            </Button>
          </Tooltip>
        )}
        <Tooltip content={`Add a node${keys("add")} — or right-click the canvas`} relationship="label">
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
        <Tooltip content={`Undo${keys("undo")}`} relationship="label">
          <Button size="small" appearance="subtle" icon={<ArrowUndoRegular />} disabled={!canUndo} onClick={() => temporal.getState().undo()} />
        </Tooltip>
        <Tooltip content={`Redo${keys("redo")}`} relationship="label">
          <Button size="small" appearance="subtle" icon={<ArrowRedoRegular />} disabled={!canRedo} onClick={() => temporal.getState().redo()} />
        </Tooltip>
        <Tooltip content={`Delete selected${keys("delete")}`} relationship="label">
          <Button size="small" appearance="subtle" icon={<DeleteRegular />} onClick={() => deleteSelection(false)} />
        </Tooltip>
        <Tooltip content={`Fit to window${keys("frameAll")}`} relationship="label">
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
        <Tooltip content="Node editor keyboard shortcuts — view and change them" relationship="label">
          <Button size="small" appearance="subtle" icon={<KeyboardRegular />} onClick={() => useDialog.getState().show({ kind: "nodeShortcuts" })} />
        </Tooltip>
        {active === "on" && (
          <span className="model-active" title="Node editor shortcuts are active (pointer is over the editor). Change them with the keyboard button.">
            Shortcuts on
          </span>
        )}
        {active === "editing" && (
          <span className="model-active editing" title="Keys go into the field you're editing. Press Enter or Esc, or move the pointer off the node, to use shortcuts again.">
            Editing field
          </span>
        )}
        <span className="model-hint">
          {hasErrors
              ? "Some steps failed — select a red step to see the error in Properties."
              : ""}
        </span>
      </Toolbar>
      <div
        className="model-canvas"
        ref={wrap}
        data-drop="model"
        onMouseMove={(e) => (mouse.current = { x: e.clientX, y: e.clientY })}
        onPointerDownCapture={onCutDown}
        onPointerMove={onCutMove}
        onPointerUp={onCutUp}
      >
        {cut && cut.length > 1 && (
          <svg className="cut-line">
            <polyline points={cut.map((p) => `${p.x - (wrap.current?.getBoundingClientRect().left ?? 0)},${p.y - (wrap.current?.getBoundingClientRect().top ?? 0)}`).join(" ")} />
          </svg>
        )}
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
          onNodeDragStart={() => (dragStep.current = beginStep())}
          onNodeDrag={(_, n) => setSpliceEdge(edgeUnder(n.id, flow.getInternalNode)?.id ?? null)}
          onNodeDragStop={(_, n) => {
            setSpliceEdge(null);
            trySplice(n.id);
            dragStep.current?.(); // the move (and any insert) is one undo step
            dragStep.current = null;
          }}
          onPaneContextMenu={(e) => {
            e.preventDefault();
            if (suppressMenu.current) {
              suppressMenu.current = false;
              return;
            }
            openMenu(e.clientX, e.clientY);
          }}
          onPaneClick={() => {
            // Clicking empty canvas releases a node field, so shortcuts apply again.
            if ((document.activeElement as HTMLElement)?.closest?.(".model")) (document.activeElement as HTMLElement).blur();
            if (useApp.getState().properties?.kind === "node") useApp.getState().setProperties(null);
          }}
          deleteKeyCode={null}
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
