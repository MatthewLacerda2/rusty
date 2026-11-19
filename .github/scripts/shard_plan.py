#!/usr/bin/env python3
"""Split a sharded nextest suite by `main`'s test times, not by count (#2397).

nextest's `--partition` kinds (`count:`, `hash:`, `slice:`; checked on 0.9.146 and
0.9.152) deal tests by count or by hash, blind to time, so the macOS leg's
balance was luck of the test list: once the longest Metal tests all landed in
`count:1/2`, about 1.6 times the GPU-seconds of 2/2 (runs 44429815128,
44435857315), and a later list was within 7%. A plan keeps the balance from
moving with the list; runner speed, which varies by about a quarter, it cannot
fix (`slowest_tests.weight` prints the prediction beside the time taken).

So the `shard-plan` job of `ci.yml` reads the leg's JUnit reports from `main`'s
last successful runs (`.github/actions/main-junit`, the same lookup the slow-test
report uses), once, and hands every shard the same plan; each shard's test step
turns it into nextest arguments. The plan is made once because each shard
downloading its own baseline could read different runs, and two plans disagree
on where a test goes: a test in neither shard is the one failure a split must
never produce.

**What a plan is.** Each of `main`'s heaviest tests (`HEAVY`, by its longest time
over the runs) is a unit of its own; every other test belongs to the bucket of its
name's last `SUFFIX` characters, which a test added since still falls in. The
units are dealt longest first to the shard with the least time so far (`deal`),
and shard `k` of `n` gets one nextest filterset (`filters`):

- for `k < n`: its own heavy tests by exact name, and the tests of its buckets
  that are not heavy, minus every earlier shard's selection;
- for `k = n`: every test no earlier shard selected.

The last shard is the complement of the others, so every test, known to `main`
or not, runs in exactly one shard, whatever the times said: a wrong plan costs
balance, never coverage. `.github/scripts/tests/shard_plan/` evaluates the
filters over test lists to hold that.

**No plan, `count:`.** With no readable baseline (a fork, an expired artifact,
the first run after a rename), or too few timed tests to give every shard one,
the plan is `{}` and every shard runs
`--partition count:k/n`, saying so in the log. The choice reads only the plan and
the shard count, never the shard's index, so every shard of a run makes the same
one.

Usage:
  `shard_plan.py plan <dir>... --shards N`: print the plan as one JSON line, and
  each shard's expected test-seconds on stderr.
  `shard_plan.py args k/n`: print the nextest arguments for shard `k/n`, one per
  line, from the plan in `$PLAN`.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from collections import defaultdict

import slowest_tests

#: How many of `main`'s longest tests are units of their own. The 120 longest
#: macOS tests held about 60% of the leg's test-seconds (2026-11-19's two runs),
#: the rest at under 4 s each, few enough that the filter stays small: each name
#: costs about 80 bytes and one argument must stay under Linux's 128 KiB.
HEAVY = 120
#: The light tests' bucket: the last two characters of the name, a few hundred
#: buckets, so no bucket outweighs a heavy test by much.
SUFFIX = 2
#: What a name or binary id may hold to be written into a filter as it is:
#: anything else stays out of the heavy units and goes by its bucket.
SAFE = re.compile(r"^[A-Za-z0-9_:\-/]+$")
#: The bucket's characters, written into a regex unescaped.
WORD = re.compile(r"^\w+$")


def units(times: dict[str, float], heavy: int = HEAVY, suffix: int = SUFFIX):
    """`main`'s tests as the plan's units: `(seconds, kind, key)` with kind
    `test` (key `binary name`) or `bucket` (key the suffix)."""
    ranked = sorted(times.items(), key=lambda kv: (-kv[1], kv[0]))
    picked = [k for k, _ in ranked if SAFE.match(k.replace(" ", "", 1)) and " " in k][:heavy]
    found = [(times[k], "test", k) for k in picked]
    buckets: dict[str, float] = defaultdict(float)
    for k, s in times.items():
        tail = k.split(" ", 1)[-1][-suffix:]
        if k not in picked and len(tail) == suffix and WORD.match(tail):
            buckets[tail] += s
    return found + [(s, "bucket", b) for b, s in buckets.items()]


def deal(found, shards: int) -> list[list[tuple[float, str, str]]]:
    """Longest first, each unit to the shard with the least time so far (the
    first such shard on a tie), so the plan is the same on every machine."""
    dealt: list[list[tuple[float, str, str]]] = [[] for _ in range(shards)]
    for unit in sorted(found, key=lambda u: (-u[0], u[1], u[2])):
        least = min(range(shards), key=lambda i: (sum(u[0] for u in dealt[i]), i))
        dealt[least].append(unit)
    return dealt


def exact(keys: list[str]) -> str:
    """A filter of exactly these `binary name` tests, grouped by binary."""
    by: dict[str, list[str]] = defaultdict(list)
    for k in sorted(keys):
        binary, name = k.split(" ", 1)
        by[binary].append(f"test(={name})")
    return " | ".join(f"(binary_id(={b}) & ({' | '.join(ns)}))" for b, ns in by.items())


def selection(own: list[tuple[float, str, str]], heavy: str) -> str:
    """What one of the first `n - 1` shards picks: its heavy tests, and its
    buckets' tests that no shard holds as heavy."""
    parts = []
    tails = sorted(u[2] for u in own if u[1] == "bucket")
    if tails:
        light = f"test(/(?:{'|'.join(tails)})$/)"
        parts.append(f"({light} & not ({heavy}))" if heavy else light)
    tests = [u[2] for u in own if u[1] == "test"]
    if tests:
        parts.append(f"({exact(tests)})")
    return " | ".join(parts) or "none()"


