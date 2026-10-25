#!/usr/bin/env python3
"""List the engine's #[ignore]d tests with their category, or run them one by one.

  test_plan_ignored.py list              → name, binary, category, reason (TSV)
  test_plan_ignored.py run <out.md> [--no-network] [--no-perf]

Each test runs on its own (`cargo test --test <bin> -- --ignored --exact <name>`), so a
test that crashes its process (e.g. a SIGSEGV bug) can't hide the others. Categories come
from the ignore reason: "BUG: …" (expected to fail until fixed), "network: …", "perf…",
anything else is "other". Exit code 1 if a non-bug test fails or a bug test passes
(that bug is fixed: unignore it and update docs/BUGS.md).
"""
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TESTS = ROOT / "crates/engine/tests"


def ignored():
    out = []
    for f in sorted(TESTS.glob("*.rs")):
        lines = f.read_text().splitlines()
        for i, line in enumerate(lines):
            m = re.search(r'#\[ignore(?:\s*=\s*"(.*)")?\]', line)
            if not m:
                continue
            reason = m.group(1) or ""
            for nxt in lines[i + 1 : i + 6]:
                fm = re.search(r"\bfn\s+(\w+)", nxt)
                if fm:
                    r, n = reason.lower(), fm.group(1)
                    if reason.startswith("BUG"):
                        cat = "bug"
                    elif r.startswith("network") or re.search(r"network|download|tiger|national_map|catalog", n):
                        cat = "network"
                    elif "perf" in r or re.search(r"perf|million|^p6_|^h14_", n):
                        cat = "perf"
                    else:
                        cat = "other"
                    out.append((fm.group(1), f.stem, cat, reason))
                    break
    return out


def run_one(name, binary, release):
    cmd = ["cargo", "test", "-p", "engine"] + (["--release"] if release else []) + ["--test", binary, "--", "--ignored", "--exact", name]
    t0 = time.time()
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    secs = time.time() - t0
    text = p.stdout + p.stderr
    if re.search(rf"test {re.escape(name)} \.\.\. ok", text):
        return "pass", secs, ""
    crash = re.search(r"signal: \d+.*|SIGSEGV|SIGABRT", text)
    panic = re.search(r"panicked at .*?\n(.*)", text)
    detail = crash.group(0) if crash else (panic.group(1).strip()[:200] if panic else text.strip().splitlines()[-1][:200] if text.strip() else "")
    return ("crash" if crash else "fail"), secs, detail


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("list", "run"):
        sys.exit(__doc__)
    tests = ignored()
    if sys.argv[1] == "list":
        for t in tests:
            print("\t".join(t))
        return
    out = Path(sys.argv[2])
    skip = {"network"} if "--no-network" in sys.argv else set()
    if "--no-perf" in sys.argv:
        skip.add("perf")
    rows, bad = [], 0
    for name, binary, cat, reason in tests:
        if cat in skip:
            rows.append((cat, name, binary, "skipped", 0, ""))
            continue
        status, secs, detail = run_one(name, binary, release=(cat == "perf"))
        if cat == "bug":
            verdict = "still open (expected)" if status != "pass" else "NOW PASSES: bug fixed? unignore + update docs/BUGS.md"
            bad += status == "pass"
        else:
            verdict = status
            bad += status != "pass"
        rows.append((cat, name, binary, verdict, secs, detail))
        print(f"{cat:8} {name:70} {verdict} ({secs:.0f}s)", flush=True)
    with out.open("w") as f:
        f.write("| Category | Test | Binary | Result | Time | Detail |\n|---|---|---|---|---|---|\n")
        for cat, name, binary, verdict, secs, detail in sorted(rows):
            f.write(f"| {cat} | `{name}` | {binary} | {verdict} | {secs:.0f} s | {detail.replace('|', '/')} |\n")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
