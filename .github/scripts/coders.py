#!/usr/bin/env python3
"""`coders.py [--dry-run] [--cap N]` — who runs, who waits, and launch from the line (#2382).

`make coders` is this. It prints every coder routine that counts against
[`running.LIMIT`] and why (the status the orchestrator had only inside a
refusal), the ones not counted and why, and the waiting line
(`dispatch/waiting.py`) in launch order; then it launches from the line while
slots are free, each launch the one `make coder` makes (`dispatch/launch.py`),
under the hold and the rules `dispatch/drain.py` keeps. The orchestrator's watch
calls it once a minute; nothing launches without that call.

`--cap N` (#2630) sets the cap on cloud coders below [`running.LIMIT`]
(`dispatch/cap.py`; N at the limit lifts it), then runs the pass under it; a
cap above the limit is refused. Every launch path reads it, and the count says
`running N of CAP (the cap; the limit is 5)` while one is set. It is also pushed
to the orchestrator lock (`orchestrating/carry.py`, `share`), whoever holds it,
so the next cloud fire, or this machine's next take, runs under it (#2667).
Every take restores the lock's copy over the local file, so a push GitHub
refused is said, and the cap then holds on this machine only until its next take
of the lock; set it again after that.

`--dry-run` says what it would drop and launch, creates nothing, writes
nothing (a `--cap` too: the pass runs under it, unsaved) and takes no lock. Exits 0; 1 on a usage error; 2 when GitHub or
claude.ai failed, the line could not be written or its lock opened, or another session held it
past `waiting.WAIT_SECONDS`.
"""

from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import ghrest
from coder import gh_json
from dispatch import cap, drain, launch, remote, running, waiting
from orchestrating import carry, lease, side


def say(message: str) -> None:
    print(f"coders: {message}", flush=True)


def show(places: list[tuple[dict, str, str]], entries: list[dict], ceiling: int) -> None:
    holding = [(found, why) for found, placed, why in places if placed == "running"]
    say(cap.told(len(holding), ceiling) + (":" if holding else ""))
    for found, why in holding:
        print(f"  {running.describe(found)}: {why}")
    for found, placed, why in places:
        if placed != "running" and running.named(placed, why):
            print(f"  not counted: {found.get('name', '?')}: {why}")
    say(f"waiting {len(entries)}" + (":" if entries else ""))
    for place, entry in enumerate(waiting.ordered(entries), 1):
        print(f"  {place}. #{entry['issue']} ({waiting.described(entry)}, asked {entry['asked']}, "
              f"brief {entry['brief']})")


def share(ceiling: int, now: datetime) -> None:
    """Put the cap on the orchestrator lock, so both sides read it; `lease.Unreachable` if not."""
    text = json.dumps({"cap": ceiling}, indent=1, sort_keys=True) + "\n"
    carry.share(side.git_in("."), cap.FILE, text, f"cap: coders' cap set to {ceiling}", now)


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None, line: Path | None = None, clock=None, shared=share) -> int:
    dry, flags = "--dry-run" in argv, [a for a in argv if a != "--dry-run"]
    if flags and (len(flags) != 2 or flags[0] != "--cap"):
        print("usage: coders.py [--dry-run] [--cap N]", file=sys.stderr)
        return 1
    clock = clock or ((lambda: now) if now else (lambda: datetime.now(timezone.utc)))
    now = clock()
    line = line or waiting.path()
    env = os.environ if env is None else env
    try:
        ceiling = set_cap(line, flags[1], dry, lambda c: shared(c, now)) if flags else None
    except ValueError as refused:
        say(f"refused: {refused}")
        return 1
    except remote.RemoteError as failed:
        say(str(failed))
        return 2
    if ceiling is None:
        ceiling, ignored = cap.read(line)
        if ignored:
            say(ignored)
    repository = ghrest.slug()
    comments: dict[int, list | None] = {}  # what `read` found, for the model's round count

    def read(number: int) -> tuple:
        comments[number] = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
        return gh(f"repos/{repository}/issues/{number}"), comments[number]

    def start(number: int, issue: dict, found: dict) -> int:
        try:
            return launch.create(number, issue, found, listed, call, repository, clock, env, dry, say,
                                 comments.get(number))
        except ValueError as refused:
            say(f"#{number} not launched: {refused}")
            return 2

    try:
        listed = launch.routines(call)
        places = running.placements(listed, 0, launch.github(gh, repository, now))
        show(places, waiting.load(line), ceiling)
        holders = {running.issue(found) for found, placed, _ in places if placed == "running"}
        return drain.run(drain.Pass(line=line, listed=listed, holders=holders, read=read,
                                    relist=lambda: launch.routines(call), start=start, say=say, dry=dry,
                                    cap=ceiling))
    except remote.RemoteError as down:  # waiting.Busy and waiting.Unwritable too
        say(str(down))
        return 2


def set_cap(line: Path, asked: str, dry: bool, shared) -> int:
    """Set the cap `asked` (`cap.check`, `ValueError` for one not 1 to the limit) here and on
    the lock (`shared(cap)`), and say so; `remote.RemoteError` when it cannot be written here."""
    ceiling = cap.check(asked)
    if not dry and (failed := cap.write(line, ceiling)):
        raise remote.RemoteError(f"could not write the cap {cap.path(line)}: {failed}")
    if not dry:
        try:
            shared(ceiling)
        except (lease.Unreachable, OSError) as unshared:
            say(f"the cap holds on this machine only: the orchestrator lock did not take it ({unshared}),"
                " and the next take of the lock puts the lock's copy back; set it again once GitHub answers")
    told = "the cap lifted" if ceiling == running.LIMIT else f"the cap {ceiling}"
    say(f"{'would set' if dry else 'set'} {told} (the limit is {running.LIMIT})")
    return ceiling


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
