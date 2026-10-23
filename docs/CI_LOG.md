# CI log

What happened on each GitHub Actions round and what changed in response.
Runs: https://github.com/AsherSeiling/OpenGeography/actions — every job also
writes a summary (checks, versions) to its run page, and failing steps publish
their key output lines as annotations.

| Date | Run | macOS | Linux | Windows | Finding → change |
|---|---|---|---|---|---|
| 2026-10-23 | [39963014994](https://github.com/AsherSeiling/OpenGeography/actions/runs/39963014994) | ❌ engine tests | ❌ AppImage self-test | ❌ engine tests | First run ever. Logs need a GitHub login, so failures were unreadable → added `scripts/ci-run.sh` (failing output → annotations). |
| 2026-10-23 | [39964176992](https://github.com/AsherSeiling/OpenGeography/actions/runs/39964176992) | ❌ engine tests | (still running at time of writing) | ❌ engine tests | macOS: `gdal-sys` has no prebuilt bindings for Homebrew's GDAL 3.13 → enable the `bindgen` feature (bindings generated for any GDAL). Windows: conda names the import library `gdal.lib`, `gdal-sys` wants `gdal_i.lib` → copy it and set `GDAL_LIB_DIR`/`GDAL_INCLUDE_DIR`. Added per-job summaries (`scripts/ci-summary.sh`). |