def filters(selections: list[str]) -> list[str]:
    """Each shard's filterset from the first `n - 1` shards' selections: shard
    `k < n` takes its own minus every earlier one's, the last every test no
    other took. Each test is then in exactly one, whatever the selections say."""
    out, earlier = [], []
    for picked in selections:
        out.append(f"({picked}) & not ({' | '.join(earlier)})" if earlier else picked)
        earlier.append(f"({picked})")
    out.append(f"not ({' | '.join(earlier)})" if earlier else "all()")
    return out


def plan(times: dict[str, float], shards: int) -> tuple[dict[str, str], list[float]]:
    """The plan (`k/n` to filterset) and each shard's expected test-seconds; no
    plan (`{}`, so every shard splits by count) when a shard would be dealt
    nothing, as with no timed test: its filter would select no test, which
    nextest fails."""
    dealt = deal(units(times), shards)
    if not all(dealt):
        return {}, [0.0] * shards
    heavy = exact([u[2] for d in dealt for u in d if u[1] == "test"])
    made = filters([selection(d, heavy) for d in dealt[:-1]])
    return ({f"{k}/{shards}": f for k, f in enumerate(made, 1)},
            [round(sum(u[0] for u in d), 1) for d in dealt])


def args(raw: str, shard: str) -> tuple[list[str], str]:
    """The nextest arguments for `k/n` and the log line saying which split.
    Any plan that does not name exactly the shards `1/n` … `n/n` is no plan."""
    k, _, n = shard.partition("/")
    count = (["--partition", f"count:{shard}"], f"split: by count (`count:{shard}`)")
    try:
        found = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError:
        return count[0], count[1] + ": the plan is not JSON"
    if not isinstance(found, dict):
        return count[0], count[1] + f": the plan is a JSON {type(found).__name__}, not a mapping"
    want = {f"{i}/{n}" for i in range(1, int(n) + 1)} if n.isdigit() else set()
    if not found:
        return count[0], count[1] + ": no plan (no report of main's to weigh by)"
    if set(found) != want or not all(isinstance(v, str) and v for v in found.values()):
        return count[0], count[1] + f": the plan names {sorted(found)} where the matrix has {sorted(want)}"
    return ["-E", found[shard]], f"split: by main's test times, shard {k} of {n}, as the shard-plan job planned"


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    made = sub.add_parser("plan")
    made.add_argument("dirs", nargs="+")
    made.add_argument("--shards", type=int, required=True)
    sub.add_parser("args").add_argument("shard")
    a = p.parse_args(argv)
    if a.cmd == "args":
        found, line = args(os.environ.get("PLAN", ""), a.shard)
        print(line, file=sys.stderr)
        print("\n".join(found))
        return 0
    try:
        times = slowest_tests.baseline(a.dirs)
    except ValueError as err:
        print(f"plan: none, each shard splits by count: {err}", file=sys.stderr)
        print("{}")
        return 0
    found, loads = plan(times, a.shards)
    if not found:
        print(f"plan: none, each shard splits by count: {len(times)} timed tests of main's "
              f"cannot fill {a.shards} shards", file=sys.stderr)
        print("{}")
        return 0
    print(f"plan: {len(times)} tests of main's over {a.shards} shards; expected test-seconds "
          + ", ".join(f"{k}/{a.shards} {s} s" for k, s in enumerate(loads, 1)), file=sys.stderr)
    print(json.dumps(found, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
