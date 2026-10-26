"""The supervisor's claim on its worktree: a `git worktree lock` naming its PID (#1450).

On 2026-10-25 `make reclaim` removed the supervisor's worktree while it slept
its "nothing left" pause: a detached HEAD on `origin/main` reads as finished,
and nothing stood in it while it waited. Its next `git` call raised
`FileNotFoundError` and the traceback killed it. `reclaim` protects a locked
worktree, so the supervisor locks its own for as long as it lives, with a
reason `reclaim` can read back ([`pid_of`]): a lock whose PID is no longer
alive is reported as stale, never removed silently.
"""

from __future__ import annotations

import os
import re
from pathlib import Path

from merging.queue import gitio


# How the lock's reason begins; the PID follows.
REASON = "merge-queue supervisor pid"

_PID = re.compile(rf"^{re.escape(REASON)} (\d+)$")


def reason(pid: int) -> str:
    return f"{REASON} {pid}"


def pid_of(locked: str | None) -> int | None:
    """The supervisor PID a worktree lock's reason names, or `None` for any other lock."""
    found = _PID.match((locked or "").strip())
    return int(found.group(1)) if found else None


def alive(pid: int) -> bool:
    """Whether a process with this PID exists (one owned by another user counts)."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def current(root: str) -> str | None:
    """The lock reason on the worktree `root` stands in (`""` for none given), or
    `None` when it is not locked."""
    top = gitio.git("rev-parse", "--show-toplevel", cwd=root).stdout.strip()
    listing = gitio.git("worktree", "list", "--porcelain", cwd=root).stdout
    for block in listing.strip().split("\n\n"):
        fields = dict(line.partition(" ")[::2] for line in block.splitlines() if line)
        if top and Path(fields.get("worktree", "")).resolve() == Path(top).resolve():
            return fields.get("locked")
    return None


def claim(root: str, pid: int) -> str | None:
    """Lock `root`'s worktree for supervisor `pid`; what to report when it could not.

    Called holding the supervisor's flock, so a lock another supervisor left is
    a dead one's (or this PID's own, after it re-executed itself) and is
    replaced. Any other lock is left as it is: it protects the worktree too.
    """
    held = current(root)
    if pid_of(held) is not None:
        gitio.git("worktree", "unlock", root, cwd=root)
    elif held is not None:
        return f"supervisor: {root} is already locked ({held or 'no reason given'}); leaving that lock."
    done = gitio.git("worktree", "lock", "--reason", reason(pid), root, cwd=root)
    if done.returncode != 0:
        return f"supervisor: could not lock {root} against `make reclaim`: {done.stderr.strip()}"
    return None


def release(root: str, pid: int) -> None:
    """Unlock `root`'s worktree if this supervisor's lock is still on it."""
    if os.path.isdir(root) and pid_of(current(root)) == pid:
        gitio.git("worktree", "unlock", root, cwd=root)
