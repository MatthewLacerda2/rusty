"""`make queue-events`: the queue's log as the orchestrator's `Monitor` reads it (#1876).

One fixed command, so the watch is never retyped. What it keeps from the
hand-typed `tail -n 30 -F queue.log | grep` it replaced, and why:

- **30 lines back, always** ([`BACK`]): a re-arm from the log's end drops what
  it said while no `Monitor` ran; on 2026-10-31 two PRs sat ready and unread for
  an hour that way, and on 2026-11-07 the `-n 0` form came back twice.
- **Only what someone acts on** ([`keep`]): hand-backs, merges (a train's
  too), every line that ends the supervisor's run ([`CARRIES_ON`]), a held
  Dependabot PR, and a pull request turning ready, opening ready, going back
  to draft or closing unmerged (the watch's own `watch: #N …` lines,
  `watch.board.transitions`). PR #1870 sat ready for 90 minutes because the
  hand-typed grep let no ready flip through. A mutation request lost after a
  merge (`after: …` dropped, could not, never appeared; #1934) passes, so a
  lost report is told from a slow one. A CI poll line is dropped.
- **Each merge once**: the watch says a merge as it lands (`#N: merged.` for
  a single take, `#N: merged — landed with the train <sha>.` for a train's
  member, `watch.trains.Yard.land`), and again in its closing summary; the
  second says nothing new.
- **Every line flushed as written**, from one process: the combined command of
  2026-11-07 put the tail in the background of a PR poll, replayed old lines
  and delivered no live one, so `#1873: merged` went unreported for 20 minutes.
- **A dead supervisor is an event** ([`follow`]): liveness ([`status.read`]) is
  checked at the start and every [`LIVENESS_SECONDS`], and a supervisor that is
  not running ends the stream with one line and status 1.

Coders' *Standing down* comments come from a light poll beside it ([`standing`]).
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Callable, Iterable

# How many of the log's lines a stream starts from.
BACK = 30

# How often the supervisor's liveness is checked, and the comments polled.
LIVENESS_SECONDS = 30
POLL_SECONDS = 120

# How often the log is read for new lines.
STEP_SECONDS = 1

# The supervisor's lines that do not end its run (`watch.supervisor.run`).
# Every other `supervisor:` line does (a give-up, a crash, a vanished worktree,
# a requested stop, a refused update with its own wording), and is passed, so
# the stream says why it stopped (#1876's review). A new wording that carries
# on is noise here until it is listed, never a silent stop.
CARRIES_ON = (
    "starting the watch.",
    "its own code changed on main; restarting itself.",
    "the watch exited ",  # "…; starting it again in …"; its give-up ends "; stopped." and passes
)

# The lines to pass. `queue: ` prefixes the watch's own; the supervisor's are bare.
KEEP = re.compile(
    r"HANDED BACK"
    r"|supervisor:.*; stopped\.$"
    r"|supervisor: (?!" + "|".join(re.escape(line) for line in CARRIES_ON) + ")"
    r"|watch: STOPPED:"
    r"|HELD #\d+"
    r"|watch: #\d+ (?:turned ready|opened as ready|back to draft|closed, not merged)"
    # A mutation request lost, never a slow one (#1934): a signal nobody hears of is no signal.
    r"|after: .*(?:dropped|could not|never appeared)"
)

MERGED = re.compile(r"#(\d+): merged\b")


def keep(line: str, merged: set[int]) -> bool:
    """Whether the orchestrator acts on `line`; `merged` holds the merges
    already passed, and gains this one."""
    found = MERGED.search(line)
    if found:
        number = int(found.group(1))
        if number in merged:
            return False
        merged.add(number)
        return True
    return bool(KEEP.search(line))


class Tail:
    """New lines of a file that is appended to, read without blocking. A file
    that is missing is waited for; one that shrank is read again from its start."""

    def __init__(self, path: Path):
        self.path, self.at, self.partial = path, 0, ""

    def start(self, back: int = BACK) -> list[str]:
        """The last `back` complete lines, and the position after them."""
        lines = self.fresh()
        return lines[-back:] if back else []

    def fresh(self) -> list[str]:
        try:
            size = self.path.stat().st_size
            if size < self.at:
                self.at, self.partial = 0, ""
            with open(self.path, errors="replace") as handle:
                handle.seek(self.at)
                text = handle.read()
                self.at = handle.tell()
        except OSError:
            return []
        text, self.partial = self.partial + text, ""
        lines = text.split("\n")
        self.partial = lines.pop()  # what follows the last newline: not yet a line
        return lines


def follow(tail: Tail, running: Callable[[], tuple[bool, list[str]]], comments: Callable[[], Iterable[str]],
           say: Callable[[str], None], sleep: Callable[[float], None], clock: Callable[[], float]) -> int:
    """Stream until the supervisor is not running; then say why and return 1.

    `running()` is [`status.read`]'s verdict, `comments()` the new *Standing
    down* lines ([`standing.Poll`]), `clock` a monotonic time in seconds."""
    merged: set[int] = set()
    for line in tail.start():
        if keep(line, merged):
            say(line)
    checked = polled = None
    while True:
        now = clock()
        if checked is None or now - checked >= LIVENESS_SECONDS:
            checked = now
            alive, why = running()
            if not alive:
                for line in why:
                    say(line)
                say("queue-events: the supervisor is not running; the stream ends. `make queue-status` says more.")
                return 1
        if polled is None or now - polled >= POLL_SECONDS:
            polled = now
            for line in comments():
                say(line)
        for line in tail.fresh():
            if keep(line, merged):
                say(line)
        sleep(STEP_SECONDS)
