"""One watch at a time: an `fcntl.flock` held for the life of the process (#928).

In scorsese an orphaned watch and a new one ran together for 50 minutes
(MatthewLacerda2/scorsese#1346), each taking pull requests the other was
rebasing. The lock lives in the checkout's git directory, which every worktree
shares, so a watch started from any worktree sees it. The kernel drops it when
the process dies, however it dies: nothing to clean up after a crash.
"""

from __future__ import annotations

import fcntl
import os
from pathlib import Path


# The file, under the checkout's common git directory.
LOCK = "merge-queue.lock"

# The exit status of a watch that found another holding the lock: nothing
# happened, and relaunching will not help while the other runs.
LOCKED_STATUS = 4


def hold(path: Path):
    """`(handle, None)` with the lock held while `handle` stays open, or
    `(None, pid)` naming the process that holds it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = os.fdopen(os.open(path, os.O_RDWR | os.O_CREAT, 0o644), "r+")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        holder = handle.read().strip() or "unknown"
        handle.close()
        return None, holder
    handle.seek(0)
    handle.truncate()
    handle.write(f"{os.getpid()}\n")
    handle.flush()
    return handle, None


def holder(path: Path) -> str | None:
    """The PID written in the lock at `path` while a live process holds it, or
    `None` when nothing does. Reads only: it never writes the file (#1450)."""
    try:
        handle = open(path)
    except FileNotFoundError:
        return None
    with handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_SH | fcntl.LOCK_NB)
        except BlockingIOError:
            return handle.read().strip() or "unknown"
        fcntl.flock(handle, fcntl.LOCK_UN)
    return None


def written(path: Path) -> str | None:
    """The PID last written into the lock at `path`, without taking it: a probe
    that flocks, even shared, can make a holder's non-blocking take fail
    (#1450). Says who took it last, not that it still holds it."""
    try:
        return path.read_text().strip() or None
    except FileNotFoundError:
        return None


def refused(path: Path, pid: str) -> str:
    return f"another watch holds {path} (PID {pid}); one watch at a time. Stop it first, or leave it running."
