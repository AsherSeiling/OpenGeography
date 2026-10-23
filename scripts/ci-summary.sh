#!/usr/bin/env bash
# Write a short report for this CI job to the run's summary page:
# platform, tool versions, and the outcome of each check.
#   UI=… ENGINE=… PACKAGE=… SELFTEST=… scripts/ci-summary.sh "<platform>"
set -u
icon() { case "$1" in success) echo "✅ passed" ;; failure) echo "❌ failed" ;; skipped|"") echo "⏭️ not reached" ;; *) echo "$1" ;; esac; }
ver() { "$@" 2>/dev/null | head -1 || true; }
{
  echo "## $1"
  echo
  echo "| Check | Result |"
  echo "|---|---|"
  echo "| UI tests (vitest) | $(icon "${UI:-}") |"
  echo "| Engine tests (cargo) | $(icon "${ENGINE:-}") |"
  echo "| Build + package | $(icon "${PACKAGE:-}") |"
  echo "| Packaged app self-test | $(icon "${SELFTEST:-}") |"
  echo
  echo "<details><summary>Versions</summary>"
  echo
  echo "- OS: $(uname -srm)"
  echo "- Rust: $(ver rustc --version)"
  echo "- Node: $(ver node --version)"
  echo "- GDAL: $(ver gdal-config --version)$(ver gdalinfo --version)"
  echo "- PROJ: $(ver projinfo --version 2>&1)"
  echo "- DuckDB library: ${DUCKDB_VERSION:-from Homebrew}"
  echo "- Commit: ${GITHUB_SHA:-unknown}"
  echo
  echo "</details>"
  echo
  echo "Failures are listed as annotations above (key lines and last lines of output)."
} >> "${GITHUB_STEP_SUMMARY:-/dev/stdout}"
