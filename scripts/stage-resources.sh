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

# 1. DuckDB spatial extension (signed by DuckDB; loaded from this file, so no download at run time).
EXT="$OUT/duckdb/spatial.duckdb_extension"
CACHED="$HOME/.duckdb/extensions/$DUCKDB_VERSION/$PLATFORM/spatial.duckdb_extension"
if [ -f "$CACHED" ]; then
  cp "$CACHED" "$EXT"
else
  curl -fsSL "https://extensions.duckdb.org/$DUCKDB_VERSION/$PLATFORM/spatial.duckdb_extension.gz" | gunzip > "$EXT"
fi

# 2. GDAL data (projections helpers, driver support files).
GDAL_DATA_SRC="${GDAL_DATA_SRC:-$(gdal-config --datadir 2>/dev/null || echo /opt/homebrew/share/gdal)}"
cp -RL "$GDAL_DATA_SRC/." "$OUT/gdal/"

# 3. PROJ database (datum grids are fetched on demand from cdn.proj.org).
PROJ_SRC="${PROJ_DATA_SRC:-$(dirname "$(projinfo --searchpaths 2>/dev/null | tail -1)/proj.db")}"
[ -f "$PROJ_SRC/proj.db" ] || PROJ_SRC=/opt/homebrew/share/proj
cp -L "$PROJ_SRC/proj.db" "$OUT/proj/"
[ -f "$PROJ_SRC/proj.ini" ] && cp -L "$PROJ_SRC/proj.ini" "$OUT/proj/"
touch "$OUT/gdalplugins/.keep"
echo "Staged resources for $PLATFORM in $OUT ($(du -sh "$OUT" | cut -f1))"
