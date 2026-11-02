#!/usr/bin/env python3
"""Size the weekly mutation sweep's matrix from what there is to mutate (#1684).

The sweep (`mutants-sweep.yml`) used to carry a hand-picked shard count, and
the sim outgrew it twice: ten shards were cut off at 6 h in 2026-10-03's run
(#749, resized to 24), and on 2026-10-31 (run 41193398365) all 24 stopped at
their 5 h 40 budget having measured 1909 of 3552 mutants. A third hand-picked
number would be outgrown the same way, so `mutants-plan` now counts the
mutants (`cargo mutants --list` builds nothing) and asks this script how many
shards that takes:

    cargo mutants --list --file ... | mutants-shards.py plan
    mutants-shards.py rate --mutants 1909 --seconds 489600

`plan` prints the matrix as JSON (`[0, 1, …, n-1]`) on stdout and its sums on
stderr. It fails, never silently caps, past GitHub's 256-job matrix ceiling,
and on a count of zero (the `--file` globs no longer match anything).

`rate` is the other half of the loop: `mutants-report` prints the run's
measured minutes a mutant beside `MINUTES_PER_MUTANT`, so the next drift shows
up in the report a week before a shard is cut off by it.
"""

from __future__ import annotations

import argparse
import json
import math
import sys

#: What one mutant costs a shard, setup included: run 41193398365 spent ≈ 136
#: runner-hours (24 shards × 5 h 40) to measure 1909 mutants, ≈ 4.3 min each.
#: The workflow's comment once said ≈ 2 min (#480); it was never re-measured.
#: Re-measure from the `rate` line of a finished sweep's report.
MINUTES_PER_MUTANT = 4.3
#: Where each shard should finish: 4 h leaves 1 h 40 of the 5 h 40 budget for a
#: slower runner, a cold cache, or the sim growing between re-measurements.
TARGET_HOURS = 4.0
#: GitHub's limit on the jobs one matrix may generate.
MATRIX_CEILING = 256


def shard_count(mutants: int, minutes: float = MINUTES_PER_MUTANT,
                hours: float = TARGET_HOURS) -> int:
    """Shards needed for `mutants` at `minutes` each, `hours` a shard."""
    if mutants <= 0:
        raise ValueError("nothing to mutate: the sweep's --file globs match no mutant")
    shards = math.ceil(mutants * minutes / (hours * 60))
    if shards > MATRIX_CEILING:
        raise ValueError(f"{mutants} mutants need {shards} shards at {minutes} min each and"
                         f" {hours} h a shard, past GitHub's {MATRIX_CEILING}-job matrix"
                         " ceiling: narrow the sweep or make a mutant cheaper")
    return shards


def count_listed(lines: list[str]) -> int:
    """How many mutants a `cargo mutants --list` printed: one per non-blank line."""
    return sum(1 for line in lines if line.strip())


def rate(mutants: int, seconds: int) -> str:
    """The run's measured minutes a mutant, beside the figure the matrix was sized by."""
    if mutants <= 0:
        return "Measured rate: no mutant was measured, so no rate."
    measured = seconds / 60 / mutants
    line = (f"Measured rate: {measured:.1f} min a mutant ({mutants} mutants in"
            f" {seconds / 3600:.1f} shard-hours); the matrix was sized at"
            f" {MINUTES_PER_MUTANT} (`MINUTES_PER_MUTANT` in"
            " `.github/scripts/mutants-shards.py`).")
    if measured > MINUTES_PER_MUTANT * 1.2:
        line += " ⚠️ Over 20 % slower than sized: raise `MINUTES_PER_MUTANT` before a shard is cut off."
    return line


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-shards.py", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("plan", help="read `cargo mutants --list` on stdin, print the matrix")
    measured = sub.add_parser("rate", help="the run's measured minutes a mutant")
    measured.add_argument("--mutants", type=int, required=True, help="mutants measured")
    measured.add_argument("--seconds", type=int, required=True, help="shard seconds spent")
    args = parser.parse_args()

    if args.command == "rate":
        print(rate(args.mutants, args.seconds))
        return
    mutants = count_listed(sys.stdin.readlines())
    try:
        shards = shard_count(mutants)
    except ValueError as err:
        sys.exit(f"mutants-shards: {err}")
    print(f"{mutants} mutants × {MINUTES_PER_MUTANT} min ÷ {TARGET_HOURS} h a shard"
          f" → {shards} shards", file=sys.stderr)
    print(json.dumps(list(range(shards))))


if __name__ == "__main__":
    main()
