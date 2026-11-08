#!/usr/bin/env python3
"""`queue-view.py status|events` — the orchestrator's read of the merge queue (#1876).

Run it:

    make queue-status   # does a supervisor run (its worktree lock's PID, alive), and the log's last line
    make queue-events   # the stream a `Monitor` runs; exits 1 when the supervisor is not running

Why each is shaped as it is: `merging/view/`.
"""

from merging.view import cli

if __name__ == "__main__":
    raise SystemExit(cli.main())
