"""The coders waiting for a slot, and launching them when one frees (#2382).

Before it, `make coder` at [`running.LIMIT`] refused and forgot the ask: in
one batch four launches with their `Brief` posted were refused, one a review
round whose PR had just gone back to draft, and that PR sat with nobody on it
until the orchestrator noticed a slot and typed the verb again. The
orchestrator kept the line in its own notes and retried on every event, a rule
a tool can hold.

- **A launch at the limit joins the line** ([`enqueue`]) and says how many are
  ahead; `--no-wait` keeps the refusal. Any other refusal (no `Brief`, a stage
  label, a closed issue) never queues. A second ask for an issue already in
  line moves nothing.
- **Order** ([`ordered`]): a round (a `Round:` line in the issue's newest
  `Brief`, `rebriefs.ROUND`) ahead of a first launch, since a draft with
  findings open is nearly done and CLAUDE.md's merge order favours finishing;
  then the issue's [`priority`] (#2615: a red-`main` fix asked with fourteen
  entries in line waited behind eight by ask time alone); then the order asked.
- **Priority** ([`priority`], [`RANKS`]): a red `main` first, told by the
  marker `main-health.sh` writes in its issue's body ([`RED_MAIN_MARKER`]),
  never by the title; then CLAUDE.md's label order, read from the one table
  the merge queue ranks by (`merging/watch/order.PRIORITY`), an issue with
  several labels taking its highest; then an issue with none of them. It is
  read when the issue is asked for and again whenever [`refresh`] reads it
  (labels change), and kept on the entry (`priority`). An entry without one (written
  before #2615, or asked when its labels could not be read) sorts among the
  lowest rank in ask order, and `make coders` says so. No preemption: a
  running coder is never stopped for a higher-ranked entry.
- **Nothing launches by itself**: `dispatch/drain.py` runs only when `make coders` is
  called (the orchestrator's watch, once a minute), never from a daemon, so
  there is no process to keep alive across a reboot.
- **The line is a file in the common git directory** ([`FILE`], beside the merge
  queue's state), written whole through `merging/state.py`, so a reboot keeps it
  and every session on the machine sees one line. Every write, and every launch
  that writes, holds [`held`]'s `flock`; `dispatch/drain.py` says what it covers.
- **The lock is waited on for [`WAIT_SECONDS`], not forever** (the review of
  #2393: a `make coder` during a slow pass hung silently): past that it raises
  [`Busy`] naming who holds it, from the note the holder writes in the lock
  file (cleared on release, so it never names a past holder), and the command
  exits 2. A lock that cannot be opened (a read-only checkout, a file where the
  directory goes) raises [`Unwritable`], said with the path, exit 2, never a
  traceback; `make coder` with an empty line launches without it, since nothing
  is queued that could launch twice. A dry run never takes it, so it creates no
  directory and no file.
- **Every entry is re-read before it launches** ([`refresh`]): one whose issue
  closed or took a stage label, or whose newest brief is refused, is dropped
  and said; one whose newest `Brief` changed since it was queued takes the
  newer one, and says so. Nothing is launched stale.
"""

from __future__ import annotations

import contextlib
import fcntl
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

from dispatch import brief, rebriefs, remote
from merging import state as files
from merging.watch import board
from merging.watch.order import PRIORITY

#: Where the line lives, under the common git directory.
FILE = "merge-queue/coders-waiting.json"


def path(root: str = ".") -> Path:
    """[`FILE`] in the common git directory of the checkout at `root`."""
    done = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=root, capture_output=True,
                          text=True, check=False)
    return Path(root, done.stdout.strip() or ".git", FILE)


#: How long a command waits for another session's hold on the line: one hold covers a routines
#: listing and a create, each up to `remote.TIMEOUT_SECONDS`.
WAIT_SECONDS = 2 * remote.TIMEOUT_SECONDS + 20


class Busy(remote.RemoteError):
    """The line stayed held past [`WAIT_SECONDS`]; the message names the holder."""


class Unwritable(remote.RemoteError):
    """The line's directory or lock file cannot be made or opened (a read-only checkout, a file
    where the directory goes); the message names the path and why."""


@contextlib.contextmanager
def held(line: Path, say: Callable[[str], None] | None = None, wait: float | None = None,
         clock=time.monotonic, sleep=time.sleep):
    """An exclusive `flock` beside `line` for a read-modify-write, waited on for `wait` seconds
    (said once if it must wait), then [`Busy`]; [`Unwritable`] when the lock file cannot be
    opened. The kernel drops it on any exit; the holder's note is cleared on release, so a
    note read is always the current holder's."""
    wait = WAIT_SECONDS if wait is None else wait
    try:
        line.parent.mkdir(parents=True, exist_ok=True)
        lock = open(line.with_suffix(".lock"), "a+")
    except OSError as failed:
        raise Unwritable(f"cannot write the waiting line {line}: {failed}") from None
    with lock:
        deadline, told = clock() + wait, False
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if clock() >= deadline:
                    raise Busy(f"the waiting line {line} is still held by {holder(lock)} "
                               f"after {wait:.0f} s; nothing done") from None
                if say and not told:
                    say(f"waiting for the line, held by {holder(lock)}")
                    told = True
                sleep(0.5)
        note(lock, f"pid {os.getpid()} ({' '.join(sys.argv) or '?'}), "
                   f"since {datetime.now(timezone.utc):%Y-%m-%dT%H:%M:%S}Z\n")
        try:
            yield
        finally:
            note(lock, "")


