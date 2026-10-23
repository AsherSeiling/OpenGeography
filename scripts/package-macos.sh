#!/usr/bin/env bash
# Build a self-contained GIS Studio.app and .dmg for macOS.
# Bundles libduckdb, libgdal and every library they depend on into
# Contents/Frameworks, plus GDAL/PROJ data and DuckDB's spatial extension
# (scripts/stage-resources.sh), so the app needs nothing else installed.
#
# Needs (build machine only): Homebrew gdal + duckdb, dylibbundler.
#   brew install gdal duckdb dylibbundler
#   scripts/package-macos.sh
#
# The result is ad-hoc signed. For distribution without Gatekeeper warnings,
# sign with a Developer ID and notarize: set APPLE_SIGNING_IDENTITY (and use
# `xcrun notarytool`); see https://v2.tauri.app/distribute/sign/macos/
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH"

scripts/stage-resources.sh
npx tauri build --bundles app

APP="$ROOT/target/release/bundle/macos/GIS Studio.app"
BIN="$APP/Contents/MacOS/gis-studio"
FW="$APP/Contents/Frameworks"
rm -rf "$FW" && mkdir -p "$FW"

# Copy all non-system libraries the binary needs (recursively) and repoint
# them at @executable_path/../Frameworks.
dylibbundler -of -b -x "$BIN" -d "$FW/" -p "@executable_path/../Frameworks/" \
  -s /opt/homebrew/lib -s /opt/homebrew/opt > /tmp/gis-studio-dylibbundler.log

# dylibbundler can add the same LC_RPATH more than once; macOS refuses to
# load a library with duplicate rpaths, so keep one of each.
dedupe_rpaths() {
  local f="$1"
  otool -l "$f" | awk '/cmd LC_RPATH/{getline; getline; print $2}' | sort | uniq -d | while read -r rp; do
    while [ "$(otool -l "$f" | awk '/cmd LC_RPATH/{getline; getline; print $2}' | grep -cxF "$rp")" -gt 1 ]; do
      install_name_tool -delete_rpath "$rp" "$f" 2>/dev/null
    done
  done
}
dedupe_rpaths "$BIN"
for f in "$FW"/*.dylib; do dedupe_rpaths "$f"; done

# Fail if anything still points outside the app (Homebrew/MacPorts).
LEAKS=$( (otool -L "$BIN"; for f in "$FW"/*.dylib; do otool -L "$f"; done) | grep -E "^\s+/(opt|usr/local)/" || true)
if [ -n "$LEAKS" ]; then
  echo "Still linked outside the app:"; echo "$LEAKS"; exit 1
fi

# Sign: libraries first, then the app (ad-hoc unless an identity is given).
IDENTITY="${APPLE_SIGNING_IDENTITY:--}"
for f in "$FW"/*.dylib; do codesign --force --timestamp=none -s "$IDENTITY" "$f"; done
codesign --force --deep --timestamp=none -s "$IDENTITY" "$APP"
codesign --verify --deep --strict "$APP"

VERSION=$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")
ARCH=$(uname -m)
DMG="$ROOT/target/release/bundle/GIS-Studio-$VERSION-macos-$ARCH.dmg"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "GIS Studio" -srcfolder "$STAGE" -ov -format UDZO "$DMG" > /dev/null
rm -rf "$STAGE"

echo "App: $APP ($(du -sh "$APP" | cut -f1), $(ls "$FW" | wc -l | tr -d ' ') bundled libraries)"
echo "DMG: $DMG ($(du -sh "$DMG" | cut -f1))"
