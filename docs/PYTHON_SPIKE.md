# Python scripting spike (Batch D, UR-20 to UR-22) — 2026-11-08

Spike only: no feature code. Decided by the user: standard CPython (python-build-standalone) in its own process; SQL and today's forms stay as options; saved old queries keep working; ask before running scripts from a project this computer didn't create.

## 1. Pinned interpreter

python-build-standalone release **20261026**, CPython **3.13.18**, `install_only_stripped` archives. SHA-256 taken from the release's `SHA256SUMS` (itself `sha256:39af6e3d…1271a73` per the GitHub API), and for macOS arm64 re-checked against my own download.

| Platform | Archive | Size | SHA-256 |
|---|---|---|---|
| macOS arm64 | `cpython-3.13.18+20261026-aarch64-apple-darwin-install_only_stripped.tar.gz` | 25.3 MB | `8392b6ea51d94391befb2c31ebe75463f045d0b69dfcf5e1bb3234e1f5fa3405` (downloaded and verified) |
| macOS x64 | `…-x86_64-apple-darwin-…` | 25.1 MB | `7412c19d896c99431b74a18a5643e110449a7f9590fbab179c3e5ab27b9aa9e3` (from SHA256SUMS) |
| Windows x64 | `…-x86_64-pc-windows-msvc-…` | 22.0 MB | `7369f7a35f261222a6ac8b3a87c1c88eeebee8af945d85346202591a64038dd3` (from SHA256SUMS) |
| Linux x64 | `…-x86_64-unknown-linux-gnu-…` | 35.4 MB | `df15fe60d3ff55bf9dcf25824b5572756bb8816ff93865a273d3301d7066a8b7` (from SHA256SUMS) |

Base URL `https://github.com/astral-sh/python-build-standalone/releases/download/20261026/` (the `+` is `%2B`). Fetch in `scripts/stage-resources.sh` into `src-tauri/resources/python/`, check the hash, never commit binaries; the engine finds it only through a `resources.rs` function (like `whitebox_exe`). Unpacked: 65 MB, 1,657 files, 9 extension modules.

## 2. Row exchange: numbers

Test: 1M rows `{POP, AREA, NAME}`, `def calculate(row): return row["POP"]/row["AREA"] if row["AREA"] else None`, run with Homebrew CPython 3.14.3 on an Apple-silicon Mac (stdlib only; same order of speed as 3.13). Python side only (parse input, call the function per row, serialise output); the Rust side is not included.

| Protocol | Python-side time for 1M rows | Input / output size |
|---|---|---|
| JSON lines (one object per row) | 2.28 s (≈440k rows/s) | 44 MB / 18 MB |
| Column-wise binary (`array('d')` per numeric column, NUL-joined strings, built into a row dict per call) | **0.20 s (≈5M rows/s)** | 24 MB / 8 MB |
| the `calculate` calls alone | 0.08 s | — |

Size cost of the libraries (PyPI wheels for CPython 3.13, compressed; the DMG grows by about the same): NumPy 2.5.4 = 5.4 to 12 MB per platform; pyarrow 26.0.1 = 37 MB macOS arm64, 29 MB Windows, 55 MB Linux.

**Not measured:** NumPy and pyarrow speed, and installed size. The sandbox refused `pip install` into the downloaded interpreter, and refused running the downloaded interpreter, so every Python number above is from the Homebrew interpreter and stdlib only.

**Recommendation:** ship **NumPy only** (the raster calculator needs it, ~5–12 MB) and **no pyarrow** (37–55 MB, to save a step the column-binary protocol already does at 5M rows/s). Rows travel as a small length-prefixed column protocol: a header (JSON: field names, types, row count), then each column as raw little-endian doubles / int64 / NUL-joined UTF-8 plus a validity bitmap for nulls. A null in an input column is `None` in the row dict. Failed rows are never nulled: the worker returns `(row index, message)` pairs and the engine shows the count and the first N. Raster blocks go the same way as raw float64 buffers viewed with `numpy.frombuffer`. JSON lines is simpler but 11× slower; keep it only as a debug mode.

## 3. macOS signing and the DMG

Checked on a stand-in `.app` (an `echo` binary plus `Contents/Resources/python`), not on the real app, and **without ever running the interpreter**.

