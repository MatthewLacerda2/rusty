"""Launching from the waiting line, one entry at a time (#2382).

`make coders` is the pass ([`run`]); `make coder` launches by hand through the
same hold ([`fresh`], [`launched_since`]). The review of #2393 found two paths
that started a second cloud coder on an issue that already had one, and this
machine loses power without warning, so a pass that dies between two steps is
an ordinary case:

- **A pass that died before its write left a launched issue in the line**:
  #9's routine created, the create for #10 raising (a timeout, the network down
  after a reboot), the line still holding [9, 10]; next tick #9's own routine is
  not counted against it (the re-brief rule), so it launched again. Two rules
  close it. **The write follows each create**, entry by entry, so a pass that
  dies loses at most the entry it was on. And **the read decides**: an entry
  whose issue has a coder routine created after both the ask and the brief it
  would launch on ([`launched_since`]) was launched already (a power cut
  between the create and the write, a timeout after the server had made it, a
  write that failed) and is dropped, said, never launched again.
- **The lock covers the decision, not the reads**: GitHub is read outside it
  (the issues in line, the running coders), and each launch takes [`held`],
  lists the routines again, re-loads the line and decides on those: an entry
  gone meanwhile or re-asked on a newer brief is left to the next pass, and a
  routine another session created since this pass's listing ([`fresh`]) holds a
  slot. So two sessions never launch one entry twice, and never pass the limit
  between them.
- **No free slot reads nothing**: the pass reads the issues in line only when at
  least one could launch.
- **A dry run takes no lock and writes nothing**, and says "would" for each
  drop and launch.

[`held`]: dispatch/waiting.py
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from dispatch import running, waiting


def key(found: dict) -> object:
    return found.get("id") or (found.get("name"), found.get("created_at"))


def fresh(listed: list[dict], before: list[dict]) -> set[int]:
    """The issues with a coder routine in `listed` that `before` did not have: each holds a slot."""
    seen = {key(found) for found in before}
    return {number for found in listed if key(found) not in seen and (number := running.issue(found)) is not None}


def launched_since(listed: list[dict], entry: dict, found: dict) -> dict | None:
    """#`entry`'s coder routine created after the entry was asked and after the brief `found`
    was posted: the launch it waits for already happened."""
    asked, posted = running.when(entry["asked"]), running.when(found.get("created_at"))
    for made in listed:
        at = running.when(made.get("created_at"))
        if (running.issue(made) == entry["issue"] and at and asked and at > asked
                and (posted is None or at > posted)):
            return made
    return None


def room(holders: set[int], number: int) -> bool:
    """Whether #`number` fits under the limit; its own coder never counts against it."""
    return len(holders - {number}) < running.LIMIT


@dataclass
class Pass:
    """One `make coders` pass: the line, the routines and running coders it read, how it reads an
    issue (`read(n)` → (issue, comments)), lists the routines again (`relist()`) and creates
    one (`start(n, issue, brief)` → `launch.create`'s status)."""

    line: Path
    listed: list[dict]
    holders: set[int]
    read: Callable[[int], tuple]
    relist: Callable[[], list[dict]]
    start: Callable[[int, dict, dict], int]
    say: Callable[[str], None]
    dry: bool


def run(go: Pass) -> int:
    """Launch from the line while slots are free: 0, or 2 when a launch or a write failed.
    `remote.RemoteError` (and `waiting.Busy`) propagate, with every launch before them written."""
    entries = waiting.load(go.line)
    if not any(room(go.holders, e["issue"]) for e in entries):
        if entries:
            go.say("no free slot: nothing launched")
        return 0
    queued = {e["issue"]: e["brief"] for e in entries}  # `refresh` moves an entry to a newer brief
    ready, dropped = [], []
    for entry in entries:
        issue, comments = go.read(entry["issue"])
        found = waiting.refresh(entry, issue, comments, go.say, go.dry)
        if found is None:
            dropped.append(entry)
        elif found:
            ready.append((entry, issue, found))
    order = waiting.ordered(entries)  # after `refresh`, which re-reads each entry's round
    status = 0
    for entry, issue, found in sorted(ready, key=lambda got: order.index(got[0])):
        if room(go.holders, entry["issue"]):
            status = launch(go, entry, issue, found, queued[entry["issue"]])
            if status:
                break
    if go.dry or not dropped:
        return status
    with waiting.held(go.line, go.say):
        on_disk = waiting.load(go.line)
        gone = {(e["issue"], e["brief"]) for e in dropped}  # a re-ask on a newer brief stays
        failed = waiting.save(go.line, [e for e in on_disk if (e["issue"], e["brief"]) not in gone])
    return wrote(go, failed) or status


def launch(go: Pass, entry: dict, issue: dict, found: dict, queued: str) -> int:
    """Launch `entry` under the lock on what is true now, then write the line without it; 0 or 2."""
    number = entry["issue"]
    if go.dry:
        go.say(f"would launch #{number} from the line")
        go.holders.add(number)
        return go.start(number, issue, found)
    with waiting.held(go.line, go.say):
        listed = go.relist()
        go.holders |= fresh(listed, go.listed)
        on_disk = next((e for e in waiting.load(go.line) if e["issue"] == number), None)
        if on_disk is None or on_disk["brief"] not in (queued, found.get("html_url")):
            go.say(f"#{number}: left the line or was asked again during this pass; the next pass decides")
            return 0
        if made := launched_since(listed, on_disk, found):
            go.say(f"#{number}: already launched ({made.get('name', '?')}, created {made.get('created_at')}); "
                   "dropped from the line, not launched again")
            return wrote(go, waiting.save(go.line, waiting.without(waiting.load(go.line), number)))
        if not room(go.holders, number):
            go.say(f"#{number}: another session took the last slot; it stays in line")
            return 0
        go.say(f"launching #{number} from the line")
        if go.start(number, issue, found) != 0:
            return 2
        go.holders.add(number)
        return wrote(go, waiting.save(go.line, waiting.without(waiting.load(go.line), number)))


def wrote(go: Pass, failed: str | None) -> int:
    """0, or 2 having said why the line could not be written."""
    if failed:
        go.say(f"could not write the waiting line {go.line}: {failed}; the next pass drops what "
               "was launched by its routine")
        return 2
    return 0
