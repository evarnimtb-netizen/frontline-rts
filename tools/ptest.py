#!/usr/bin/env python3
"""Release tests with the test binaries run side by side (docs/process/dev-speed.md).

  python3 -I tools/ptest.py LOG [--include-ignored] [--jobs N] -- CARGO-ARGS...

Builds `cargo test --release --no-run CARGO-ARGS` and runs every test binary
from its package directory, N at a time (default 3), instead of one after the
other as `cargo test` does: the same binaries, tests and assertions in about
a third of the wall time, because most binaries wait on one long test. A
binary that fails is run again alone, and both results are reported: failing
only under load is a finding, failing alone is a failure. Doc-tests run
through cargo when a crate has any. Everything goes to LOG; stdout gets one
line of counts and the first 40 lines of each failing test. Exit 1 on failure.
"""
import json
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor


def main():
    args = sys.argv[1:]
    log = args.pop(0)
    cut = args.index("--")
    own, cargo = args[:cut], args[cut + 1:]
    extra = ["--include-ignored"] if "--include-ignored" in own else []
    jobs = int(own[own.index("--jobs") + 1]) if "--jobs" in own else 3
    t0 = time.time()
    b = subprocess.run(["cargo", "test", "--release", "--no-run", "--message-format=json", *cargo],
                       capture_output=True, text=True)
    if b.returncode != 0:
        open(log, "w").write(b.stderr)
        print("build failed")
        print("\n".join(l for l in b.stderr.splitlines() if l.startswith("error"))[:1500])
        sys.exit(1)
    bins = []
    for line in b.stdout.splitlines():
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("reason") == "compiler-artifact" and r.get("profile", {}).get("test") and r.get("executable"):
            bins.append((r["target"]["name"], r["executable"], os.path.dirname(r["manifest_path"])))

    def run(b):
        name, exe, cwd = b
        s = time.time()
        p = subprocess.run([exe, *extra], cwd=cwd, capture_output=True, text=True)
        return name, p.returncode, p.stdout + p.stderr, time.time() - s, b

    with ThreadPoolExecutor(jobs) as ex:
        res = list(ex.map(run, bins))
    retried = []
    for i, (name, rc, out, secs, b) in enumerate(res):
        if rc != 0:
            again = run(b)
            retried.append((name, again[1]))
            res[i] = (name, again[1], out + f"\n==== {name}: run again alone ====\n" + again[2], secs + again[3], b)
    # Doc-tests exist only in library crates that have some; cargo decides.
    d = subprocess.run(["cargo", "test", "--release", "--doc", "-q", *cargo], capture_output=True, text=True)
    doc_out = d.stdout + d.stderr
    doc_bad = d.returncode != 0 and "no library targets found" not in doc_out
    passed = failed = 0
    with open(log, "w") as f:
        for name, rc, out, secs, _ in res:
            f.write(f"==== {name} ({secs:.0f} s, exit {rc}) ====\n{out}\n")
        f.write(f"==== doc-tests (exit {d.returncode}) ====\n{doc_out}\n")
    text = "\n".join(out.split("run again alone ====\n")[-1] for _, _, out, _, _ in res) + "\n" + doc_out
    for m in re.finditer(r"^test result: \S+ (\d+) passed; (\d+) failed", text, re.M):
        passed += int(m.group(1))
        failed += int(m.group(2))
    bad = [name for name, rc, _, _, _ in res if rc != 0]
    flaky = [name for name, rc in retried if rc == 0]
    slow = sorted(res, key=lambda r: -r[3])[:3]
    print(f"{passed} passed, {failed} failed, {len(bins)} binaries, {time.time() - t0:.0f} s"
          + (f"; failed under load but passed alone: {', '.join(flaky)}" if flaky else "")
          + f"; slowest: {', '.join(f'{n} {s:.0f} s' for n, _, _, s, _ in slow)}")
    for name, rc, out, _, _ in res:
        if rc == 0:
            continue
        print(f"  FAILED binary {name} (exit {rc})")
        show = n = 0
        for line in out.split("run again alone ====\n")[-1].splitlines():
            if re.match(r"^---- .* stdout ----$", line):
                show, n = 1, 0
            elif line == "failures:":
                show = 0
            if show and n < 40:
                print("    " + line)
                n += 1
    sys.exit(1 if bad or doc_bad else 0)


if __name__ == "__main__":
    main()