- `codesign --force --deep -s -` of the whole bundle and `codesign --verify --deep --strict` both pass, **but** `--deep` does not sign Mach-O files under `Resources/`: `codesign --verify --strict` on `python3.13` says "code object is not signed at all". The shipped PBS binaries carry an ad-hoc linker signature, so they would probably still launch locally, but a Developer ID / notarized build needs every `python/bin/python3.13`, `*.so`, `*.dylib` signed explicitly (same identity, hardened runtime) before the app is sealed. `package-macos.sh` has to gain that loop (it only signs `Frameworks/*.dylib` today).
- Verified: signing the nested files first, then the app, verifies clean. `spctl` rejects the ad-hoc result, exactly as it does for today's app.
- DMG: the current DMG is 172.1 MB. A bare interpreter adds **36.9 MB** (33.5 MB after deleting `test`, `idlelib`, `tkinter`, `turtledemo`, `ensurepip`, `lib2to3`; trimming barely helps). With NumPy about 42–49 MB; with NumPy and pyarrow about 80–90 MB.
- Not verified: that the interpreter runs from inside a signed app with hardened runtime (needs checking for entitlements, e.g. `allow-unsigned-executable-memory` for ctypes/libffi); that `pip`-style extra packages work (not offered); notarization.
- **Windows and Linux: nothing verified** beyond the archive names, sizes and hashes. Not run, not signed (Windows Authenticode), no antivirus check, no AppImage/deb handling of a bundled interpreter.

## 4. Where Python becomes the default

| # | Place | Files | Language today | When it runs / how it stays fast |
|---|---|---|---|---|
| 1 | Field calculator | `nodes/calculate_field.rs` (`expression`, `ParamKind::Expression`); dialog in `ui/src/dialogs/AttributeDialogs.tsx` (~l.182); (`nodes/calculate_geometry.rs` is separate, not listed) | SQL (DuckDB) | Once per run, result is materialised: no draw-time cost. One Python process per run. |
| 2 | Definition query | `layers.rs` (`definition_query`, l.75), used by `table.rs` l.33, `layout/frame.rs` l.465, `layout/tabular.rs` l.54, tiles; editor `PropertiesPane.tsx` (`DefinitionQuery`) with `query/QueryEditor.tsx` | SQL in storage. The editor already has Builder / **Python** / SQL, but its "Python" is a subset translated to SQL by `ui/src/query/pyToSql.ts` | Runs on every tile and table page, so Python must **not** run per tile. Evaluate once per (script hash, data version) into a hidden boolean column (or fid set) in DuckDB; tiles filter on that. If the script is inside the `pyToSql` subset keep translating it to SQL (no process at all). Re-evaluate when data or script change. |
| 3 | Select by attributes | `ui/src/dialogs/Dialogs.tsx` (`SelectByAttributesDialog`, l.411) with `QueryEditor` | SQL / Builder / `pyToSql` subset | One-shot, runs once; same cache as 2. |
| 4 | Filter node | `nodes/filter.rs` (`where`, `ParamKind::Query`), `dialogs/NodeQueryDialog.tsx` | SQL / Builder / subset | Once per model run, like 1. |
| 5 | Label expressions | **do not exist yet**: labels are a field (`ui/src/map/style.ts` `labelField`, `MapView.tsx` l.479, `layers/nodeStyle.ts` `label_field`) | field name only | New. Would be materialised as a hidden label column per (script hash, data version), computed once, never per tile or per frame. |
| 6 | Raster calculator | `nodes/raster_ops_tools.rs` (`expr` l.28 and l.47; reclass `rules` l.69), `raster_ops.rs` l.130 (`fasteval`, block by block) | map-algebra expression (fasteval) | Once per run, writes a file; display reads the file. Python form takes NumPy blocks; per-block pipe cost is small next to GDAL I/O. |

Internal SQL in `lib.rs` l.1013 (contour labels) is generated, not user-facing, and stays SQL.

## 5. Design rules the spike keeps open

Errors carry row number and message, failed rows are counted and shown, nothing is silently nulled; kill the process to cancel and report through `progress::Task`; trust record = hash of the script, stored in app data (`resources.rs`), never in the project; scrubbed environment (`env_clear()`, then only what Python needs: no Census key, Earthdata token, keychain); every new Tauri command mirrored in `crates/devserver`; Python, NumPy and Blockly in `docs/OPEN_SOURCE.md` and `THIRD_PARTY_NOTICES.md`; cases in `docs/TEST_PLAN.md`.
