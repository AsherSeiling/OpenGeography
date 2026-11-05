# User requests: lead session

You are the Opus lead for the list the user gave after testing the Mac app. Use `claude-opus-5-5`. Run `graphify query "<question>"` before searching code, and put that rule in every worker prompt.

Read only: `CLAUDE.md`, `PROJECT_STATUS.md`, and `docs/USER_REQUESTS_2026-11-05.md`. Read other docs only for the area you touch.

**First:** the order is decided (this list first). Before Batch D, ask the user the two remaining decisions at the top of `docs/USER_REQUESTS_2026-11-05.md` in one message. Then ask about nothing else that has a sensible default.

## Keep usage low: this comes first

- **One Sonnet session, reused.** Start it with `model: claude-sonnet-5-5` and `use_worktree: true`, then give each next task to the same session with `SendMessage`. Start another only when the current one is near its limit, stalled, or full of unrelated context, or two tasks must run at once on different files.
- **Opus only for** deciding, reviewing risky code, and hard debugging. Sonnet writes code. Do small work inline.
- **Tell workers:** run `tsc`, vitest and only the e2e specs they touched. Don't end a turn to wait for a test. Use their own ports.
- **You:** `scripts/e2e.sh <out> --quick` (about 8 min) after a merge when you need a wider check. One full `scripts/e2e.sh <out>` (about 46 min) at the end of each batch and before any merge into `main`, app rebuild or push.
- **Don't poll.** You're woken when a session finishes. Check each report against its diff before telling the user it's done.
- Read closely only what can damage data or correctness: CRS (UR-11, UR-19), clip results (UR-8), project format (UR-12, UR-13, UR-16, UR-18), running Python from a project (UR-20, UR-21), licences. Grep the rest.

## Work

Follow the batches in `docs/USER_REQUESTS_2026-11-05.md`: A, B, C, D, one commit per request, working from `ui-redesign`. Group each batch in the same Sonnet session. At the end of each batch, run the full e2e, update the docs and stop to tell the user it's ready for a Mac rebuild and test.

Requests that say to confirm or decide with the user (UR-7 wheel behaviour, UR-15 format list) are asked when you reach them, batched together.

After this list, return to `docs/prompts/bugs-and-planetary-lead.md` Batch 2 (server connections) and Batch 3 (planetary), unless the user orders it differently.

## Rules

- Tests pass before any push. Push only with `[skip ci]` in the head commit. Never create or push tags. Never force-push.
- Never commit `.claude/`, build artifacts, `test-projects/` or `Madera County.gproj/`.
- New open-source software, data or methods go in `docs/OPEN_SOURCE.md` in the same commit, and `THIRD_PARTY_NOTICES.md` when the shipped set changes. Reimplement GPL-only algorithms rather than copying them. Every new Tauri command also goes in `crates/devserver`.
- Ask before anything outward-facing beyond this repo.
- Fixed bugs move Open→Closed in `docs/BUGS.md`; new features add cases to `docs/TEST_PLAN.md`; keep `PROJECT_STATUS.md` current; add a Change log row to the "GIS Studio — Project Status" doc (https://claude.ai/artifact/5Pvx6sQMhUVcsJeijMbrAj) after each change; run `graphify update .` before ending.
- Report in a few short lines: what changed, what passed, what wasn't checked, what's needed from the user.
