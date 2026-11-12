#!/usr/bin/env python3
"""Release tests with the test binaries run side by side (docs/process/dev-speed.md).

  python3 -I tools/ptest.py LOG [--include-ignored] [--jobs N] -- CARGO-ARGS...

Builds `cargo test --release --no-run CARGO-ARGS` and runs every test binary
from its package directory, N at a time (default 3), instead of one after the
other as `cargo test` does: the same binaries, tests and assertions in about
a third of the wall time, because most binaries wait on one long test. A
binary that fails is run again alone, and both results are reported: failing
only under load is a finding, failing alone is a failure. Doc-tests run
through cargo when a crate has any. Result cache: a binary that passed is skipped ("cached") while the sha256 of
the executable, its flags, mods/, the godot/ text files and
target/release/frontline-cli are unchanged; failures are never cached, and
packages that use sockets or timing (net, godot-bridge, simbench) never are.
PTEST_NO_CACHE=1 turns it off (tools/check.sh full sets it). Each binary's
seconds go to target/check/ptest-times.json; the longest run first.
Everything goes to LOG; stdout gets one
line of counts and the first 40 lines of each failing test. Exit 1 on failure.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor


CACHE_DIR = "target/check/ptest-cache"
TIMES = "target/check/ptest-times.json"
NO_CACHE_PKGS = {"net", "godot-bridge", "simbench"}  # real sockets or timing
GODOT_EXT = (".gd", ".tscn", ".tres", ".godot", ".cfg", ".md", ".json", ".toml", ".gdextension")


def hash_tree(h, root, exts=None):
    for d, dirs, files in os.walk(root):
        dirs[:] = sorted(x for x in dirs if x not in (".godot", "bin", ".git"))
        for f in sorted(files):
            if exts and not f.endswith(exts):
                continue
            path = os.path.join(d, f)
            h.update(path.encode())
            try:
                h.update(open(path, "rb").read())
            except OSError:
                pass


def env_hash():
    h = hashlib.sha256()
    hash_tree(h, "mods")
    hash_tree(h, "godot", GODOT_EXT)
    cli = "target/release/frontline-cli"
    if os.path.exists(cli):
        h.update(open(cli, "rb").read())
    return h.hexdigest()


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

    use_cache = not os.environ.get("PTEST_NO_CACHE")
    envh = env_hash() if use_cache else ""
    os.makedirs(CACHE_DIR, exist_ok=True)
    try:
        times = json.load(open(TIMES))
    except (OSError, ValueError):
        times = {}

    def tkey(b):
        return f"{os.path.basename(b[2])}/{b[0]}"

    def ckey(b):
        if os.path.basename(b[2]) in NO_CACHE_PKGS:
            return None
        h = hashlib.sha256()
        h.update(open(b[1], "rb").read())
        h.update(" ".join(extra).encode())
        h.update(envh.encode())
        return h.hexdigest()

    cached = 0

    def run(b):
        nonlocal cached
        name, exe, cwd = b
        s = time.time()
        key = ckey(b) if use_cache else None
        cf = os.path.join(CACHE_DIR, key) if key else None
        if cf and os.path.exists(cf):
            cached += 1
            return name, 0, open(cf).read() + "(cached)\n", 0.0, b
        p = subprocess.run([exe, *extra], cwd=cwd, capture_output=True, text=True)
        out = p.stdout + p.stderr
        if cf and p.returncode == 0:
            keep = [l for l in out.splitlines() if l.startswith("test result:")]
            open(cf, "w").write("\n".join(keep) + "\n")
        return name, p.returncode, out, time.time() - s, b

    # Longest known binaries first, unknown ones before those.
    order = sorted(bins, key=lambda b: -times.get(tkey(b), 1e9))
    with ThreadPoolExecutor(jobs) as ex:
        res = list(ex.map(run, order))
    for name, rc, out, secs, b in res:
        if rc == 0 and secs > 0:
            times[tkey(b)] = secs
    try:
        json.dump(times, open(TIMES, "w"))
    except OSError:
        pass
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
    print(f"{passed} passed, {failed} failed, {len(bins)} binaries, {cached} cached, {time.time() - t0:.0f} s"
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
