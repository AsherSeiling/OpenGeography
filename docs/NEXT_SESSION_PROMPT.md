# Prompt for the next session (Sonnet)

Open a new Claude Code session in this folder, pick **Sonnet** as the model, and paste everything inside the block below.

> Status 2026-10-23: a Sonnet session already started this task (commit `036e780` switched the Linux CI job). If that session is still open, continue there instead; a new session should begin by checking the latest CI run and docs/CI_LOG.md, then finish whatever acceptance criteria are left.

```text
You are continuing GIS Studio, an open-source node-based desktop GIS (Tauri 2 + Rust engine + DuckDB Spatial + GDAL/PROJ + React/Fluent UI) in this folder.

First read CLAUDE.md (rules and commands) and PROJECT_STATUS.md (state and next tasks). Read only the files the task needs; grep for symbols first.

TASK: make the Linux build GPL-free, like macOS and Windows already are.
Right now the Linux CI job (.github/workflows/build.yml, job "linux") installs Ubuntu's libgdal-dev, so the AppImage bundles GPL-only libraries (poppler, librttopo) and the license gate only warns.

Do this:
1. Switch the Linux job to conda-forge GDAL with the same pins as the macOS and Windows jobs: `libgdal-core=3.12`, `proj`, and `"libspatialite=*=lgpl*"`. Copy how those jobs set GDAL_HOME / PKG_CONFIG_PATH / library paths, and use setup-micromamba as Windows does. Keep apt only for Tauri's system packages (webkit2gtk, appindicator, rsvg, patchelf, libclang, pkg-config). Drop libgdal-dev, gdal-bin and proj-bin.
2. Watch for conda libraries (libstdc++, libcurl, libssl, libxml2, glib, sqlite) shadowing the system ones that WebKitGTK links against. Don't put the conda lib dir on LD_LIBRARY_PATH globally. Prefer an rpath or point only the GDAL build at it. Make sure the AppImage bundles the conda GDAL, PROJ and GEOS, and still uses the system WebKitGTK.
3. Make sure scripts/stage-resources.sh copies GDAL_DATA and PROJ_DATA from the conda prefix on Linux.
4. When the job is green, make the Linux license gate strict: remove `--warn` and rename the step.

Acceptance criteria:
- All three CI jobs pass: UI tests, engine tests, package, and the clean-environment `--self-test` of the AppImage.
- The Linux license gate passes in strict mode: no poppler, x265 or rttopo in the extracted AppImage.
- Nothing changes in the macOS or Windows jobs except what is shared on purpose.

How to verify:
- `gh` is NOT installed, and CI logs need a login. Read results through the public GitHub API:
  - runs: https://api.github.com/repos/AsherSeiling/OpenGeography/actions/runs?per_page=3
  - jobs: .../actions/runs/{run_id}/jobs
  - failure lines: .../check-runs/{job_id}/annotations
  scripts/ci-run.sh publishes the key output lines of a failing step as annotations.
- Wait for runs in the background with a long interval. Don't poll every few seconds.

Constraints (from CLAUDE.md, which wins on any conflict):
- Keep GDAL at 3.12 (gdal crate 0.19 can't build against 3.13) and DuckDB at 1.5.6.
- Never add a QGIS dependency, and never copy GPL code. Project license is MIT OR Apache-2.0.
- Commit per feature. End commit messages with the Co-Authored-By line. Never commit .claude/ or build artifacts. Push to origin main.
- Log every CI round in docs/CI_LOG.md: run id, what failed, what you changed.
- When done, update PROJECT_STATUS.md (known issue 2, next tasks, exact next action), the README licensing note, and the platform/licensing rows in docs/ROADMAP.md.
- If you hit a design decision you can't settle from the docs (for example, conda vs. system WebKitGTK can't coexist), stop and explain the options instead of guessing. That decision goes to an Opus session.

After this task, the next tasks in PROJECT_STATUS.md are:
- Layouts (M2): design it in an Opus session first.
- Attribute history between nodes.
- ArcGIS REST catalog.
- Census join.
Don't start them in this session unless asked.
```
