// UX12: tool help inside the app. Everything about a tool comes from its ToolSpec (name, description, inputs, output, each setting's help
// text); the engine adds the citation (when the tool implements a published method) and a before-and-after picture, which is the tool's
// real result on a small built-in sample, so the picture can never disagree with the tool.
import { Popover, PopoverSurface, PopoverTrigger } from "@fluentui/react-components";
import { QuestionCircle16Regular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { api, errorText, type HelpLayer, type ToolHelpInfo } from "../ipc/api";
import { useRun } from "../store/model";

const KIND = { vector: "Features", table: "Table", raster: "Raster", any: "Any data" } as const;

/** Draw one picture panel. Paths come from the engine as SVG path data (polygons, lines) or `cx="…" cy="…"` (points). */
export function HelpPanel({ title, layers, view }: { title: string; layers: HelpLayer[]; view: [number, number, number, number] }) {
  const [x, y, w, h] = view;
  return (
    <figure className="thelp-fig">
      <svg viewBox={`${x} ${y} ${w} ${h}`} role="img" aria-label={title} preserveAspectRatio="xMidYMid meet">
        {layers.map((l, i) =>
          l.paths.map((d, k) => {
            const cls = `thelp-${l.role} thelp-${l.kind}`;
            const pts = [...d.matchAll(/cx="([-\d.e]+)"\s+cy="([-\d.e]+)"/g)];
            if (pts.length) return pts.map((m, j) => <circle key={`${i}-${k}-${j}`} className={cls} cx={m[1]} cy={m[2]} r={w * 0.018} />);
            return <path key={`${i}-${k}`} className={cls} d={d} vectorEffect="non-scaling-stroke" />;
          }),
        )}
      </svg>
      <figcaption>{title}</figcaption>
    </figure>
  );
}

export function ToolHelpBody({ tool }: { tool: string }) {
  const spec = useRun((s) => s.specs[tool]);
  const [info, setInfo] = useState<ToolHelpInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setInfo(null);
    setErr(null);
    api.toolHelp(tool).then((h) => live && setInfo(h), (e) => live && setErr(errorText(e)));
    return () => {
      live = false;
    };
  }, [tool]);
  if (!spec) return null;
  const settings = spec.params.filter((p) => !p.hidden);
  return (
    <div className="thelp" data-testid="tool-help">
      <h4>{spec.name}</h4>
      <p>{spec.description}</p>
      {info?.picture && (
        <div className="thelp-pics" data-testid="tool-help-picture">
          <HelpPanel title="Before" layers={info.picture.before} view={info.picture.view} />
          <HelpPanel title="After" layers={info.picture.after} view={info.picture.view} />
        </div>
      )}
      {info?.picture && <div className="thelp-note">Made by running {spec.name} on a small sample with its default settings.</div>}
      {info && !info.picture && info.pictureNote && <div className="thelp-note">{info.pictureNote}</div>}
      {err && <div className="thelp-note">Could not make the picture: {err}</div>}
      {!!spec.inputs.length && (
        <>
          <h5>Inputs</h5>
          <ul>{spec.inputs.map((p) => <li key={p.id}>{p.label} <em>{KIND[p.kind]}</em></li>)}</ul>
        </>
      )}
      {spec.output && (
        <>
          <h5>Output</h5>
          <ul><li><em>{KIND[spec.output]}</em></li></ul>
        </>
      )}
      {settings.length > 0 && (
        <>
          <h5>Settings</h5>
          <ul>
            {settings.map((p) => (
              <li key={p.id}>
                <b>{p.label}</b>{p.optional ? " (optional)" : ""}{p.help ? ` — ${p.help}` : ""}
              </li>
            ))}
          </ul>
        </>
      )}
      {info?.citation && (
        <>
          <h5>Method</h5>
          <p className="thelp-cite" data-testid="tool-help-citation">
            {info.citation.text}
            {info.citation.url && (
              <>
                {" "}
                <a href={info.citation.url} target="_blank" rel="noreferrer">{info.citation.url}</a>
              </>
            )}
          </p>
        </>
      )}
    </div>
  );
}

/** The "?" on a node card: opens the tool's help. */
export function HelpButton({ tool, name }: { tool: string; name: string }) {
  return (
    <Popover positioning="below-start" withArrow>
      <PopoverTrigger disableButtonEnhancement>
        <button className="bnode-eye nodrag" aria-label={`Help for ${name}`} title={`What ${name} does, with a before-and-after picture`} onClick={(e) => e.stopPropagation()}>
          <QuestionCircle16Regular />
        </button>
      </PopoverTrigger>
      <PopoverSurface className="thelp-pop">
        <ToolHelpBody tool={tool} />
      </PopoverSurface>
    </Popover>
  );
}
