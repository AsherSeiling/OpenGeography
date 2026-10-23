# CI log

What happened on each GitHub Actions round and what changed in response.
Runs: https://github.com/AsherSeiling/OpenGeography/actions — every job also
writes a summary (checks, versions) to its run page, and failing steps publish
their key output lines as annotations.

| Date | Run | macOS | Linux | Windows | Finding → change |
|---|---|---|---|---|---|
| 2026-10-23 | [39963014994](https://github.com/AsherSeiling/OpenGeography/actions/runs/39963014994) | ❌ engine tests | ❌ AppImage self-test | ❌ engine tests | First run ever. Logs need a GitHub login, so failures were unreadable → added `scripts/ci-run.sh` (failing output → annotations). |
| 2026-10-23 | [39964176992](https://github.com/AsherSeiling/OpenGeography/actions/runs/39964176992) | ❌ engine tests | (still running at time of writing) | ❌ engine tests | macOS: `gdal-sys` has no prebuilt bindings for Homebrew's GDAL 3.13 → enable the `bindgen` feature (bindings generated for any GDAL). Windows: conda names the import library `gdal.lib`, `gdal-sys` wants `gdal_i.lib` → copy it and set `GDAL_LIB_DIR`/`GDAL_INCLUDE_DIR`. Added per-job summaries (`scripts/ci-summary.sh`). |
| 2026-10-23 | [39970427976](https://github.com/AsherSeiling/OpenGeography/actions/runs/39970427976) | ❌ gdal crate compile | ❌ AppImage self-test | ❌ gdal crate compile | Linux builds and packages; its self-test found the AppImage bundler strips the DuckDB spatial extension (breaks its metadata) → `NO_STRIP=true`. `bindgen` was wrong: gdal 0.19 doesn't compile against GDAL 3.13 (API changes) and Windows bindgen types differ → reverted; pin GDAL 3.12 (prebuilt bindings) via conda-forge `libgdal-core=3.12` on macOS and Windows. macOS CI also uses prebuilt libduckdb 1.5.6 (Homebrew now ships 2.0). Newer pushes cancel older runs. |
| 2026-10-23 | [39971872593](https://github.com/AsherSeiling/OpenGeography/actions/runs/39971872593) | ✅ **all green** | ❌ AppImage self-test | ❌ 4 engine tests | macOS passes end to end. Linux: NO_STRIP wasn't enough — the AppImage bundler also patches the extension as an ELF library → ship it gzip-compressed and unpack on first use. Windows: GDAL writes failed ("Could not move file") → write GeoPackages in place (`USE_TMP_FILE false`); the project-reopen test held the file open in two engines (Windows locks it) → close the first. |
