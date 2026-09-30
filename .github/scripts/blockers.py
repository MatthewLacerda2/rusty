#!/usr/bin/env python3
"""`blockers.py [--fix]` — find "Blocked by #N" that GitHub never recorded (#624).

The dependency graph *is* the plan (CLAUDE.md), and every tool and agent that
plans reads it through GitHub's native `blocked_by` relationships. A blocker
written only in an issue's prose is invisible to all of them: on 2026-09-30 the
batch started #399 while its body said "Blocked by #396" and #396 was open, and
a sweep then found 21 such prose-only mentions across 13 open issues.

This lists every open issue whose body says `Blocked by #N` (any case, lists
like "#396 and #397" included) where N is an **open issue** that the issue's
recorded `dependencies/blocked_by` does not hold. Closed blockers, pull
requests and self-references are skipped: none of them blocks anything now.
With `--fix` it records each one through the REST API.

Recording touches `planning` issues too, deliberately: writing down a blocker
the prose already states changes nobody's scope, it only makes it visible.

Exits 0 when nothing is missing (or `--fix` recorded all of it), 1 when an
audit found gaps, so `make blockers` reads as a check.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys

# "Blocked by" (optionally bold, optionally with a colon) and the issue list
# after it: "#396", "#396, #397", "#396 and #397", "#396 & #397", and items
# bolded or followed by a reason, as #399 wrote it:
# "**Blocked by #396** (materials must reference shaders first) and **#393**".
ITEM = r"[*_]*#\d+[*_]*(?:\s*\([^)\n]*\))?"
MENTION = re.compile(
    rf"\bblocked[*_]*\s+by\b[*_:]*\s*((?:{ITEM}(?:\s*(?:,|&|\band\b)\s*|\s+)?)+)",
    re.IGNORECASE,
)
NEGATION = re.compile(r"\b(?:not|no longer|never)\s*[*_]*$", re.IGNORECASE)


class Gh:
    """`gh` with JSON output parsed; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, *args: str) -> object:
        done = self.runner(["gh", *args], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            sys.exit(f"blockers: gh {' '.join(args)}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None


def mentioned(body: str | None) -> list[int]:
    """The issue numbers `body` says it is blocked by, in order, without repeats."""
    found: list[int] = []
    for match in MENTION.finditer(body or ""):
        if NEGATION.search(body[: match.start()]):
            continue
        for number in re.findall(r"#(\d+)", match.group(1)):
            if int(number) not in found:
                found.append(int(number))
    return found


def missing(gh: Gh, repo: str) -> list[tuple[int, dict]]:
    """(issue, blocker) pairs the prose states and GitHub does not record."""
    issues = gh("issue", "list", "--repo", repo, "--state", "open",
                "--limit", "1000", "--json", "number,body")
    blockers: dict[int, dict] = {}
    gaps = []
    for issue in sorted(issues, key=lambda i: i["number"]):
        wanted = [n for n in mentioned(issue["body"]) if n != issue["number"]]
        if not wanted:
            continue
        path = f"repos/{repo}/issues/{issue['number']}/dependencies/blocked_by?per_page=100"
        recorded = {b["number"] for b in gh("api", path) or []}
        for number in wanted:
            if number in recorded:
                continue
            if number not in blockers:
                blockers[number] = gh("api", f"repos/{repo}/issues/{number}")
            blocker = blockers[number]
            if blocker["state"] == "open" and "pull_request" not in blocker:
                gaps.append((issue["number"], blocker))
    return gaps


def record(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "POST", f"repos/{repo}/issues/{issue}/dependencies/blocked_by",
       "-F", f"issue_id={blocker['id']}")


def main(argv: list[str], gh: Gh | None = None) -> int:
    gh = gh or Gh()
    fix = "--fix" in argv
    unknown = [a for a in argv if a != "--fix"]
    if unknown:
        sys.exit(f"blockers: unknown argument(s) {' '.join(unknown)}; usage: blockers.py [--fix]")
    repo = gh("repo", "view", "--json", "nameWithOwner")["nameWithOwner"]
    gaps = missing(gh, repo)
    for issue, blocker in gaps:
        if fix:
            record(gh, repo, issue, blocker)
        verb = "recorded" if fix else "not recorded"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    if not gaps:
        print("blockers: every prose 'Blocked by' with an open blocker is recorded")
        return 0
    if fix:
        return 0
    print(f"blockers: {len(gaps)} prose-only blocker(s); `make blockers ARGS=--fix` records them")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
