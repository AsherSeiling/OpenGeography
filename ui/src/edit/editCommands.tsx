// The ribbon commands of shape editing (SHAPE_EDITING_PLAN E2). They go into the one command registry (ribbon/commands.tsx).
import { RIcon } from "../ribbon/icons/RIcon";
import type { Command } from "../ribbon/commands";
import { useApp } from "../store/store";
import { addByCoordinates as _abc, copySelected, pasteCopied, reshapeSelected, discardEdits, editRedo, editUndo, finishEditing, fixInvalid, newLayer as _nl, saveEdits, setEditTool, startEditing, toggleSnapping, undoLastSave, useEditDialogs, useSaved } from "./editActions";
import { useEdit, type EditTool } from "./editStore";

void _nl;
void _abc;
const editing = () => !!useEdit.getState().layerId;
const layer = () => {
  const s = useApp.getState();
  const id = useEdit.getState().layerId;
  return id ? s.layers[id] : undefined;
};
const tool = (t: EditTool) => () => useEdit.getState().tool === t;
const fits = (kind: "Point" | "Line" | "Polygon") => () => {
  const l = layer();
  return editing() && (l?.geomType === kind || l?.geomType === "Mixed");
};

export const editCommandList: Command[] = [
  { id: "startEditing", label: "Start Editing", icon: <RIcon name="edit-start" />, run: () => void startEditing(), enabled: () => !editing(), tip: "Edit the shapes of the selected layer. Changes are kept apart until you Save; Discard drops them. The original file on disk is never changed" },
  { id: "newLayer", label: "New Layer", icon: <RIcon name="layer-new" />, run: () => useEditDialogs.getState().setNewLayer(true), enabled: () => !editing(), tip: "Make an empty point, line or polygon layer with its own fields and coordinate system, then draw its shapes" },
  {
    id: "undoLastSave",
    label: "Undo Last Save",
    icon: <RIcon name="undo-save" />,
    run: () => void undoLastSave(),
    enabled: () => {
      const id = useApp.getState().activeLayer;
      const v = id ? useSaved.getState().saved[id] : undefined;
      return !editing() && !!id && v !== undefined && useApp.getState().layers[id]?.version === v;
    },
    tip: "Put the layer back the way it was before its latest shape-edit Save (only while nothing else has changed it)",
  },
  { id: "editSave", label: "Save Edits", icon: <RIcon name="edit-save" />, run: () => void saveEdits(), enabled: editing, tip: "Apply the pending edits to the layer in one step", keys: "" },
  { id: "editDiscard", label: "Discard Edits", icon: <RIcon name="edit-discard" />, run: () => void discardEdits(), enabled: editing, tip: "Drop every pending edit; the layer stays as it was saved" },
  { id: "editFinish", label: "Finish Editing", icon: <RIcon name="edit-start" />, run: () => finishEditing(), enabled: editing, tip: "Ask whether to save or discard the edits, then leave edit mode" },
  { id: "editUndo", label: "Undo", icon: <RIcon name="undo" />, run: () => void editUndo(), enabled: () => !!useEdit.getState().status?.canUndo, keys: "⌘Z", tip: "Undo the last edit step in this session" },
  { id: "editRedo", label: "Redo", icon: <RIcon name="redo" />, run: () => void editRedo(), enabled: () => !!useEdit.getState().status?.canRedo, keys: "⌘⇧Z", tip: "Redo the step you undid" },
  { id: "editPoint", label: "Point", icon: <RIcon name="edit-point" />, run: () => setEditTool("point"), enabled: fits("Point"), checked: tool("point"), tip: "Click the map to add a point" },
  { id: "editLine", label: "Line", icon: <RIcon name="edit-line" />, run: () => setEditTool("line"), enabled: fits("Line"), checked: tool("line"), tip: "Click to add vertices; double-click to finish the line" },
  { id: "editPolygon", label: "Polygon", icon: <RIcon name="edit-polygon" />, run: () => setEditTool("polygon"), enabled: fits("Polygon"), checked: tool("polygon"), tip: "Click to add vertices; double-click to finish the polygon" },
  { id: "editModify", label: "Move Vertices", icon: <RIcon name="edit-vertex" />, run: () => setEditTool("modify"), enabled: editing, checked: tool("modify"), tip: "Drag a vertex to move it, drag an edge to add one, Alt-click a vertex to delete it" },
  { id: "editDelete", label: "Delete Features", icon: <RIcon name="edit-delete" />, run: () => setEditTool("delete"), enabled: editing, checked: tool("delete"), tip: "Click a feature to delete it (Undo brings it back)" },
  { id: "editSnap", label: "Snapping", icon: <RIcon name="edit-snap" />, run: toggleSnapping, enabled: editing, checked: () => useEdit.getState().snapping, tip: "Snap to the vertices and edges of the layer being edited, within 10 screen pixels" },
  { id: "editFix", label: "Fix Shapes", icon: <RIcon name="edit-fix" />, run: () => void fixInvalid(useApp.getState().activeMapId), enabled: () => (useEdit.getState().status?.invalid.length ?? 0) > 0, tip: "Repair shapes that cross themselves or are degenerate (they are otherwise kept as drawn)" },
  { id: "editSelect", label: "Select", icon: <RIcon name="edit-select" />, run: () => setEditTool("select"), enabled: editing, checked: tool("select"), tip: "Click a feature to select it for Merge, Split, Holes and the other reshape commands. Shift-click adds or removes" },
  { id: "editRect", label: "Rectangle", icon: <RIcon name="edit-rect" />, run: () => setEditTool("rect"), enabled: fits("Polygon"), checked: tool("rect"), tip: "Drag two corners: a rectangle with straight edges in the map's coordinate system" },
  { id: "editCircle", label: "Circle", icon: <RIcon name="edit-circle" />, run: () => setEditTool("circle"), enabled: fits("Polygon"), checked: tool("circle"), tip: "Drag from the centre: a true geodesic circle on the ellipsoid, drawn with 48 vertices" },
  { id: "editMerge", label: "Merge", icon: <RIcon name="edit-merge" />, run: () => void reshapeSelected("merge"), enabled: editing, tip: "Join the selected features into one (the first keeps its attributes)" },
  { id: "editSplit", label: "Split", icon: <RIcon name="edit-split" />, run: () => setEditTool("split"), enabled: fits("Polygon"), checked: tool("split"), tip: "Select a polygon, then draw a line across it from one edge to another" },
  { id: "editHole", label: "Cut Hole", icon: <RIcon name="edit-hole" />, run: () => setEditTool("hole"), enabled: fits("Polygon"), checked: tool("hole"), tip: "Select a polygon, then draw a hole completely inside it" },
  { id: "editFillHoles", label: "Fill Holes", icon: <RIcon name="edit-hole" />, run: () => void reshapeSelected("fill_holes"), enabled: fits("Polygon"), tip: "Remove the holes of the selected polygons" },
  { id: "editExplode", label: "Explode", icon: <RIcon name="edit-explode" />, run: () => void reshapeSelected("explode"), enabled: editing, tip: "Turn a multipart feature into one feature per part" },
  { id: "editMulti", label: "Make Multipart", icon: <RIcon name="edit-explode" />, run: () => void reshapeSelected("make_multipart"), enabled: editing, tip: "Join the selected features into one multipart feature" },
  { id: "editCopy", label: "Copy", icon: <RIcon name="edit-copy" />, run: copySelected, enabled: () => true, tip: "Copy the selected features (of the layer being edited, or the active layer's map selection)" },
  { id: "editPaste", label: "Paste", icon: <RIcon name="edit-paste" />, run: () => void pasteCopied(), enabled: editing, tip: "Paste the copied features into this layer. Shapes are moved into its coordinate system, fields are matched by name, and unmatched ones are reported" },
  { id: "editCoords", label: "Enter Coordinates", icon: <RIcon name="edit-coords" />, run: () => useEditDialogs.getState().setCoords(true), enabled: editing, tip: "Type positions: x, y in the layer's coordinate system, longitude and latitude, USNG/MGRS, or a bearing and distance from a start (geodesic)" },
];
