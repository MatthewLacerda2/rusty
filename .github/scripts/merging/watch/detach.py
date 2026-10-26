"""Starting a supervisor: no stale stop request, and a start that is confirmed (#1450).

On 2026-10-26 a `make queue-stop` written for a supervisor that had already
died (its worktree reclaimed) outlived it. The next `make queue-daemon`
started a fresh supervisor, which consumed the stale file on its first check
and exited, while `queue-daemon` had already printed "supervisor started": the
orchestrator believed a queue ran for about 20 minutes while none did.

So a supervisor that holds its lock clears a stop request no live watch could
be the target of ([`clear_stale_stop`]), and `--detach` reports "started" only
once its child still holds the supervisor lock a few seconds in ([`confirm`]).

The review of PR #1451 found two ways those could go wrong. A supervisor whose
own code changed re-executes itself, and its flock does not survive the
`exec`: had the restarted process cleared the stop file too, a `make
queue-stop` written while it updated would be deleted and the queue would keep
merging. So only a fresh start clears one; a restart carries [`RESTARTED`] in
its environment, which nobody types. And a probe that took a shared flock on
the supervisor's lock could make the child's own exclusive, non-blocking take
fail (it would exit 4); [`confirm`] reads the PID the holder wrote instead,
and polls for the whole window rather than looking once at its end.
"""

from __future__ import annotations

import subprocess
import time
from pathlib import Path

from merging.watch import ending, lock


# How long `--detach` watches its child before reporting it started: long
# enough for a stale stop, a refused checkout or a held lock to end it.
CONFIRM_SECONDS = 5

# How many of the log's last lines a failed start prints.
TAIL_LINES = 20

# How often [`confirm`] looks at its child.
POLL_SECONDS = 0.1

# Set in the environment of a supervisor re-executing itself (never typed).
RESTARTED = "MERGE_QUEUE_SUPERVISOR_RESTARTED"

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


def confirm(child: subprocess.Popen, held: Path, log: Path, seconds: float = CONFIRM_SECONDS) -> tuple[int | None, str | None]:
    """Watch the detached supervisor `child` for `seconds`: `(status, why)` when
    it exited, `(None, None)` when it runs and was seen holding the lock at
    `held`, and `(None, note)` when it runs but was never seen holding it (not
    a failure: it may be slow to start). Reads the PID the holder writes, and
    never takes the lock itself."""
    deadline, seen = time.monotonic() + seconds, False
    while True:
        status = child.poll()
        if status is not None:
            return status, "\n".join([f"queue-daemon: the supervisor (PID {child.pid}) exited {status}; the end of {log}:", *tail(log)])
        seen = seen or lock.written(held) == str(child.pid)
        if time.monotonic() >= deadline:
            break
        time.sleep(POLL_SECONDS)
    if seen:
        return None, None
    return None, f"queue-daemon: the supervisor (PID {child.pid}) runs but was not seen holding {held}; unconfirmed, see {log}."
