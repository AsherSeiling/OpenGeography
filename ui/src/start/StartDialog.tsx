// UX2: the Start page. Recent projects, new project from a template, sample data. Shown at start in the desktop app (unless turned off)
// and from File › Start Page. Nothing here reads or writes a project except by the same Open and New actions the ribbon uses.
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from "@fluentui/react-components";
import { addWorldOutline, forgetProject, generateTestData, newProjectFromTemplate, openProject } from "../actions";
import { useSettings } from "../store/settings";
import { projectLabel, projectWhere, SAMPLE_DATA, START_TEMPLATES, type SampleData } from "./recent";

export function StartDialog({ onClose }: { onClose: () => void }) {
  const settings = useSettings((s) => s.settings);
  const recent = settings.recentProjects ?? [];
  const sample = async (d: SampleData) => {
    onClose();
    if (d.id === "world") await addWorldOutline();
    else await generateTestData(d.id, 1000);
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ maxWidth: 760 }} data-testid="start-page">
        <DialogBody>
          <DialogTitle>Start</DialogTitle>
          <DialogContent>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 16 }}>
              <section aria-label="Recent projects" className="stack">
                <strong>Recent projects</strong>
                {recent.length === 0 && <div className="muted small">Projects you open or save will be listed here.</div>}
                {recent.map((p) => (
                  <div key={p} className="btn-row" data-testid="recent-project">
                    <Button
                      appearance="subtle"
                      style={{ flex: 1, justifyContent: "flex-start", textAlign: "left", height: "auto" }}
                      title={p}
                      onClick={() => {
                        onClose();
                        void openProject(p);
                      }}
                    >
                      <div>
                        <div>{projectLabel(p)}</div>
                        <div className="muted small">{projectWhere(p)}</div>
                      </div>
                    </Button>
                    <Button size="small" appearance="subtle" aria-label={`Remove ${projectLabel(p)} from the list`} title="Remove from this list (the project is not deleted)" onClick={() => void forgetProject(p)}>
                      ×
                    </Button>
                  </div>
                ))}
                <Button size="small" onClick={() => { onClose(); void openProject(); }}>Open another project…</Button>
              </section>
              <section aria-label="New project" className="stack">
                <strong>New project</strong>
                {START_TEMPLATES.map((t) => (
                  <Button key={t.id} appearance="subtle" style={{ justifyContent: "flex-start", textAlign: "left", height: "auto" }} onClick={() => { onClose(); void newProjectFromTemplate(t); }}>
                    <div>
                      <div>{t.name}</div>
                      <div className="muted small">{t.what}</div>
                    </div>
                  </Button>
                ))}
              </section>
              <section aria-label="Sample data" className="stack">
                <strong>Sample data</strong>
                {SAMPLE_DATA.map((d) => (
                  <Button key={d.id} appearance="subtle" style={{ justifyContent: "flex-start", textAlign: "left", height: "auto" }} onClick={() => void sample(d)}>
                    <div>
                      <div>{d.name}</div>
                      <div className="muted small">{d.what}</div>
                    </div>
                  </Button>
                ))}
              </section>
            </div>
          </DialogContent>
          <DialogActions fluid>
            <Checkbox label="Show this page when the app starts" checked={settings.showStartPage !== false} onChange={(_, d) => void useSettings.getState().update({ showStartPage: !!d.checked })} />
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
