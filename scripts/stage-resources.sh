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
touch "$OUT/gdalplugins/.keep"
cp "$ROOT/THIRD_PARTY_NOTICES.md" "$OUT/THIRD_PARTY_NOTICES.md"
echo "Staged resources for $PLATFORM in $OUT ($(du -sh "$OUT" | cut -f1))"

# WhiteboxTools program for the hydrology tools (MIT; built from a pinned release, see the script). WHITEBOX_SKIP=1 leaves it out.
if [ -z "${WHITEBOX_SKIP:-}" ]; then "$ROOT/scripts/build-whitebox.sh"; fi
