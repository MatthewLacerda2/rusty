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
- **A change that is no lease** ([`amend`]): the coders' cap is set from either
  side while the other holds the lock, so it is a commit that keeps the tip's
  first line (holder and time) and changes only its tree; a holder's renewal
  that loses that race reads the tip again and renews on top of it.
- **Nothing is deleted**: the container cannot delete a branch, so a release is
  one more commit. The branch grows by a few small commits an hour.
"""

from __future__ import annotations

import re
import socket
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Callable

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
    message: str = ""  # the tip's first line, which [`amend`] keeps


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
        return Lease(match["holder"], when(match["at"]), tip, tree, match["holder"], first)
    match = FREE.match(first)
    if match is None:
        return Lease(None, None, tip, tree, message=first)
    return Lease(None, when(match["at"]), tip, tree, match["by"], first)


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


#: How many times a write that lost the race to another push reads the tip again and retries.
TRIES = 3

Tree = Callable[[Lease], str]


def write(git, lease: Lease, message: str, tree: Tree | None, *more: str) -> bool:
    """Push one commit on `lease`'s tip, its tree `tree(lease)` (the tip's when `None`), its
    message `message` and then each of `more`; whether GitHub took it."""
    parents = ["-p", lease.tip] if lease.tip else []
    body = [arg for line in more for arg in ("-m", line)]
    status, sha, err = git("commit-tree", tree(lease) if tree else lease.tree, *parents, "-m", message, *body)
    if status != 0:
        raise Unreachable(f"making the lock's commit: {err.strip()[:300]}")
    status, _, _ = git("push", "--quiet", "origin", f"{sha.strip()}:{REF}")
    return status == 0


def raced(git, before: Lease) -> Lease:
    """The lease after a push GitHub refused: someone else's push moved it, or [`Unreachable`]."""
    after = read(git)
    if after.tip == before.tip:
        raise Unreachable(f"pushing {BRANCH} failed with nobody else on it")
    return after


def take(git, me: str, now: datetime, tree: Tree | None = None,
         renewing: bool = False) -> tuple[Lease, Lease | None]:
    """Take the lease for `me` (or, `renewing`, renew the one `me` holds, refused when it is
    not its own any more): `(the lease before, the lease now)`, the second `None` when it is
    refused (the first then says who holds it). A push GitHub refused is read back and tried
    again on the new tip, up to [`TRIES`] times: a racer that took it is named; a push that
    left the lease as it was (the cap, [`amend`]) is no loss. A failure to reach GitHub
    raises [`Unreachable`]."""
    before = read(git)
    for _ in range(TRIES):
        if refusal(before, me, now) or (renewing and before.holder != me):
            return before, None
        if write(git, before, f"orchestrator lock: held by {me} at {stamp(now)}", tree):
            return before, read(git)
        before = raced(git, before)
    return before, None


def release(git, me: str, now: datetime, tree: Tree | None = None) -> str | None:
    """Free the lease `me` holds, saving `tree`; why not, when another holds it now."""
    lease = read(git)
    for _ in range(TRIES):
        if lease.holder != me:
            return f"not released: the lock is held by {lease.holder or 'nobody'}, not {me}"
        if write(git, lease, f"orchestrator lock: free at {stamp(now)} (released by {me})", tree):
            return None
        lease = raced(git, lease)
    return f"not released: {BRANCH} kept moving under the release; read it with `make orchestrator ARGS=status`"


def amend(git, tree: Tree, why: str, now: datetime) -> None:
    """Push a commit that changes the tip's tree and leaves the lease as it stands: the
    same first line, so holder and time are unchanged, then `why`. A lock never taken gets
    a first `free` commit. Retried as [`take`] is; [`Unreachable`] when it never lands."""
    lease = read(git)
    for _ in range(TRIES):
        message = (lease.tip and lease.message) or f"orchestrator lock: free at {stamp(now)}"
        if write(git, lease, message, tree, why):
            return
        lease = raced(git, lease)
    raise Unreachable(f"{BRANCH} kept moving under the change: {why}")
