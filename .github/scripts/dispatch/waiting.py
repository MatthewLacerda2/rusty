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
  then the order asked.
- **Nothing launches by itself**: [`fill`] runs only when `make coders` is
  called (the orchestrator's watch, once a minute), never from a daemon, so
  there is no process to keep alive across a reboot.
- **The line is a file in the common git directory** ([`FILE`], beside the merge
  queue's state), written whole through `merging/state.py`, so a reboot keeps it
  and every session on the machine sees one line. A read-modify-write holds
  [`held`]'s `flock`, so two sessions never launch one entry twice.
- **Every entry is re-read before it launches** ([`refresh`]): one whose issue
  closed or took a stage label, or whose newest brief is refused, is dropped
  and said; one whose newest `Brief` changed since it was queued launches on the
  newer one, once, and says so. Nothing is launched stale.
"""

from __future__ import annotations

import contextlib
import fcntl
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Callable

from dispatch import brief, rebriefs, running
from merging import state as files

#: Where the line lives, under the common git directory.
FILE = "merge-queue/coders-waiting.json"


def path(root: str = ".") -> Path:
    """[`FILE`] in the common git directory of the checkout at `root`."""
    done = subprocess.run(["git", "rev-parse", "--git-common-dir"], cwd=root, capture_output=True,
                          text=True, check=False)
    return Path(root, done.stdout.strip() or ".git", FILE)


@contextlib.contextmanager
def held(line: Path):
    """An exclusive `flock` beside `line` for a read-modify-write; the kernel drops it on any exit."""
    line.parent.mkdir(parents=True, exist_ok=True)
    with open(line.with_suffix(".lock"), "a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


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


def ordered(entries: list[dict]) -> list[dict]:
    """Rounds first, then the order asked."""
    return sorted(entries, key=lambda e: (not e.get("round"), e["asked"], e["issue"]))


def ahead(entries: list[dict], number: int) -> int:
    """How many entries go before #`number`."""
    return next((i for i, e in enumerate(ordered(entries)) if e["issue"] == number), len(entries))


def enqueue(entries: list[dict], number: int, found: dict, now: datetime) -> tuple[list[dict], bool]:
    """The line with #`number` on the brief `found`, and whether it was added; an issue already in
    line keeps its place and takes the newer brief."""
    stamp = f"{now:%Y-%m-%dT%H:%M:%S}Z"
    entry = {"issue": number, "brief": found.get("html_url", ""), "round": is_round(found)}
    for kept in entries:
        if kept["issue"] == number:
            kept.update(entry)
            return entries, False
    return [*entries, {**entry, "asked": stamp}], True


def without(entries: list[dict], number: int) -> list[dict]:
    return [e for e in entries if e["issue"] != number]


def refresh(entry: dict, issue: dict | None, comments: list | None, say: Callable[[str], None]) -> dict | None:
    """The newest brief `entry` would launch on, `None` to drop it, or `{}` to keep it
    unlaunched because GitHub could not be read. Updates the entry's brief and round."""
    number = entry["issue"]
    if not isinstance(issue, dict) or comments is None:
        say(f"#{number}: could not read it from GitHub; kept in line, not launched")
        return {}
    try:
        found = brief.read(issue, comments)
    except brief.Refused as refused:
        say(f"#{number}: dropped from the line: {refused}")
        return None
    if found.get("html_url", "") != entry["brief"]:
        say(f"#{number}: a newer brief since it was queued ({found.get('html_url', '')}); that one goes")
        entry["brief"] = found.get("html_url", "")
    entry["round"] = is_round(found)
    return found


def fill(entries: list[dict], places: list[tuple[dict, str, str]], read: Callable[[int], tuple],
         launch: Callable[[int, dict, dict], int], say: Callable[[str], None]) -> tuple[list[dict], int]:
    """Launch from the line while slots are free; the line left, and 2 if a launch failed.

    `places` is [`running.placements`] with nobody left out; `read(n)` answers (issue, comments);
    `launch(n, issue, brief)` is `launch.create`'s status. A queued issue's own coder never counts
    against it (the re-brief rule), and each launch takes a slot for the rest of the pass."""
    holders = {running.issue(found) for found, placed, _ in places if placed == "running"}
    read_back = {}
    for entry in entries:
        issue, comments = read(entry["issue"])
        found = refresh(entry, issue, comments, say)
        if found is not None:
            read_back[entry["issue"]] = (entry, issue, found)
    left, launched, status = [], set(), 0
    for entry in ordered([e for e, _, _ in read_back.values()]):
        number = entry["issue"]
        _, issue, found = read_back[number]
        if status or not found or len((holders | launched) - {number}) >= running.LIMIT:
            left.append(entry)
        elif launch(number, issue, found) == 0:
            launched.add(number)
        else:
            status = 2
            left.append(entry)
    return left, status
