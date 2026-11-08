"""Is the queue's supervisor running? Its worktree lock's PID, checked alive (#1876).

After a power cut rebooted the machine on 2026-11-07, `pgrep -f queue-daemon`
matched the checking shell's own command line, and a dead supervisor was
reported running for about an hour while #1872 sat labelled and unmerged. So
liveness is read where the supervisor writes it: the `git worktree lock` it
holds for its life (`watch.tenancy.claim`), whose reason names its PID. That
lock outlives a reboot, and a reboot hands PIDs out again, so a live PID is
also asked what it runs ([`identity`]): one that is not `queue-daemon.py` is
another process wearing a dead supervisor's number. This asks one PID, never
the list of every process.
"""

from __future__ import annotations

import subprocess
import time
from pathlib import Path
from typing import Callable, NamedTuple

from merging.queue import gitio
from merging.watch import supervisor, tenancy

# What a supervisor's command line holds, whether started or re-executed (`daemon.effects`).
SCRIPT = "queue-daemon.py"


class Status(NamedTuple):
    running: bool
    lines: list[str]


def locks(listing: str) -> list[tuple[str, int]]:
    """`(worktree, pid)` for each worktree a supervisor's lock is on, from
    `git worktree list --porcelain`."""
    found = []
    for block in listing.strip().split("\n\n"):
        fields = dict(line.partition(" ")[::2] for line in block.splitlines() if line)
        pid = tenancy.pid_of(fields.get("locked"))
        if pid is not None:
            found.append((fields.get("worktree", "?"), pid))
    return found


def identity(pid: int) -> str | None:
    """The command line of process `pid` (`ps`, on Linux and macOS alike), or
    `None` when `ps` could not say. `-ww` keeps it whole: an exported `COLUMNS`
    would otherwise cut `queue-daemon.py` off it (#1876's review)."""
    try:
        done = subprocess.run(["ps", "-ww", "-o", "command=", "-p", str(pid)], capture_output=True, text=True, check=False)
    except OSError:
        return None
    return done.stdout.strip() or None if done.returncode == 0 else None


def is_supervisor(pid: int, alive: Callable[[int], bool] = tenancy.alive, ask: Callable[[int], str | None] = identity) -> bool:
    """Whether `pid` is alive and runs the supervisor. A `ps` that cannot say
    leaves the PID's liveness as the answer."""
    if not alive(pid):
        return False
    command = ask(pid)
    return command is None or SCRIPT in command


def ago(seconds: float) -> str:
    minutes = max(0, int(seconds // 60))
    return f"{minutes} min ago" if minutes < 120 else f"{minutes / 60:.1f} h ago"


def last_line(log: Path, now: float) -> str:
    """The log's last line and how old it is, or that there is no log."""
    try:
        text, written = log.read_text(errors="replace"), log.stat().st_mtime
    except OSError:
        return f"queue-status: no log at {log}."
    lines = [line for line in text.splitlines() if line.strip()]
    if not lines:
        return f"queue-status: {log} is empty."
    return f"queue-status: log {log}, last line {ago(now - written)}: {lines[-1].strip()}"


def judge(held: list[tuple[str, int]], check: Callable[[int], bool]) -> Status:
    """The verdict on the supervisor locks `held`, with `check` telling a live
    supervisor's PID. Running when any one is."""
    if not held:
        return Status(False, ["queue-status: no supervisor running: no worktree carries a supervisor's lock. Start one with `make queue-daemon`."])
    live = [f"queue-status: supervisor running, PID {pid}, in {tree}." for tree, pid in held if check(pid)]
    if live:
        return Status(True, live)
    return Status(False, [
        f"queue-status: no supervisor running: {tree}'s lock names PID {pid}, which is not a live supervisor"
        " (a stale lock: `make reclaim` reports it). Start one with `make queue-daemon`."
        for tree, pid in held
    ])


def read(root: str, check: Callable[[int], bool] = is_supervisor) -> Status:
    """The supervisor's state from the worktrees of `root`'s checkout. A
    listing git cannot give is "not running": nothing proves it is."""
    listing = gitio.git("worktree", "list", "--porcelain", cwd=root)
    if listing.returncode != 0:
        return Status(False, [f"queue-status: git could not list the worktrees: {listing.stderr.strip()}"])
    return judge(locks(listing.stdout), check)


def common_dir(root: str) -> Path:
    return Path(root, gitio.git("rev-parse", "--git-common-dir", cwd=root).stdout.strip() or ".git")


def log_path(root: str) -> Path:
    return common_dir(root) / supervisor.LOG


def report(root: str, now: float | None = None) -> tuple[int, list[str]]:
    """`make queue-status`: exit 0 when a supervisor runs, 1 when none does."""
    state = read(root)
    return (0 if state.running else 1), [*state.lines, last_line(log_path(root), time.time() if now is None else now)]
