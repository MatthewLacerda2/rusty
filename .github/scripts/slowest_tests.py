#!/usr/bin/env python3
"""Print where a nextest JUnit report's time went (#1976, #2326).

The macOS leg ran past its 30-minute timeout on 2026-11-09, and finding what to
speed up meant per-test timings nobody could read: the job summary
`test-summary/action` writes is not served by the REST API a cloud session reads,
and nextest's log names only tests past a minute. So each suite job prints, from
the report it already writes, in the log where an agent reads it:

- its slowest tests (the count is the step's argument; ten when none is given);
- the shard's accounting: the `gpu` test group against the rest, each as tests
  and test-seconds, the group's seconds over its cap (`max-threads`) and the
  rest's over the runner's cores, beside the shard's wall time. Whichever share
  is near the wall paces the shard: a GPU-bound one gets shorter with a higher
  cap, a core-bound one does not;
- the GPU tests' median and p10 (the floor one headless renderer costs), and
  test-seconds per test binary.

Which tests are GPU tests is the `gpu` group's own filter, read from
`.config/nextest.toml` with its cap, so the two never drift: the filter's regexes
are taken from the file, and a filter of another shape prints that it cannot be
read (and fails this script's test) instead of a wrong split.

Usage: `slowest_tests.py <junit.xml> [count]`. A missing or unreadable report
prints why and exits 0: this step informs, it never gates (the suite's own step
does, and the workflow step is `continue-on-error` besides). A suite cut off by
the job's timeout leaves no report, so the step has no timings to print then.
"""

from __future__ import annotations

import os
import re
import sys
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

NEXTEST = Path(__file__).resolve().parents[2] / ".config" / "nextest.toml"

#: The shape of the `gpu` group's filter: integration tests (`kind(test)`) whose
#: path matches the first regex, and any test whose name matches the second.
FILTER_SHAPE = re.compile(r"\(kind\(test\) & test\(/(?P<integration>[^/]+)/\)\) \| test\(/(?P<any>[^/]+)/\)")


@dataclass(frozen=True)
class Case:
    binary: str  # nextest's binary id, the JUnit `classname`: `crate`, `crate::name`, `crate::bin/name`
    name: str
    seconds: float


def cases(xml: str) -> tuple[list[Case], float | None]:
    """Every timed `testcase`, and the run's wall time (the root's `time`), if any."""
    root = ET.fromstring(xml)
    found = []
    for case in root.iter("testcase"):
        try:
            seconds = float(case.get("time", ""))
        except ValueError:
            continue
        found.append(Case(case.get("classname", ""), case.get("name", ""), seconds))
    try:
        wall = float(root.get("time", ""))
    except ValueError:
        wall = None
    return found, wall


def slowest(xml: str, count: int = 10) -> list[tuple[float, str]]:
    """The `count` longest `testcase`s, as (seconds, `classname name`), longest first."""
    rows = [(c.seconds, f"{c.binary} {c.name}".strip()) for c in cases(xml)[0]]
    return sorted(rows, key=lambda c: (-c[0], c[1]))[:count]


def is_integration(binary: str) -> bool:
    """nextest's `kind(test)`: an integration test binary, `crate::name`, never
    `crate::bin/name` or `crate::example/name` (a library's id is the bare crate)."""
    return "::" in binary and "/" not in binary.split("::", 1)[1]


def gpu_rule(config: str) -> tuple[Callable[[Case], bool], int]:
    """The `gpu` group's membership test and cap, from nextest.toml's text.
    Raises `ValueError` naming what it could not read."""
    import tomllib  # 3.11+, as CI's runners have; imported here so the top N still prints without it

    data = tomllib.loads(config)
    try:
        cap = int(data["test-groups"]["gpu"]["max-threads"])
    except (KeyError, TypeError, ValueError) as err:
        raise ValueError(f"no `[test-groups] gpu.max-threads` ({err})") from None
    filters = [o.get("filter", "") for o in data.get("profile", {}).get("default", {}).get("overrides", [])
               if o.get("test-group") == "gpu"]
    if len(filters) != 1:
        raise ValueError(f"{len(filters)} overrides put tests in the `gpu` group, expected one")
    shape = FILTER_SHAPE.fullmatch(filters[0].strip())
    if not shape:
        raise ValueError(f"the `gpu` filter {filters[0]!r} is not the shape this script reads; "
                         "update `FILTER_SHAPE` in slowest_tests.py")
    integration, anywhere = re.compile(shape["integration"]), re.compile(shape["any"])
    return (lambda c: bool(anywhere.search(c.name))
            or (is_integration(c.binary) and bool(integration.search(c.name)))), cap


def quantile(sorted_seconds: list[float], q: float) -> float:
    """The nearest-rank `q` quantile of a sorted, non-empty list."""
    return sorted_seconds[min(len(sorted_seconds) - 1, max(0, round(q * (len(sorted_seconds) - 1))))]


def accounting(found: list[Case], wall: float | None, is_gpu: Callable[[Case], bool],
               cap: int, cores: int) -> list[str]:
    """The shard's lines: the group against the rest, the GPU floor, per-binary totals."""
    gpu = sorted(c.seconds for c in found if is_gpu(c))
    rest = [c.seconds for c in found if not is_gpu(c)]
    shown_wall = f"{wall:.0f} s" if wall is not None else "unknown"
    lines = [f"gpu: {len(gpu)} tests, {sum(gpu):.0f} s, S/cap {cap} = {sum(gpu) / cap:.0f} s; "
             f"rest: {len(rest)} tests, {sum(rest):.0f} s, R/cores {cores} = {sum(rest) / cores:.0f} s; "
             f"shard wall {shown_wall}"]
    if gpu:
        lines.append(f"gpu tests: median {quantile(gpu, 0.5):.3f} s, p10 {quantile(gpu, 0.1):.3f} s")
    per_binary: dict[str, list[float]] = {}
    for c in found:
        per_binary.setdefault(c.binary, []).append(c.seconds)
    lines.append("test-seconds per binary:")
    for binary, seconds in sorted(per_binary.items(), key=lambda b: (-sum(b[1]), b[0])):
        lines.append(f"{sum(seconds):9.1f}s  {len(seconds):5} tests  {binary}")
    return lines


def main(argv: list[str], config: Path = NEXTEST, cores: int | None = None) -> int:
    path, count = argv[1], int(argv[2]) if len(argv) > 2 else 10
    try:
        with open(path, encoding="utf-8") as report:
            xml = report.read()
        rows = slowest(xml, count)
    except (OSError, ET.ParseError) as err:
        print(f"no timings: {path}: {err}")
        return 0
    print(f"the {len(rows)} slowest tests in {path}:")
    for seconds, name in rows:
        print(f"{seconds:9.3f}s  {name}")
    try:
        is_gpu, cap = gpu_rule(config.read_text(encoding="utf-8"))
    except (OSError, ImportError, ValueError) as err:
        print(f"no group accounting: {config}: {err}")
        return 0
    found, wall = cases(xml)
    print(*accounting(found, wall, is_gpu, cap, cores or os.cpu_count() or 1), sep="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
