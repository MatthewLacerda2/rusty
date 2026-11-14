#!/usr/bin/env python3
"""Size the weekly mutation sweep from what there is to mutate and what it cost (#1684).

Every figure this sweep was once sized by was outgrown: ten shards were cut off
at 6 h in 2026-10-03's run (#749, a hand-set shard count, resized to 24); on
2026-10-31 (run 41193398365) all 24 stopped at their 5 h 40 budget having
measured 1909 of 3552 mutants (a hand-set cost a mutant, ~2 min, never
re-measured); and on 2026-11-07 (run 42367735766) a re-measured 4.3 min still
left shards 6, 11, 12 and 13 at their budget, because the shards spread from
3.7 to 6.7 min a mutant and the matrix was sized for the mean. So nothing here
is set by hand any more:

    cargo mutants --list --file ... | mutants-shards.py plan --reports DIR --source URL
    mutants-shards.py rate --reports DIR --sized-at 6.7

`plan` counts the mutants (one per listed line), reads the shard reports of the
newest earlier sweep from `DIR` (one directory per shard, as `mutants-plan`
downloads them: `elapsed-seconds` and the four outcome lists), and prints
`shards=`, `parallel=` and `minutes=` lines for `$GITHUB_OUTPUT` on stdout and
its arithmetic on stderr. It fails, never silently caps, past GitHub's 256-job
matrix ceiling, and on a count of zero (the `--file` globs match nothing).

- **The cost a mutant is the slowest shard's measured rate**, not the mean
  (`calibrate`): round-robin sharding gives every shard the same mix of
  modules, so what spreads the shards is the runner and the few mutants that
  run to their timeout, and either can land on any shard. Sizing every shard
  so the slowest one measured would end at `TARGET_HOURS` keeps the 1 h 40 of
  budget above it for the next spread. It costs nothing in runner-hours, only
  shards (a few minutes of setup each). A cut-off shard is measured too: its
  seconds over the mutants it finished is a rate, a slightly high one.
- **One bad shard never sizes the sweep** (PR #1903's review): a shard that
  measured fewer than `MIN_MEASURED` mutants is no rate (one mutant in 25 min,
  from a degraded runner or a fatal error just after its baseline, read as
  25 min a mutant, 419 shards at 4018 mutants: past the ceiling, so the plan
  went red, no report was written, and the next plan read the same report),
  and the slowest rate is capped at `OUTLIER_CAP` times the mean.
- **The wall clock is the mean's**: shards run in waves of `parallel`, each
  wave about a mean shard long. `parallel` is the smallest width, up to
  `MAX_PARALLEL`, whose waves end `SPARE_HOURS` before Sunday 23:59 UTC
  (the estimate counts no wait for a runner), itself 5 h 24 before
  `coverage.yml` (Mondays 05:23) cold-builds on the same runners: the
  narrower the sweep, the more runners pull requests keep. When even the cap
  ends late it says so and the sweep runs at the cap: a late sweep beats none.
- **`MINUTES_PER_MUTANT` is only the fallback**, for a first run or a lookup
  that found no readable report; the summary names which source sized it.

`has-rate --reports DIR` exits 0 only when `DIR` holds a rate, so the lookup
passes over a report that holds none and tries an older sweep.

`unfinished --shards DIR --total N` is the report's verdict on each shard
(`unfinished`): only a sweep where every shard finished marks its commit
`sweep-complete`, and a shard whose mutants mostly timed out did not finish.

`rate` is the report's half: `mutants-report` prints this run's mean and
slowest rate beside the figure the plan used, which the next sweep sizes from.
"""

from __future__ import annotations

import argparse
import datetime as dt
import math
import sys
from pathlib import Path
from typing import NamedTuple

#: The fallback cost a mutant, setup included, when no earlier sweep can be
#: read: the slowest shard of run 42367735766 (2026-11-07), 20401 s for 51.
MINUTES_PER_MUTANT = 6.7
#: Where the slowest shard should finish: 4 h leaves 1 h 40 of the 5 h 40
#: budget for a slower runner, a cold cache, or the sim growing in a week.
TARGET_HOURS = 4.0
#: GitHub's limit on the jobs one matrix may generate.
MATRIX_CEILING = 256
#: The most shards at once: the rest of the account's concurrent jobs belong
#: to pull requests, and batches run on weekends too.
MAX_PARALLEL = 10
#: Slack the width keeps before Sunday 23:59 UTC: the wave estimate counts no
#: queueing for a runner and no shard slower than the mean.
SPARE_HOURS = 2.0
#: The fewest mutants a shard must have measured to count as a rate: a fifth of
#: the ≈ 50 each shard of run 42367735766 measured. Below it, the baseline build
#: and one timed-out mutant are most of the figure.
MIN_MEASURED = 10
#: The most the slowest rate may exceed the mean by: run 42367735766's spread was
#: 6.7 / 5.1 ≈ 1.3, so 1.5 keeps headroom over a real spread and drops an outlier.
OUTLIER_CAP = 1.5
#: The outcome lists a shard appends one line a measured mutant to.
OUTCOMES = ("caught", "missed", "timeout", "unviable")


