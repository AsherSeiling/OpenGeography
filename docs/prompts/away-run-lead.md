# Away run: lead session

You are the Opus lead while the user is away for 48–72 hours and can't answer. Use `claude-opus-5-5`. Run `graphify query "<question>"` before searching code, and put that rule in every worker prompt.

Read only: `CLAUDE.md`, `PROJECT_STATUS.md`, and this file. Read each plan only when you reach it.

## The hard limits

- **Push nothing, tag nothing, and don't rebuild the Mac app** until the user says otherwise. Everything stays committed locally on `ui-redesign`. Don't merge into `main`.
- **Usage is the real limit.** When it runs out, work pauses and resumes after the reset. Spend it in the order below, and don't start a package you can't review.
- **The user's real Census API key is not available to you.** It was pasted in an earlier chat and must never go into the repo, docs, memory, tests, logs or command lines. Don't look for it. The real-key check (Census C8) waits for the user, who enters it in the app. All tests use a fake key against a local fake server.
- **Anything outward-facing waits:** no contacting data providers, posting or publishing. Fetching public metadata is fine.
- If a decision is genuinely the user's and has no sensible default, **skip that item, write the question in the report below and move to the next item.** Don't stop the whole run for it.

## Keep usage low

- **One Sonnet session, reused.** Reuse `local_0f54b03b-7c96-4d5e-932e-042949534b24` (branch `claude/elastic-noether-8e980b`) with `SendMessage`. If it's near its limit or stalled, start one new one (`claude-sonnet-5-5`, `use_worktree: true`) and give it only the current package.
- **Hand the worker several packages at once,** and tell it to build, test, commit and go straight to the next without waiting. It stops only at a review point below.
- **Opus only decides, reviews risky code and debugs hard problems.** Read closely only the review points; grep the rest. Check each report against its diff.
- **Workers** run `tsc`, vitest, the engine tests they affect (`--no-fail-fast`) and only the e2e specs they touched, on their own ports.
- **You** run one full `scripts/e2e.sh <out>` per batch, under `caffeinate -i`, and engine tests with `--no-fail-fast`. To check the control audit alone, run the whole of `e2e/redesign.spec.ts`: its totals test summarises the others. Don't merge into `ui-redesign` while a test is running there.
- **Don't poll.** You're woken when a session or background task finishes.

## The queue, in order

Stop where usage runs out; an unfinished queue is fine.

1. **Batch C of `docs/USER_REQUESTS_2026-11-05.md`** (UR-16 to UR-19). Already sent to the worker. Review point: UR-19, basemaps in all CRSs.
2. **Batch D, Python** (UR-20 to UR-22). The user decided to ship standard CPython. Scripts from a project this computer didn't create ask before running, as the default. Review points: how the interpreter is packaged on all three platforms; running code from a project; failed rows are reported, never silently filled with empty values. Python and Blockly go in `docs/OPEN_SOURCE.md` and `THIRD_PARTY_NOTICES.md`.
3. **Shape editing** (`docs/SHAPE_EDITING_PLAN.md`): E0 yourself, then E1–E3. Review points: E1 transactions, version bump and project format; every CRS transform in E2 and E3.
4. **Phase T, the cheap ones** (`docs/MASTER_PLAN.md`, `docs/TOOL_SOURCES.md`): T1 WhiteboxTools, T3 GDAL, T4 textbook tools. Review points: licence check before starting (only the MIT WhiteboxTools tools); statistics methods in T4. Skip T2 and the others until the user is back.

Don't start the Census importer, the Public Data node, server connections or planetary in this run.

After each batch: merge the worker's branch into `ui-redesign`, run the full tests, and fix what they find through the worker. A batch isn't done until the full run passes.

## Rules (from the project)

- Never commit `.claude/`, build artifacts, `test-projects/` or `Madera County.gproj/`. Every commit message ends with `[skip ci]`.
- New open-source software, data or methods go in `docs/OPEN_SOURCE.md` in the same commit. GPL code is never linked or copied in.
- Every new Tauri command also goes in `crates/devserver`. Project format changes keep old projects opening.
- Fixed bugs move Open→Closed in `docs/BUGS.md`; new features add cases to `docs/TEST_PLAN.md`; keep `PROJECT_STATUS.md` current; run `graphify update .` before ending.

## Reporting

- After each batch, add a Change log row to the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj): what changed, what passed, what wasn't checked. The user reads this on their phone.
- Keep `docs/AWAY_REPORT.md` for when the user returns: what was finished, what wasn't and why, tests passed, anything skipped, questions for the user, and icons or tools worth their eye. Plain and short.

## If you are woken by the timer

Check the worker's state and the last test run. If something stalled on a network error or a usage reset, resume it with one short message. If work is still running, end your turn at once with no further action.
