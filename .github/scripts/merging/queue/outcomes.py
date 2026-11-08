"""What a turn can end as, and the words the queue prints.

The summary names merged, green and dry apart so the report never claims a merge
it did not make ([`summary`]).
"""

from __future__ import annotations


WAIT, GO, STOP = "wait", "go", "stop"

# The head moved during the CI wait to a sha this queue never pushed (#1799):
# nothing was judged about it, so under `--watch` the take ends [`REQUEUED`]
# and remembers nothing; a named take still hands it back.
HEAD_MOVED = "head moved"

# Red for the runners' reasons only ([`verdict.runners.outage`], #869): re-run it once.
RERUN = "rerun"

# How the hand-back after that one re-run begins: GitHub's failure, not the branch's.
RUNNERS = "runners unavailable"

# What the summary calls each ending. Merged, green and dry are separate so the
# report never claims a merge it did not make.
MERGED, GREEN, DRY, HANDED_BACK = "merged", "green", "dry run", "handed back"

# A hand-back no push fixes ([`verdict.protocol.refusal`], #1279: #1275's fixed
# description sat until the daemon restarted). `AWAITS_EDIT`: what the description
# or labels claim, re-read when they change; `AWAITS_ISSUE`: the issue it closes
# is staged, blocked or closed, re-read when that issue no longer blocks it.
# `AWAITS_REVIEW`: no comment records its review decision ([`verdict.reviewed`],
# #1764), re-read when one appears (or, like an edit, when its labels change).
AWAITS_EDIT, AWAITS_ISSUE = "handed back: description", "handed back: issue"
AWAITS_REVIEW = "handed back: review"
AWAITING = (AWAITS_EDIT, AWAITS_ISSUE, AWAITS_REVIEW)

# A hand-back for outwaiting its runs (#1831): the deadline passed with nothing
# red and nothing green. #1820's and #1827's runs went green after it during
# GitHub's Actions incidents of 2026-11-04 and sat parked, so the watch asks
# again ([`watch.stalls`]) and brings the head back once every run on it is green.
STALLED = "handed back: stalled"

# A hand-back for a red gating run on the head (#1951). #1920 was handed back on
# 2026-11-08 for a `ci` run that failed during GitHub's 504 outage; the run was
# re-run green an hour later and the PR sat passed over for seven more, because
# the only thing that re-admitted it was a change from a run mark GitHub had
# failed to give at the hand-back. So a red hand-back is its own kind: the watch
# re-reads its head's runs and brings it back once they are green
# ([`watch.handbacks.greened`]), and it carries the runs the verdict read red
# ([`Red.mark`]), so the #869 mark never has to come from a later read.
RED = "handed back: red"


class Handed(tuple):
    """A hand-back from the CI wait: `(number, state, why)`, carrying `mark`,
    the gating runs on the head as the verdict read them
    ([`watch.handbacks.attempts`]), so the watch never takes one from a later read."""

    mark: str

    def __new__(cls, number: int, state: str, why: str, mark: str):
        made = super().__new__(cls, (number, state, why))
        made.mark = mark
        return made


def Red(number: int, why: str, mark: str) -> Handed:  # noqa: N802 - reads as the kind it builds
    """A [`RED`] hand-back ([`Handed`])."""
    return Handed(number, RED, why, mark)


class Marked(list):
    """A CI wait's stop lines, carrying the runs' `mark` ([`Handed.mark`])."""

    mark: str

    def __init__(self, lines, mark: str):
        super().__init__(lines)
        self.mark = mark


class Awaiting(tuple):
    """An `AWAITS_*` hand-back: the `(number, state, why)` every caller unpacks,
    carrying what the pull request claimed as it was judged ([`watch.handbacks.claim`])
    and the issues that refused it ([`verdict.closing.blocking`])."""

    claim: str
    issues: tuple[int, ...]

    def __new__(cls, number: int, state: str, why: str, claim: str, issues=()):
        made = super().__new__(cls, (number, state, why))
        made.claim, made.issues = claim, tuple(issues)
        return made

# The machine failed, not the branch ([`stopping.Stopped`]); and what the stop left untouched.
STOPPED, NOT_TAKEN = "stopped the queue", "not taken"

# A watched pull request that turned draft or closed between listing and taking,
# or whose branch was pushed to during its take ([`refusal.MOVED`], #1568), and
# what [`taking.take`] says about one.
NOT_READY = "not ready"

# A merge train's member that goes back in line as it was: its train was red or
# stalled (its half is tested next), or a member ahead of it did not land (#1785);
# or a single take whose head moved during its CI wait ([`HEAD_MOVED`], #1799).
# Nothing is remembered against it.
REQUEUED = "back in line"

NOT_OPEN, A_DRAFT = "it is {}.", "it is a draft; mark it ready first."


def ordered(numbers: list[int]) -> list[int]:
    """The queue as given, repeats dropped, order kept — a repeat is a typo."""
    seen: set[int] = set()
    return [n for n in numbers if not (n in seen or seen.add(n))]


def line(number: int, state: str, why: str) -> str:
    """One result as the summary words it; a train's merge is said this way live too."""
    return f"#{number}: {state} — {why}"


def summary(results: list[tuple[int, str, str]]) -> list[str]:
    """The report — the only thing an unattended run leaves behind.

    A reason on every line, merges included, and the cleanup named rather than
    done (see the module doc).
    """
    lines = [line(number, state, why) for number, state, why in results]
    merged = [n for n, state, _ in results if state == MERGED]
    if merged:
        lines.append(
            "Merged: "
            + ", ".join(f"#{n}" for n in merged)
            + ". Their worktrees (.claude/worktrees/) and branches are still here —"
            " remove the worktrees and delete the branches when nobody is standing in one."
        )
    return lines


def say(line: str, *rest: str) -> None:
    print(f"queue: {line}", flush=True)
    for extra in rest:
        print(f"  {extra}", flush=True)
