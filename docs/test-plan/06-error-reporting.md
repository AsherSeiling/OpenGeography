# 6. Error reporting: how a failure is recorded, routed and triaged

Applies to every case in this plan and to anything a tester notices outside it.

## 6.1 Recording a failure (the tester's steps)

1. **Stop and keep the evidence.** Do not close the error pop-up yet. Click **Copy details** (it copies title, message, `While: <action>`, `Source:` and the stack). If the pop-up did not appear, note that: *a failure without a pop-up is itself a bug* (see 6.5).
2. **Capture the logs.**
   - Browser build: the `./target/release/devserver` terminal output and the browser console (`window.__gis` hooks help; Playwright runs write `e2e-devserver.log`, `e2e-vite.log` in `test-results/<time>/`).
   - Packaged app: launch from a terminal (`"GIS Studio.app/Contents/MacOS/gis-studio"`, `gis-studio.exe`, the AppImage) so stderr is visible; UI errors are also written there as `[ui:<level>] …` by `frontend_log`.
3. **Make it reproducible.** Save the project (File → Save As) in a new folder, or note the input files and where they came from (URL, vintage, bounding box). For downloads, record the area, datasets and time.
4. **Write the entry** using the template below.

### Bug template (copy into `docs/BUGS.md`)

```markdown
### <ID> — <one-line summary> (<Severity>) — OPEN
- **Case:** <case ID from docs/TEST_PLAN_FULL.md or docs/TEST_PLAN.md, or "none (found ad hoc)">
- **Build:** commit <short sha> (`git rev-parse --short HEAD`), <packaged app | devserver+vite | cargo test>, version <x.y.z>
- **OS:** <macOS 15.x arm64 | Windows 11 | Ubuntu 24.04>, display scale, locale
- **Steps:** 1. … 2. … 3. … (exact menu names; data; settings)
- **Expected:** <what should happen, incl. the kind of message expected>
- **Actual:** <what happened>
- **Error pop-up text:** <title + message pasted from "Copy details"; or "none shown">
- **Log excerpt:** <devserver/app stderr lines; browser console errors>
- **Repro data:** <project folder path or attachment, input files, download parameters; "generated test data N points" if reproducible without files>
- **Frequency:** <always | n of m runs | after specific sequence>
- **Severity:** <Blocker | High | Medium | Low> — see 6.4
- **Suspect:** <file/function if known>
- **Regression test:** <test name to add, or "to be added">
```

## 6.2 Where it goes

1. **`docs/BUGS.md`** — one entry per failure under the right heading (`Found by tests`, `Found by e2e`, `Reported by the user`). The **next ID**: the highest existing ID with the same prefix + 1 (prefixes in use: `B-T<n>` tests, `E2E-<n>` browser, `U-<n>` user reports). Add a row to the table at the top of the section and the detail block below it. Use `B-T`/`E2E`/`U` according to who found it; automated plan runs use `B-T`.
2. **Status doc** — add a line under "State"/"Known issues" in `PROJECT_STATUS.md` for any High or Blocker, and tell the orchestrator so the "GIS Studio — Project Status" Claude Doc is updated.
3. **Failing test** — where practical write the regression test first and mark it `#[ignore = "BUG: <ID> …"]` (engine) or `test.fixme("… BUG <ID> …")` (e2e). `scripts/test_plan_ignored.py` then lists and runs it as category `bug`; when it starts passing the run fails and says to unignore it and update `docs/BUGS.md`.
4. **Case table** — put the bug ID into that case's Auto column (`auto (BUG U-5)`) and, if the case did not exist, add it to the right file under `docs/test-plan/`.
5. When fixed: mark the row **Fixed** with date and the fix summary (as the existing rows do), unignore the test, link the commit.

## 6.3 Mapping automated failures to case IDs

`scripts/run-test-plan.sh` writes `test-results/<time>/summary.md` with one row per stage (`static`, `ui`, `engine`, `engine-ignored`, `e2e`, `package`, `self-test`), plus a log per stage. To get from a failing line to a case ID:

| Failing output | Look up |
|---|---|
| `test <name> ... FAILED` in `engine.log` / `ignored.md` | `grep -rn "<name>" docs/TEST_PLAN.md docs/test-plan/` — the Auto column holds the test name |
| `FAIL ui/src/…/<file>.test.ts > <describe> > <it>` in `ui.log` | the same grep on `<file>.test.ts` and the describe name |
| Playwright `[chromium] › <spec>.spec.ts › M-<n> …` in `e2e.md` | the `M-<n>` id is the manual case (TP §14); the spec file name is in the Auto column |
| `SELF-TEST FAILED: …` | PKG-01…PKG-03 |

**Convention required going forward (to automate: planned in the first automation batch):** every new test names its case ID so the mapping is mechanical — Rust: `fn imp_02_antimeridian_import_is_kept()`; vitest/Playwright: `it("CRS-06 …")`, `test("M-4 …")`. A small script `scripts/map_failures.py` reads the stage logs, extracts failed test names, matches case IDs from the name prefix or from the Auto columns, and appends a "Failed cases" table to `summary.md` (columns: case ID, priority, test, log file). Until it exists, use the grep above.

## 6.4 Triage rules