def note(lock, text: str) -> None:
    """Replace the holder's note in `lock` with `text`."""
    lock.seek(0)
    lock.truncate()
    lock.write(text)
    lock.flush()


def holder(lock) -> str:
    """The note the session holding `lock` wrote, or `another session`."""
    lock.seek(0)
    return lock.read().strip() or "another session"


def readable(entry: object) -> bool:
    return (isinstance(entry, dict) and isinstance(entry.get("issue"), int)
            and isinstance(entry.get("brief"), str) and isinstance(entry.get("asked"), str))


def load(line: Path) -> list[dict]:
    """The entries on disk; none when the file is missing or unreadable, an unreadable entry skipped."""
    got = files.read(line)
    return [e for e in got if readable(e)] if isinstance(got, list) else []


def save(line: Path, entries: list[dict]) -> str | None:
    """Write the line whole; why not, if it failed."""
    return files.write(line, ordered(entries))


def is_round(found: dict) -> bool:
    return rebriefs.ROUND.search(found.get("body") or "") is not None


#: What `main-health.sh` writes at the top of the issue it files for a red workflow on `main`.
RED_MAIN_MARKER = "<!-- main-health:workflow="

#: The ranks, best first: a red `main`, CLAUDE.md's label order, then an issue with none of them.
RED_MAIN, UNLABELLED = "red main", "unlabelled"
RANKS = (RED_MAIN, *PRIORITY, UNLABELLED)


def priority(issue: object) -> str | None:
    """`issue`'s rank in [`RANKS`]; `None` when it is not an issue read from GitHub."""
    if not isinstance(issue, dict):
        return None
    if RED_MAIN_MARKER in (issue.get("body") or ""):
        return RED_MAIN
    labels = board.names(issue.get("labels"))
    return next((label for label in PRIORITY if label in labels), UNLABELLED)


def rank(entry: dict) -> int:
    """Where `entry`'s priority sorts; none read sorts with the lowest."""
    return RANKS.index(entry["priority"]) if entry.get("priority") in RANKS else len(RANKS) - 1


def ordered(entries: list[dict]) -> list[dict]:
    """Rounds first, then by priority, then the order asked."""
    return sorted(entries, key=lambda e: (not e.get("round"), rank(e), e["asked"], e["issue"]))


def ahead(entries: list[dict], number: int) -> int:
    """How many entries go before #`number`."""
    return next((i for i, e in enumerate(ordered(entries)) if e["issue"] == number), len(entries))


def enqueue(entries: list[dict], number: int, found: dict, now: datetime,
            issue: dict | None = None) -> tuple[list[dict], bool]:
    """The line with #`number` (the `issue` read from GitHub) on the brief `found`, and whether it
    was added; an issue already in line keeps its ask time and takes the newer brief and priority."""
    stamp = f"{now:%Y-%m-%dT%H:%M:%S}Z"
    entry = {"issue": number, "brief": found.get("html_url", ""), "round": is_round(found)}
    if (ranked := priority(issue)) is not None:
        entry["priority"] = ranked
    for kept in entries:
        if kept["issue"] == number:
            kept.update(entry)
            return entries, False
    return [*entries, {**entry, "asked": stamp}], True


def without(entries: list[dict], number: int) -> list[dict]:
    return [e for e in entries if e["issue"] != number]


def refresh(entry: dict, issue: dict | None, comments: list | None, say: Callable[[str], None],
            dry: bool = False) -> dict | None:
    """The newest brief `entry` would launch on, `None` to drop it, or `{}` to keep it
    unlaunched because GitHub could not be read. Updates the entry's brief, round and priority."""
    number = entry["issue"]
    if not isinstance(issue, dict) or comments is None:
        say(f"#{number}: could not read it from GitHub; kept in line, not launched")
        return {}
    try:
        found = brief.read(issue, comments)
    except brief.Refused as refused:
        say(f"#{number}: {'would be dropped' if dry else 'dropped'} from the line: {refused}")
        return None
    if found.get("html_url", "") != entry["brief"]:
        say(f"#{number}: a newer brief since it was queued ({found.get('html_url', '')}); "
            "a launch from the line takes it")
        entry["brief"] = found.get("html_url", "")
    entry["round"] = is_round(found)
    entry["priority"] = priority(issue)
    return found


def relabel(entries: list[dict], seen: dict[int, dict]) -> list[dict]:
    """`entries` with the round and priority a pass read (`seen`, by issue) written onto them."""
    return [{**e, **seen.get(e["issue"], {})} for e in entries]


def described(entry: dict) -> str:
    """An entry's round and rank, as `make coders` lists them."""
    kind = "round" if entry.get("round") else "first launch"
    ranked = entry.get("priority")
    return f"{kind}, {ranked if ranked in RANKS else 'priority unread: sorted with the lowest'}"