class Rate(NamedTuple):
    """Minutes a mutant: `slowest` sizes the shards, `mean` the wall clock.

    `measured` is False for the fallback: no shard report held a rate.
    """
    slowest: float
    mean: float
    source: str
    measured: bool = True


def shard_rate(shard: Path) -> tuple[int, int] | None:
    """(seconds, mutants measured) of one shard's report, or None if it holds no rate.

    No rate: `elapsed-seconds` absent, not a whole number, or not positive, or
    fewer than `MIN_MEASURED` mutants measured (a shard that never passed its
    baseline, or stopped soon after).
    """
    try:
        seconds = int((shard / "elapsed-seconds").read_text().strip())
    except (OSError, ValueError):
        return None
    measured = sum(counted(shard, kind) for kind in OUTCOMES)
    if seconds <= 0 or measured < MIN_MEASURED:
        return None
    return seconds, measured


def counted(shard: Path, kind: str) -> int:
    """How many mutants a shard's `<kind>.txt` lists; 0 when it has none."""
    try:
        return sum(1 for line in (shard / f"{kind}.txt").read_text().splitlines() if line.strip())
    except OSError:
        return 0


def unfinished(shard: Path) -> str:
    """Why one shard's run is not a finished measurement, or '' when it is.

    `budget`: it stopped itself at its budget (`timeout`'s 124/137). `timeouts:t/v`:
    it finished, but most of its viable mutants timed out, so it measured the tool
    breaking, not the code (#2164: every mutant of a member crate timed out and the
    report read clean). `exit:<code>`: anything else, `baseline` (its suite failed
    unmutated) or `none` (no exit code: cut off before its run) included.
    """
    try:
        rc = (shard / "exit-code").read_text().strip() or "none"
    except OSError:
        rc = "none"
    if rc in ("124", "137"):
        return "budget"
    if rc not in ("0", "2", "3"):
        return f"exit:{rc}"
    timed_out = counted(shard, "timeout")
    viable = timed_out + counted(shard, "caught") + counted(shard, "missed")
    return f"timeouts:{timed_out}/{viable}" if timed_out * 2 > viable else ""


def calibrate(reports: Path | None, source: str = "") -> Rate:
    """The rate the shard reports under `reports` measured, or the fallback.

    The slowest shard's rate, capped at `OUTLIER_CAP` times the mean.
    """
    rates = [r for shard in sorted(reports.iterdir()) if shard.is_dir()
             if (r := shard_rate(shard))] if reports and reports.is_dir() else []
    if not rates:
        return Rate(MINUTES_PER_MUTANT, MINUTES_PER_MUTANT,
                    f"`MINUTES_PER_MUTANT` ({MINUTES_PER_MUTANT}): no earlier shard report held a rate",
                    measured=False)
    mean = sum(s for s, _ in rates) / 60 / sum(m for _, m in rates)
    slowest = min(max(s / 60 / m for s, m in rates), mean * OUTLIER_CAP)
    return Rate(slowest, mean, f"{len(rates)} shard reports of {source or 'an earlier sweep'}")


def shard_count(mutants: int, minutes: float = MINUTES_PER_MUTANT,
                hours: float = TARGET_HOURS) -> int:
    """Shards needed for `mutants` at `minutes` each, `hours` a shard."""
    if mutants <= 0:
        raise ValueError("nothing to mutate: the sweep's --file globs match no mutant")
    shards = math.ceil(mutants * minutes / (hours * 60))
    if shards > MATRIX_CEILING:
        raise ValueError(f"{mutants} mutants need {shards} shards at {minutes:.1f} min each and"
                         f" {hours} h a shard, past GitHub's {MATRIX_CEILING}-job matrix"
                         " ceiling: narrow the sweep or make a mutant cheaper")
    return shards


def hours_to_deadline(now: dt.datetime) -> float:
    """Hours from `now` to the coming Sunday 23:59 UTC (today's, on a Sunday)."""
    now = now.astimezone(dt.timezone.utc)
    sunday = now.date() + dt.timedelta(days=(6 - now.weekday()) % 7)
    deadline = dt.datetime.combine(sunday, dt.time(23, 59), dt.timezone.utc)
    return max(0.0, (deadline - now).total_seconds() / 3600)


