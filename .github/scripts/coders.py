#!/usr/bin/env python3
"""`coders.py [--dry-run]` — who runs, who waits, and launch from the line (#2382).

`make coders` is this. It prints every coder routine that counts against
[`running.LIMIT`] and why (the status the orchestrator had only inside a
refusal), the ones not counted and why, and the waiting line
(`dispatch/waiting.py`) in launch order; then it launches from the line while
slots are free, each launch the one `make coder` makes (`dispatch/launch.py`),
under the hold and the rules `dispatch/drain.py` keeps. The orchestrator's watch
calls it once a minute; nothing launches without that call.

`--dry-run` says what it would drop and launch, creates nothing, writes
nothing and takes no lock. Exits 0; 1 on a usage error; 2 when GitHub or
claude.ai failed, the line could not be written or its lock opened, or another session held it
past `waiting.WAIT_SECONDS`.
"""

from __future__ import annotations

import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import ghrest
from coder import gh_json
from dispatch import drain, launch, remote, running, waiting


def say(message: str) -> None:
    print(f"coders: {message}", flush=True)


def show(places: list[tuple[dict, str, str]], entries: list[dict]) -> None:
    holding = [(found, why) for found, placed, why in places if placed == "running"]
    say(f"running {len(holding)} of {running.LIMIT}" + (":" if holding else ""))
    for found, why in holding:
        print(f"  {running.describe(found)}: {why}")
    for found, placed, why in places:
        if placed != "running" and running.named(placed, why):
            print(f"  not counted: {found.get('name', '?')}: {why}")
    say(f"waiting {len(entries)}" + (":" if entries else ""))
    for place, entry in enumerate(waiting.ordered(entries), 1):
        print(f"  {place}. #{entry['issue']} ({waiting.described(entry)}, asked {entry['asked']}, "
              f"brief {entry['brief']})")


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None, line: Path | None = None, clock=None) -> int:
    if any(a != "--dry-run" for a in argv):
        print("usage: coders.py [--dry-run]", file=sys.stderr)
        return 1
    dry = "--dry-run" in argv
    clock = clock or ((lambda: now) if now else (lambda: datetime.now(timezone.utc)))
    now = clock()
    line = line or waiting.path()
    env = os.environ if env is None else env
    repository = ghrest.slug()

    def read(number: int) -> tuple:
        return (gh(f"repos/{repository}/issues/{number}"),
                ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments"))

    def start(number: int, issue: dict, found: dict) -> int:
        try:
            return launch.create(number, issue, found, listed, call, repository, clock, env, dry, say)
        except ValueError as refused:
            say(f"#{number} not launched: {refused}")
            return 2

    try:
        listed = launch.routines(call)
        places = running.placements(listed, 0, launch.github(gh, repository, now))
        show(places, waiting.load(line))
        holders = {running.issue(found) for found, placed, _ in places if placed == "running"}
        return drain.run(drain.Pass(line=line, listed=listed, holders=holders, read=read,
                                    relist=lambda: launch.routines(call), start=start, say=say, dry=dry))
    except remote.RemoteError as down:  # waiting.Busy and waiting.Unwritable too
        say(str(down))
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
