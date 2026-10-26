"""`queue-daemon.py` — keep the watch running across its planned exits (#908).

The 2026-10-05/06 batch relaunched the watch about twelve times by hand: every
`--for` deadline, every "nothing left" between waves, every merge of the
queue's own code, every network death. Each was an orchestrator turn, and a
forgotten one stalls every ready pull request silently. One relaunch went
wrong too: `pkill -f "merge-queue.py --watch"` killed scorsese's watch as well.

So a supervisor, detached from whoever started it (a Claude background command
dies at two hours), runs the watch, reads how it ended, and decides
([`next_step`]):

- **deadline reached** or a **hand-back** (1): start it again at once; the
  hand-back lines are already in the log.
- **nothing left**: start it again after [`PAUSE_SECONDS`] — a wave of coders
  launching leaves no pull request open for a while.
- **relaunch** (5, #967): the queue's code changed on `main`; start it again.
- **the machine or GitHub failed** (3): back off ([`BACKOFF`]) and retry, a
  bounded number of times in a row, then give up.
- **stop requested** (6) or **another watch holds the lock** (4): end.

It ends with its own statuses too ([`supervise`], #1450): **its worktree is
gone** (7), say `make reclaim` removed it mid-wait; or **it crashed** (8), any
exception the loop did not expect, logged as one `supervisor: crashed:` line
instead of a traceback. Both lines end `stopped.`, the word the orchestrator's
`Monitor` greps for.

Before every launch it moves its checkout to `origin/main` (refusing one with
uncommitted changes), so the watch always runs `main`'s code; when the
supervisor's own code changed with it, it re-executes itself first. A graceful
stop is a file ([`ending.STOP_FILE`], `make queue-stop`), never a kill by pattern:
the watch obeys it between takes, the supervisor between watches.
"""

from __future__ import annotations

from merging.watch import ending, lock


# How long to wait after "nothing left" before looking again.
PAUSE_SECONDS = 10 * 60

# The waits before each retry after the machine or GitHub failed, in a row.
BACKOFF = (60, 120, 240, 480, 960)

# How often a wait looks for the stop file.
STEP_SECONDS = 5

# The supervisor's own lock, beside the watch's: one supervisor per checkout family.
LOCK = "merge-queue-supervisor.lock"

# Its log, under the common git directory.
LOG = "merge-queue/queue.log"

AGAIN, GIVE_UP = "again", "give up"

# Its own exit statuses, past the watch's (#1450): the worktree it runs in was
# removed, or something it did not expect was raised.
GONE_STATUS = 7

CRASHED_STATUS = 8


def next_step(status: int, ended: str, failures: int) -> tuple[str, float, str]:
    """What to do after a watch exited with `status`, its `watch ended:` text
    `ended`, and `failures` machine failures in a row before this one:
    ([`AGAIN`] after so many seconds, or [`GIVE_UP`]), and why."""
    if status == ending.STOPPED_STATUS:
        return GIVE_UP, 0, "a stop was requested"
    if status == lock.LOCKED_STATUS:
        return GIVE_UP, 0, "another watch holds the lock"
    if status == ending.MACHINE_STATUS:
        if failures >= len(BACKOFF):
            return GIVE_UP, 0, f"the machine or GitHub failed {failures + 1} times in a row"
        return AGAIN, BACKOFF[failures], f"the machine or GitHub failed; retry {failures + 1} of {len(BACKOFF)}"
    if status == ending.RELAUNCH_STATUS:
        return AGAIN, 0, "the queue's code changed on main"
    if status in (0, ending.HANDED_BACK_STATUS):
        if ended.startswith(ending.DEADLINE):
            return AGAIN, 0, "its deadline"
        return AGAIN, PAUSE_SECONDS, "nothing left for now"
    return GIVE_UP, 0, f"an exit status it does not know ({status})"


def run(fx) -> int:
    """Supervise until told to stop or there is nothing sensible to retry.

    `fx` carries the effects: `gone()` (the worktree's path once it no longer
    exists, else `None`), `stop_requested()` (and removes the file),
    `update()` (the checkout onto `origin/main`; why not, or `None`),
    `restart_needed()` (the supervisor's own code changed), `restart()`
    (re-executes; does not return), `watch()` (one watch, its output passed
    through; `(status, ended)`), `sleep(seconds)` and `say(line)`.
    """
    failures = 0
    while True:
        if fx.gone():
            return _gone(fx)
        if fx.stop_requested():
            fx.say("supervisor: a stop was requested; not starting the watch again.")
            return ending.STOPPED_STATUS
        refused = fx.update()
        if refused:
            fx.say(f"supervisor: {refused}")
            return 1
        if fx.restart_needed():
            fx.say("supervisor: its own code changed on main; restarting itself.")
            fx.restart()
        fx.say("supervisor: starting the watch.")
        status, ended = fx.watch()
        step, delay, why = next_step(status, ended, failures)
        failures = failures + 1 if status == ending.MACHINE_STATUS else 0
        if step == GIVE_UP:
            fx.say(f"supervisor: the watch exited {status}: {why}; stopped.")
            return status
        fx.say(f"supervisor: the watch exited {status}: {why}; starting it again in {delay / 60:g} minute(s).")
        waited = 0.0
        while waited < delay:
            if fx.gone():
                return _gone(fx)
            if fx.stop_requested():
                fx.say("supervisor: a stop was requested; not starting the watch again.")
                return ending.STOPPED_STATUS
            fx.sleep(min(STEP_SECONDS, delay - waited))
            waited += STEP_SECONDS


def _gone(fx) -> int:
    fx.say(f"supervisor: worktree {fx.gone()} is gone; stopped.")
    return GONE_STATUS


def supervise(fx) -> int:
    """[`run`], ending with a line and a status rather than a traceback: a git
    call in a worktree removed between two checks raises `FileNotFoundError`."""
    try:
        return run(fx)
    except Exception as err:  # noqa: BLE001 -- the point: one line, not a traceback
        if fx.gone():
            return _gone(fx)
        fx.say(f"supervisor: crashed: {type(err).__name__}: {err}; stopped.")
        return CRASHED_STATUS
