"""The board as the stream last told it, kept across watches (#2215).

[`board.transitions`] compares two listings; a watch's first listing has no
earlier one of its own. Before #2215 it announced nothing, and the supervisor
sleeps ten minutes between a watch that found no open pull request and the
next: on 2026-11-15 PRs #2210 to #2214 opened in that gap, #2210 turned ready
in it, and `make queue-events` passed none of them. So each listing the watch
reports is kept here ([`BOARD_MEMORY`], beside the hand-back memory), and the
next watch's first listing is compared with it.

- **Written by the one process that lists**, the watch, atomically: a
  temporary file beside it, then a rename, so a reader sees the old listing or
  the new one, never half of one.
- **A file that cannot be read is absent** ([`recall`]): truncated by a power
  cut, edited by hand, from an older shape. The watch must start, so it starts
  as if fresh ([`board.transitions`] with `None`), never with a traceback.
- **A stale file is read as it is** (a supervisor stopped for a day): the
  differences against it are still true statements about what changed while no
  watch ran. A pull request closed meanwhile is reported closed once, since the
  next write no longer holds it.
- **What this watch merged is never kept** ([`Kept.write`] drops `landed`), so a
  watch that ends right after a merge does not leave the next one to call it
  "closed, not merged by this watch".
- **A dry run writes nothing**: its lines go to a terminal, not the log the
  stream reads, so it must not mark anything told.
"""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path

# The file's name, beside `handbacks.MEMORY` in the queue's state directory.
BOARD_MEMORY = "board.json"

Heads = dict[int, tuple[bool, str, bool]]


def recall(path: Path) -> Heads | None:
    """The kept listing, as [`board.heads`] gives it; `None` when there is
    none or it cannot be read as one."""
    try:
        raw = json.loads(path.read_text())
        kept = {}
        for number, (draft, head, queued) in raw.items():
            if not (isinstance(draft, bool) and isinstance(head, str) and isinstance(queued, bool)):
                return None
            kept[int(number)] = (draft, head, queued)
        return kept
    except (OSError, ValueError, TypeError, AttributeError):
        return None


def remember(path: Path, heads: Heads) -> None:
    """Write `heads` to `path` in one rename. A write that fails loses the
    memory, never the watch: the next one starts fresh."""
    text = json.dumps({str(n): list(v) for n, v in sorted(heads.items())}, indent=1) + "\n"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        handle, temporary = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
        try:
            with os.fdopen(handle, "w") as out:
                out.write(text)
            os.replace(temporary, path)
        except BaseException:
            os.unlink(temporary)
            raise
    except OSError:
        pass


class Kept:
    """One watch's view of the kept listing: read once at launch, rewritten
    when what it would hold changes. `enabled` is false for a dry run."""

    def __init__(self, path: Path, enabled: bool = True):
        self.path, self.enabled = path, enabled
        self.written = recall(path)

    def first(self) -> Heads | None:
        """What a watch's first listing is compared with."""
        return self.written

    def write(self, heads: Heads | None, landed: set[int]) -> None:
        if heads is None or not self.enabled:
            return
        keep = {n: v for n, v in heads.items() if n not in landed}
        if keep != self.written:
            remember(self.path, keep)
            self.written = keep
