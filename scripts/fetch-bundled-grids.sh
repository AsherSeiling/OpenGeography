#!/usr/bin/env bash
# Put the datum grids listed in crates/engine/assets/bundled_grids.txt next to proj.db in the staged resources, downloading each
# from https://cdn.proj.org (the PROJ-data server) once into target/grid-cache and checking its SHA-256 against the manifest.
#   scripts/fetch-bundled-grids.sh <destination folder>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$1"
CACHE="$ROOT/target/grid-cache"
MANIFEST="$ROOT/crates/engine/assets/bundled_grids.txt"
mkdir -p "$DEST" "$CACHE"
sum() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
n=0
while read -r name sha; do
  case "$name" in ""|"#"*) continue ;; esac
  f="$CACHE/$name"
  if [ ! -f "$f" ] || [ "$(sum "$f")" != "$sha" ]; then
    curl -fsSL --retry 3 -o "$f.part" "https://cdn.proj.org/$name"
    mv "$f.part" "$f"
    [ "$(sum "$f")" = "$sha" ] || { echo "Checksum mismatch for $name (expected $sha)"; rm -f "$f"; exit 1; }
  fi
  cp "$f" "$DEST/$name"
  n=$((n + 1))
done < "$MANIFEST"
echo "Bundled $n datum grids ($(du -sh "$DEST" | cut -f1) in $DEST)"
