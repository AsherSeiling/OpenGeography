#!/usr/bin/env bash
# Definitions (WKT2) for catalog CRSs that the engine's built-in PROJ
# database doesn't know (newer EPSG codes, many ESRI codes). The engine uses
# these as a fallback (see crs::resolve). Regenerate after updating the catalog:
#   cargo test -p engine every_catalog -- --ignored   # writes the list of unknown codes
#   scripts/build-extra-crs.sh
set -euo pipefail
LIST="${1:-$(python3 -c 'import tempfile;print(tempfile.gettempdir())')/unusable_crs.txt}"
OUT="$(dirname "$0")/../crates/engine/assets/extra_crs.json"
python3 - "$LIST" "$OUT" <<'PY'
import json, subprocess, sys
codes = [l.split(" ", 1)[0] for l in open(sys.argv[1]) if l.strip()]
out = {}
for c in codes:
    r = subprocess.run(["projinfo", "-o", "WKT2_2019", "--single-line", "-q", c], capture_output=True, text=True)
    wkt = r.stdout.strip()
    if r.returncode == 0 and wkt.startswith(("PROJCRS", "GEOGCRS", "BOUNDCRS", "COMPOUNDCRS", "GEODCRS")):
        out[c] = wkt
json.dump(out, open(sys.argv[2], "w"), separators=(",", ":"), sort_keys=True)
print(f"Wrote {len(out)} of {len(codes)} definitions to {sys.argv[2]}")
PY
