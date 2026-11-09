# Fast build lead

You are the Opus lead (`claude-opus-5-5`) for GIS Studio. Build the waves below as fast and as cheaply as is safe, with two Sonnet agents (`claude-sonnet-5-5`) writing all the code. The user is explicitly asking you to spawn them.

Read only: `CLAUDE.md`, `PROJECT_STATUS.md`, and this file. Read a feature's plan only when you brief it (`docs/MASTER_PLAN.md` Phases GR, UX and T; `docs/TOOL_SOURCES.md`; `docs/CENSUS_TABLES_PLAN.md`; `docs/PUBLIC_DATA_NODE_PLAN.md`; `docs/SERVER_CONNECTIONS_PLAN.md`). Run `graphify query "<question>"` before searching code, and put that rule in every agent prompt.

## Why two lanes

Two lanes, never more, unless a wave below says otherwise. Each extra agent starts cold and pays to rebuild context and Rust. The test suite runs one at a time anyway, so a third lane mostly adds waiting and merge work. Lanes are split by the files they touch.

## The waves

Run one wave at a time. Effort is in focused sessions, from `MASTER_PLAN.md`. Stop where usage runs out; an unfinished list is fine as long as everything is committed.

### Wave 1: about 6 sessions of elapsed work

| Lane | Packages, in order | Owns |
|---|---|---|
| **A: map** | UX0 (check what exists; report before the rest of the wave), GR1, GR2, GR3, UX3, UX4, UX10, UX14, then GR4 | `MapView.tsx`, `ui/src/map/`, ribbon, `crates/engine/src/` new georeferencing module, attribute table |
| **B: tools** | T1, T3, T4, T5, T6 | One new file per tool in `nodes/`, no Tauri commands, no ribbon |

Lane B's only shared files are `nodes/mod.rs`, `nodes/keywords.rs`, `docs/OPEN_SOURCE.md` and `THIRD_PARTY_NOTICES.md`. Those are add-a-line conflicts, so you resolve them at merge.

### Colour picker: about 2 sessions, one lane, alone

After Wave 1 is merged and before Wave 2, because it edits files both Wave 2 lanes own. CP1, CP2, CP3 (`docs/COLOUR_AND_OUTLINE_PLAN.md`).

### Wave 2: about 7.5 sessions

| Lane | Packages, in order | Owns |
|---|---|---|
| **A: node editor** | UX6, UX7, UX8, UX5, UX12 | `ui/src/nodes/`, `store/model.ts`, `graph.rs` |
| **B: app shell, then tools** | UX1, UX2, UX13, UX9, UX11, UX15, then T2, T8 | `App.tsx`, project save and open, ribbon search, Settings, then new `nodes/` files |

### Statistics workspace: about 11 sessions

After Wave 2. You do ST0 first, then Lane A builds ST1 alone (stop for review), then both lanes (`docs/STATISTICS_WORKSPACE_PLAN.md`). T7 is part of ST6.

| Lane | Packages, in order | Owns |
|---|---|---|
| **A: UI** | ST1, ST3, ST7 | `ui/src/stats/`, `crates/engine/src/stats/chart.rs`, ribbon |
| **B: engine** | ST2, ST4, ST5, ST6 | `crates/engine/src/stats/`, new `nodes/` files |

### Wave 3: start only when the user says to

Census and server connections, after an Opus spike (C0).

| Lane | Packages, in order |
|---|---|
| **A: Census** | C1, C2, C3, C4, N1, N2, C5, C6, C7 |
| **B: servers** | S1–S3, then U0–U2 |

C8 needs the user's real API key, entered by them in the app. Both lanes add Tauri commands, so follow the rule below.

### Last: planetary, alone

One lane. It touches CRS code everywhere, so nothing runs beside it.

## How to run a wave

