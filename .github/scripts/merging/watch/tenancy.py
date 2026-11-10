"""The supervisor's claim on its worktree: a `git worktree lock` naming its PID (#1450).

On 2026-10-25 `make reclaim` removed the supervisor's worktree while it slept
its "nothing left" pause: a detached HEAD on `origin/main` reads as finished,
and nothing stood in it while it waited. Its next `git` call raised
`FileNotFoundError` and the traceback killed it. `reclaim` protects a locked
worktree, so the supervisor locks its own for as long as it lives, with a
reason `reclaim` can read back ([`pid_of`]): a lock whose PID is no longer
alive is reported as stale, never removed silently.

On 2026-11-10, after a power loss, the supervisor was restarted from the
repository root (#1983). git refuses to lock the main working tree, and the
supervisor logged that and carried on: it detached the operator's checkout onto
`origin/main`, merged two pull requests from there, and `make queue-status`
said no supervisor ran, since it reads liveness from this lock. So the main
working tree is refused before anything forks ([`main_tree`], [`refusal`]),
and a claim that fails stops the supervisor rather than leaving it unlocked.
"""

from __future__ import annotations

import os
import re
import shlex
from pathlib import Path

from merging.queue import gitio


# Where the orchestrator keeps the supervisor's worktree, from the main checkout (`issue-batch`).
WORKTREE = Path(".claude", "worktrees", "merge-queue")

# How the lock's reason begins; the PID follows.
REASON = "merge-queue supervisor pid"

_PID = re.compile(rf"^{re.escape(REASON)} (\d+)$")


def reason(pid: int) -> str:
    return f"{REASON} {pid}"


def pid_of(locked: str | None) -> int | None:
    """The supervisor PID a worktree lock's reason names, or `None` for any other lock."""
    found = _PID.match((locked or "").strip())
    return int(found.group(1)) if found else None


# The harness's lock on an `Agent` worktree: `claude agent agent-<id> (pid <pid> start <ticks>)`.
_AGENT = re.compile(r"^claude agent\b.*\(pid (\d+)(?: start \d+)?\)$")


def agent_pid_of(locked: str | None) -> int | None:
    """The PID a harness agent's worktree lock names, or `None` for any other lock (#1658)."""
    found = _AGENT.match((locked or "").strip())
    return int(found.group(1)) if found else None


def alive(pid: int) -> bool:
    """Whether a process with this PID exists (one owned by another user counts)."""
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def current(root: str) -> str | None:
    """The lock reason on the worktree `root` stands in (`""` for none given), or
    `None` when it is not locked."""
    top = gitio.git("rev-parse", "--show-toplevel", cwd=root).stdout.strip()
    listing = gitio.git("worktree", "list", "--porcelain", cwd=root).stdout
    for block in listing.strip().split("\n\n"):
        fields = dict(line.partition(" ")[::2] for line in block.splitlines() if line)
        if top and Path(fields.get("worktree", "")).resolve() == Path(top).resolve():
            return fields.get("locked")
    return None


def main_tree(root: str) -> bool:
    """Whether `root` stands in the repository's main working tree, where git
    keeps its own directory (`--git-dir` is `--git-common-dir`), not a linked one.
    A `root` git cannot read is not called main: what runs next meets it."""
    own = gitio.git("rev-parse", "--git-dir", cwd=root)
    common = gitio.git("rev-parse", "--git-common-dir", cwd=root)
    if own.returncode or common.returncode or not own.stdout.strip():
        return False
    return Path(root, own.stdout.strip()).resolve() == Path(root, common.stdout.strip()).resolve()


def refusal(root: str, forwarded: list[str]) -> str:
    """The one line refusing to supervise from the main working tree at `root`:
    the exact command when [`WORKTREE`] exists, else how to make it."""
    top = gitio.git("rev-parse", "--show-toplevel", cwd=root).stdout.strip() or root
    tree = Path(top, WORKTREE)
    again = "make queue-daemon" + (f" ARGS={shlex.quote(shlex.join(forwarded))}" if forwarded else "")
    if tree.is_dir():
        how = f"run `cd {shlex.quote(str(tree))} && {again}`"
    else:
        made = f"git -C {shlex.quote(top)} worktree add --detach {shlex.quote(str(tree))} origin/main"
        how = f"make one and run it there: `{made} && cd {shlex.quote(str(tree))} && {again}`"
    return (f"queue-daemon: {top} is the main working tree, which the supervisor would detach onto"
            f" origin/main and cannot lock; {how}.")


def claim(root: str, pid: int) -> str | None:
    """Lock `root`'s worktree for supervisor `pid`; why not, when it could not.

    Called holding the supervisor's flock, so a lock another supervisor left is
    a dead one's (or this PID's own, after it re-executed itself) and is
    replaced. Any other lock is left as it is: it protects the worktree too,
    but names no supervisor, so the caller stops as for a failed lock (#1983).
    """
    held = current(root)
    if pid_of(held) is not None:
        gitio.git("worktree", "unlock", root, cwd=root)
    elif held is not None:
        return f"{root} is already locked ({held or 'no reason given'}), not by a supervisor"
    done = gitio.git("worktree", "lock", "--reason", reason(pid), root, cwd=root)
    if done.returncode != 0:
        return f"could not lock {root} against `make reclaim`: {done.stderr.strip()}"
    return None


def release(root: str, pid: int) -> None:
    """Unlock `root`'s worktree if this supervisor's lock is still on it."""
    if os.path.isdir(root) and pid_of(current(root)) == pid:
        gitio.git("worktree", "unlock", root, cwd=root)
