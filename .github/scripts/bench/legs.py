"""The three legs of `make bench-ab` (#2381): `main`, the PR's head, `main` again.

Each leg is one `bench-scenes` run of its tree's build, into a folder of its
own under `out/bench/ab/pr-<n>/` (emptied first, so no leg compares with an
older run). The PR's leg and the second `main` leg both run with `--before` the
first `main` leg's folder, so each prints its change against the same numbers;
`table` reads the three folders. Legs run back to back and never interleave
scene by scene: interleaving halves the drift but doubles the scene loads, and
three legs is what #2298 measured with.

Each leg runs in a memory-capped scope of its own where the machine gives one
(`systemd-run --user --scope -p MemoryMax=…`, Linux), the cap a fraction
([`CAP_FRACTION`]) of the memory available when the run starts, read then, never
written down: a leg that runs away is stopped instead of swapping the next
leg's timings. Where there is none (macOS, no user session) the legs run plain
and the run says so.
"""

from __future__ import annotations

import shutil
from dataclasses import dataclass
from pathlib import Path

CAP_FRACTION = 0.75


@dataclass(frozen=True)
class Leg:
    name: str
    binary: Path
    out: Path
    before: Path | None


def plan(main_bin: Path, pr_bin: Path, root: Path) -> list[Leg]:
    """The three legs, in order, their reports under `root`."""
    first = root / "main"
    return [
        Leg("main", main_bin, first, None),
        Leg("pr", pr_bin, root / "pr", first),
        Leg("main-again", main_bin, root / "main-again", first),
    ]


@dataclass(frozen=True)
class Ask:
    """What every leg is asked for, passed through to `bench-scenes`."""
    project: Path
    scenes: str | None = None
    runs: int | None = None
    size: str | None = None
    sets: tuple[str, ...] = ()


def command(leg: Leg, ask: Ask) -> list[str]:
    """`bench-scenes`' command line for `leg`."""
    argv = [str(leg.binary), "--project", str(ask.project)]
    if ask.scenes:
        argv += ["--scenes", ask.scenes]
    if ask.runs:
        argv += ["--runs", str(ask.runs)]
    if ask.size:
        argv += ["--size", ask.size]
    for pair in ask.sets:
        argv += ["--set", pair]
    if leg.before is not None:
        argv += ["--before", str(leg.before)]
    return argv + [str(leg.out)]


def available_bytes(meminfo: str) -> int | None:
    """`MemAvailable` from `/proc/meminfo`'s text, in bytes."""
    for line in meminfo.splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) * 1024
    return None


def scope(cap: int | None, has_systemd: bool) -> tuple[list[str], str]:
    """The prefix a leg runs under, and the line that says what it is."""
    if cap is None or not has_systemd:
        return [], "legs run plain: no memory-capped scope on this machine (systemd-run --user)"
    prefix = ["systemd-run", "--user", "--scope", "--quiet", "-p", f"MemoryMax={cap}", "--"]
    gib = cap / 1024**3
    return prefix, f"each leg in its own scope capped at {gib:.1f} GiB ({CAP_FRACTION:.0%} of the memory available now)"


def run(legs: list[Leg], ask: Ask, prefix: list[str], runner, before_each=None) -> list[tuple[Leg, int]]:
    """Run each leg in order through `runner(argv) -> exit code`, stopping at a failed one.

    `before_each(leg)` returns a refusal to stop on (a build started mid-run), or `None`.
    """
    done = []
    for leg in legs:
        said = before_each(leg) if before_each else None
        if said:
            raise RuntimeError(f"before the {leg.name} leg: {said}")
        if leg.out.exists():
            shutil.rmtree(leg.out)
        code = runner(prefix + command(leg, ask))
        done.append((leg, code))
        if code != 0:
            break
    return done
