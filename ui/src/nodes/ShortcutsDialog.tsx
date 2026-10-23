// Node editor → Shortcuts: view and rebind the node editor's keys.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from "@fluentui/react-components";
import { AddRegular, ArrowResetRegular, DismissRegular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { ACTIONS, conflicts, defaultBindings, displayCombo, eventCombo, GESTURES, useKeymap, type NodeAction } from "./keymap";

const label = (a: NodeAction) => ACTIONS.find((x) => x.id === a)!.label;

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const bindings = useKeymap((s) => s.bindings);
  const { setBinding, reset } = useKeymap.getState();
  const [recording, setRecording] = useState<NodeAction | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const defaults = defaultBindings();

  // Capture the next key combination while recording.
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setRecording(null);
        return;
      }
      const combo = eventCombo(e);
      if (!combo) return; // a lone modifier: keep waiting
      const taken = conflicts(bindings, combo, recording);
      setBinding(recording, [...bindings[recording], combo]);
      setNote(taken.length ? `${displayCombo(combo)} moved from "${taken.map(label).join(", ")}" to "${label(recording)}".` : null);
      setRecording(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, bindings]);

  const groups = [...new Set(ACTIONS.map((a) => a.group))];
  const changed = (a: NodeAction) => JSON.stringify(bindings[a]) !== JSON.stringify(defaults[a]);

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !recording && onClose()}>
      <DialogSurface style={{ maxWidth: 640 }}>
        <DialogBody>
          <DialogTitle>Node editor shortcuts</DialogTitle>
          <DialogContent className="stack">
            <div className="muted">Blender-style keys. They work only while the pointer is over the node editor (or you are working in it), so they never affect the map or the attribute table.</div>
            <div className="keymap">
              {groups.map((g) => (
                <div key={g} className="keymap-group">
                  <div className="keymap-group-title">{g}</div>
                  {ACTIONS.filter((a) => a.group === g).map((a) => (
                    <div key={a.id} className={`keymap-row ${recording === a.id ? "recording" : ""}`} title={a.description}>
                      <span className="keymap-label">
                        {a.label}
                        {changed(a.id) && <span className="keymap-custom" title="Changed from the default"> •</span>}
                      </span>
                      <span className="keymap-keys">
                        {bindings[a.id].map((c) => (
                          <span key={c} className="keychip">
                            {displayCombo(c)}
                            <button title="Remove this shortcut" onClick={() => setBinding(a.id, bindings[a.id].filter((x) => x !== c))}>
                              <DismissRegular />
                            </button>
                          </span>
                        ))}
                        {recording === a.id ? (
                          <span className="keychip rec">Press keys… (Esc cancels)</span>
                        ) : (
                          <button className="keyadd" title="Add a shortcut: click, then press the keys" onClick={() => (setNote(null), setRecording(a.id))}>
                            <AddRegular />
                          </button>
                        )}
                        {changed(a.id) && (
                          <button className="keyadd" title="Reset to default" onClick={() => reset(a.id)}>
                            <ArrowResetRegular />
                          </button>
                        )}
                      </span>
                    </div>
                  ))}
                </div>
              ))}
              <div className="keymap-group">
                <div className="keymap-group-title">Mouse</div>
                {GESTURES.map((g) => (
                  <div key={g.label} className="keymap-row">
                    <span className="keymap-label">{g.label}</span>
                    <span className="keymap-keys muted">{g.keys.replace("Mod", displayCombo("Mod"))}</span>
                  </div>
                ))}
              </div>
            </div>
            {note && <div className="src-warn">{note}</div>}
          </DialogContent>
          <DialogActions>
            <Button icon={<ArrowResetRegular />} onClick={() => (reset(), setNote("All shortcuts reset to the Blender defaults."))}>
              Reset all
            </Button>
            <Button appearance="primary" onClick={onClose} disabled={!!recording}>
              Done
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
