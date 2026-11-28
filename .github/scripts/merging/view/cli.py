"""`queue-view.py`'s command line: `status` (`make queue-status`) and `events` (`make queue-events`)."""

from __future__ import annotations

import argparse
import functools
import os
import time
from datetime import datetime, timezone

import ghrest
from dispatch import launch, remote, waiting
from merging.queue import pulls
from merging.verdict import github
from merging.view import chores, events, quiet, shepherding, standing, status, stranded
from merging.watch import handbacks


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="queue-view.py", description="The orchestrator's read of the merge queue (#1876).")
    parser.add_argument("what", choices=("status", "events", "quiet"),
                        help="status: does a supervisor run, and the log's last line; events: the stream a Monitor runs;"
                             " quiet: the same log, its routine handled and only what needs someone said")
    parser.add_argument("--no-drain", action="store_true", help="quiet: leave the coders' drain pass (`make coders`) to someone else")
    parser.add_argument("--no-shepherd", action="store_true",
                        help="quiet: start no shepherd (`make shepherd`) and act on no review record; say ready lines instead")
    parser.add_argument("--once", action="store_true",
                        help="quiet: one poll and one drain pass, then exit 0, with no supervisor to follow (a cloud fire, #2667)")
    parser.add_argument("--since", metavar="TIME", type=lambda t: datetime.fromisoformat(t.replace("Z", "+00:00")),
                        help="quiet --once: say coders' comments posted since TIME (the last fire's), not since now")
    parser.add_argument("--root", default=".", metavar="DIR", help="any worktree of the checkout (default: here)")
    opts = parser.parse_args(argv)
    root = os.path.abspath(opts.root)
    say = functools.partial(print, flush=True)
    if opts.what == "status":
        code, lines = status.report(root)
        # A ready, labelled, green pull request the watch still passes over (#1951).
        lines += stranded.report(status.common_dir(root) / handbacks.MEMORY, time.time(),
                                 lambda: pulls.open_pulls(ghrest.slug()),
                                 lambda pull: pulls.head_reading(ghrest.slug(), pull, early=True))
        for line in lines:
            say(line)
        return code
    poll = standing.Poll(standing.gh_get, opts.since or datetime.now(timezone.utc))
    if opts.what == "quiet" and opts.once:
        return quiet.once(quiet.Quiet(quiet_effects(root, poll, say, opts.no_drain, opts.no_shepherd)))
    if opts.what == "quiet":
        return quiet.follow(quiet.Quiet(quiet_effects(root, poll, say, opts.no_drain, opts.no_shepherd)), events.Tail(status.log_path(root)),
                            running=lambda: status.read(root), sleep=time.sleep, clock=time.monotonic)
    return events.follow(
        events.Tail(status.log_path(root)),
        running=lambda: status.read(root),
        comments=poll,
        say=say,
        sleep=time.sleep,
        clock=time.monotonic,
    )


def quiet_effects(root: str, poll: standing.Poll, say, no_drain: bool, no_shepherd: bool = False) -> argparse.Namespace:
    """What [`quiet.Quiet`] does and reads, on this machine and GitHub (#2628)."""
    common = status.common_dir(root)
    main = chores.main_checkout(common)
    repo = ghrest.slug()
    # The `Reader`'s own lines (a compare it could not read) reach this output too, so they carry its prefix.
    pulls.reader(repo).say = lambda line: say(f"{quiet.PREFIX} {line}")

    def ready(number: int) -> tuple[dict | None, list[dict] | None]:
        try:
            pull = pulls.look_standing(number)
        except (github.Unreachable, SystemExit):
            return None, None
        return pull, ghrest.pages(standing.gh_get, f"repos/{repo}/issues/{number}/comments")

    def main_runs() -> list[dict] | None:
        listed = standing.gh_get(f"repos/{repo}/actions/runs?branch=main&per_page=50")
        return listed.get("workflow_runs") if isinstance(listed, dict) else None

    def coders() -> list[dict] | None:
        try:
            return launch.routines(remote.call)
        except remote.RemoteError:
            return None

    # With `--no-shepherd` the poll still finds the ready pull requests and says their lines.
    flock = argparse.Namespace(
        shepherd=None if no_shepherd else (
            lambda number: chores.run(["python3", ".github/scripts/shepherd.py", str(number)], main)),
        coder=lambda issue: shepherding.relaunch(lambda command: chores.run(command, main), issue),
        pulls=lambda: ghrest.pages(standing.gh_get, f"repos/{repo}/pulls?state=open"),
        thread=lambda number: ghrest.pages(standing.gh_get, f"repos/{repo}/issues/{number}/comments"),
        coders=coders,
        waiting=lambda: {e["issue"] for e in waiting.load(waiting.path(str(main)))},
        ready=ready,
    )
    return argparse.Namespace(
        shepherd=flock,
        after_merge=lambda number: chores.after_merge(main, number),
        drain=(lambda: []) if no_drain else (lambda: chores.drain(main)),
        ready=ready,
        bugs=lambda: ghrest.pages(standing.gh_get, f"repos/{repo}/issues?state=open&labels=bug"),
        main_runs=main_runs,
        comments=poll,
        say=say,
        path=(common / handbacks.MEMORY).with_name(quiet.STATE),
    )

