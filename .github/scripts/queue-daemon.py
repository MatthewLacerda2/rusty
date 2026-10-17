#!/usr/bin/env python3
"""`queue-daemon.py` — keep the merge-queue watch running across its planned exits (#908).

Run it:

    make queue-daemon ARGS="--no-check --since 2026-10-16T19:00"
    make queue-stop

What it decides and why is in `merging/watch/supervisor.py`; the command line in
`merging/watch/daemon.py`.
"""

from merging.watch import daemon

if __name__ == "__main__":
    raise SystemExit(daemon.main())
