"""What [`watch.loop.run`] does to the world, bound to this repository ([`effects`])."""

from __future__ import annotations

import argparse
import functools
import time
from pathlib import Path

from merging.queue import gitio, outcomes, pulls, stopping, taking
from merging.watch import board, handbacks, mainline, order


def merge_tree_git(root: str, *args: str) -> tuple[int, str]:
    """[`watch.order.clashing`]'s `git`: status and output, `(-1, "")` when git
    could not run at all (a pair it cannot answer fails open)."""
    try:
        done = gitio.git(*args, cwd=root)
    except OSError:
        return -1, ""
    return done.returncode, done.stdout


def effects(repo: str, opts: argparse.Namespace) -> argparse.Namespace:
    """What [`watch.loop.run`] does to the world, bound to this repository."""
    common = gitio.git("rev-parse", "--git-common-dir", cwd=opts.root).stdout.strip() or ".git"
    return argparse.Namespace(
        pulls=lambda: pulls.quietly(
            "pr", "list", "--state", "open", "--limit", "200", "--json", board.LIST_FIELDS
        ),
        issue_labels=pulls.issue_labels,
        clashes=functools.partial(
            order.clashing, git=functools.partial(merge_tree_git, opts.root), known={}, bots=pulls.BOTS
        ),
        main_conflicts=functools.partial(
            mainline.conflicts, git=functools.partial(merge_tree_git, opts.root), known={}, bots=pulls.BOTS
        ),
        turn=lambda number, tick: taking.unready(
            stopping.drain([number], lambda n: taking.take(repo, n, opts, tick=tick))[0]
        ),
        head=lambda number: (pulls.quietly("pr", "view", str(number), "--json", "headRefOid") or {}).get("headRefOid"),
        attempts=lambda number, sha: pulls.head_runs(repo, sha),
        clock=time.monotonic,
        sleep=time.sleep,
        say=outcomes.say,
        memory=Path(opts.root, common, handbacks.MEMORY),
        bots=pulls.BOTS,
        merged=outcomes.MERGED,
        # A dry run previews every ready one; it hands nothing back for real.
        ends={outcomes.GREEN, outcomes.DRY} | ({outcomes.HANDED_BACK} if opts.dry_run else set()),
        stops={outcomes.STOPPED},
        skips={outcomes.NOT_READY},
    )
