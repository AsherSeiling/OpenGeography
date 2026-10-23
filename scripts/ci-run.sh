#!/usr/bin/env bash
# Run a CI step; if it fails, publish the key output lines as GitHub error
# annotations (shown on the run's summary page and readable without logs access).
#   scripts/ci-run.sh "<title>" <command> [args...]
set -uo pipefail
title="$1"; shift
log="$(mktemp)"
"$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  # Panics, errors and failed tests first, then the end of the output.
  key="$(grep -nE "panicked|error(\[|:)|Error:|FAILED|failed|not found|cannot|No such file" "$log" | tail -n 40)"
  tail_lines="$(tail -n 40 "$log")"
  PY=$(command -v python3 || command -v python); enc() { "$PY" -c 'import sys; s=sys.stdin.read()[-6000:]; print(s.replace("%","%25").replace("\r","%0D").replace("\n","%0A"))'; }
  echo "::error title=$title (key lines)::$(printf '%s' "$key" | enc)"
  echo "::error title=$title (last lines)::$(printf '%s' "$tail_lines" | enc)"
fi
rm -f "$log"
exit "$status"
