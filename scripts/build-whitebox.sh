#!/usr/bin/env bash
# Build the WhiteboxTools program (MIT, John Lindsay) from a pinned release and stage it for the hydrology tools:
#   src-tauri/resources/whitebox/whitebox_tools[.exe] + LICENSE.txt
# Source: https://github.com/jblindsay/whitebox-tools at tag v2.4.0 (commit below), built with the Rust toolchain on this machine.
#   scripts/build-whitebox.sh            clone the tag into target/whitebox-src, build, stage
#   WHITEBOX_SRC=<checkout> scripts/build-whitebox.sh    use an existing checkout (offline); its commit must match
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TAG="v2.4.0"
COMMIT="28ed01f506f2024cfb52db85348b887f2ded4e5d"
OUT="$ROOT/src-tauri/resources/whitebox"
EXE="whitebox_tools"; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE="whitebox_tools.exe" ;; esac
if [ -x "$OUT/$EXE" ] && [ -z "${WHITEBOX_REBUILD:-}" ]; then echo "WhiteboxTools already staged ($OUT/$EXE)"; exit 0; fi
SRC="${WHITEBOX_SRC:-$ROOT/target/whitebox-src}"
if [ ! -d "$SRC/.git" ]; then
  rm -rf "$SRC"
  git clone -q --depth 1 --branch "$TAG" https://github.com/jblindsay/whitebox-tools.git "$SRC"
fi
HAVE="$(git -C "$SRC" rev-parse HEAD)"
[ "$HAVE" = "$COMMIT" ] || { echo "WhiteboxTools checkout is $HAVE, expected $COMMIT ($TAG)"; exit 1; }
cd "$SRC"
# The release's Cargo.lock holds a stale checksum for tsp-rs (the crate was republished); drop it so cargo refills it.
if grep -q '^checksum = "02f971d0e820c6e73d2c71cf139e2cbe6962bdacb456efc4a78c2a41b4244426"$' Cargo.lock; then
  grep -v '^checksum = "02f971d0e820c6e73d2c71cf139e2cbe6962bdacb456efc4a78c2a41b4244426"$' Cargo.lock > Cargo.lock.new && mv Cargo.lock.new Cargo.lock
fi
cargo build --release -p whitebox_tools
mkdir -p "$OUT"
cp "target/release/$EXE" "$OUT/$EXE"
cp LICENSE.txt "$OUT/LICENSE.txt"
echo "WhiteboxTools $TAG staged in $OUT"
