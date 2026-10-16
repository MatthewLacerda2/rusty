"""What a turn can end as, and the words the queue prints.

The summary names merged, green and dry apart so the report never claims a merge
it did not make ([`summary`]).
"""

from __future__ import annotations


WAIT, GO, STOP = "wait", "go", "stop"

# Red for the runners' reasons only ([`verdict.runners.outage`], #869): re-run it once.
RERUN = "rerun"

# How the hand-back after that one re-run begins: GitHub's failure, not the branch's.
RUNNERS = "runners unavailable"

# What the summary calls each ending. Merged, green and dry are separate so the
# report never claims a merge it did not make.
MERGED, GREEN, DRY, HANDED_BACK = "merged", "green", "dry run", "handed back"

# The machine failed, not the branch ([`stopping.Stopped`]); and what the stop left untouched.
STOPPED, NOT_TAKEN = "stopped the queue", "not taken"

# A watched pull request that turned draft or closed between listing and taking,
# and what [`taking.take`] says about one.
NOT_READY = "not ready"

NOT_OPEN, A_DRAFT = "it is {}.", "it is a draft; mark it ready first."


def ordered(numbers: list[int]) -> list[int]:
    """The queue as given, repeats dropped, order kept — a repeat is a typo."""
    seen: set[int] = set()
    return [n for n in numbers if not (n in seen or seen.add(n))]


def summary(results: list[tuple[int, str, str]]) -> list[str]:
    """The report — the only thing an unattended run leaves behind.

    A reason on every line, merges included, and the cleanup named rather than
    done (see the module doc).
    """
    lines = [f"#{number}: {state} — {why}" for number, state, why in results]
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
