#!/usr/bin/env bash
# Build the WhiteboxTools program (MIT, John Lindsay) from a pinned release and stage it for the hydrology tools:
#   src-tauri/resources/whitebox/whitebox_tools[.exe] + WhiteboxTools-MIT-LICENSE.txt
# Source: https://github.com/jblindsay/whitebox-tools at tag v2.4.0 (commit below), built with the Rust toolchain on this machine.
#   scripts/build-whitebox.sh            clone the tag into target/whitebox-src, build, stage
#   WHITEBOX_SRC=<checkout> scripts/build-whitebox.sh    use an existing checkout (offline); its commit must match
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAG="v2.4.0"
COMMIT="28ed01f506f2024cfb52db85348b887f2ded4e5d"
OUT="$ROOT/src-tauri/resources/whitebox"
EXE="whitebox_tools"; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE="whitebox_tools.exe" ;; esac
# In CI the tests find the program through GIS_STUDIO_WHITEBOX (a native path on Windows).
export_env() { if [ -n "${GITHUB_ENV:-}" ]; then P="$OUT/$EXE"; command -v cygpath >/dev/null && P="$(cygpath -w "$P")"; echo "GIS_STUDIO_WHITEBOX=$P" >> "$GITHUB_ENV"; fi; }
if [ -x "$OUT/$EXE" ] && [ -z "${WHITEBOX_REBUILD:-}" ]; then echo "WhiteboxTools already staged ($OUT/$EXE)"; export_env; exit 0; fi
SRC="${WHITEBOX_SRC:-$ROOT/target/whitebox-src}"
# A folder restored from a build cache can hold a stripped .git: git then reads the parent repository instead. Trust only a checkout
# whose own HEAD is the pinned commit; otherwise (and unless the caller gave one) clone it again.
HAVE="$(git -C "$SRC" rev-parse HEAD 2>/dev/null || true)"
if [ "$HAVE" != "$COMMIT" ] && [ -z "${WHITEBOX_SRC:-}" ]; then
  rm -rf "$SRC"
  git clone -q --depth 1 --branch "$TAG" https://github.com/jblindsay/whitebox-tools.git "$SRC"
  HAVE="$(git -C "$SRC" rev-parse HEAD)"
fi
[ "$HAVE" = "$COMMIT" ] || { echo "WhiteboxTools checkout is $HAVE, expected $COMMIT ($TAG)"; exit 1; }
cd "$SRC"
# The release's Cargo.lock holds a stale checksum for tsp-rs (the crate was republished); drop it so cargo refills it.
if grep -q '^checksum = "02f971d0e820c6e73d2c71cf139e2cbe6962bdacb456efc4a78c2a41b4244426"$' Cargo.lock; then
  grep -v '^checksum = "02f971d0e820c6e73d2c71cf139e2cbe6962bdacb456efc4a78c2a41b4244426"$' Cargo.lock > Cargo.lock.new && mv Cargo.lock.new Cargo.lock
fi
cargo build --release -p whitebox_tools
mkdir -p "$OUT"
cp "target/release/$EXE" "$OUT/$EXE"
rm -f "$OUT/LICENSE.txt"; cp LICENSE.txt "$OUT/WhiteboxTools-MIT-LICENSE.txt"  # unique name: the Windows installer flattens resources and GDAL ships a LICENSE.TXT too
export_env
echo "WhiteboxTools $TAG staged in $OUT"
