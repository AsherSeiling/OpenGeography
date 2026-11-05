# UI redesign: verification (WP9, 2026-11-04)

What was run, what it showed, and what was not checked. Browser only (Chrome through Playwright against the dev server). **Not checked: the packaged app, Windows, Linux, WebKit.**

## Control audit (`e2e/redesign.spec.ts`)
A crawler: for each context it clicks every control in the ribbon band of every tab, every menu item and submenu item, the top-left cell, quick access, search, the Insert rail, and the controls of each pane, and records whether the page changed (text with open menus and tooltips left out, control states, field values, dock, stores, map view, an engine call that changes something, a request for a native dialog). No change, or an uncaught error, fails the control. Disabled controls are not clicked; they are listed per context.

Contexts: map with no layers; map with two layers, an active layer and a selection; map with a node on the tree; imported points with a finished run and a citation; layout with nothing selected, a map frame, a text, and three items selected. Panes are audited from a freshly built context for each control.

| | Count |
|---|---|
| Controls and menu items recorded | 886 |
| Clicked (opened a menu, or changed the page) | 827 (762 changed the page, 64 opened a menu to be walked through) |
| Disabled in that context (not clicked) | 49 |
| Not exercised: the control's name changed after an earlier click in the same run | 10 |
| Dead (changed nothing) after the exceptions below | 0 |
| Threw | 0 |

**Exceptions (20 clicks, named in `NO_CHANGE_OK` with the reason):** Explore when it is already the tool; View ▸ Map and Attribute Table when that pane is already showing; Fit Page when it is already on; picking WGS 84 in the CRS list when the map is already WGS 84; Align Left, Centre and Right for a single element (or for items that share x and width, as the start elements do: x 12.7, width 254 on a 279.4 mm page); Same Width as First and Distribute Horizontally for the same elements. Each was checked against the numbers, not assumed.

**Stopped at a native file dialog (22 controls).** In a browser a native dialog is `window.prompt`; the audit answers Cancel (confirms and alerts: OK). Nothing was written. The audit therefore did **not** exercise what follows the dialog for: layout Export PDF, PNG and SVG and Save as Template (frame, page and text contexts), Insert ▸ Picture (rail and toolbar), Add Data (empty and layer contexts), Save, project menu Open and Save As, and Save as new layer… on the node bar. `e2e/redesign-walkthrough.spec.ts` does export a layout to PDF through the prompt with a real path (16 KB file written), and the older specs cover Save/Open (`misc.spec.ts` M-23) and the exports.

**Disabled in every context tried (not exercised):** Export Series (needs a configured series; `series.spec.ts` C3.2 covers it), Delete Template ▸ "No templates of yours yet" and Map Layers ▸ "Every layer in the project is already on this map" (placeholder rows, disabled by design), Preview on map in Properties (stays disabled after a finished run on the audit's node; Run to here and Preview on the node bar were not separately confirmed live), Redo in the Node Editor bar (live only after an Undo; the quick-access Redo was exercised after an Undo). Ungroup is live only for a group: it was exercised after Group, from the ribbon and from the Items pane.

**What the audit found and fixed:** Reset Panes while a layout was open crashed the dock ("panel with id contents already exists") after the new workspace rearrangement (my own change; fixed in `panes.ts` and `layout/actions.ts`). The Coordinate System dialog's quick picks listed the UTM zone twice outside North America (CRS-1, latent; exposed by MM-1). Nothing else was dead. Honest limit: "changed something" is not "did the right thing"; the older specs check the results of most commands.

## Walk-through (`e2e/redesign-walkthrough.spec.ts`)
Scenario each time: 20 test polygons; Shift+A "buffer"; the tab-row search "dissolve"; Shift+A "simplify" (picked by click); the tree reads Layer Input → Buffer → Dissolve → Simplify → Output with four wires (each pick wires itself); Run (all five nodes "done"); a Thematic layout from the start gallery; Export › PDF through the prompt (file written, 15 to 18 KB).

| Mode | What I saw (screenshots in `docs/screenshots/ui-redesign/`) |
|---|---|
| Light, 1600×950 | As above, passes. Menus, node cards, Properties and ribbon draw as in the mock-up; Run is outlined. |
| Dark | Same scenario passes; text, node cards and the layout page (white, as it must be) read well. |
| 1024×700 | Ribbon has no clipped control and the page does not scroll sideways; the Shift+A menu sits inside the window (682×454 at 336,244); in the layout workspace the tabs read Items, Catalog, Map, Layout 1, Properties, Sources, none cut. The page area is small (about 40 % zoom) and the Properties fields run to the window edge. |
| Keyboard only | Shift+A, type, Enter three times (pointer parked over the canvas); Run reached by focus and Enter; ArrowRight on the empty menu field changes category; the layout PDF export by focus and Enter. |
| Layout workspace | Bottom panel closed; Items column present and readable; Properties on the right; Back to Map restores the bottom panel and Map Layers / Catalog (`layout-leftovers.spec.ts`). |

I looked at the screenshots listed above (map, node tree, layout workspace, narrow layout). I did not hand-drive the app with a real mouse and keyboard beyond what the specs drive, and did not look at every dialog.

## AA, focus, labels (`e2e/redesign-a11y.spec.ts`)
Text contrast is computed from the browser's colours (text against the first opaque background behind it): map view 95 text elements, layout workspace 136, in both themes; lowest ratio 4.72:1 (light), 5.64:1 (dark); none below AA. Every button, link, menu item, tab, field and combobox on screen has an accessible name (zero without). Tabbing through the first 45 stops shows a focus indicator everywhere; the probe cannot see two that were checked by hand in CSS: the search box (ring on its wrapper, `.search:focus-within`) and Catalog rows (border colour, `.cat-row:focus-visible`). **Link text on a hovered `--surface-0` was 4.05:1 (light), BUGS UI-1: fixed in CSS (`theme/base.css`), no token change.** Not covered by the probe: dialogs, text on images and canvases, the layout page.

## Test results at this commit (my branch, 2026-11-04)
- `npx tsc -p tsconfig.json` clean; `npx vitest run` 598 passed (79 files); `cargo test -p engine` 540 passed, 0 failed, 28 ignored (network and long tests).
- e2e on my own devserver, the specs this work touches or could affect (layout*, map, multi-map, shift-a, model, table, misc, series, templates, topo, split, dock-overflow, ribbon-width, crs-ui, catalog, basemaps, appearance, choose-layer, plus the new a11y and walk-through specs): 141 passed, 0 failed after one test change below. The whole `scripts/e2e.sh` was **not** run by me; the lead runs it on the merge.
- Test edits for changed behaviour (targets, not dropped checks): `layout-items.spec.ts` "Model canvas stays tall" now asserts the Node Editor is gone under a layout; `catalog.spec.ts` CAT-3 goes Back to Map before looking at Map Layers, which a layout now replaces with Items.
- Seen flaky once and passing on re-run: `layout-editor.spec.ts` "keyboard: the rail, the tabs of Properties and the menus work without a mouse" (timed out in one batch).
