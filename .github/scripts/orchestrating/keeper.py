"""Renew the lease while a long command runs, and say once when it is lost (#2667).

The local supervisor runs its watch for hours, and a cloud fire runs the watch
for most of its hour; a lease renewed only between them would go stale
mid-merge and let the other side in. So a daemon thread calls `renew()` every
`lease.RENEW`: it answers `None` while the lease is still this side's, or why
not, and the first "why not" calls `lost(why)` once (the supervisor asks its
watch to stop between pull requests) and ends the thread. A renewal that could
not reach GitHub is retried on the next beat: the lease goes stale only after
`lease.STALE`, several beats later.
"""

from __future__ import annotations

import threading

from orchestrating import lease


class Keeper:
    def __init__(self, renew, lost, say, every: float = lease.RENEW.total_seconds()):
        self.renew, self.lost, self.say, self.every = renew, lost, say, every
        self.stopping = threading.Event()
        self.thread = threading.Thread(target=self.run, name="orchestrator-lease", daemon=True)

    def __enter__(self) -> Keeper:
        self.thread.start()
        return self

    def __exit__(self, *_) -> None:
        self.stopping.set()
        self.thread.join(timeout=self.every)

    def beat(self) -> bool:
        """One renewal; whether to keep going."""
        try:
            why = self.renew()
        except (lease.Unreachable, OSError) as failed:
            self.say(f"orchestrator: renewing the lock failed ({failed}); trying again in "
                     f"{self.every / 60:g} minute(s)")
            return True
        if why:
            self.lost(why)
            return False
        return True

    def run(self) -> None:
        while not self.stopping.wait(self.every):
            if not self.beat():
                return
