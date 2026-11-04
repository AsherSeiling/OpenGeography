# UI redesign: continue as lead

You are the Opus lead for the rest of GIS Studio's UI redesign. Work on branch `ui-redesign`. Read `CLAUDE.md`, then `docs/UI_REDESIGN_PLAN.md` §2b, §2c, §6, §7 and `docs/LAYOUT_UI_OPTIONS.md`. Do not re-read the plan's finished parts. Run `graphify query "<question>"` before searching code.

**Approved by the user, do not reopen:**
- Layout editor: option B with add-ons 1, 2, 3, 4, 7, 9, and the menu levels in `docs/LAYOUT_UI_OPTIONS.md`.
- Look: the two mock-ups in `docs/design/`, which win on looks; the plan wins on behaviour.
- The user told you to merge each package into `ui-redesign` once it passes review, without asking each time. Nothing is pushed until the rules below are met.

**Done and merged:** WP1 tokens, WP2 keywords, WP3 tool menu, WP4 Catalog, WP5 node cards, WP6 shell, WP7 panes, WP8 layout and dialog styling, WP8b Colours, WP10 and WP11 multiple maps, WP12 basemaps, WP13 layout editor.

**Left, in order:**
1. **WP9 verify:** the control audit and walk-through in plan §7, then the end-of-work items.

**How to work (user chose this to save usage, 2026-11-03):**
- Sonnet writes code; `model: claude-sonnet-5-5`, `use_worktree: true`, `source_branch: ui-redesign`, `wake_when_done: true`.
- Workers run tsc, vitest and only their own e2e specs. Run the full `scripts/e2e.sh` once per merge batch, on `ui-redesign`.
- One real-app review per batch. Read closely only what can damage data or correctness: CRS handling, the project file format and old-project migration, basemap licences and citations. Grep the rest.
- Workers don't edit `ui/src/theme/hex.test.ts`, `theme/tokens.css` or `docs/*`; they report those changes and you apply them.
- Give each worker its own ports (`GIS_DEVSERVER_ADDR`, `GIS_VITE_PORT`, `VITE_GIS_DEVSERVER`), and tell it: kill only your own processes by PID, run long tests in the foreground with a timeout, and do not end your turn to wait for them.
- The app only starts sessions from the user's own message, so ask for a click before each batch. Merge and review turns need no click.

**Rules:** tests pass before any push; push only with `[skip ci]`; never tags or force-push; never commit `.claude/`, build artifacts, `test-projects/` or `Madera County.gproj/`; new open-source or data goes in `docs/OPEN_SOURCE.md` in the same commit; ask before anything outward-facing beyond this repo. After each change, add a row to the Change log of the user's status doc (see memory `status-doc`).

**Open items:** BUGS.md UI-1 (recheck link hover contrast in the walk-through). The Node Editor Run button is still solid blue in the app; the mock-up has it outlined, so make it match in WP13 or WP9. WP9 also fixes BUGS.md MM-1 and MM-2, and problems 1 and 2 in `docs/LAYOUT_UI_OPTIONS.md` (Model pane under the page, Items in a narrow column), which WP13 could not reach. Merged session worktrees under `.claude/worktrees/` can be removed once their sessions are idle; the user wants the folder small. `Show in Folder` and `Details` commands were removed from the Catalog with the file browser; the user was told.

**Done when:** plan §7 is met, before and after screenshots are in `docs/screenshots/ui-redesign/`, `PROJECT_STATUS.md` is updated, `graphify update .` has run, and you report what changed, what you decided, and what is left for the user.