def parallel(shards: int, shard_hours: float, hours_left: float,
             cap: int = MAX_PARALLEL) -> tuple[int, float, bool]:
    """(width, estimated hours, ends in time): the narrowest width that fits, else the cap."""
    widest = max(1, min(cap, shards))
    for width in range(1, widest + 1):
        hours = math.ceil(shards / width) * shard_hours
        if hours <= hours_left:
            return width, hours, True
    return widest, math.ceil(shards / widest) * shard_hours, False


def count_listed(lines: list[str]) -> int:
    """How many mutants a `cargo mutants --list` printed: one per non-blank line."""
    return sum(1 for line in lines if line.strip())


def plan(mutants: int, rate: Rate, now: dt.datetime) -> tuple[list[str], str]:
    """The `$GITHUB_OUTPUT` lines and the summary for `mutants` at `rate`."""
    shards = shard_count(mutants, rate.slowest)
    shard_hours = math.ceil(mutants / shards) * rate.mean / 60
    left = hours_to_deadline(now)
    width, hours, in_time = parallel(shards, shard_hours, left - SPARE_HOURS)
    summary = (f"{mutants} mutants × {rate.slowest:.1f} min (the slowest shard's rate; mean"
               f" {rate.mean:.1f}, from {rate.source}) ÷ {TARGET_HOURS} h a shard → {shards} shards,"
               f" {width} at a time: ≈ {hours:.0f} h of the {left:.0f} h to Sunday 23:59 UTC,"
               f" {SPARE_HOURS:.0f} h kept spare")
    if not in_time:
        summary += (f". ⚠️ Even {width} wide it runs late, past its spare and maybe into Monday:"
                    " it runs anyway, but narrow the sweep or make a mutant cheaper")
    outputs = [f"shards={list(range(shards))}".replace(" ", ""), f"parallel={width}",
               f"minutes={rate.slowest:.2f}"]
    return outputs, summary


def report_rate(rate: Rate, sized_at: float | None) -> str:
    """This run's measured rates, beside the figure its matrix was sized by."""
    if not rate.measured:
        return "Measured rate: no shard measured a mutant, so no rate."
    line = (f"Measured rate: {rate.mean:.1f} min a mutant on average, {rate.slowest:.1f} on"
            f" the slowest shard ({rate.source}); the matrix was sized at"
            f" {sized_at if sized_at is not None else '?'}. The next sweep sizes from the slowest.")
    if sized_at is not None and rate.slowest > sized_at * 1.2:
        line += " ⚠️ Over 20 % slower than sized: a shard may have reached its budget."
    return line


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-shards.py", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    planning = sub.add_parser("plan", help="read `cargo mutants --list` on stdin, print the outputs")
    planning.add_argument("--reports", type=Path, help="an earlier sweep's shard reports")
    planning.add_argument("--source", default="", help="what the reports are, for the summary")
    planning.add_argument("--now", type=dt.datetime.fromisoformat, help="ISO time (default: now)")
    probe = sub.add_parser("has-rate", help="exit 0 if the shard reports hold a rate, else 1")
    probe.add_argument("--reports", type=Path, required=True, help="a sweep's shard reports")
    measured = sub.add_parser("rate", help="this run's measured minutes a mutant")
    measured.add_argument("--reports", type=Path, required=True, help="this run's shard reports")
    measured.add_argument("--sized-at", type=float, help="the minutes the plan sized by")
    unfin = sub.add_parser("unfinished", help="print ` k(why)` for each shard that did not finish")
    unfin.add_argument("--shards", type=Path, required=True, help="one `mutants-shard-<k>` dir a shard")
    unfin.add_argument("--total", type=int, required=True, help="the matrix's shard count")
    args = parser.parse_args()

    if args.command == "unfinished":
        whys = ((k, unfinished(args.shards / f"mutants-shard-{k}")) for k in range(args.total))
        print("".join(f" {k}({why})" for k, why in whys if why))
        return
    if args.command == "has-rate":
        sys.exit(0 if calibrate(args.reports).measured else 1)
    if args.command == "rate":
        print(report_rate(calibrate(args.reports, "this run"), args.sized_at))
        return
    now = args.now or dt.datetime.now(dt.timezone.utc)
    if now.tzinfo is None:
        now = now.replace(tzinfo=dt.timezone.utc)
    try:
        outputs, summary = plan(count_listed(sys.stdin.readlines()),
                                calibrate(args.reports, args.source), now)
    except ValueError as err:
        sys.exit(f"mutants-shards: {err}")
    print(summary, file=sys.stderr)
    print("\n".join(outputs))


if __name__ == "__main__":
    main()
