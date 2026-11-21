#!/usr/bin/env python3
"""`blockers.py [--fix | --lift <issue> <blocker>]` — diff prose "Blocked by #N"
against GitHub's recorded relationships, both ways (#624, #2489).

The dependency graph *is* the plan (CLAUDE.md), and every tool and agent that
plans reads it through GitHub's native `blocked_by` relationships. A blocker
written only in an issue's prose is invisible to all of them: on 2026-09-30 the
batch started #399 while its body said "Blocked by #396" and #396 was open, and
a sweep then found 21 such prose-only mentions across 13 open issues. Each one
whose blocker is an **open issue** is listed; `--fix` records it. Closed
blockers, pull requests and self-references are skipped: none blocks anything.

The other way, a recorded open blocker the body no longer states (#2489): the
merge queue reads the relationship, not the prose, so a dependency lifted by
editing the body still handed back the PR that closed the issue. **Prose can
decide what to record; it never decides what to delete** (each review of PR
#2516 found another wording that cost a deletion), so such a blocker `#B` of
issue `#I` is reported, exit 1, as one of:

- **stale**, when `#B` is named nowhere in `#I`'s body and `#I` nowhere in
  `#B`'s (a token search, `deps.prose.named`; struck-through text is not named);
- **mentioned**, when either body names the other but not as a blocker (a
  reason, "see #B", a wording the parser misses);

and is silent when **agreed** (`#B`'s body says `Blocks #I`). Each report line
ends with the command that lifts it: `--lift I B` removes that one relationship,
the caller naming both numbers, and is the only deletion; `--fix` only records.

A `Blocked by` clause holding a reference the parser could not read is a
**doubt**: always reported and exit 1, whatever the record holds, so the
all-clear means every clause was read. A recorded blocker in another repository
(`repository_url`) is named and left alone; a relationship to a closed issue is
history; a sub-issue link is not a `blocked_by` relationship and is never read.
Only issues whose listing reports an open recorded blocker
(`issue_dependencies_summary.blocked_by`) or whose body states one are asked
for their record, so a board of unblocked issues costs one listing. Every call
is REST, so it runs from a cloud session too (#970).

A `Blocked by` that took no reference at all ("Blocked by the shader rewrite")
is a doubt too, so every clause is either read or reported.

Recording and lifting touch `planning` issues too, deliberately: making the
record match what the prose already states changes nobody's scope.

Exits 0 when prose and record agree (or `--fix` recorded every gap, and nothing
is stale, mentioned or in doubt), 1 otherwise, so `make blockers` reads as a
check.
"""

from __future__ import annotations

import json
import subprocess
import sys
from typing import NamedTuple

import ghrest
from deps.prose import named, stated

USAGE = "usage: blockers.py [--fix | --lift <issue> <blocker>]"


class Gh:
    """`gh` with JSON output parsed; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, *args: str) -> object:
        done = self.runner(["gh", *args], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            sys.exit(f"blockers: gh {' '.join(args)}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None


def has_record(issue: dict) -> bool:
    """Whether the listing says `issue` has an open recorded blocker; asked when it
    doesn't say. A closed blocker is history, never stale, so no reason to ask."""
    summary = issue.get("issue_dependencies_summary")
    return summary is None or bool(summary.get("blocked_by", summary.get("total_blocked_by")))


def ours(blocker: dict, repo: str) -> bool:
    """Whether a recorded blocker is an issue of this repository."""
    url = blocker.get("repository_url")
    return url is None or url.lower().endswith(f"/repos/{repo}".lower())


class Report(NamedTuple):
    """(issue, blocker) pairs: prose-only, stale, recorded and mentioned but not
    as a blocker (with whose body names it), recorded in another repository; and
    (issue, references) for each `Blocked by` clause it could not read."""

    gaps: list
    stale: list
    mentioned: list
    foreign: list
    doubts: list


def audit(gh: Gh, repo: str) -> Report:
    issues = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/issues?state=open") or []
    cache: dict[int, dict] = {}

    def fetch(number: int) -> dict:
        if number not in cache:
            cache[number] = gh("api", f"repos/{repo}/issues/{number}")
        return cache[number]

    report = Report([], [], [], [], [])
    for issue in sorted((i for i in issues if "pull_request" not in i), key=lambda i: i["number"]):
        number = issue["number"]
        prose = stated(issue["body"], "blocked by", repo)
        if prose.unread:
            report.doubts.append((number, prose.unread))
        wanted = [n for n in prose.numbers if n != number]
        if not wanted and not has_record(issue):
            continue
        recorded = gh("api", ghrest.blocked_by(repo, number)) or []
        report.foreign.extend((number, b) for b in recorded if not ours(b, repo))
        local = [b for b in recorded if ours(b, repo)]
        for blocker in local:
            if blocker.get("state") != "open" or blocker["number"] in wanted:
                continue
            theirs = fetch(blocker["number"]).get("body")
            if number in stated(theirs, "blocks", repo).numbers:
                continue
            if blocker["number"] in named(issue["body"], repo):
                report.mentioned.append((number, blocker, number))
            elif number in named(theirs, repo):
                report.mentioned.append((number, blocker, blocker["number"]))
            else:
                report.stale.append((number, blocker))
        held = {b["number"] for b in local}
        for want in wanted:
            if want not in held:
                blocker = fetch(want)
                if blocker["state"] == "open" and "pull_request" not in blocker:
                    report.gaps.append((number, blocker))
    return report


