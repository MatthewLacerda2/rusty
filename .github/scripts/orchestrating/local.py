"""The local supervisor's side of the lock (#2667).

`make queue-daemon` takes the orchestrator lock after its own file lock and
before its first watch, renews it in the background for its whole life
(`orchestrating/keeper.py`) and releases it on its way out, so a cloud fire
never merges while it runs, and it never starts while a cloud fire holds the
lock. Its holder is `local@<host> queue-daemon`, with no PID: the supervisor's
file lock already keeps it to one per machine, so a supervisor that restarts
or crashed takes its own lease back at once instead of waiting it out.

- **Refused**: one `supervisor: … stopped.` line naming the holder, exit
  [`lease.HELD_STATUS`] (4, as another watch holding the file lock).
- **GitHub unreachable at start**: `supervisor: … stopped.`, exit 3, as the
  watch's machine failure: a supervisor that cannot see the lock does not merge.
- **Lost while running** (a cloud fire took it as stale after a renewal
  failed for `lease.STALE`): the watch is asked to stop between pull requests
  (`ending.STOP_FILE`), and the supervisor ends with 6.
"""

from __future__ import annotations

import contextlib
from pathlib import Path

from merging.watch import ending
from orchestrating import keeper, lease, side

HOLDER = "queue-daemon"


def me() -> str:
    return lease.holder(lease.local_place(), HOLDER)


@contextlib.contextmanager
def held(root: str, common: Path, say):
    """Yield `None` while the lease is held, or the status to exit with when it is not."""
    mine = side.Side(side.git_in(root), me(), common / "merge-queue")
    try:
        refused, restored = mine.take()
    except (lease.Unreachable, OSError) as failed:
        say(f"supervisor: the orchestrator lock could not be taken ({failed}); stopped.")
        yield ending.MACHINE_STATUS
        return
    if refused:
        say(f"supervisor: {refused}; stopped.")
        yield lease.HELD_STATUS
        return
    if restored:
        say(f"queue-daemon: took the orchestrator lock and restored {', '.join(restored)} from it.")

    def lost(why: str) -> None:
        say(f"supervisor: {why}; asking the watch to stop between pull requests.")
        (common / ending.STOP_FILE).parent.mkdir(parents=True, exist_ok=True)
        (common / ending.STOP_FILE).touch()
    try:
        with keeper.Keeper(mine.renew, lost, say):
            yield None
    finally:
        # Released only once the keeper's thread has ended: a renewal still in
        # flight would push on the same tip and leave the release refused.
        try:
            why = mine.release()
        except (lease.Unreachable, OSError) as failed:
            why = f"the orchestrator lock was not released ({failed}); it goes stale in {lease.STALE}"
        say(f"queue-daemon: {why or 'released the orchestrator lock.'}")
