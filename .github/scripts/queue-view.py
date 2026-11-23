#!/usr/bin/env python3
"""`queue-view.py status|events|quiet` — the orchestrator's read of the merge queue (#1876, #2628).

Run it:

    make queue-status   # does a supervisor run (its worktree lock's PID, alive), and the log's last line
    make queue-events   # the stream a `Monitor` runs; exits 1 when the supervisor is not running
    make queue-quiet    # the same stream, its routine handled (reclaim, the drain), only what needs someone said

Why each is shaped as it is: `merging/view/`.
"""

from merging.view import cli

if __name__ == "__main__":
    raise SystemExit(cli.main())
