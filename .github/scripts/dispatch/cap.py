"""A cap on cloud coders below the limit, set without a code change (#2630).

[`running.LIMIT`] is the ceiling CLAUDE.md names. The operator asked for four
for a while (his plan's tier changed), and the only way then was a wrapper loop
outside the repository. So a lower cap is one file beside the waiting line
([`FILE`], under the common git directory, untracked), written by
`make coders ARGS='--cap N'` through `merging/state.py`, and read by every path
that launches: `make coder`, `make coders` and its pass (`dispatch/drain.py`).

- **A cap above the limit is refused** ([`check`]), and so is one under one: a
  pause is not what a cap is for. `--cap` at the limit removes the file.
- **A file that cannot be read is no cap** ([`read`]), said once by the reader:
  a hand-edited or truncated file never stops every launch.
- **Lowering it stops no running coder**: the slots over the cap drain as their
  pull requests merge, the way the limit works.
"""

from __future__ import annotations

from pathlib import Path

from dispatch import running
from merging import state as files

#: Beside `waiting.FILE`, in the same directory.
FILE = "coders-cap.json"


def path(line: Path) -> Path:
    """The cap's file beside the waiting line `line`."""
    return line.with_name(FILE)


def check(asked: str) -> int:
    """`asked` as a cap; `ValueError` saying why for one that is not 1 to [`running.LIMIT`]."""
    if not asked.isdigit() or not 1 <= int(asked) <= running.LIMIT:
        raise ValueError(f"--cap takes a number from 1 to {running.LIMIT} (the limit, CLAUDE.md), not {asked!r}")
    return int(asked)


def read(line: Path) -> tuple[int, str | None]:
    """The cap in force and, for a file that holds no cap it can use, why it was ignored."""
    target = path(line)
    if not target.exists():
        return running.LIMIT, None
    got = files.read(target)
    found = got.get("cap") if isinstance(got, dict) else None
    if isinstance(found, int) and not isinstance(found, bool) and 1 <= found <= running.LIMIT:
        return found, None
    return running.LIMIT, f"the cap file {target} holds no cap from 1 to {running.LIMIT}; ignored"


def write(line: Path, cap: int) -> str | None:
    """Set the cap (the limit removes the file); why not, if it failed."""
    target = path(line)
    if cap == running.LIMIT:
        try:
            target.unlink(missing_ok=True)
        except OSError as failed:
            return str(failed)
        return None
    return files.write(target, {"cap": cap})


def told(count: int, cap: int) -> str:
    """`running N of CAP`, and the limit beside a cap under it."""
    return f"running {count} of {cap}" + (f" (the cap; the limit is {running.LIMIT})" if cap < running.LIMIT else "")
