"""`merging.view`: the orchestrator's read of the queue, `make queue-status` and `make queue-events` (#1876).

The orchestrator used to watch the queue with a `Monitor` command it typed by
hand on every re-arm, and on 2026-11-07 the hand-typed form failed four ways:
a `tail -n 0` that dropped what the log said between two watches, a combined
command that replayed old lines and delivered no live one (`#1873: merged`
unreported for 20 minutes), no event for a coder's ready flip (PR #1870 sat
ready and unread for 90 minutes), and a `pgrep -f queue-daemon` that matched
the checking shell's own command line after a reboot, so a dead supervisor was
reported running for about an hour. A watch that is a sentence is retyped
wrong; these are the two fixed commands instead:

- [`status`]: whether a supervisor runs, read from its worktree lock's PID
  (`watch.tenancy.pid_of`) and checked alive, never by matching a pattern
  against every command line; and the log's last line, with its age.
- [`events`]: the log from 30 lines back, filtered to what the orchestrator
  acts on, plus coders' *Standing down* comments ([`standing`]), and an exit
  with status 1 the moment the supervisor is not running, so a dead queue is
  an event and not a silence.

Nothing here is loaded by the watch, so a change to it never relaunches one
(`watch.freshness` tracks only the modules the watch imported).
"""
