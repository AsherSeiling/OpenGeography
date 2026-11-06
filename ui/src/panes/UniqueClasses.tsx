// The class list of a unique-values style (UR-12): start empty, add a class by typing a value or picking one of the field's values,
// put several values in one class, give the leftover values ("all other values") a colour and legend text, or leave them undrawn.
import { Button, Checkbox, Dropdown, Input, Option } from "@fluentui/react-components";
import { useEffect, useState } from "react";
import { api, errorText, type LayerInfo } from "../ipc/api";
import { moveClass, reverseClasses } from "../map/categoryOrder";
import type { LayerStyle } from "../store/store";
import { addAllValues, addValueClass, addValueToClass, classValues, groupClasses, removeClass, ungroupClass } from "../layers/uniqueClasses";

export function UniqueClasses({ info, style, setStyle, setErr }: { info: LayerInfo; style: LayerStyle; setStyle: (p: Partial<LayerStyle>) => void; setErr: (m: string | null) => void }) {
  const classes = style.classes ?? [];
  const field = style.field ?? "";
  const [values, setValues] = useState<[string, number][] | null>(null);
  const [typed, setTyped] = useState("");
  const [checked, setChecked] = useState<number[]>([]);
  const [dragFrom, setDragFrom] = useState<number | null>(null);

  // The field's values (most frequent first), for the picker and "Add all values".
  useEffect(() => {
    let live = true;
    setValues(null);
    if (field) api.uniqueValues(info.id, field, 500).then((v) => live && setValues(v)).catch((e) => live && setErr(errorText(e)));
    return () => { live = false; };
  }, [info.id, info.version, field]);
  useEffect(() => setChecked([]), [classes.length, field]);

  const set = (next: typeof classes) => setStyle({ classes: next });
  const add = (value: string, count?: number) => {
    const r = addValueClass(classes, value, count ?? values?.find(([v]) => v === value)?.[1]);
    if ("error" in r) return setErr(r.error);
    setErr(null);
    set(r.classes);
  };
  const free = (values ?? []).filter(([v]) => !classes.some((c) => classValues(c).includes(v)));

  return (
    <>
      <div className="btn-row">
        <Button size="small" disabled={!values || free.length === 0} onClick={() => set(addAllValues(classes, values ?? []))} title="Make a class for every value of the field that has none yet">
          Add all values{values ? ` (${free.length})` : ""}
        </Button>
        <Button size="small" disabled={classes.length === 0} onClick={() => set([])}>Remove all</Button>
      </div>
      <div className="btn-row">
        <Dropdown size="small" placeholder="Pick a value…" aria-label="Pick a value to add" value="" selectedOptions={[]} disabled={free.length === 0} style={{ minWidth: 0, flex: 1 }} onOptionSelect={(_, d) => d.optionValue !== undefined && add(d.optionValue)}>
          {free.slice(0, 300).map(([v, n]) => (
            <Option key={v} value={v} text={v || "<empty>"}>{`${v || "<empty>"} (${n})`}</Option>
          ))}
        </Dropdown>
        <Input size="small" aria-label="Type a value to add" placeholder="or type a value" value={typed} onChange={(_, d) => setTyped(d.value)} onKeyDown={(e) => e.key === "Enter" && typed !== "" && (add(typed), setTyped(""))} style={{ flex: 1, minWidth: 0 }} />
        <Button size="small" disabled={typed === ""} onClick={() => (add(typed), setTyped(""))}>Add class</Button>
      </div>
      {classes.length === 0 && <div className="muted small">No classes yet. Every feature is drawn as "all other values" until you add some.</div>}
      <div className="classes" data-testid="classes">
        {classes.map((c, i) => (
          <div
            key={c.value}
            className={`class-row ${dragFrom === i ? "dragging" : ""}`}
            draggable
            onDragStart={(e) => { setDragFrom(i); e.dataTransfer.effectAllowed = "move"; }}
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => { e.preventDefault(); if (dragFrom !== null) set(moveClass(classes, dragFrom, i)); setDragFrom(null); }}
            onDragEnd={() => setDragFrom(null)}
          >
            <Checkbox aria-label={`Select ${c.label}`} checked={checked.includes(i)} onChange={(_, d) => setChecked(d.checked ? [...checked, i] : checked.filter((x) => x !== i))} />
            <span className="class-grip" title="Drag to change the drawing order" aria-hidden>⠿</span>
            <input type="color" aria-label={`Colour of ${c.label}`} value={c.color} onChange={(e) => set(classes.map((x, j) => (j === i ? { ...x, color: e.target.value } : x)))} />
            <Input size="small" className="grow" aria-label={`Legend text of class ${i + 1}`} value={c.label} onChange={(_, d) => set(classes.map((x, j) => (j === i ? { ...x, label: d.value } : x)))} title={`Values: ${classValues(c).map((v) => v || "<empty>").join(", ")}`} />
            {(c.values?.length ?? 0) > 0 && (
              <Button size="small" appearance="subtle" aria-label={`Split ${c.label} into one class per value`} title={`${classValues(c).length} values: ${classValues(c).join(", ")}. Click to split`} onClick={() => set(ungroupClass(classes, i))}>
                {classValues(c).length} values ⤫
              </Button>
            )}
            <Button size="small" appearance="subtle" aria-label={`Draw ${c.label} higher`} disabled={i === 0} onClick={() => set(moveClass(classes, i, i - 1))}>▲</Button>
            <Button size="small" appearance="subtle" aria-label={`Draw ${c.label} lower`} disabled={i === classes.length - 1} onClick={() => set(moveClass(classes, i, i + 1))}>▼</Button>
            <Button size="small" appearance="subtle" aria-label={`Remove ${c.label}`} onClick={() => set(removeClass(classes, i))}>✕</Button>
          </div>
        ))}
        <div className="class-row other-row" data-testid="other-class">
          <span className="class-grip" aria-hidden />
          <input type="color" aria-label="Colour of all other values" value={style.fill} onChange={(e) => setStyle({ fill: e.target.value, ...(info.geomType !== "Polygon" ? { stroke: e.target.value } : {}) })} />
          <Input size="small" className="grow" aria-label="Legend text of all other values" placeholder="Other" value={style.otherLabel ?? ""} onChange={(_, d) => setStyle({ otherLabel: d.value })} title="The legend text of the values that have no class above" />
          <Checkbox label="All other values" checked={!style.otherHidden} onChange={(_, d) => setStyle({ otherHidden: d.checked ? undefined : true })} />
        </div>
      </div>
      <div className="btn-row">
        <Button size="small" disabled={checked.length < 2} onClick={() => { set(groupClasses(classes, checked)); setChecked([]); }} title="Draw the ticked classes as one class, in the colour of the first">
          Group ticked classes
        </Button>
        {checked.length === 1 && (
          <Dropdown size="small" placeholder="Add a value to the ticked class…" aria-label="Add a value to the ticked class" value="" selectedOptions={[]} disabled={free.length === 0} onOptionSelect={(_, d) => {
            if (d.optionValue === undefined) return;
            const r = addValueToClass(classes, checked[0], d.optionValue);
            if ("error" in r) setErr(r.error);
            else { setErr(null); set(r.classes); }
          }}>
            {free.slice(0, 300).map(([v, n]) => (
              <Option key={v} value={v} text={v || "<empty>"}>{`${v || "<empty>"} (${n})`}</Option>
            ))}
          </Dropdown>
        )}
        <Button size="small" onClick={() => set(reverseClasses(classes))}>Reverse order</Button>
      </div>
      <div className="muted small">The top of the list draws on top, on the map, in layouts and in the legend. Values with no class are "all other values", drawn in the colour beside it (or not at all when it is unticked).</div>
    </>
  );
}
