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
- **A launched entry is dropped whether or not a slot is free** ([`launched`]):
  with none free, a pass that only said "no free slot" kept a launched issue
  listed as waiting, and the operator, believing the list, ran `make coder` for
  it and started a second coder. `make coder` drops its own entry the same way.
- **No free slot reads no more than that**: the issues in line are read only
  for an entry [`launched_since`] may match or, with a slot free, in line order
  until no entry left could take one ([`fill`]). The order is the one stored: a
  brief that turns an entry into a round, or a label that changes its priority
  (#2615), is written back by the pass that read it ([`forget`]) and moves it
  from the next pass.

`launched_since` has two limits not closed here, which a reader deciding
whether to trust the line needs:

- **It compares the server's `created_at` with a local `asked`.** A local clock
  ahead by more than the time from ask to create (right after a reboot, before
  the clock syncs) stamps the ask after the routine it led to, and the launch is
  not seen.
- **The routines API lists only the account's 20 newest routines.** A launch
  pushed out of that list by 20 newer ones is not seen.
- **A dry run takes no lock and writes nothing**, and says "would" for each
  drop and launch.

[`held`]: dispatch/waiting.py
"""

from __future__ import annotations

from dataclasses import dataclass, field
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
    #: The round and priority each entry read this pass has now, by issue: [`forget`] writes them.
    seen: dict[int, dict] = field(default_factory=dict)


def run(go: Pass) -> int:
    """Launch from the line while slots are free: 0, or 2 when a launch or a write failed.
    `remote.RemoteError` (and `waiting.Busy`) propagate, with every launch before them written."""
    entries = waiting.load(go.line)
    read: dict[int, tuple] = {}
    dropped = launched(go, entries, read)
    entries = [e for e in entries if e not in dropped]
    status = 0
    if not any(room(go.holders, e["issue"]) for e in entries):
        if entries:
            go.say("no free slot: nothing launched")
    else:
        status = fill(go, entries, read, dropped)
    return forget(go, dropped) or status


def launched(go: Pass, entries: list[dict], read: dict[int, tuple]) -> list[dict]:
    """The entries already launched, said and to drop, slot free or not, so the line printed
    is true: GitHub is read only for an entry with a routine made after its ask."""
    gone = []
    for entry in entries:
        if not launched_since(go.listed, entry, {}):
            continue
        issue, comments = read.setdefault(entry["issue"], go.read(entry["issue"]))
        found = refreshed(go, entry, issue, comments)
        if found is None:
            gone.append(entry)
        elif found and (made := launched_since(go.listed, entry, found)):
            go.say(f"#{entry['issue']}: already launched ({made.get('name', '?')}, created "
                   f"{made.get('created_at')}); {'would be ' if go.dry else ''}dropped from the line, "
                   "not launched again")
            gone.append(entry)
    return gone


def fill(go: Pass, entries: list[dict], read: dict[int, tuple], dropped: list[dict]) -> int:
    """Read the entries in line order and launch each that fits, stopping once no entry left
    could take a slot: an issue that already holds one still can, a new one cannot."""
    queued = {e["issue"]: e["brief"] for e in entries}  # `refresh` moves an entry to a newer brief
    for entry in waiting.ordered(entries):
        if not room(go.holders, entry["issue"]):
            continue
        issue, comments = read[entry["issue"]] if entry["issue"] in read else go.read(entry["issue"])
        found = refreshed(go, entry, issue, comments)
        if found is None:
            dropped.append(entry)
        elif found and (status := launch(go, entry, issue, found, queued[entry["issue"]])):
            return status
    return 0


def refreshed(go: Pass, entry: dict, issue, comments) -> dict | None:
    """[`waiting.refresh`], noting in `go.seen` the round and priority it read."""
    found = waiting.refresh(entry, issue, comments, go.say, go.dry)
    if found:
        go.seen[entry["issue"]] = {"round": entry["round"], "priority": entry["priority"]}
    return found


def forget(go: Pass, dropped: list[dict]) -> int:
    """Write the line without `dropped` and with what `go.seen` read, under the lock; a re-ask on
    a newer brief stays. 0 or 2."""
    if go.dry or not (dropped or stale(go)):
        return 0
    with waiting.held(go.line, go.say):
        on_disk = waiting.load(go.line)
        gone = {(e["issue"], e["brief"]) for e in dropped}
        kept = waiting.relabel([e for e in on_disk if (e["issue"], e["brief"]) not in gone], go.seen)
        failed = waiting.save(go.line, kept)
    return wrote(go, failed)


def stale(go: Pass) -> bool:
    """Whether an entry on disk holds a round or priority other than the one this pass read."""
    on_disk = waiting.load(go.line)
    return waiting.relabel(on_disk, go.seen) != on_disk


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
