#!/usr/bin/env bash
# Browser end-to-end suite (docs/TEST_PLAN.md §14 cases a browser can do).
#
#   scripts/e2e.sh <out-dir>        writes <out-dir>/e2e.md; exit 1 if any test fails
#
# Builds the release devserver, starts it and vite on free ports, runs Playwright
# headless against them, and always stops both servers.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH
OUT="${1:?usage: scripts/e2e.sh <out-dir>}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

free_port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'; }
API_PORT=$(free_port); VITE_PORT=$(free_port)
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done; wait 2>/dev/null; }
trap cleanup EXIT INT TERM

cargo build --release -p devserver > "$OUT/e2e-build.log" 2>&1 || { echo "devserver build failed (see $OUT/e2e-build.log)"; printf '# Browser end-to-end\n\nFAILED: devserver build failed, see e2e-build.log\n' > "$OUT/e2e.md"; exit 1; }

GIS_DEVSERVER_ADDR="127.0.0.1:$API_PORT" ./target/release/devserver > "$OUT/e2e-devserver.log" 2>&1 & PIDS+=($!)
GIS_VITE_PORT=$VITE_PORT VITE_GIS_DEVSERVER="http://127.0.0.1:$API_PORT" npx vite > "$OUT/e2e-vite.log" 2>&1 & PIDS+=($!)

wait_for() { for _ in $(seq 1 120); do curl -s -o /dev/null "$1" && return 0; sleep 0.5; done; return 1; }
wait_for "http://localhost:$VITE_PORT/" && curl -s -o /dev/null -X POST -d '{}' "http://127.0.0.1:$API_PORT/invoke/list_layers" || {
  echo "servers did not start"; printf '# Browser end-to-end\n\nFAILED: servers did not start, see e2e-*.log\n' > "$OUT/e2e.md"; exit 1; }

export E2E_VITE_PORT=$VITE_PORT E2E_API_PORT=$API_PORT E2E_JSON="$OUT/e2e.json" E2E_ARTIFACTS="$OUT/e2e-artifacts"
npx playwright test > "$OUT/e2e.log" 2>&1
RC=$?
tail -25 "$OUT/e2e.log"
python3 scripts/e2e_report.py "$OUT/e2e.json" > "$OUT/e2e.md"
exit $RC