def record(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "POST", f"repos/{repo}/issues/{issue}/dependencies/blocked_by",
       "-F", f"issue_id={blocker['id']}")


def remove(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "DELETE",
       f"repos/{repo}/issues/{issue}/dependencies/blocked_by/{blocker['id']}")


def lift(gh: Gh, repo: str, issue: int, number: int) -> int:
    """Remove the one recorded relationship "`issue` blocked by `number`"."""
    recorded = [b for b in gh("api", ghrest.blocked_by(repo, issue)) or []
                if b["number"] == number and ours(b, repo)]
    if not recorded:
        print(f"#{issue} is not recorded as blocked by #{number}: nothing to lift")
        return 0
    remove(gh, repo, issue, recorded[0])
    print(f"#{issue} blocked by #{number} ({recorded[0]['title']}): lifted")
    body = gh("api", f"repos/{repo}/issues/{issue}").get("body")
    if number in stated(body, "blocked by", repo).numbers:
        print(f"#{issue}'s body still says it is blocked by #{number}: take that out, or "
              "`make blockers ARGS=--fix` records it again")
    elif number in named(body, repo):
        print(f"#{issue}'s body still names #{number}, not as a blocker")
    return 0


def lift_args(argv: list[str]) -> tuple[int, int] | None:
    """The (issue, blocker) of `--lift <issue> <blocker>`, or `None` without it."""
    if argv[:1] != ["--lift"]:
        return None
    try:
        issue, number = (int(a.lstrip("#")) for a in argv[1:])
    except ValueError:
        sys.exit(f"blockers: --lift takes two issue numbers; {USAGE}")
    return issue, number


def main(argv: list[str], gh: Gh | None = None) -> int:
    gh = gh or Gh()
    lifting = lift_args(argv)
    fix = argv == ["--fix"]
    if argv and not fix and not lifting:
        sys.exit(f"blockers: unknown argument(s) {' '.join(argv)}; {USAGE}")
    repo = ghrest.slug()
    if lifting:
        return lift(gh, repo, *lifting)
    report = audit(gh, repo)
    for issue, blocker in report.gaps:
        if fix:
            record(gh, repo, issue, blocker)
        verb = "recorded" if fix else "in the body, not recorded on GitHub"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    for issue, blocker in report.stale:
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): recorded on "
              f"GitHub, named in neither body: lift it with {lift_command(issue, blocker)}")
    for issue, blocker, where in report.mentioned:
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): recorded, and "
              f"mentioned in #{where}'s body but not as a blocker: say "
              f"`Blocked by #{blocker['number']}`, or lift it with {lift_command(issue, blocker)}")
    for issue, refs in report.doubts:
        print(f"#{issue}: a \"Blocked by\" holds {', '.join(refs)} in words it could not read: "
              "write each blocker as `#N` (a list, a bullet each, or an issue URL)")
    for issue, blocker in report.foreign:
        where = blocker.get("repository_url", "").rsplit("/repos/", 1)[-1]
        print(f"#{issue} blocked by {where}#{blocker['number']}: in another repository, left alone")
    if not (report.gaps or report.stale or report.mentioned or report.doubts):
        print("blockers: every open blocker in a body is recorded, and every recorded one is in a body")
        return 0
    if fix and not (report.stale or report.mentioned or report.doubts):
        return 0
    print(f"blockers: {len(report.gaps)} prose-only, {len(report.stale)} stale, "
          f"{len(report.mentioned)} mentioned, {len(report.doubts)} unread; "
          "`make blockers ARGS=--fix` records the first; a stale or mentioned one is lifted "
          "with its line's `--lift`, an unread one by rewriting the body")
    return 1


def lift_command(issue: int, blocker: dict) -> str:
    """The command that removes the one relationship "`issue` blocked by `blocker`"."""
    return f"`make blockers ARGS='--lift {issue} {blocker['number']}'`"


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
