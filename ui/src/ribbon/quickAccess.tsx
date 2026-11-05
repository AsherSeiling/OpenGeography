// Save, Undo and Redo, in line with the ribbon tabs. Undo and Redo act on whatever is active: the open layout, else the active node tree.
import { Tooltip } from "@fluentui/react-components";
import { ArrowRedoRegular, ArrowUndoRegular } from "@fluentui/react-icons";
import type { ReactNode } from "react";
import { useStore } from "zustand";
import { flushParamEdit, useModel } from "../store/model";
import { useLayouts } from "../store/layout";
import { commands } from "./commands";

/** Undo and Redo on whatever is active: the open layout, else the active node tree. Used by the buttons and the app-wide shortcuts. */
export function undoActive() {
  if (useLayouts.getState().id) return useLayouts.getState().undo();
  flushParamEdit();
  useModel.temporal.getState().undo();
}
export function redoActive() {
  if (useLayouts.getState().id) return useLayouts.getState().redo();
  flushParamEdit();
  useModel.temporal.getState().redo();
}

/** What Undo and Redo would act on right now, and whether there is anything to do. */
export function useHistory() {
  const layoutId = useLayouts((s) => s.id);
  const layoutUndo = useStore(useLayouts.temporal, (s) => s.pastStates.length > 0);
  const layoutRedo = useStore(useLayouts.temporal, (s) => s.futureStates.length > 0);
  const treeUndo = useStore(useModel.temporal, (s) => s.pastStates.length > 0);
  const treeRedo = useStore(useModel.temporal, (s) => s.futureStates.length > 0);
  const inLayout = !!layoutId;
  return {
    target: inLayout ? "layout" : "node tree",
    canUndo: inLayout ? layoutUndo : treeUndo,
    canRedo: inLayout ? layoutRedo : treeRedo,
    undo: undoActive,
    redo: redoActive,
  };
}

export function QuickAccess() {
  const h = useHistory();
  const save = commands.save;
  const btn = (id: string, icon: ReactNode, label: string, tip: string, disabled: boolean, run: () => void) => (
    <Tooltip content={tip} relationship="description" withArrow>
      <button className="qa-btn" data-qa={id} disabled={disabled} aria-label={label} onClick={run}>
        {icon}
        <span className="qa-label">{label}</span>
      </button>
    </Tooltip>
  );
  return (
    <div className="qa" role="toolbar" aria-label="Quick access">
      {btn("save", save.icon, "Save", `Save the project (${save.keys})`, false, () => save.run())}
      {btn("undo", <ArrowUndoRegular />, "Undo", `Undo the last change to the ${h.target}`, !h.canUndo, h.undo)}
      {btn("redo", <ArrowRedoRegular />, "Redo", `Redo the last undone change to the ${h.target}`, !h.canRedo, h.redo)}
    </div>
  );
}
