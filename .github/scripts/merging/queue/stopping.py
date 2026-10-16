"""A machine failure stops the queue; a hand-back only skips an entry ([`drain`]).

## A hand-back skips the entry; it does not stop the queue

Only the machine does (see above). Every branch is rebased onto `main` as `main` is at its own turn, and CI
judges that tree, so nothing a hand-back leaves behind can make a later merge
unsound (scorsese#495). Each hand-back is named in the summary, and the exit
status is non-zero if there was any.
"""

from __future__ import annotations

from merging.queue import outcomes
from merging.verdict import github


class Stopped(Exception):
    """A failure that is the machine's, not the branch's: the queue stops.

    Raised through [`rebase.advance`] (whose `finally` still removes its worktree) and
    caught by [`drain`], which names every later entry as not taken.
    """

    def __init__(self, lines: list[str]):
        super().__init__(lines[0])
        self.lines = lines


def unreachable(down: Exception) -> list[str]:
    """A stop's lines when GitHub stayed down through `mergeable.gh`'s retries."""
    return [
        f"GitHub unreachable after {len(github.RETRIES)} retries: {down}",
        "Nothing is known about this branch; relaunch the watch once GitHub answers.",
    ]


def drain(numbers: list[int], turn) -> list[tuple[int, str, str]]:
    """Each pull request's `turn` in order, until the machine fails ([`Stopped`]).

    A hand-back carries on to the next; a stop names the one it happened on
    and every later one as not taken, and takes no more.
    """
    results = []
    for at, number in enumerate(numbers):
        try:
            try:
                results.append(turn(number))
            except github.Unreachable as down:
                raise Stopped(unreachable(down)) from down
        except Stopped as stop:
            outcomes.say(f"#{number}: {stop.lines[0]}", *stop.lines[1:])
            results.append((number, outcomes.STOPPED, stop.lines[0]))
            results += [(n, outcomes.NOT_TAKEN, f"the queue stopped at #{number}.") for n in numbers[at + 1:]]
            break
    return results
