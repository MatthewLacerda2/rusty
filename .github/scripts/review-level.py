#!/usr/bin/env python3
"""`review-level.py N` — the effort level for PR #N's independent review (#1569).

`make review-level PR=N` is this. It prints `high` or `medium` on the first
line, then the paths that decided it, read from the PR's files on GitHub
against the table in `dispatch/review_level.py`. The dispatcher passes the
level into `independent-review`'s prompt; before this, the reviewer chose it by
reading the diff against a rule that is mostly about paths.

Exits 0 with a level, 2 when the PR could not be read. REST only (`gh api`), so
it runs from a cloud session too.
"""

from __future__ import annotations

import json
import subprocess
import sys

import ghrest
from dispatch import review_level

USAGE = "usage: review-level.py PR"


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def report(level: str, decided: list[tuple[str, str]], count: int) -> str:
    if not decided:
        return f"{level}\n  no path of the table among the {count} changed"
    return "\n".join([level, *(f"  {why}: {path}" for why, path in decided)])


def main(argv: list[str], gh=gh_json) -> int:
    if len(argv) != 1 or not argv[0].lstrip("#").isdigit():
        print(USAGE, file=sys.stderr)
        return 2
    number = int(argv[0].lstrip("#"))
    files = ghrest.pages(gh, f"repos/{ghrest.slug()}/pulls/{number}/files")
    if files is None:
        print(f"review-level: could not read the files of PR #{number}", file=sys.stderr)
        return 2
    paths = review_level.changed_paths(files)
    level, decided = review_level.level(paths)
    print(report(level, decided, len(paths)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
