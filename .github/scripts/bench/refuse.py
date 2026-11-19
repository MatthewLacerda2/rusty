"""What stops a timed run before it starts (#2381): a timed benchmark runs alone.

Two of #2298's measurements were wrong for a reason a check would have
refused: a baseline taken while two local agents built beside it (its CPU rows
spread 9%), and a run on a project whose textures were never imported (`gpu_mb`
2176 against 1216 once imported). So `make bench-ab` refuses:

- **another timed run**: a lock file under `out/bench/` naming its PID (one
  whose PID no longer runs is stale and taken over);
- **a build or a bench on the machine**: a `cargo`, `rustc` or `bench-scenes`
  process ([`BUSY`]), from the process table, before any build and again
  before each leg (a build started mid-run would skew the legs after it);
- **un-imported textures**: the images an import would encode now, which
  `bench-scenes --pending-imports` lists from the engine's own check
  (`asset::texture_import::pending`), so the refusal and the import never
  disagree. It needs a built engine, so it runs on `main`'s build, before the
  PR's build and before any leg.

`FORCE=1` runs anyway, and the table says what was overridden.
"""

from __future__ import annotations

import os
from pathlib import Path

LOCK = Path("out/bench/.timed-run.lock")
# Processes that skew a timed run: a build, or another bench.
BUSY = ("cargo", "rustc", "bench-scenes")


def alive(pid: int) -> bool:
    """Whether a process `pid` runs (signal 0 checks without signalling)."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def take_lock(path: Path, pid: int, is_alive=alive) -> str | None:
    """Take the timed-run lock for `pid`; the refusal when a live run holds it."""
    try:
        held = int(path.read_text().split()[0])
    except (OSError, ValueError, IndexError):
        held = None
    if held is not None and held != pid and is_alive(held):
        return f"another timed run holds {path} (PID {held}): a timed benchmark runs alone"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"{pid}\n")
    return None


def release(path: Path, pid: int) -> None:
    """Drop the lock if `pid` still holds it."""
    try:
        if int(path.read_text().split()[0]) == pid:
            path.unlink()
    except (OSError, ValueError, IndexError):
        pass


def busy(ps: str, me: int) -> list[str]:
    """The processes in `ps` (`ps -A -o pid=,comm=`) that skew a timed run, as `name (PID n)`."""
    found = []
    for line in ps.splitlines():
        parts = line.split(None, 1)
        if len(parts) != 2 or not parts[0].isdigit() or int(parts[0]) == me:
            continue
        name = os.path.basename(parts[1].strip())
        if name in BUSY:
            found.append(f"{name} (PID {parts[0]})")
    return found


def busy_refusal(found: list[str]) -> str | None:
    """The refusal for what [`busy`] found, if anything."""
    if not found:
        return None
    return f"a build or a bench runs on the machine ({', '.join(found)}): a timed benchmark runs alone"


def pending_refusal(code: int, said: str) -> str | None:
    """The refusal for `bench-scenes --pending-imports`' answer: exit 1 lists the images."""
    if code == 0:
        return None
    if code == 1:
        return f"the project has un-imported textures (make import PROJECT=<dir>):\n{said.strip()}"
    return f"bench-scenes --pending-imports failed (exit {code}): {said.strip()}"
