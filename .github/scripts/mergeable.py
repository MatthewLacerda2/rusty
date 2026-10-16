#!/usr/bin/env python3
"""`mergeable.py N` — say whether pull request N has really been checked.

The decision lives in `merging/verdict/`; this is its command line.

Run it:

    make mergeable PR=524
    python3 .github/scripts/mergeable.py 524

`merge-queue.py` beside this file asks the same question in a loop and imports
this decision rather than restating it. The decision is [`merging.verdict.decide.judge`], pure over
plain dictionaries, so all of it is tested without a network
(`make scripts`). Python, stdlib only, because it is a few `gh` calls and a
decision — and `.github/scripts/` is where that lives.
"""

from __future__ import annotations

import sys

from merging.verdict import decide, github


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit("usage: mergeable.py PULL_REQUEST_NUMBER")
    number = sys.argv[1]

    try:
        repo = github.gh("repo", "view", "--json", "nameWithOwner")["nameWithOwner"]
        pull = github.gh("pr", "view", number, "--json", github.PULL_FIELDS)
        runs, jobs = github.evidence(repo, pull["headRefOid"])
    except github.Unreachable as down:
        sys.exit(f"mergeable: GitHub unreachable after {len(github.RETRIES)} retries: {down}")

    ok, lines = decide.judge(pull, runs, jobs, github.paths(pull))
    head, *rest = lines
    where = sys.stdout if ok else sys.stderr
    print(f"mergeable: #{number}: {head}", file=where)
    for line in rest:
        print(f"  {line}", file=where)
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
