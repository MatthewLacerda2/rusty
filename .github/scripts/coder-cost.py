#!/usr/bin/env python3
"""`coder-cost.py` — what coder sessions cost in wall-clock, and where it goes (#2452).

`make coder-cost` is this. It lists the account's routines (`dispatch/remote.py`),
reads the run log of every fired coder routine's session it has not measured yet,
and prints, per session, the issue, first brief or round (and the round's cause),
the seconds of sandbox, reading (to the first edit), gates and the rest, and its
turns; then, for first briefs and for rounds, the cold start's median share and
its spread, and the pooled split. `dispatch/cost.py` and `dispatch/runlog.py` have
the rules. What it measured is kept in `.git/coder-cost.json`, so a later run adds
to it (the listing holds only the 20 newest routines).

Its own verb rather than `make rebriefs ARGS=--cost`: `rebriefs` counts comments
on GitHub and needs nothing else, this one needs the claude.ai login behind
`claude mcp serve` and writes a file, so it runs only on the orchestrator's machine.

A report, never a gate: exits 0 with the numbers, 1 on a usage error, 2 when
claude.ai could not be listed or the kept file could not be written.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import ghrest
from coder import gh_json
from dispatch import cost, launch, remote
from merging import state


def kept_path() -> Path:
    """[`cost.FILE`] in the common git directory, so every worktree adds to one file."""
    done = subprocess.run(["git", "rev-parse", "--git-common-dir"], capture_output=True, text=True, check=False)
    return Path(done.stdout.strip() or ".git", cost.FILE)


def main(argv: list[str], gh=gh_json, ask=remote.answer, path: Path | None = None) -> int:
    if argv:
        print("usage: coder-cost.py", file=sys.stderr)
        return 1
    path = path or kept_path()
    repository = ghrest.slug()
    threads: dict[int, list | None] = {}

    def comments(number: int) -> list | None:
        if number not in threads:
            threads[number] = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
        return threads[number]

    try:
        listed = launch.routines(lambda arguments: ask(arguments)[:2])
    except remote.RemoteError as down:
        print(f"coder-cost: {down}", file=sys.stderr)
        return 2
    kept = cost.load(path)
    done = cost.read(listed, kept, ask, comments)
    records = {**kept, **done.records}
    if done.records and (failed := state.write(path, records)):
        print(f"coder-cost: could not keep the measurements in {path}: {failed}", file=sys.stderr)
        print(cost.report(records, len(done.records), done.skipped))
        return 2
    print(cost.report(records, len(done.records), done.skipped))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
