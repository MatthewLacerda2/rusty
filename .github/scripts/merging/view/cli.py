"""`queue-view.py`'s command line: `status` (`make queue-status`) and `events` (`make queue-events`)."""

from __future__ import annotations

import argparse
import functools
import os
import time
from datetime import datetime, timezone

from merging.view import events, standing, status


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="queue-view.py", description="The orchestrator's read of the merge queue (#1876).")
    parser.add_argument("what", choices=("status", "events"),
                        help="status: does a supervisor run, and the log's last line; events: the stream a Monitor runs")
    parser.add_argument("--root", default=".", metavar="DIR", help="any worktree of the checkout (default: here)")
    opts = parser.parse_args(argv)
    root = os.path.abspath(opts.root)
    say = functools.partial(print, flush=True)
    if opts.what == "status":
        code, lines = status.report(root)
        for line in lines:
            say(line)
        return code
    poll = standing.Poll(standing.gh_get, datetime.now(timezone.utc))
    return events.follow(
        events.Tail(status.log_path(root)),
        running=lambda: status.read(root),
        comments=poll,
        say=say,
        sleep=time.sleep,
        clock=time.monotonic,
    )
