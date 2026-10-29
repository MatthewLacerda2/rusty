"""`merge-queue.py`'s arguments and entry point."""

from __future__ import annotations

import argparse
import os
import sys
from datetime import datetime
from pathlib import Path

import ghrest

from merging.queue import checks, gitio, outcomes, stopping, taking, world
from merging.verdict import github
from merging.watch import ending, lock, loop


# How often GitHub is asked again. A cold run is about ten minutes; a tighter
# poll buys only API calls.
POLL_SECONDS = 30

# The ceiling on one branch, in minutes: well past a cold run (the macOS job
# is the slow one since Windows left the gate, #741), because giving up early
# hands back a branch about to go green. Reaching it is a hand-back, never a merge.
DEADLINE_MINUTES = 45


def since(text: str) -> datetime:
    """`--since`'s time: ISO 8601, a date alone meaning its midnight; local time
    unless it names a zone."""
    try:
        when = datetime.fromisoformat(text)
    except ValueError as bad:
        raise argparse.ArgumentTypeError(f"not an ISO 8601 time: {text!r}") from bad
    return when if when.tzinfo else when.astimezone()


def parse(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="merge-queue.py",
        description=(
            "Merge `main` into each pull request in turn, push it, wait for CI and"
            " squash-merge it. Merging stays serialized; this only does the waiting."
        ),
    )
    parser.add_argument("prs", metavar="PR", type=int, nargs="*", help="pull request numbers, merged in the order given")
    parser.add_argument(
        "--watch", action="store_true",
        help=(
            "name no PR: take each ready one labelled `queue`, fewest conflicts in line first, then label priority; a hand-back is skipped and announced"
            f" ('{ending.HANDED_BACK_LINE} #N'); exit when nothing is left: 1 if anything was handed back,"
            f" {ending.MACHINE_STATUS} if the machine or GitHub failed,"
            f" {lock.LOCKED_STATUS} if another watch holds the lock, {ending.RELAUNCH_STATUS} if its own code changed on main"
            " (update the checkout, then relaunch); the last line is '{ending.RESUME} <command>'"
        ),
    )
    parser.add_argument(
        "--idle", type=float, default=ending.IDLE_MINUTES, metavar="MINUTES",
        help=f"--watch: exit when nothing is ready and no open PR has moved for this long (default {ending.IDLE_MINUTES})",
    )
    parser.add_argument(
        "--for", dest="for_minutes", type=float, metavar="MINUTES",
        help=(
            "--watch: take no new PR once this long has passed; finish the one in hand, then exit cleanly,"
            f" as 'nothing left' does, with a line saying '{ending.DEADLINE}' (relaunch it then)."
            f" With --deadline it must fit in {ending.BACKGROUND_MINUTES} minutes, what a background command survives"
        ),
    )
    parser.add_argument(
        "--since", type=since, metavar="TIME",
        help="--watch: report board changes only for PRs opened at or after TIME (ISO 8601, e.g. 2026-10-16T19:00): the batch's own",
    )
    parser.add_argument("--no-merge", action="store_true", help="stop at green and hand each branch back rather than merging it")
    parser.add_argument(
        "--dry-run", action="store_true",
        help="read everything and merge `main` in locally, but push, comment and merge nothing; judges each head as it is today",
    )
    parser.add_argument(
        "--deadline", type=float, default=DEADLINE_MINUTES, metavar="MINUTES",
        help=f"give up waiting on one branch after this long; Dependabot's rebase and CI share it (default {DEADLINE_MINUTES})",
    )
    parser.add_argument(
        "--poll", type=float, default=POLL_SECONDS, metavar="SECONDS",
        help=f"how often to ask GitHub again (default {POLL_SECONDS})",
    )
    parser.add_argument(
        "--no-check", action="store_true",
        help="push an updated head without compile-checking it first (CI still judges it)",
    )
    parser.add_argument(
        "--target-dir", metavar="DIR",
        help="where the pre-push checks build (default: target/merge-queue under --root; never a worktree's own)",
    )
    parser.add_argument("--root", default=".", metavar="DIR", help="the git checkout to update in (default: the current directory)")
    opts = parser.parse_args(argv)
    if bool(opts.prs) == opts.watch:
        parser.error("name the pull requests, or pass --watch; not both, not neither")
    if opts.for_minutes is not None and not opts.watch:
        parser.error("--for bounds a --watch; named pull requests end on their own")
    if opts.since is not None and not opts.watch:
        parser.error("--since limits a --watch's board; named pull requests have none")
    over = ending.over_budget(opts.for_minutes, opts.deadline)
    if over:
        parser.error(over)
    return opts


def lock_path(root: str) -> Path:
    """[`watch.lock.LOCK`] in the checkout's common git directory, which every worktree shares."""
    common = gitio.git("rev-parse", "--git-common-dir", cwd=root).stdout.strip() or ".git"
    return Path(root, common, lock.LOCK)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    opts = parse(argv)
    if not opts.watch:
        return start(opts, argv)
    held, holder = lock.hold(lock_path(opts.root))
    if held is None:
        outcomes.say(f"refusing to start: {lock.refused(lock_path(opts.root), holder)}")
        return lock.LOCKED_STATUS
    with held:
        return start(opts, argv)


def start(opts: argparse.Namespace, argv: list[str]) -> int:
    """The queue or the watch, once [`main`] holds the watch's lock."""
    if not (opts.no_check or opts.dry_run):
        refused = checks.preflight(checks.target_dir(opts))
        if refused:
            outcomes.say(f"refusing to start: {refused[0]}", *refused[1:])
            return 1
    try:
        repo = ghrest.slug()
    except github.Unreachable as down:
        outcomes.say(*stopping.unreachable(down))
        return ending.MACHINE_STATUS
    if opts.watch:
        results, why, status = loop.run(world.effects(repo, opts), opts)
    else:
        results = stopping.drain(outcomes.ordered(opts.prs), lambda number: taking.take(repo, number, opts))
    print()
    for line in outcomes.summary(results):
        outcomes.say(line)
    if opts.watch:
        outcomes.say(f"watch ended: {why}")
        outcomes.say(ending.resume(os.getcwd(), sys.argv[0], argv))
        return status
    return 0 if all(state in (outcomes.MERGED, outcomes.GREEN, outcomes.DRY) for _, state, _ in results) else 1
