#!/usr/bin/env bash
# Regenerate ui/src/crs/crs-list.json from PROJ's proj.db.
# Usage: scripts/build-crs-list.sh [path/to/proj.db]
set -euo pipefail
DB="${1:-$(projinfo --searchpaths 2>/dev/null | head -1)/proj.db}"
OUT="$(dirname "$0")/../ui/src/crs/crs-list.json"
sqlite3 -json "$DB" "
  SELECT c.auth_name || ':' || c.code AS id, c.name AS name,
         CASE c.type WHEN 'projected' THEN 'p' ELSE 'g' END AS t,
         COALESCE(MIN(e.name), '') AS area,
         ROUND(MIN(e.west_lon), 2) AS w, ROUND(MIN(e.south_lat), 2) AS s,
         ROUND(MAX(e.east_lon), 2) AS e, ROUND(MAX(e.north_lat), 2) AS n
  FROM crs_view c
  LEFT JOIN usage u ON u.object_table_name = c.table_name AND u.object_auth_name = c.auth_name AND u.object_code = c.code
  LEFT JOIN extent e ON e.auth_name = u.extent_auth_name AND e.code = u.extent_code
  WHERE c.auth_name IN ('EPSG', 'ESRI') AND c.deprecated = 0
    AND c.type IN ('projected', 'geographic 2D')
  GROUP BY c.auth_name, c.code, c.name, c.type
  ORDER BY c.auth_name = 'ESRI', CAST(c.code AS INTEGER)" > "$OUT"
# Compact: round bounds to 2 decimals, one array per row: [id, name, type, area, w, s, e, n]
python3 - "$OUT" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))
# Skip systems the engine's PROJ can't compute (planetary bodies, a few
# historic grids); see scripts/crs-exclude.txt.
import os
excl_path = os.path.join(os.path.dirname(sys.argv[1]), "..", "..", "..", "scripts", "crs-exclude.txt")
excl = {l.split(" ", 1)[0] for l in open(excl_path)} if os.path.exists(excl_path) else set()
out = [[r["id"], r["name"], r["t"], r["area"]] + [None if r[k] is None else round(r[k], 2) for k in "wsen"] for r in rows if r["id"] not in excl]
json.dump(out, open(sys.argv[1], "w"), separators=(",", ":"))
print(f"Wrote {len(out)} CRSs to {sys.argv[1]}")
PY
