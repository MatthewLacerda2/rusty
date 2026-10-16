"""Why the watch exits, and with which status (#751, #697)."""

from __future__ import annotations

import shlex

from merging.watch import board


# How long nothing moving among the drafts reads as "nothing left". A coder
# pushes often (cloud-brief.md), but a cold build and `make gates` can run an
# hour between pushes; past this the orchestrator is better woken than not.
IDLE_MINUTES = 90

# Consecutive listings GitHub may fail before the watch stops as the machine's
# problem rather than retrying forever. At the default poll, five minutes.
FAILS = 10

# How the `--for` exit begins, so the starter can tell it from "nothing left".
DEADLINE = "deadline reached"

# What a background command survives: the harness kills it at two hours,
# wherever it is (#697). `--for` plus the per-PR `--deadline` must fit in it.
BACKGROUND_MINUTES = 120

# How the exit begins when the queue's own code changed on `main` (#967):
# update the checkout to `origin/main`, then start the watch again.
RELAUNCH = "relaunch"

# How the last line begins: the command that starts this watch again (#928).
RESUME = "watch: resume:"

# How each hand-back's line begins, the moment it happens: grep for this.
HANDED_BACK_LINE = "HANDED BACK"

# The exit statuses (#751, after scorsese#615), the most urgent one wins:
# 1 says read the hand-back lines; 3 says the machine or GitHub failed, and
# nothing is known to be wrong with any branch. 0 is everything else, the
# `--for` deadline included (relaunch it, nothing to read).
HANDED_BACK_STATUS = 1

MACHINE_STATUS = 3

# The queue's code changed on `main` ([`RELAUNCH`]): relaunch from the new code.
RELAUNCH_STATUS = 5

# Somebody asked the watch to stop ([`STOP_FILE`], #908): do not relaunch it.
STOPPED_STATUS = 6

# The file that asks it, under the checkout's common git directory: `make
# queue-stop` writes it, the watch and its supervisor remove it as they obey.
STOP_FILE = "merge-queue/stop"

# How the exit begins when it was asked.
STOP = "stop requested"


def finished(pulls: list[dict], quiet: float, idle: float) -> str | None:
    """Why to exit with nothing to take, or `None` to keep watching.

    Called only when [`order.pick`] found nothing. `quiet` is how long, in seconds,
    no [`board.heads`] have changed.
    """
    if not pulls:
        return "no open pull request is left."
    if quiet >= idle:
        return (
            f"nothing is ready and labelled `{board.QUEUE_LABEL}`, and none of the {len(pulls)} open pull request(s) has moved"
            f" in {idle / 60:.0f} minutes: " + ", ".join(f"#{p.get('number')}" for p in pulls) + "."
        )
    return None


def overdue(started: float, now: float, minutes: float | None) -> bool:
    """Whether `--for` has run out. Asked only before taking a pull request."""
    return minutes is not None and now - started >= minutes * 60


def deadline_reached(results: list[tuple[int, str, str]], minutes: float, merged: str) -> str:
    count = sum(1 for _, state, _ in results if state == merged)
    return f"{DEADLINE} ({minutes:g} minutes); {count} merged, nothing in hand. Relaunch the watch."


def status(handed: list[int], machine: bool, relaunch: bool = False, stopped: bool = False) -> int:
    """The watch's exit status: the most urgent thing its starter has to do.
    A relaunch outranks a hand-back: its lines are already printed, and the
    next watch must not run the old code. A requested stop outranks both:
    nobody is to start it again."""
    if stopped:
        return STOPPED_STATUS
    if machine:
        return MACHINE_STATUS
    if relaunch:
        return RELAUNCH_STATUS
    return HANDED_BACK_STATUS if handed else 0


def relaunch(paths: list[str], results: list[tuple[int, str, str]], merged: str) -> str:
    """Why the watch ends when its own code changed on `main` (#967)."""
    count = sum(1 for _, state, _ in results if state == merged)
    return (
        f"{RELAUNCH}: the queue's code changed on main ({', '.join(paths)}); {count} merged, nothing in hand."
        " Update this checkout to origin/main and start the watch again."
    )


def handed_back(number: int, why: str) -> str:
    """The line a hand-back prints as it happens, beginning [`HANDED_BACK_LINE`]."""
    return f"{HANDED_BACK_LINE} #{number}: {why} Passed over until its head moves; the watch carries on."


def over_budget(for_minutes: float | None, deadline: float) -> str | None:
    """Why `--for` and `--deadline` cannot both fit in a background command's
    life ([`BACKGROUND_MINUTES`]), or `None`. The take in hand at the deadline
    can run a whole `--deadline` past it, so the two add up."""
    if for_minutes is None or for_minutes + deadline <= BACKGROUND_MINUTES:
        return None
    return (
        f"--for {for_minutes:g} plus --deadline {deadline:g} is {for_minutes + deadline:g} minutes, past the"
        f" {BACKGROUND_MINUTES} a background command survives; the take in hand could be killed mid-push."
        f" Lower one, e.g. --for {max(BACKGROUND_MINUTES - deadline, 0):g}."
    )


def resume(cwd: str, script: str, argv: list[str]) -> str:
    """The last line: [`RESUME`] and the exact command that starts this watch again."""
    return f"{RESUME} cd {shlex.quote(cwd)} && {shlex.join(['python3', script, *argv])}"
