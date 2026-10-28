// Map Series (C3.2): one page per feature of an index layer; map frames that follow the series move to each feature.
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Dropdown, Field, Input, MessageBar, MessageBarBody, Option } from "@fluentui/react-components";
import { useState } from "react";
import { parseMargin, parseSeriesScale, withSeries } from "../layout/series";
import type { SeriesSpec } from "../layout/types";
import { useLayouts } from "../store/layout";
import { useApp } from "../store/store";

export function SeriesDialog({ onClose }: { onClose: () => void }) {
  const cur = useLayouts((s) => s.doc?.series ?? null);
  const hasFollower = useLayouts((s) => !!s.doc?.elements.some((e) => e.kind === "map_frame" && e.followsSeries));
  const layers = useApp((s) => s.layers);
  const order = useApp((s) => s.order);
  const vector = order.map((id) => layers[id]).filter((l) => l && (l.geomType === "Point" || l.geomType === "Line" || l.geomType === "Polygon" || l.geomType === "Mixed"));
  const [spec, setSpec] = useState<SeriesSpec>(() => withSeries(cur, cur ? {} : { indexLayer: vector[0]?.id ?? "", nameField: vector[0]?.fields[0]?.name ?? "" }, ""));
  const [margin, setMargin] = useState(String(spec.marginPct));
  const [scale, setScale] = useState(spec.scale ? String(spec.scale) : "");
  const layer = layers[spec.indexLayer];
  const fields = layer?.fields ?? [];
  const marginOk = parseMargin(margin) !== null;
  const scaleOk = spec.extent !== "fixed_scale" || parseSeriesScale(scale) !== null;
  const ok = !!layer && !!spec.nameField && marginOk && scaleOk;
  const pick = (p: Partial<SeriesSpec>) => setSpec((s) => ({ ...s, ...p }));
  const apply = () => {
    const out: SeriesSpec = {
      ...spec,
      marginPct: parseMargin(margin) ?? 10,
      ...(spec.extent === "fixed_scale" ? { scale: parseSeriesScale(scale) ?? undefined } : { scale: undefined }),
      sortField: spec.sortField || undefined,
    };
    useLayouts.getState().setSeries(out);
    onClose();
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Map Series</DialogTitle>
          <DialogContent className="stack">
            <div className="muted small">One page for each feature of an index layer (for example, one map per county). Select a map frame and tick "Follows the map series" in its properties; it then moves to each feature.</div>
            <Field label="Index layer" size="small">
              <Dropdown size="small" placeholder="Choose a layer" value={layer?.name ?? ""} selectedOptions={spec.indexLayer ? [spec.indexLayer] : []} onOptionSelect={(_, d) => pick({ indexLayer: d.optionValue ?? "", nameField: layers[d.optionValue ?? ""]?.fields[0]?.name ?? "", sortField: undefined })}>
                {vector.map((l) => <Option key={l.id} value={l.id}>{l.name}</Option>)}
              </Dropdown>
            </Field>
            <div className="row2">
              <Field label="Page name field" size="small">
                <Dropdown size="small" value={spec.nameField} selectedOptions={[spec.nameField]} onOptionSelect={(_, d) => pick({ nameField: d.optionValue ?? "" })}>
                  {fields.map((f) => <Option key={f.name} value={f.name}>{f.name}</Option>)}
                </Dropdown>
              </Field>
              <Field label="Order pages by" size="small">
                <Dropdown size="small" value={spec.sortField || `${spec.nameField} (the name)`} selectedOptions={[spec.sortField ?? ""]} onOptionSelect={(_, d) => pick({ sortField: d.optionValue || undefined })}>
                  <Option value="" text={`${spec.nameField} (the name)`}>{`${spec.nameField} (the name)`}</Option>
                  {fields.filter((f) => f.name !== spec.nameField).map((f) => <Option key={f.name} value={f.name}>{f.name}</Option>)}
                </Dropdown>
              </Field>
            </div>
            <Field label="Map extent of each page" size="small">
              <Dropdown size="small" value={spec.extent === "fit" ? "Fit the feature" : "Fixed scale"} selectedOptions={[spec.extent]} onOptionSelect={(_, d) => pick({ extent: d.optionValue as SeriesSpec["extent"] })}>
                <Option value="fit">Fit the feature</Option>
                <Option value="fixed_scale">Fixed scale</Option>
              </Dropdown>
            </Field>
            {spec.extent === "fit" ? (
              <Field label="Space around the feature (%)" size="small" validationState={marginOk ? "none" : "error"} validationMessage={marginOk ? undefined : "A number from 0 to 50"}>
                <Input size="small" value={margin} onChange={(_, d) => setMargin(d.value)} />
              </Field>
            ) : (
              <Field label="Scale 1 :" size="small" validationState={scaleOk ? "none" : "error"} validationMessage={scaleOk ? undefined : "A whole number, for example 25000"}>
                <Input size="small" value={scale} placeholder="25000" onChange={(_, d) => setScale(d.value)} />
              </Field>
            )}
            <div className="muted small">Text can use {"{page_name}"}, {"{page_number}"}, {"{page_count}"} and {"{page_<field>}"} (for example {"{page_pop}"}).</div>
            {cur && !hasFollower && <MessageBar intent="warning"><MessageBarBody>No map frame follows the series yet.</MessageBarBody></MessageBar>}
          </DialogContent>
          <DialogActions>
            {cur && <Button appearance="subtle" onClick={() => { useLayouts.getState().setSeries(null); onClose(); }}>Remove series</Button>}
            <Button appearance="secondary" onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!ok} onClick={apply}>{cur ? "Apply" : "Create series"}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
