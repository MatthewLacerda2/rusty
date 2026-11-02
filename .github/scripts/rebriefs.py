#!/usr/bin/env python3
"""`rebriefs.py [--since DATE]` — how many briefs issues take, and why (#1724).

`make rebriefs` is this. It reads every issue comment of the repository over
REST, keeps the `Brief` comments, and prints the issues briefed, the briefs,
briefs per issue, the issues that took one brief, and the re-briefs by their
`Round:` line (`dispatch/rebriefs.py` has the counting rules). `--since` keeps
the briefs created on or after that date (`2026-10-29` or
`2026-10-29T18:00Z`); a batch's closing report passes its start.

A report, never a gate: exits 0 with the numbers, 2 when GitHub could not be
read or the date is not one.
"""

from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime, timezone

import ghrest
from dispatch import rebriefs

USAGE = "usage: rebriefs.py [--since DATE]"


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def since_of(argv: list[str]) -> str | None:
    """The `--since` value normalised to UTC ISO text, `""` for none, `None` when malformed."""
    if not argv:
        return ""
    if len(argv) != 2 or argv[0] != "--since":
        return None
    try:
        moment = datetime.fromisoformat(argv[1].replace("Z", "+00:00"))
    except ValueError:
        return None
    if moment.tzinfo is not None:  # GitHub's times are UTC; a naive date is read as UTC too
        moment = moment.astimezone(timezone.utc).replace(tzinfo=None)
    return moment.strftime("%Y-%m-%dT%H:%M:%SZ")


def main(argv: list[str], gh=gh_json) -> int:
    since = since_of(argv)
    if since is None:
        print(USAGE, file=sys.stderr)
        return 2
    comments = ghrest.pages(gh, f"repos/{ghrest.slug()}/issues/comments?sort=created&direction=asc")
    if comments is None:
        print("rebriefs: could not read the issue comments from GitHub", file=sys.stderr)
        return 2
    print(rebriefs.report(rebriefs.tally(comments, since), argv[1] if argv else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
