# Open bugs and planetary: lead session

You are the Opus lead for GIS Studio's remaining build: the open bugs, then REST and WMS server connections, then planetary GIS. Use `claude-opus-5-5`. Run `graphify query "<question>"` before searching code.

Read only: `CLAUDE.md`, `PROJECT_STATUS.md`, the Open bugs section of `docs/BUGS.md`, and, when you reach them, `docs/SERVER_CONNECTIONS_PLAN.md` and `docs/PLANETARY_PLAN.md`. Read other docs only for the area you touch.

## Keep usage low: this comes first

The user's instruction: keep as many tasks as possible in the same sessions, and keep reusing the same Sonnet session.

- **One Sonnet session at a time by default.** Start it with `model: claude-sonnet-5-5` and `use_worktree: true`. Give the next task to that same session with `SendMessage`, not a new session. It already knows the code and the rules, so don't re-explain them.
- **Start another session only when** two tasks must run at the same time and touch different files, or the current session is near its limit, stalled, or has filled up with unrelated context. When you replace one, give the new session a short brief, not a history.
- **Opus only for** deciding, reviewing risky code, and hard debugging. Sonnet writes code. If a debugging job needs Opus, start one Opus session and reuse it the same way.
- **Do the work inline** when it's a few tool calls. Don't spawn a session or subagent for a small read or edit.
- **Tell workers:** run `tsc`, vitest and only the e2e specs you touched. Don't end your turn to wait for a test. Run long tests in the foreground with a timeout. Use your own ports (`GIS_DEVSERVER_ADDR`, `GIS_VITE_PORT`, `VITE_GIS_DEVSERVER`) and kill only your own processes, by PID.
- **You:** `scripts/e2e.sh <out> --quick` (about 8 min, no control audit) after a merge into `ui-redesign` or the working branch when you need a wider check than the worker ran. One full `scripts/e2e.sh` (about 46 min) at the end of each batch and before any merge into `main`, app rebuild or push, plus one real-app review per merge batch. Read closely only what can damage data or correctness: CRS, geodesics, the project format and old-project migration, licences. Grep the rest.
- **Don't poll.** You're woken when a session finishes. Check a worker's report against its diff before telling the user it's done.
- The app only starts sessions from the user's own message. When you need a new session, ask for one message, and batch what you need into it.

## Work, in order

**Batch 1: open bugs, one Sonnet session, one commit per bug.** Work from `ui-redesign`, which isn't merged into `main` yet.
- **E2E-5, first:** intermittent failure of the label-drag test, which blocks the merge. Rerun with `scripts/e2e.sh <out-dir> e2e/labels.spec.ts --repeat-each=3`. Once fixed, run the whole suite once (about 47 minutes) and merge `ui-redesign` into local `main` if it passes. Don't push without asking the user.
- **U-6:** blank layer after New Project; add an e2e test.
- **MM-4:** layer lists reading the active map's `order`. Decide what each list should mean before fixing: a node input may mean any project layer; a layout frame list means the frame's map.
- **TP-11:** result rasters inherit the DEM renderer.
- **LE-1:** named text styles aren't saved in the project; needs an engine field and a project format check.
- **E2E-3, E2E-4:** flaky tests; fix only if they recur. **C-1b** stays accepted; don't work on it.

Then stop and report, and tell the user the bug batch is ready for the Mac app rebuild and test. That rebuild is handled outside this session. Don't start Batch 2 (now S1–S4, then U0–U2 from `docs/SERVER_CONNECTIONS_PLAN.md` §4a: every USGS service with a download path) until the user says that test is done.

**Batch 2: server connections, `docs/SERVER_CONNECTIONS_PLAN.md` §4.**
- **S1, S2, S3, S4:** give them to the same Sonnet session, in order, reviewing and merging each before sending the next.
- Read closely: the project format change, the CRS rule, how untrusted server text is handled, and all of S4, the sign-in work (§3a). Secrets must never reach a project, log, error, citation, URL or file on disk.
- Secrets are always entered by the user. Tests use fake credentials against a local fake server.

**Batch 3: planetary, `docs/PLANETARY_PLAN.md` §10.**
- **P0:** you do this spike yourself, and correct the plan if it was wrong.
- Use the server layer from Batch 2 for the USGS WMS basemaps; don't add a second WMS path.
- **P1 to P6:** give them to the same Sonnet session one at a time, in order, reviewing and merging each before sending the next. Replace the session only by the rule above. The plan allows some parallel work, but use it only if the user asks for speed over usage.
- **P1:** read the pivot and geodesic diff closely. Earth test values must not change.
- **P7:** you run the final check.

## Decided by the user, don't reopen

- The extraterrestrial mode belongs to each map, chosen when the map is created.
- Server connections, then planetary, are the last features before the first release. The Python SDK comes after.
- An open tool goes in directly if it and all its libraries can be packaged inside the app. Craterstats is bundled, with crater model ages in the first release.
- Defaults unless the user says otherwise (plan §12): Moon and Mars get everything; Mercury, Venus and Europa get a basemap and curated data.

## Rules

- Tests pass before any push. Push only with `[skip ci]` in the head commit. Never create or push tags. Never force-push.
- Never commit `.claude/`, build artifacts, `test-projects/` or `Madera County.gproj/`.
- New open-source software, data or methods go in `docs/OPEN_SOURCE.md` in the same commit, and `THIRD_PARTY_NOTICES.md` when the shipped set changes.
- Every new Tauri command is also added to `crates/devserver`.
- No API keys or accounts. Ask before anything outward-facing beyond this repo, including contacting data providers.
- Fixed bugs move from Open to Closed in `docs/BUGS.md`. New features add cases to `docs/TEST_PLAN.md`.
- Keep `PROJECT_STATUS.md` current, add a Change log row to the "GIS Studio — Project Status" doc after each change (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj), and run `graphify update .` before ending.

## When you report

Say what changed, what passed, what wasn't checked, and what you need from the user, in a few short lines.
