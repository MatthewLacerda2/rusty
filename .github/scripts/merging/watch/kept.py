"""The board as the stream last told it, kept across watches (#2215).

[`board.transitions`] compares two listings; a watch's first listing has no
earlier one of its own. Before #2215 it announced nothing, and the supervisor
sleeps ten minutes between a watch that found no open pull request and the
next: on 2026-11-15 PRs #2210 to #2214 opened in that gap, #2210 turned ready
in it, and `make queue-events` passed none of them. So each listing the watch
reports is kept here ([`BOARD_MEMORY`], beside the hand-back memory), and the
next watch's first listing is compared with it ([`Board.report`]).

- **The whole listing is kept, never the `--since` view of it**, with each pull
  request's opening time, and `--since` filters only what is said. Every batch
  starts its supervisor with a new `--since`: a board kept filtered under the
  old one made the next watch call an old, still open draft "closed", or every
  older ready pull request "opened as ready" (the independent review of PR
  #2216). A pull request is closed only when it is gone from the full listing,
  and said under `--since` by the time it opened, kept for the closed ones.
- **One closed between watches is asked whether it merged** ([`Board.report`]):
  `make queue PRS=N` or a merge by hand while the supervisor sleeps is no
  merge of this watch, and "closed, not merged by this watch" read as an
  unmerged close. So the first listing asks GitHub once per such number and
  says `merged`, `closed, not merged`, or `closed (merged or not)` when GitHub
  does not answer ([`board.closed_line`]). A close inside one watch keeps its
  old words, which are true there.
- **Written through [`state.write`]**: a reader sees the old listing or the
  new, never half of one; a file that cannot be read, or holds another shape,
  is absent ([`recall`]), and the watch starts as if fresh
  ([`board.transitions`] with `None`), never with a traceback.
- **A write that fails removes the old file** ([`Board.keep`]) and is tried
  again at the next listing: the old file would make the next watch repeat
  what this one said. Only if the removal fails too (a directory nobody can
  write) does the next watch repeat a line, and every line it repeats was true.
- **A stale file is read as it is** (a supervisor stopped for a day): the
  differences against it are still true statements about what changed while no
  watch ran. A pull request closed meanwhile is said once, since the next
  write no longer holds it.
- **What this watch merged is never kept** ([`Board.keep`] drops `landed`), so
  the next watch does not ask about it or say it.
- **A dry run writes nothing**: its lines go to a terminal, not the log the
  stream reads, so it must not mark anything told.
"""

from __future__ import annotations

from pathlib import Path

from merging import state
from merging.watch import board

# The file's name, beside `handbacks.MEMORY` in the queue's state directory.
BOARD_MEMORY = "board.json"

Heads = dict[int, tuple[bool, str, bool]]
Opened = dict[int, "str | None"]


def recall(path: Path) -> tuple[Heads, Opened] | None:
    """The kept listing, as [`board.heads`] and [`board.opened`] give it;
    `None` when there is none or it cannot be read as one."""
    raw = state.read(path)
    try:
        heads, opened = {}, {}
        for number, (draft, head, queued) in raw["heads"].items():
            if not (isinstance(draft, bool) and isinstance(head, str) and isinstance(queued, bool)):
                return None
            heads[int(number)] = (draft, head, queued)
        for number, when in raw["opened"].items():
            if when is not None and not isinstance(when, str):
                return None
            opened[int(number)] = when
        return heads, opened
    except (KeyError, ValueError, TypeError, AttributeError):
        return None


def remember(path: Path, heads: Heads, opened: Opened) -> str | None:
    """Keep the listing at `path` whole; why not, if it failed."""
    return state.write(path, {
        "heads": {str(n): list(v) for n, v in heads.items()},
        "opened": {str(n): when for n, when in opened.items() if n in heads},
    })


class Board:
    """One watch's view of the board it told: read once at launch, rewritten
    when what it would hold changes. `enabled` is false for a dry run."""

    def __init__(self, path: Path, enabled: bool = True):
        self.path, self.enabled = path, enabled
        kept = recall(path)
        self.told, self.opened = kept if kept else (None, {})
        self.first = True  # the next report compares with another watch's listing
        self.saved = kept

    def report(self, pulls: list[dict], landed: set[int], since, was_merged) -> list[str]:
        """The lines that say how `pulls` differs from the board last told,
        `--since` applied to what is said; and now it is told. `was_merged(n)`
        asks GitHub, on the first report only, about each number gone since
        the kept listing (`True`, `False`, `None` when it did not answer)."""
        now, opened = board.heads(pulls), board.opened(pulls)
        times = {**self.opened, **opened}
        about = {n for n, when in times.items() if board.fresh(when, since)}
        closes = None
        if self.first and self.told is not None:
            closes = {n: was_merged(n) for n in sorted(self.told) if n not in now and n not in landed and n in about}
        lines = board.transitions(self.told, now, landed, about, closes)
        self.told, self.opened, self.first = now, opened, False
        self.keep(landed)
        return lines

    def keep(self, landed: set[int]) -> None:
        """Write what was told, less what this watch merged, if it changed."""
        if self.told is None or not self.enabled:
            return
        record = ({n: v for n, v in self.told.items() if n not in landed},
                  {n: w for n, w in self.opened.items() if n in self.told and n not in landed})
        if record == self.saved:
            return
        if remember(self.path, *record) is None:
            self.saved = record
        else:
            state.drop(self.path)  # a stale file would make the next watch repeat lines
            self.saved = None
