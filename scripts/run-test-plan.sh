#!/usr/bin/env bash
# Run the automated part of docs/TEST_PLAN.md and write one summary.
#
#   scripts/run-test-plan.sh [--quick] [--no-network] [--package] [--e2e]
#
#   default      static + UI + engine + every #[ignore]d engine test, one by one
#   --quick      static + UI + engine only (what CI runs)
#   --no-network skip the ignored tests that need the internet
#   --package    also build the Mac app and run its --self-test (macOS only)
#   --e2e        also run the browser end-to-end suite (scripts/e2e.sh) if it exists
#
# Output: test-results/<timestamp>/summary.md (+ one log per stage). Exit 1 if any
# stage fails. Manual cases (docs/TEST_PLAN.md §14) are listed at the end of the
# summary; they need a person.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH=/opt/homebrew/opt/rustup/bin:/opt/homebrew/bin:$PATH

QUICK=0; NONET=""; PACKAGE=0; E2E=0
for a in "$@"; do
  case "$a" in
    --quick) QUICK=1 ;; --no-network) NONET="--no-network" ;; --package) PACKAGE=1 ;; --e2e) E2E=1 ;;
    *) echo "unknown option $a"; exit 2 ;;
  esac
done

OUT="test-results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUT"
SUMMARY="$OUT/summary.md"
FAILED=0
{ echo "# Test plan run $(date '+%Y-%m-%d %H:%M')"; echo; echo "Commit \`$(git rev-parse --short HEAD)\`$(git diff --quiet || echo ' (uncommitted changes)') · $(uname -sm)"; echo; echo "| Stage | Result | Detail |"; echo "|---|---|---|"; } > "$SUMMARY"

stage() { # name, command...
  local name="$1"; shift
  local log="$OUT/$name.log" t0=$SECONDS
  echo "== $name"
  if "$@" > "$log" 2>&1; then r="pass"; else r="FAIL"; FAILED=1; fi
  local detail
  detail=$(grep -E "^test result:|Tests +[0-9]+|error(\[|:)|FAILED|SELF-TEST" "$log" | tr -s ' ' | sed 's/|/\//g' | head -8 | paste -sd ';' -)
  echo "| $name | $r ($((SECONDS - t0)) s) | ${detail:0:400} |" >> "$SUMMARY"
  echo "   $r"
}

stage static npx tsc -p tsconfig.json
stage ui npx vitest run
stage engine cargo test -p engine

if [ $QUICK -eq 0 ]; then
  stage engine-ignored python3 scripts/test_plan_ignored.py run "$OUT/ignored.md" $NONET
fi

if [ $E2E -eq 1 ]; then
  if [ -x scripts/e2e.sh ]; then stage e2e scripts/e2e.sh "$OUT"; else echo "| e2e | skipped | scripts/e2e.sh not written yet |" >> "$SUMMARY"; fi
fi

if [ $PACKAGE -eq 1 ]; then
  stage package scripts/package-macos.sh
  stage self-test env -i HOME="$HOME" PATH=/usr/bin:/bin "target/release/bundle/macos/GIS Studio.app/Contents/MacOS/gis-studio" --self-test
fi

{
  if [ -f "$OUT/ignored.md" ]; then echo; echo "## Ignored engine tests (run one by one)"; echo; cat "$OUT/ignored.md"; fi
  [ -f "$OUT/e2e.md" ] && { echo; echo "## Browser end-to-end"; echo; cat "$OUT/e2e.md"; }
  echo; echo "## Manual cases (need a person)"; echo
  echo "From docs/TEST_PLAN.md §14. Cases the e2e suite covers are marked there as \`auto: e2e …\`; the rest:"; echo
  grep -E "^- \*\*M-[0-9]+" docs/TEST_PLAN.md | grep -v "auto: e2e" | sed -E 's/^- \*\*(M-[0-9]+[^*]*)\*\*.*/- [ ] \1/'
} >> "$SUMMARY"

echo; echo "Summary: $SUMMARY"
exit $FAILED
