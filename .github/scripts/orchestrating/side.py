"""One side of the lock: a holder's name, its git, and the state directory it carries (#2667).

What the cloud fire (`orchestrator.py`) and the local supervisor
(`merging/watch/daemon.py`) both do with the lease, so they do it alike:

- [`Side.take`]: take it, restoring the carried state when the last writer was
  in another place (`carry.restores`) and was not this holder.
- [`Side.renew`]: renew it with the state as it is now; refused once the lease
  is not this holder's any more, so a side that lost it never overwrites the
  new holder's state.
- [`Side.release`]: free it, with the state as it is now.
"""

from __future__ import annotations

import subprocess
from datetime import datetime, timezone
from pathlib import Path

from orchestrating import carry, lease


def git_in(root: str):
    """A `git(*args, stdin=None) -> (status, stdout, stderr)` that runs in `root`."""
    def git(*args: str, stdin: str | None = None) -> tuple[int, str, str]:
        done = subprocess.run(["git", *args], cwd=root, input=stdin, capture_output=True, text=True, check=False)
        return done.returncode, done.stdout, done.stderr
    return git


def describe(found: lease.Lease) -> str:
    if found.holder:
        return f"held by {found.holder}, renewed {lease.stamp(found.at) if found.at else '?'}"
    if found.tip is None:
        return "free (never taken)"
    return f"free since {lease.stamp(found.at) if found.at else '?'} (released by {found.by or '?'})"


class Side:
    def __init__(self, git, me: str, directory: Path, clock=None):
        self.git, self.me, self.directory = git, me, directory
        self.clock = clock or (lambda: datetime.now(timezone.utc))
        self.before: lease.Lease | None = None  # the lease [`take`] found, read by the fire's `last` line

    def take(self) -> tuple[str | None, list[str]]:
        """`(why refused or None, the state files restored)`; raises `lease.Unreachable`."""
        now = self.clock()
        before, after = lease.take(self.git, self.me, now)
        self.before = before
        if after is None:
            return lease.refusal(before, self.me, now) or f"lost the lock to {before.holder}", []
        restored = []
        if before.tip and self.me not in (before.holder, before.by) and carry.restores(
                lease.place_of(before.by), lease.place_of(self.me)):
            restored = carry.restore(self.git, before.tree, self.directory)
        return None, restored

    def renew(self) -> str | None:
        """`None` while the lease is still this holder's, else why not."""
        before, after = lease.take(self.git, self.me, self.clock(), carry.tree(self.git, self.directory),
                                   renewing=True)
        return None if after else f"the orchestrator lock is no longer {self.me}'s: {describe(before)}"

    def release(self) -> str | None:
        return lease.release(self.git, self.me, self.clock(), carry.tree(self.git, self.directory))
