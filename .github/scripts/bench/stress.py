"""The stress yard's legs of `make bench-ab STRESS=1` (#2450): `make bench`'s scenario in three legs.

A change to the sim (physics, navigation, the ECS, animation) shows on the
fixture's stress yard, where fifty rigs and a hundred agents run, and often
not at all on a game level held by a standing camera. `make bench` compares a
tree with its own last run (`out/bench/bench.json`): no second tree, no floor.
Before this, sim PRs were compared on the yard by a shell script outside the
repository that built `play` in the PR's tree after `bench-ab` had put it back
at its bare head, so it measured the PR **without `main` merged in**: one PR's
navigation row read 18% slower because `main` held a navigation change the
PR's leg lacked. So the yard runs here, in the same three legs as the scenes
(`main`, the PR's head with `main` merged in, `main` again), on the same trees,
**before** `bench-ab` puts the PR's tree back ([`run`] is called inside its
`try`, the put-back is its `finally`; `tests/bench/test_stress_flow.py` holds
the order).

Each leg is [`RUNS`] runs by default (a yard run is seconds), each a `play`
of the leg's own tree's fixture (`<tree>/tests/fixtures/project` and its
`scenarios/bench/bench.lua`, so each tree's yard is its own, as `make bench`
runs it) into `out/bench/ab/pr-<n>/<leg>/stress-<i>/`, emptied first: `play`
compares with a `bench.json` its out folder holds, and an empty one holds none,
so neither `out/bench/bench.json` nor an older run is read or written. Every run
goes through the legs' memory-capped scope prefix and the busy check before it
(`bench-ab.py`). The section ([`section`]) is the scenes' shape
(`table.section`): each row the average of the leg's runs, the lead largest
change first, the floor from the second `main` leg, a row absent on `main` said
so, and a row whose `main` value is zero shown as its two values. A window
prefix such as `firefight.` is part of a row's name.
"""

from __future__ import annotations

import json
import shutil
from dataclasses import dataclass
from pathlib import Path

from bench import table

RUNS = 3
PROJECT = Path("tests/fixtures/project")
SCENARIO = PROJECT / "scenarios" / "bench" / "bench.lua"


@dataclass(frozen=True)
class Leg:
    name: str
    tree: Path
    binary: Path
    runs: tuple[Path, ...]


def plan(main: tuple[Path, Path], pr: tuple[Path, Path], root: Path, runs: int | None = None) -> list[Leg]:
    """The three legs, in order; `main` and `pr` are each `(tree, play binary)`."""
    n = runs or RUNS

    def leg(name, tree_bin):
        return Leg(name, tree_bin[0], tree_bin[1], tuple(root / name / f"{table.STRESS_RUN}{i}" for i in range(n)))
    return [leg("main", main), leg("pr", pr), leg("main-again", main)]


def command(leg: Leg, out: Path) -> list[str]:
    """`play`'s command line for one run of `leg` into `out`."""
    return [str(leg.binary), "--project", str(leg.tree / PROJECT), str(leg.tree / SCENARIO), str(out)]


def run(legs: list[Leg], prefix: list[str], runner, before_each=None) -> list[tuple[Leg, int]]:
    """Every run of each leg in order through `runner(argv) -> exit code`, stopping at a
    failed one; `(leg, code)` per leg begun, its last run's code.

    `before_each(leg)` is asked before every run and returns a refusal to stop on, or `None`."""
    done = []
    for leg in legs:
        code = 0
        for out in leg.runs:
            said = before_each(leg) if before_each else None
            if said:
                raise RuntimeError(f"before a run of the stress {leg.name} leg: {said}")
            if out.exists():
                shutil.rmtree(out)
            code = runner(prefix + command(leg, out))
            if code != 0:
                break
        done.append((leg, code))
        if code != 0:
            break
    return done


def report(out: Path) -> dict:
    """One run's `bench.json` (`dev::bench::report::Report`), `{}` when it wrote none."""
    try:
        return json.loads((out / "bench.json").read_text())
    except (OSError, ValueError):
        return {}


def averages(leg: Leg) -> dict[str, float]:
    """Each row's average over the leg's runs that report it, in the first run's order."""
    sums: dict[str, list[float]] = {}
    for out in leg.runs:
        for metric, summary in report(out).get("metrics", []):
            sums.setdefault(metric, []).append(summary["avg"])
    return {m: sum(v) / len(v) for m, v in sums.items()}


def section(legs: list[Leg]) -> str:
    """The table's stress section over the three legs (main, PR, main again)."""
    rows = tuple(averages(leg) for leg in legs)
    said = [list(dict.fromkeys(w for out in leg.runs for w in report(out).get("warnings", []))) for leg in legs]
    runs = len(legs[0].runs)
    title = f"**stress yard** (`make bench`'s scenario, {runs} run{'s' if runs != 1 else ''} a leg, averaged)"
    return table.section(title, rows, table.notes_of(said, "`play`'s notes"))
