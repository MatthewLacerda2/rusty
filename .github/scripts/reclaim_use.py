"""Is something using a worktree? `reclaim.py` asks before removing a finished one (#2217).

Git's answers (clean tree, every commit on a remote, HEAD on `origin/main`) say
whether removing a worktree loses commits, not whether anybody is in it. On
2026-11-15 a measurement agent had built release binaries in
`.claude/worktrees/local-experiments`, detached at `origin/main` with nothing
committed, and was running them; reclaim, run after a merge as `issue-batch`
says, read it as finished and removed it mid-run. The earlier cases were each
answered where they happened (the supervisor's lock, #1450; a sentence in
`independent-review` for a reviewer's checkout), which left "whoever uses a
worktree must lock it", a convention nobody checks. So reclaim checks:

- **A live process in it** (`processes`): any process whose working directory
  or executable lies under the worktree. Linux reads `/proc/<pid>/cwd` and
  `/proc/<pid>/exe` (an executable rebuilt under a running process reads as
  `<path> (deleted)`, and still counts); elsewhere (macOS) `lsof -d cwd,txt`,
  where `txt` is the program and what it maps. Neither answering means the
  process check is unknown, and the worktree is kept and says so.
- **A recent write** (`recent_write`): an entry modified within the idle window,
  an hour by default (`reclaim.py --idle MINUTES`; 0 turns it off, for a run by
  hand that means it). An mtime in the future (clock skew after a reboot, an
  extracted archive) also keeps it, and is named as such so it can be spotted.
  The probe runs only for a worktree every other check would remove.

The write probe is bounded, because reclaim runs after every merge and a
worktree's `target/` holds tens of thousands of build files. It stats the
worktree itself and every entry down to `DEPTH` levels below it, lists no
directory deeper than that, and never lists cargo's bulk directories (`BULK`:
`deps/`, `.fingerprint/`, `build/`, `incremental/`), whose own mtime it still
reads; it stops at the first entry inside the window. A directory's mtime moves
when an entry is created, renamed or removed in it, which is how cargo and
rustc land their outputs, so a build in `target/<profile>/` or in a nested
target such as `target/merge-queue/<profile>/` shows. It can miss a file
rewritten in place below the depth or inside a bulk directory (a log appended
at `out/run/1/log.txt`); a file `.gitignore` does not cover is `git status`'s
anyway, and whatever is still writing it is a live process.
"""

from __future__ import annotations

import contextlib
import os
import time
from pathlib import Path

DEPTH = 4  # entries this many levels below the worktree are stat'ed; their directories are not listed
BULK = frozenset({"deps", ".fingerprint", "build", "incremental"})  # cargo's thousands of files
SKIP = frozenset({".git"})  # a linked worktree's `.git` is a file naming the common git directory


def processes(run) -> list[tuple[int, Path, str]] | None:
    """(pid, a path it stands in or runs, its command) for every process; `None` if unknown."""
    proc = Path("/proc")
    if (proc / "self" / "cwd").exists():
        return list(from_proc(proc))
    try:
        done = run("lsof", "-d", "cwd,txt", "-Fpcn")
    except OSError:
        return None
    return parse_lsof(done.stdout) if done.stdout else None


def from_proc(proc: Path):
    for entry in proc.glob("[0-9]*"):
        command = ""
        with contextlib.suppress(OSError):
            command = (entry / "comm").read_text().strip()
        for link in ("cwd", "exe"):
            with contextlib.suppress(OSError):
                yield int(entry.name), Path(os.readlink(entry / link).removesuffix(" (deleted)")), command


def parse_lsof(out: str) -> list[tuple[int, Path, str]]:
    """`lsof -F pcn`: a `p<pid>` line, its `c<command>`, then an `n<path>` per file."""
    found, pid, command = [], 0, ""
    for line in out.splitlines():
        if line.startswith("p"):
            pid, command = int(line[1:]), ""
        elif line.startswith("c"):
            command = line[1:]
        elif line.startswith("n"):
            found.append((pid, Path(line[1:]), command))
    return found


def recent_write(tree: Path, since: float) -> float | None:
    """The mtime of the first entry found under `tree` written after `since` (not the newest); `None` if none."""
    with contextlib.suppress(OSError):
        mtime = tree.stat().st_mtime
        if mtime > since:
            return mtime
    return walk(tree, since, 1)


def walk(directory: Path, since: float, depth: int) -> float | None:
    try:
        entries = list(os.scandir(directory))
    except OSError:
        return None
    for entry in entries:
        if entry.name in SKIP:
            continue
        with contextlib.suppress(OSError):
            mtime = entry.stat(follow_symlinks=False).st_mtime
            if mtime > since:
                return mtime
    for entry in entries:
        if depth < DEPTH and entry.name not in SKIP | BULK and entry.is_dir(follow_symlinks=False):
            found = walk(Path(entry.path), since, depth + 1)
            if found is not None:
                return found
    return None


SKEW = 60  # seconds past now an mtime may read before it is "in the future": a write landing mid-probe


def written(tree: Path, idle_minutes: int) -> str | None:
    """What under `tree` was written inside the idle window, as `reclaim.verdict` names it; `None` if nothing."""
    if idle_minutes <= 0:
        return None
    now = time.time()
    mtime = recent_write(tree, now - idle_minutes * 60)
    if mtime is None:
        return None
    if mtime > now + SKEW:
        return "something under it has an mtime in the future (clock skew, an extracted archive?)"
    return f"something under it was written within the last {idle_minutes} minute(s)"
