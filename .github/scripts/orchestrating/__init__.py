"""The orchestrator's lock and the state it carries, shared by a cloud fire and this machine (#2667).

- [`lease`]: one orchestrator at a time, a commit on the `orchestrator-lock`
  branch that both sides take and renew by a plain fast-forward push.
- [`carry`]: the merge queue's state files travel in that commit's tree, so a
  cloud session's memory outlives it and the next holder starts from it.
- [`keeper`]: renews the lease in the background while a long command runs,
  and says when another side took it.
- [`schedule`]: the hourly routine `make orchestrator` creates.

`orchestrator.py` is the command line; the local supervisor
(`merging/watch/daemon.py`) takes the same lease.
"""
