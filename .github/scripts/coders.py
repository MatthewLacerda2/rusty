#!/usr/bin/env python3
"""`coders.py [--dry-run]` — who runs, who waits, and launch from the line (#2382).

`make coders` is this. It prints every coder routine that counts against
[`running.LIMIT`] and why (the status the orchestrator had only inside a
refusal), the ones not counted and why, and the waiting line
(`dispatch/waiting.py`) in launch order; then it launches from the line while
slots are free, each launch the one `make coder` makes (`dispatch/launch.py`).
The orchestrator's watch calls it once a minute; nothing launches without that
call.

`--dry-run` says what it would launch, creates nothing and writes nothing.
Exits 0, or 2 when GitHub or claude.ai failed.
"""

from __future__ import annotations

import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import ghrest
from coder import gh_json
from dispatch import launch, remote, running, waiting


def say(message: str) -> None:
    print(f"coders: {message}", flush=True)


def show(places: list[tuple[dict, str, str]], entries: list[dict]) -> None:
    holding = [(found, why) for found, placed, why in places if placed == "running"]
    say(f"running {len(holding)} of {running.LIMIT}" + (":" if holding else ""))
    for found, why in holding:
        print(f"  {running.describe(found)}: {why}")
    for found, placed, why in places:
        if placed == "unknown" or why == running.STALE or why.startswith(running.BLOCKED):
            print(f"  not counted: {found.get('name', '?')}: {why}")
    say(f"waiting {len(entries)}" + (":" if entries else ""))
    for place, entry in enumerate(waiting.ordered(entries), 1):
        kind = "round" if entry.get("round") else "first launch"
        print(f"  {place}. #{entry['issue']} ({kind}, asked {entry['asked']}, brief {entry['brief']})")


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None, line: Path | None = None) -> int:
    if any(a != "--dry-run" for a in argv):
        print("usage: coders.py [--dry-run]", file=sys.stderr)
        return 1
    dry = "--dry-run" in argv
    now = now or datetime.now(timezone.utc)
    line = line or waiting.path()
    env = os.environ if env is None else env
    repository = ghrest.slug()

    def read(number: int) -> tuple:
        return (gh(f"repos/{repository}/issues/{number}"),
                ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments"))

    def start(number: int, issue: dict, found: dict) -> int:
        say(f"{'would launch' if dry else 'launching'} #{number} from the line")
        try:
            return launch.create(number, issue, found, listed, call, repository, now, env, dry, say)
        except ValueError as refused:
            say(f"#{number} not launched: {refused}")
            return 2

    try:
        with waiting.held(line):
            listed = launch.routines(call)
            places = running.placements(listed, 0, launch.github(gh, repository, now))
            entries = waiting.load(line)
            show(places, entries)
            left, status = waiting.fill(entries, places, read, start, say)
            failed = None if dry else waiting.save(line, left)
    except remote.RemoteError as down:
        say(str(down))
        return 2
    if failed:
        say(f"could not write the waiting line {line}: {failed}")
        return 2
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
