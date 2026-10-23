#!/usr/bin/env bash
# Fail (or with --warn, just report) if GPL-only libraries are bundled.
# GIS Studio is MIT OR Apache-2.0; these come in through optional GDAL
# features (PDF via poppler, HEIF via x265, SpatiaLite topology via librttopo).
#   scripts/license-gate.sh <folder with bundled libraries> [--warn]
set -uo pipefail
dir="$1"; mode="${2:-}"
found=$(find "$dir" -type f \( -iname '*poppler*' -o -iname '*x265*' -o -iname '*rttopo*' \) 2>/dev/null | sed "s|^$dir/||" | sort)
if [ -z "$found" ]; then
  echo "License gate: no GPL-only libraries bundled in $dir"
  exit 0
fi
msg="GPL-only libraries bundled: $(echo "$found" | tr '\n' ' ')"
if [ "$mode" = "--warn" ]; then
  echo "::warning title=License gate::$msg"
  echo "WARNING: $msg"
  exit 0
fi
echo "::error title=License gate::$msg"
echo "ERROR: $msg"
exit 1