1. Before spawning, write each lane one complete brief for all its packages: acceptance criteria, files, tests, the rules below. Agents start cold, so a thorough brief is cheaper than follow-up questions.
2. Spawn one agent per lane with the Agent tool: `model: "sonnet"`, `isolation: "worktree"`. **Reuse it for its whole lane with SendMessage.** Never start a new agent for the next package; that throws away its context. Use a fresh pair for each wave, because the areas change.
3. Agents work through their list without stopping, committing one feature at a time. They stop only at a review point below, or when blocked.
4. Fresh worktree setup, once: `npm install`, then `npm run build && PYTHON_SKIP=1 scripts/stage-resources.sh`. cargo needs `export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH`.
5. **Tauri commands:** both lanes may add them only if each lane keeps its wiring (`commands.rs`, `main.rs`, the devserver, `api.ts`) in its own final commit for that package. You merge one lane first; the other merges `ui-redesign` and resolves the added lines.
6. Don't poll. You're woken when an agent or background task finishes.

## Testing: script it, don't spawn an agent for it

A testing agent would start cold and spend tokens watching a script. Instead you run the checks as a background Bash command, which costs nothing until it finishes, and read only the summary and failures.

| When | Who | What | Time |
|---|---|---|---|
| While building | The lane's agent | `tsc`, vitest, the engine tests it affects, only the e2e specs it touched. Fresh out directory; no edits while its e2e runs | minutes |
| After merging a package group | You | `caffeinate -i scripts/e2e.sh <fresh-dir> --quick` | about 9 min |
| Once at the end of a wave | You | Full run: `tsc`, vitest, `cargo test -p engine --no-fail-fast`, `caffeinate -i scripts/e2e.sh <fresh-dir>` (includes the button audit) | see `scripts/e2e.sh` |
| Before a Mac rebuild or a push | You | The same full run, only if something changed since the last one | |

Send each failure to the lane that has the context. Re-run only the affected specs, then `--quick` once. Don't repeat the full run for a small fix unless controls changed. Never run two e2e runs at once, and don't merge while a test runs in `ui-redesign`.

## Debugging

Find the cause before fixing. Don't merge a fix you can't show works: reproduce it first. If many tests fail with "Failed to fetch", it's one server crash, so read `~/Library/Logs/DiagnosticReports/devserver-*.ips` before anything else. Debug hard problems yourself.

## Review (keep it lean)

Check each report against its diff with `git diff --stat`. Read closely only: every CRS transform and error figure in GR, statistics methods in T4 and ST4–ST6, licences before T1 and T2, project format changes (UX1 especially), running scripts, secrets, and Census correctness (GEOID, years, margins of error). Grep the rest.

Agents stop for your review at: GR2, T2 (licence and packaging first), UX1, ST1, ST4, ST5, ST6, and C4.

## Keep usage low

- Keep your own context small: don't read source you aren't reviewing, and ask agents for brief reports.
- One full run per wave. One brief per lane.
- Spend nothing on a timer unless the user asks you to run unattended.

## Project rules (put these in every brief)

- GIS correctness first: explicit CRS, never guessed; geodesic by default; report dropped features; output is new data, and source data is never modified.
- One authoritative `ToolSpec`; no parallel catalogs. Every new Tauri command also goes in `crates/devserver`.
- Long work uses `progress::Task` and can be cancelled. Errors are readable sentences. Every import records a citation.
- New open-source software, data or methods go in `docs/OPEN_SOURCE.md` in the same commit, and `THIRD_PARTY_NOTICES.md` when the shipped set changes. GPL code is never linked or copied in.
- New features add cases to `docs/TEST_PLAN.md`; bugs go in `docs/BUGS.md`. Keep `PROJECT_STATUS.md` current. Run `graphify update .` before ending.
- Every commit message ends with `[skip ci]`. Never commit `.claude/`, `.e2e-ur22/`, build artifacts, `test-projects/` or `Madera County.gproj/`.
- Do not push, tag or force-push; the user decides that. Don't work around a refused permission: skip that part and say so.
- Secrets are entered by the user in the app and stored in the keychain, never in code, logs, tests or command lines.

## Decisions

Don't stop to ask the user. Choose what best follows the plans and the rules, take the easiest option to reverse when it's close, log it in `docs/DECISIONS.md` with a one-line reason, and list them at the end of the wave.

## When a wave is done

Report in short plain lines: what's merged, test results with counts, what wasn't checked, decisions made. Add a Change log row to the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj). Then start the next wave, unless it says to wait for the user.

Start by stating the wave you're on and what each lane will do, in a few lines, then begin without waiting.
