#!/usr/bin/env python3
"""Turn Playwright's JSON report into the table in <out-dir>/e2e.md.
Test titles start with the manual case id (e.g. "M-4 ..."); fixme tests are listed as known bugs."""
import json, re, sys

def walk(suite, path=()):
    for s in suite.get("suites", []):
        yield from walk(s, path + (s["title"],))
    for spec in suite.get("specs", []):
        for t in spec.get("tests", []):
            res = t.get("results") or [{}]
            r = res[-1]
            status = t.get("status", "")
            yield spec["title"], spec.get("file", ""), status, r.get("duration", 0), (r.get("error") or {}).get("message", "")

data = json.load(open(sys.argv[1]))
rows = [x for s in data.get("suites", []) for x in walk(s)]
print("| Case | Test | Result | Time |\n|---|---|---|---|")
bad = 0
for title, file, status, ms, err in rows:
    m = re.match(r"(M-\d+)\s*(.*)", title)
    case, name = (m.group(1), m.group(2)) if m else ("-", title)
    label = {"expected": "pass", "skipped": "skipped (known bug)", "unexpected": "FAIL", "flaky": "flaky"}.get(status, status)
    bad += status in ("unexpected", "flaky")
    print(f"| {case} | {file}::{name} | {label} | {ms/1000:.1f} s |")
    if status == "unexpected":
        msg = re.sub(r"\x1b\[[0-9;]*m", "", err).replace("\n", " ").replace("|", "/")[:300]
        print(f"|  | `{msg}` | | |")
st = data.get("stats", {})
print(f"\n{st.get('expected',0)} passed, {st.get('unexpected',0)} failed, {st.get('skipped',0)} skipped (fixme), {st.get('flaky',0)} flaky; {st.get('duration',0)/1000:.0f} s")
