#!/usr/bin/env python3
"""`conflicts.py SINCE` — the conflicts with `main` the queue met since then (#1723).

Reads the record [`merging.queue.conflict.record`] keeps under the common git
directory, which every worktree shares, and prints its totals: how many
conflicts, how many the sorted-list rule resolved, and what share of the
conflicted files it could have. SINCE is an age (`12h`, `3d`) or an ISO date or
time. A report, not a gate: it exits 0 whatever it finds.

    make conflicts SINCE=24h
"""

from __future__ import annotations

import sys
from datetime import datetime, timezone
from merging.queue.conflict import record

USAGE = "usage: conflicts.py SINCE   (12h, 3d, 2026-11-02 or 2026-11-02T18:00Z)"


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print(USAGE, file=sys.stderr)
        return 2
    try:
        start = record.when(argv[0], datetime.now(timezone.utc))
    except ValueError:
        print(USAGE, file=sys.stderr)
        return 2
    for line in record.totals(record.since(record.read(record.path(".")), start)):
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
