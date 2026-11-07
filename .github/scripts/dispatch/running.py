"""How many cloud coders are running, and whether one more may start (#1856).

The operator set the limit on 2026-11-05: **at most [`LIMIT`] cloud coders at
once** (CLAUDE.md, *Coding parallelises; merging does not*). Review, not coder
count, was a batch's bottleneck, so a sixth coder only queues more diffs for
one orchestrator to read.

**What the routines API tells about a one-off routine**, read on 2026-11-07
from the batch's own routines:

- not fired yet: `enabled` true, `ended_reason` empty, no `last_run`;
- fired, its session still working: `last_run.status` is
  `ROUTINE_RUN_STATUS_PENDING`, with a `fired_at` and no `finished_at`;
- fired, its session ended: `last_run.status` is `ROUTINE_RUN_STATUS_SUCCEEDED`
  (or a failure), with a `finished_at`. A coder ends when its pull request is
  ready, by the standing rules (`cloud-brief.md`), so this one no longer counts.

A fired routine the list shows **without** a `last_run` cannot be placed; it
is named as unknown and not counted, rather than guessed. The list holds only
the account's 20 newest routines (`routine.ENVIRONMENT_FROM`), so a running
coder pushed off it by 20 newer ones is not seen either. A run never marked
finished would count forever: the refusal names every counted routine and
when it fired, so a stale one is plain to see, and `--over-limit` launches
anyway.

A re-brief is not one more coder: the issue being launched is left out of
the count, so its own earlier routine never blocks it.
"""

from __future__ import annotations

import re

from dispatch import routine

#: The most cloud coders at once (the operator, 2026-11-05).
LIMIT = 5

RUNNING = {"ROUTINE_RUN_STATUS_PENDING", "ROUTINE_RUN_STATUS_RUNNING"}

ISSUE = re.compile(re.escape(routine.NAME_PREFIX) + r"(\d+)\b")


def issue(found: dict) -> int | None:
    """The issue a coder routine is for, from its name; `None` for any other routine."""
    match = ISSUE.match(str(found.get("name", "")))
    return int(match.group(1)) if match else None


def state(found: dict) -> str:
    """`waiting` (not fired), `running`, `ended`, or `unknown` (fired, no run shown)."""
    run = found.get("last_run")
    if isinstance(run, dict) and run:
        return "running" if run.get("status") in RUNNING and not run.get("finished_at") else "ended"
    if found.get("last_fired_at") or found.get("ended_reason"):
        return "unknown"
    return "waiting" if found.get("enabled") else "ended"


def coders(routines: list[dict], launching: int) -> tuple[list[dict], list[dict]]:
    """The coder routines that count (waiting or running), and the ones that cannot be placed.

    Routines for `launching` are left out: launching it again is a re-brief.
    """
    counted, unknown = [], []
    for found in routines:
        number = issue(found)
        if number is None or number == launching:
            continue
        placed = state(found)
        if placed in ("waiting", "running"):
            counted.append(found)
        elif placed == "unknown":
            unknown.append(found)
    return counted, unknown


def describe(found: dict) -> str:
    run = found.get("last_run") or {}
    when = f"fired {run['fired_at'][:16]}Z" if run.get("fired_at") else f"fires {found.get('run_once_at', '?')}"
    return f"{found.get('name', '?')} ({when})"
