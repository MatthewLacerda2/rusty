#!/usr/bin/env python3
"""Print the slowest tests of a nextest JUnit report (#1976).

The macOS leg ran past its 30-minute timeout on 2026-11-09, and finding what to
speed up meant per-test timings nobody could read: the job summary
`test-summary/action` writes is not served by the REST API a cloud session reads,
and nextest's log names only tests past a minute. So the job prints the top ten
from the report it already writes, in the log, where an agent reads them.

Usage: `slowest_tests.py <junit.xml> [count]`. A missing or unreadable report
prints why and exits 0: this step informs, it never gates (the suite's own step
does, and the workflow step is `continue-on-error` besides). A suite cut off by
the job's timeout leaves no report, so the step has no timings to print then.
"""

from __future__ import annotations

import sys
import xml.etree.ElementTree as ET


def slowest(xml: str, count: int = 10) -> list[tuple[float, str]]:
    """The `count` longest `testcase`s, as (seconds, `classname name`), longest first."""
    cases = []
    for case in ET.fromstring(xml).iter("testcase"):
        try:
            seconds = float(case.get("time", ""))
        except ValueError:
            continue
        cases.append((seconds, f"{case.get('classname', '')} {case.get('name', '')}".strip()))
    return sorted(cases, key=lambda c: (-c[0], c[1]))[:count]


def main(argv: list[str]) -> int:
    path, count = argv[1], int(argv[2]) if len(argv) > 2 else 10
    try:
        with open(path, encoding="utf-8") as report:
            rows = slowest(report.read(), count)
    except (OSError, ET.ParseError) as err:
        print(f"no timings: {path}: {err}")
        return 0
    print(f"the {len(rows)} slowest tests in {path}:")
    for seconds, name in rows:
        print(f"{seconds:9.3f}s  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