| Severity | Definition | Blocks a merge to `main` | Blocks a release tag (`v0.x.0`) |
|---|---|---|---|
| **Blocker** | Data loss or corruption (project, source file overwritten), crash / process abort (SIGSEGV), wrong result with no warning (CRS guessed, wrong geometry), secret leaked | **Yes**, fix first | **Yes** |
| **High** | Feature unusable on a main path, raw JS/Rust text in front of the user in a normal flow, hang with no cancel, platform-specific failure on a supported OS | **Yes** if the branch touches the area or introduced it; otherwise allowed with a BUGS entry and orchestrator approval | **Yes** |
| **Medium** | Wrong behaviour with a workaround, unclear message, performance miss > 2× budget | No | Allowed if listed in release notes; > 5 open Medium in one area blocks |
| **Low** | Cosmetic, rough edge | No | No |

Additional rules:
- A **P0 case that fails** is at least High; a P0 that crashes or loses data is Blocker.
- A new **failing P0/P1 test must never be silently ignored**: `#[ignore]` only with `BUG: <ID>` in the reason and an entry in `docs/BUGS.md`.
- CI red on `main` blocks everything until green or the failure is a documented, owned bug with a test marked ignored.
- Time to first response: Blocker same day; High before the next merge batch; Medium/Low at planning.
- Flaky tests (fail then pass on rerun) are bugs: log as Medium with the failure rate; never retry to green without a BUGS entry (cf. the layout SIGSEGV note in `PROJECT_STATUS.md`).

## 6.5 Error-path checklist: every failure shows a readable sentence, never a raw error

Run this checklist before every release and when a new feature adds an error path. Each line is a case; tick it in the release run's summary. "Pop-up" = the global error dialog (`ui/src/errors/ErrorDialog.tsx`); "inline" = message inside a dialog/pane (preferred for validation).

| ID | Check | How | Lvl | Auto | Pri |
|---|---|---|---|---|---|
| ERR-01 | An engine `Err(String)` from any command reaches the user as one sentence: no `Error:` prefix chains, no `Caused by`, no `anyhow` context dumps, no file paths of the *build machine*, no Rust type names | Table-driven vitest over `friendly()` + engine test collecting error strings from every command's bad-input case (CMD-03) and asserting a regex blacklist (`panicked at`, `Caused by`, `Binder Error`, `Catalog Error`, `duckdb`, `__node_`, `GEOS`, `TopologyException`, `thread '`, `unwrap()`, `RUST_BACKTRACE`, `[object Object]`, `undefined`, `NaN`) | E+V | partial: store.test.ts "turns raw JS errors into plain language"; to automate blacklist | P0 |
| ERR-02 | A JS exception inside a React pane shows the pane's error box (PaneBoundary), other panes stay usable, "Reopen" works | Throw in each of the 8 panes with a test hook | V+2E | partial: crashes.test.ts; to automate per pane | P0 |
| ERR-03 | `window.onerror` and `unhandledrejection` produce a pop-up with friendly text and "Copy details"; ResizeObserver noise is ignored | Fire both events | V | partial: store.test.ts | P0 |
| ERR-04 | Repeated identical errors collapse (count), queue keeps last 20, dismiss goes to the next | — | V | auto: store.test.ts | P1 |
| ERR-05 | Engine unreachable (devserver killed): "Can't reach the engine" and the app does not spin | Kill devserver during session | 2E | to automate | P0 |
| ERR-06 | Every user action wrapper (`action("…")`) reports with `While: <action>` context | Grep + test that every ribbon command catches | V | to automate (UI-01 harness) | P1 |
| ERR-07 | Validation errors appear inline in their dialog (Add Field, Calc Field, Define Projection, Export, TIGER, National Map, Elevation, Query) and do not also pop up | 2E per dialog | 2E | partial: M-11/M-12 e2e; to automate rest | P0 |
| ERR-08 | Download failures (offline, HTML, truncated, 4xx/5xx, timeout, cancelled) show the dialog's message naming the service; cancel is not an error | TIG-/TNM-/DEM- cases | E+2E | to automate (HTTP-MOCK) | P0 |
| ERR-09 | Node-tree failures show on the failing node and in the Output; downstream says "skipped because <node> failed" | EXE-01 | 2E | partial: TP 6.15 | P0 |
| ERR-10 | Panic in a command thread is caught: the app shows "Something went wrong in the engine" and stays alive; DuckDB connection poisoned state recovered | Inject panic via a test-only tool | E | to automate | P0 |
| ERR-11 | Native crashes (SIGSEGV in GDAL/GEOS) are not catchable: ensure each risky GDAL call null-checks (grep guard) and add a regression test per found crash | B-T5 class | E | partial: rasters.rs tests; to automate lint script for `*AppOptionsNew` | P0 |
| ERR-12 | Out-of-memory / disk-full / read-only paths give a sentence naming what to free or change | IMP-18, EXE-12, PRJ-07 | E+M | to automate / manual | P1 |
| ERR-13 | Secrets never appear in pop-up, details, logs (SEC-06) | grep of fixtures | V | pending feature | P0 |
| ERR-14 | Messages are spelled consistently: sentence case, end with a period or an action ("Define Projection first"), name the object (layer, file, setting) | Review checklist + simple lint test on message tables | M | manual | P2 |
| ERR-15 | Error details never block the app when very long (10 000 lines): collapsible, scrollable, copy works | Push a 1 MB error | V+2E | to automate | P2 |
| ERR-16 | Accessible: dialog traps focus, Esc dismisses, Enter on default button | — | 2E | to automate | P2 |
| ERR-17 | Errors raised in the Tauri build and in the devserver build read identically (same engine text) | Compare CMD-03 outputs across both servers | E | to automate | P1 |
| ERR-18 | Failure without a pop-up (silent) is detected: Playwright fixture `errors` fails any test that logs `console.error` not matched to a known pop-up | Already used in e2e `errors` fixture; make it default | 2E | partial: e2e `errors` fixture | P1 |
