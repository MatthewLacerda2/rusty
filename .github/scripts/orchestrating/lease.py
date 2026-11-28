"""One orchestrator at a time: a commit on the `orchestrator-lock` branch (#2667).

The orchestrator can run as an hourly cloud routine or on this machine
(`make queue-daemon`), and two of them merging at once is what the merge queue's
one-at-a-time rule forbids. A file lock (`merging/watch/lock.py`) cannot see
across machines, so the lock lives on GitHub, where both sides reach it.

- **A branch, not another ref.** The cloud container's git proxy refused a push
  to `refs/rusty/orchestrator-lock` (HTTP 403, 2026-11-28) and accepts a branch.
  No workflow runs on a push to a branch other than `main`.
- **A fast-forward push is the compare-and-swap.** Each take, renewal or
  release is a new commit whose parent is the tip just read, pushed without
  force. Two sides that read the same tip race; GitHub accepts the first and
  refuses the other as not a fast-forward (seen on 2026-11-28), and the loser
  reads the winner back ([`take`]). A comment edited in place has no such check.
- **The first line says who and when**: `orchestrator lock: held by <holder>
  at <time>` or `orchestrator lock: free at <time> (released by <holder>)`.
  A holder is `<place> <detail>`, its place `cloud` or `local@<host>`
  ([`holder`]).
- **Stale after [`STALE`]**: a holder renews every [`RENEW`] while it works
  (`orchestrating/keeper.py`); one that died without releasing is taken over
  once its last renewal is older than that, and the refusal names it and its age.
- **Nothing is deleted**: the container cannot delete a branch, so a release is
  one more commit. The branch grows by a few small commits an hour.
"""

from __future__ import annotations

import re
import socket
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone

BRANCH = "orchestrator-lock"
REF = f"refs/heads/{BRANCH}"
TRACK = f"refs/remotes/origin/{BRANCH}"

#: How often a holder renews, and when a lease nobody renewed may be taken.
RENEW = timedelta(minutes=10)
STALE = timedelta(minutes=45)

#: The exit status of a side that found another holding the lease, as the watch's lock (4).
HELD_STATUS = 4

HELD = re.compile(r"^orchestrator lock: held by (?P<holder>.+) at (?P<at>\S+)$")
FREE = re.compile(r"^orchestrator lock: free at (?P<at>\S+)(?: \(released by (?P<by>.+)\))?$")
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"


class Unreachable(RuntimeError):
    """GitHub could not be read or written."""


@dataclass(frozen=True)
class Lease:
    holder: str | None  # `None`: free, or never taken
    at: datetime | None
    tip: str | None  # `None`: the branch does not exist yet
    tree: str = EMPTY_TREE
    by: str | None = None  # who wrote it last: the holder, or the side that released it


def holder(place: str, detail: str) -> str:
    """`<place> <detail>`, the holder's name in the lease."""
    return f"{place} {' '.join(detail.split())}"


def local_place() -> str:
    return f"local@{socket.gethostname()}"


def place_of(name: str | None) -> str:
    return (name or "").split(" ", 1)[0]


def stamp(at: datetime) -> str:
    return at.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def when(text: str) -> datetime | None:
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None


def parse(tip: str, tree: str, message: str) -> Lease:
    """The lease a commit's message states; an unreadable one reads as free."""
    first = (message.strip().splitlines() or [""])[0]
    if match := HELD.match(first):
        return Lease(match["holder"], when(match["at"]), tip, tree, match["holder"])
    match = FREE.match(first)
    if match is None:
        return Lease(None, None, tip, tree)
    return Lease(None, when(match["at"]), tip, tree, match["by"])


def read(git) -> Lease:
    """The lease on GitHub now. `git(*args, stdin=None)` answers `(status, stdout, stderr)`."""
    status, _, err = git("fetch", "--quiet", "origin", f"+{REF}:{TRACK}")
    if status != 0:
        if "couldn't find remote ref" in err:
            return Lease(None, None, None)
        raise Unreachable(f"fetching {BRANCH}: {err.strip()[:300]}")
    status, out, err = git("log", "-1", "--format=%H %T%n%B", TRACK)
    if status != 0:
        raise Unreachable(f"reading {BRANCH}: {err.strip()[:300]}")
    head, _, message = out.partition("\n")
    tip, tree = head.split()
    return parse(tip, tree, message)


def refusal(lease: Lease, me: str, now: datetime) -> str | None:
    """Why `me` may not take `lease` now, or `None`: free, its own, or stale."""
    if lease.holder is None or lease.holder == me:
        return None
    if lease.at is None or now - lease.at >= STALE:
        return None
    minutes = int((now - lease.at).total_seconds() // 60)
    return (f"the orchestrator lock is held by {lease.holder}, renewed {minutes} minute(s) ago "
            f"(taken as stale after {int(STALE.total_seconds() // 60)}); one orchestrator at a time")


def write(git, lease: Lease, message: str, tree: str | None) -> bool:
    """Push one commit on `lease`'s tip, with `tree` (the tip's when `None`); whether GitHub took it."""
    parents = ["-p", lease.tip] if lease.tip else []
    status, sha, err = git("commit-tree", tree or lease.tree, *parents, "-m", message)
    if status != 0:
        raise Unreachable(f"making the lock's commit: {err.strip()[:300]}")
    status, _, _ = git("push", "--quiet", "origin", f"{sha.strip()}:{REF}")
    return status == 0


def take(git, me: str, now: datetime, tree: str | None = None,
         renewing: bool = False) -> tuple[Lease, Lease | None]:
    """Take the lease for `me` (or, `renewing`, renew the one `me` holds, refused when it is
    not its own any more): `(the lease before, the lease now)`, the second `None` when it is
    refused (the first then says who holds it). A push GitHub refused is read back: lost to a
    racer, the racer is named; a failure to reach it raises [`Unreachable`]."""
    before = read(git)
    if refusal(before, me, now) or (renewing and before.holder != me):
        return before, None
    if write(git, before, f"orchestrator lock: held by {me} at {stamp(now)}", tree):
        return before, read(git)
    after = read(git)
    if after.tip == before.tip:
        raise Unreachable(f"pushing {BRANCH} failed with nobody else on it")
    return after, None


def release(git, me: str, now: datetime, tree: str | None = None) -> str | None:
    """Free the lease `me` holds, saving `tree`; why not, when another holds it now."""
    lease = read(git)
    if lease.holder != me:
        return f"not released: the lock is held by {lease.holder or 'nobody'}, not {me}"
    if not write(git, lease, f"orchestrator lock: free at {stamp(now)} (released by {me})", tree):
        return f"not released: {BRANCH} moved under the release; read it with `make orchestrator ARGS=status`"
    return None
