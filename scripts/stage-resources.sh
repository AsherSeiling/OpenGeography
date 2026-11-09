#!/usr/bin/env bash
# Stage files the installed app needs into src-tauri/resources/ (bundled by
# Tauri): DuckDB's spatial extension and GDAL/PROJ data.
#   scripts/stage-resources.sh [duckdb-platform]   e.g. osx_arm64, linux_amd64, windows_amd64
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/src-tauri/resources"
DUCKDB_VERSION="${DUCKDB_VERSION:-v1.5.6}"
detect() {
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) echo osx_arm64 ;; Darwin-x86_64) echo osx_amd64 ;;
    Linux-x86_64) echo linux_amd64 ;; Linux-aarch64) echo linux_arm64 ;;
    MINGW*|MSYS*|CYGWIN*) echo windows_amd64 ;; *) echo unknown ;;
  esac
}
PLATFORM="${1:-$(detect)}"
mkdir -p "$OUT/duckdb" "$OUT/gdal" "$OUT/proj" "$OUT/gdalplugins"

# 1. DuckDB spatial extension (signed by DuckDB), shipped gzip-compressed so
#    Linux packagers don't rewrite it as a library; the app unpacks it on
#    first use (engine/src/resources.rs), so no download at run time.
for NAME in spatial httpfs; do
  EXT="$OUT/duckdb/$NAME.duckdb_extension.gz"
  rm -f "$OUT/duckdb/$NAME.duckdb_extension"
  CACHED="$HOME/.duckdb/extensions/$DUCKDB_VERSION/$PLATFORM/$NAME.duckdb_extension"
  if [ -f "$CACHED" ]; then
    gzip -c "$CACHED" > "$EXT"
  else
    curl -fsSL -o "$EXT" "https://extensions.duckdb.org/$DUCKDB_VERSION/$PLATFORM/$NAME.duckdb_extension.gz"
  fi
done

# 2. GDAL data (projections helpers, driver support files).
GDAL_DATA_SRC="${GDAL_DATA_SRC:-$(gdal-config --datadir 2>/dev/null || echo /opt/homebrew/share/gdal)}"
cp -RL "$GDAL_DATA_SRC/." "$OUT/gdal/"

# 3. PROJ database (datum grids are fetched on demand from cdn.proj.org).
PROJ_SRC="${PROJ_DATA_SRC:-$(dirname "$(projinfo --searchpaths 2>/dev/null | tail -1)/proj.db")}"
[ -f "$PROJ_SRC/proj.db" ] || PROJ_SRC=/opt/homebrew/share/proj
cp -L "$PROJ_SRC/proj.db" "$OUT/proj/"
[ -f "$PROJ_SRC/proj.ini" ] && cp -L "$PROJ_SRC/proj.ini" "$OUT/proj/"
# Datum grids (NAD27 <-> NAD83 for the US): DuckDB's own PROJ cannot fetch them from the network, so they ship with the app.
cp -L "$ROOT"/crates/engine/assets/proj-grids/*.tif "$OUT/proj/"
# More grids for Canada, Australia, New Zealand, the UK and Europe: downloaded once from the PROJ-data server, checksum-pinned (see the manifest).
"$ROOT/scripts/fetch-bundled-grids.sh" "$OUT/proj"
mkdir -p "$OUT/natural-earth" && cp -L "$ROOT"/crates/engine/assets/natural-earth/*.tif "$OUT/natural-earth/"
touch "$OUT/gdalplugins/.keep"
cp "$ROOT/THIRD_PARTY_NOTICES.md" "$OUT/THIRD_PARTY_NOTICES.md"
echo "Staged resources for $PLATFORM in $OUT ($(du -sh "$OUT" | cut -f1))"

# WhiteboxTools program for the hydrology tools (MIT; built from a pinned release, see the script). WHITEBOX_SKIP=1 leaves it out.
if [ -z "${WHITEBOX_SKIP:-}" ]; then "$ROOT/scripts/build-whitebox.sh"; fi

# CPython for user scripts (Batch D): python-build-standalone, an exact release pinned per platform and checked against a SHA-256 before it is
# unpacked. Nothing is committed; the archive is cached outside the repo. PYTHON_SKIP=1 leaves it out (the Python script nodes then say so).
# NumPy (raster calculator) is added later, pinned with hashes, and is not part of this step. Docs: docs/PYTHON_SPIKE.md, docs/OPEN_SOURCE.md.
stage_python() {
  local PBS_RELEASE=20261026 PY_VERSION=3.13.18 triple sha
  case "$PLATFORM" in
    osx_arm64)     triple=aarch64-apple-darwin;     sha=8392b6ea51d94391befb2c31ebe75463f045d0b69dfcf5e1bb3234e1f5fa3405 ;;
    osx_amd64)     triple=x86_64-apple-darwin;      sha=7412c19d896c99431b74a18a5643e110449a7f9590fbab179c3e5ab27b9aa9e3 ;;
    windows_amd64) triple=x86_64-pc-windows-msvc;   sha=7369f7a35f261222a6ac8b3a87c1c88eeebee8af945d85346202591a64038dd3 ;;
    linux_amd64)   triple=x86_64-unknown-linux-gnu; sha=df15fe60d3ff55bf9dcf25824b5572756bb8816ff93865a273d3301d7066a8b7 ;;
    *) echo "No pinned Python for platform '$PLATFORM'; scripts will not be available in this build."; return 0 ;;
  esac
  local name="cpython-${PY_VERSION}+${PBS_RELEASE}-${triple}-install_only_stripped.tar.gz"
  local cache="${GIS_STUDIO_BUILD_CACHE:-$HOME/.cache/gis-studio}/python"
  mkdir -p "$cache"
  local archive="$cache/$name"
  local sum; sum() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
  if [ ! -f "$archive" ] || [ "$(sum "$archive")" != "$sha" ]; then
    curl -fsSL -o "$archive.part" "https://github.com/astral-sh/python-build-standalone/releases/download/${PBS_RELEASE}/${name//+/%2B}"
    mv "$archive.part" "$archive"
  fi
  if [ "$(sum "$archive")" != "$sha" ]; then
    echo "Python archive $name does not match its pinned SHA-256 (expected $sha); refusing to use it." >&2
    rm -f "$archive"
    return 1
  fi
  rm -rf "$OUT/python"
  mkdir -p "$OUT/python"
  tar -xzf "$archive" -C "$OUT/python" --strip-components=1
  # Parts nothing here uses (saves about 3 MB; the interpreter itself is most of the size).
  rm -rf "$OUT/python/lib/python3.13"/{test,idlelib,tkinter,turtledemo,ensurepip,lib2to3} "$OUT/python/lib/python3.13/lib-dynload"/_tkinter*
  echo "Staged Python $PY_VERSION ($triple, release $PBS_RELEASE) in $OUT/python ($(du -sh "$OUT/python" | cut -f1))"
}
if [ -z "${PYTHON_SKIP:-}" ]; then stage_python; fi
