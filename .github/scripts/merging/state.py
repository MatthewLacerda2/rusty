"""The queue's JSON state files under `.git/merge-queue/`: one writer, one reader (#2215).

The hand-back memory ([`watch.handbacks`]), the kept board ([`watch.kept`])
and the mutation requests ([`after.slots`]) are each read back by the next
watch. This machine loses power without warning, and a file written in place
can be cut short: a `handed-back.json` read as `{}` forgets every hand-back,
and each is taken again.

- **[`write`] never leaves half a file**: the text goes to a temporary file in
  the same directory, is flushed to disk, then renamed over the old one, so a
  reader sees the old contents or the new, never part of either. It returns why
  it failed instead of raising: a state file that cannot be written loses a
  memory, never the watch.
- **One fixed temporary per state file** ([`part`], `.<name>.part`): a write cut
  between the flush and the rename leaves it behind, and nothing sweeps the
  directory, so a uniquely named one would pile up one per cut. The next write
  of that file overwrites it. One watch runs at a time (`watch.lock`), so no
  two writers share it.
- **Nothing else under `merging/` writes a file** but the appends and locks that
  say why inline (`# state-ok: <why>`), held by `.github/scripts/tests/state/test_writers.py`.
- **[`read`] answers `None` for a file that is missing or is not JSON**
  (truncated, edited by hand), never a traceback. Whether what it holds has the
  right shape is the caller's question.
"""

from __future__ import annotations

import json
import os
from pathlib import Path


def write(path: Path, data: object) -> str | None:
    """Write `data` as JSON to `path` in one rename; why not, if it failed."""
    text = json.dumps(data, indent=1, sort_keys=True) + "\n"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = part(path)
        try:
            with open(temporary, "w") as out:
                out.write(text)
                out.flush()
                os.fsync(out.fileno())
            os.replace(temporary, path)
        except BaseException:
            drop(temporary)  # the next write overwrites it if this cannot
            raise
    except OSError as failed:
        return str(failed)
    return None


def part(path: Path) -> Path:
    """Where [`write`] puts `path`'s text before the rename."""
    return path.with_name(f".{path.name}.part")


def read(path: Path) -> object | None:
    """What `path` holds, parsed; `None` when it is missing or not JSON."""
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def drop(path: Path) -> None:
    """Remove `path` if it can be: what a failed [`write`] leaves is stale."""
    try:
        path.unlink(missing_ok=True)
    except OSError:
        pass
