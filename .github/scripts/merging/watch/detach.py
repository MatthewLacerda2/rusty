"""Starting a supervisor: no stale stop request, and a start that is confirmed (#1450).

On 2026-10-26 a `make queue-stop` written for a supervisor that had already
died (its worktree reclaimed) outlived it. The next `make queue-daemon`
started a fresh supervisor, which consumed the stale file on its first check
and exited, while `queue-daemon` had already printed "supervisor started": the
orchestrator believed a queue ran for about 20 minutes while none did.

So a supervisor that holds its lock clears a stop request no live watch could
be the target of ([`clear_stale_stop`]), and `--detach` reports "started" only
once its child still holds the supervisor lock a few seconds in ([`confirm`]).
"""

from __future__ import annotations

import subprocess
from pathlib import Path

from merging.watch import ending, lock


# How long `--detach` watches its child before reporting it started: long
# enough for a stale stop, a refused checkout or a held lock to end it.
CONFIRM_SECONDS = 5

# How many of the log's last lines a failed start prints.
TAIL_LINES = 20

CLEARED = "queue-daemon: removed a stop request left for a supervisor that is gone."


def clear_stale_stop(common: Path) -> str | None:
    """Remove a leftover stop request, and say so; `None` when there was none to remove.

    Called holding the supervisor lock, so no other supervisor is alive to obey
    it. A watch run bare (`make queue ARGS=--watch`) reads the same file, so it
    stays while anything holds the watch's lock.
    """
    stop = common / ending.STOP_FILE
    if not stop.exists() or lock.holder(common / lock.LOCK) is not None:
        return None
    stop.unlink(missing_ok=True)
    return CLEARED


def tail(log: Path, lines: int = TAIL_LINES) -> list[str]:
    try:
        return log.read_text(errors="replace").splitlines()[-lines:]
    except OSError:
        return []


def confirm(child: subprocess.Popen, held: Path, log: Path, seconds: float = CONFIRM_SECONDS) -> str | None:
    """Why the detached supervisor `child` is not running after `seconds`, or
    `None` when it still runs and holds the lock at `held`."""
    try:
        status = child.wait(timeout=seconds)
    except subprocess.TimeoutExpired:
        status = None
    if status is None and lock.holder(held) == str(child.pid):
        return None
    how = f"exited {status}" if status is not None else f"does not hold {held}"
    return "\n".join([f"queue-daemon: the supervisor (PID {child.pid}) {how}; the end of {log}:", *tail(log)])
