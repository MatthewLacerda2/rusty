#!/usr/bin/env python3
"""`blockers.py [--fix]` — diff prose "Blocked by #N" against GitHub's record (#624, #2489).

The dependency graph *is* the plan (CLAUDE.md), and every tool and agent that
plans reads it through GitHub's native `blocked_by` relationships. A blocker
written only in an issue's prose is invisible to all of them: on 2026-09-30 the
batch started #399 while its body said "Blocked by #396" and #396 was open, and
a sweep then found 21 such prose-only mentions across 13 open issues.

This lists every open issue whose body says `Blocked by #N` (any case, lists
like "#396 and #397" included) where N is an **open issue** that the issue's
recorded `dependencies/blocked_by` does not hold. Closed blockers, pull
requests and self-references are skipped: none of them blocks anything now.
With `--fix` it records each one through the REST API. Every call is REST, so
it runs from a cloud session too (#970).

The other direction is stale relationships (#2489): a recorded blocker that is
an **open** issue and that the body no longer states. The merge queue reads the
relationship, not the prose, so a dependency lifted by editing the body still
handed back the PR that closed the issue, and the relationship had to be
deleted by hand through the REST API. The body is the source of truth for an
open blocker: `--fix` removes each stale one (`DELETE
…/dependencies/blocked_by/{issue_id}`). A relationship to a closed issue is
history and blocks nothing, so it is left alone; a sub-issue link is not a
`blocked_by` relationship and is never read. Only issues whose listing reports
a recorded blocker (`issue_dependencies_summary`) are asked for it, so a board
of unblocked issues costs one listing.

Recording and removing touch `planning` issues too, deliberately: making the
record match what the prose already states changes nobody's scope.

Exits 0 when prose and record agree (or `--fix` made them agree), 1 when an
audit found a difference, so `make blockers` reads as a check.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys

import ghrest

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


def has_record(issue: dict) -> bool:
    """Whether the listing says `issue` has a recorded blocker; asked when it doesn't say."""
    summary = issue.get("issue_dependencies_summary")
    return summary is None or bool(summary.get("total_blocked_by", summary.get("blocked_by")))


def audit(gh: Gh, repo: str) -> tuple[list[tuple[int, dict]], list[tuple[int, dict]]]:
    """(missing, stale): (issue, blocker) pairs the prose states and GitHub does not
    record, and open recorded blockers the prose no longer states."""
    listed = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/issues?state=open") or []
    issues = [i for i in listed if "pull_request" not in i]
    blockers: dict[int, dict] = {}
    gaps, stale = [], []
    for issue in sorted(issues, key=lambda i: i["number"]):
        wanted = [n for n in mentioned(issue["body"]) if n != issue["number"]]
        if not wanted and not has_record(issue):
            continue
        recorded = gh("api", ghrest.blocked_by(repo, issue["number"])) or []
        stale += [(issue["number"], b) for b in recorded
                  if b.get("state") == "open" and b["number"] not in wanted]
        held = {b["number"] for b in recorded}
        for number in wanted:
            if number in held:
                continue
            if number not in blockers:
                blockers[number] = gh("api", f"repos/{repo}/issues/{number}")
            blocker = blockers[number]
            if blocker["state"] == "open" and "pull_request" not in blocker:
                gaps.append((issue["number"], blocker))
    return gaps, stale


def record(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "POST", f"repos/{repo}/issues/{issue}/dependencies/blocked_by",
       "-F", f"issue_id={blocker['id']}")


def remove(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "DELETE",
       f"repos/{repo}/issues/{issue}/dependencies/blocked_by/{blocker['id']}")


def main(argv: list[str], gh: Gh | None = None) -> int:
    gh = gh or Gh()
    fix = "--fix" in argv
    unknown = [a for a in argv if a != "--fix"]
    if unknown:
        sys.exit(f"blockers: unknown argument(s) {' '.join(unknown)}; usage: blockers.py [--fix]")
    repo = ghrest.slug()
    gaps, stale = audit(gh, repo)
    for issue, blocker in gaps:
        if fix:
            record(gh, repo, issue, blocker)
        verb = "recorded" if fix else "in the body, not recorded on GitHub"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    for issue, blocker in stale:
        if fix:
            remove(gh, repo, issue, blocker)
        verb = "removed" if fix else "recorded on GitHub, not in the body"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    if not gaps and not stale:
        print("blockers: every open blocker in a body is recorded, and every recorded one is in its body")
        return 0
    if fix:
        return 0
    print(f"blockers: {len(gaps)} prose-only, {len(stale)} stale recorded blocker(s); "
          "`make blockers ARGS=--fix` records the first and removes the second")
    return 1

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
