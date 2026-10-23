"""What [`watch.loop.run`] does to the world, bound to this repository ([`effects`])."""

from __future__ import annotations

import argparse
import functools
import time
from pathlib import Path

from merging.queue import gitio, outcomes, pulls, stopping, taking
from merging.watch import ending, freshness, handbacks, mainline, order


def merge_tree_git(root: str, *args: str) -> tuple[int, str]:
    """[`watch.order.clashing`]'s `git`: status and output, `(-1, "")` when git
    could not run at all (a pair it cannot answer fails open)."""
    try:
        done = gitio.git(*args, cwd=root)
    except OSError:
        return -1, ""
    return done.returncode, done.stdout


def git_at(*args: str, cwd: str) -> tuple[int, str]:
    """[`watch.freshness`]'s `git`, run in `cwd`."""
    return merge_tree_git(cwd, *args)


def consume(path: Path) -> bool:
    """Whether `path` exists, removing it if so: a request obeyed once."""
    try:
        path.unlink()
    except FileNotFoundError:
        return False
    return True


def effects(repo: str, opts: argparse.Namespace) -> argparse.Namespace:
    """What [`watch.loop.run`] does to the world, bound to this repository.
    Called once, at launch: that is when the running code's blobs are noted."""
    code = freshness.launched(freshness.running(), git_at)
    common = gitio.git("rev-parse", "--git-common-dir", cwd=opts.root).stdout.strip() or ".git"
    return argparse.Namespace(
        pulls=functools.partial(pulls.open_pulls, repo),
        issue_labels=pulls.issue_labels,
        clashes=functools.partial(
            order.clashing, git=functools.partial(merge_tree_git, opts.root), known={}, bots=pulls.BOTS
        ),
        main_conflicts=functools.partial(
            mainline.conflicts, git=functools.partial(merge_tree_git, opts.root), known={}, bots=pulls.BOTS
        ),
        stop_requested=lambda: consume(Path(opts.root, common, ending.STOP_FILE)),
        stale_code=lambda: freshness.changed(code, functools.partial(merge_tree_git, opts.root)),
        turn=lambda number, tick: taking.unready(
            stopping.drain([number], lambda n: taking.take(repo, n, opts, tick=tick))[0]
        ),
        head=functools.partial(pulls.head_of, repo),
        attempts=lambda number, sha: pulls.head_runs(repo, sha),
        clock=time.monotonic,
        sleep=time.sleep,
        say=outcomes.say,
        memory=Path(opts.root, common, handbacks.MEMORY),
        bots=pulls.BOTS,
        merged=outcomes.MERGED,
        # A dry run previews every ready one; it hands nothing back for real.
        ends={outcomes.GREEN, outcomes.DRY} | ({outcomes.HANDED_BACK, outcomes.RECLAIM} if opts.dry_run else set()),
        stops={outcomes.STOPPED},
        skips={outcomes.NOT_READY},
    )
